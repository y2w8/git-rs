use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use anyhow::Context;
use flate2::{Compression, write::GzEncoder};
use gix::objs::tree::EntryKind;
use tar::Builder;

use crate::{
    error::Error,
    git::{
        commit::Commit,
        refs::Ref,
        tree::{Blob, PathView},
    },
};

#[derive(Clone)]
pub struct Repository {
    pub name: String,
    pub description: String,
    pub owner: Option<String>,

    // Path on disk (not exposed to clients)
    pub path: PathBuf,
    // ThreadSafeRepository holds shared repo state so it can live across web worker threads.
    // Call `self.gix()` on a worker thread to get a thread-local `gix::Repository` with object caches.
    pub gix: gix::ThreadSafeRepository,
}

impl Repository {
    pub fn open(path: &Path) -> Result<Self, Error> {
        let name = path
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or(Error::Failed)?;

        let raw_desc = std::fs::read(path.join("description")).unwrap_or_default();
        let description = match String::from_utf8(raw_desc) {
            Ok(s) if !s.is_empty() => s,
            Ok(_) => "There is no description!".to_string(),
            Err(_) => "Failed to get the description!".to_string(),
        };

        let gix = gix::ThreadSafeRepository::open(path).map_err(|_| Error::Failed)?;

        let owner = gix
            .to_thread_local()
            .config_snapshot()
            .string("gitweb.owner")
            .map(|v| v.to_string());

        Ok(Self {
            name: name.to_string(),
            description,
            owner,
            path: path.to_path_buf(),
            gix,
        })
    }

    /// Converts the shared `ThreadSafeRepository` handle into a thread-local `gix::Repository`
    /// that caches decoded Git objects in memory for the current request.
    pub fn gix(&self) -> gix::Repository {
        self.gix.to_thread_local()
    }

    /// Looks for a `README*` file in the root folder (`Tree`) of the `HEAD` commit.
    pub fn readme(&self) -> Option<String> {
        let gix_repo = self.gix();
        // 1. Follow HEAD -> branch -> latest Commit object
        let commit = gix_repo.head().ok()?.peel_to_commit().ok()?;
        // 2. Get the root directory snapshot (Tree) of that Commit
        let tree = commit.tree().ok()?;

        // 3. Find the first entry whose filename starts with "readme" (case-insensitive)
        for entry in tree.iter().flatten() {
            if entry
                .filename()
                .to_string()
                .to_lowercase()
                .starts_with("readme")
            {
                // Load the file contents (Blob) from Git's object database
                let blob = entry.object().ok()?.into_blob();
                return Some(String::from_utf8_lossy(&blob.data).into_owned());
            }
        }
        None
    }

    /// Streams a commit or branch's directory tree into an in-memory `.tar.gz` archive.
    pub fn archive(
        &self,
        current_ref: &Ref,
        commit: &Option<String>,
    ) -> Result<(Vec<u8>, String), anyhow::Error> {
        let gix_repo = self.gix();
        // Resolve the root directory Tree either from a specific commit hash or from a branch/tag ref
        let (tree, filename) = if let Some(commit) = commit {
            (
                self.find_commit(&gix_repo, commit)?.gix.tree()?,
                format!("{commit}.tar.gz"),
            )
        } else {
            (
                gix_repo.find_reference(&current_ref.name)?.peel_to_tree()?,
                format!("{}.tar.gz", current_ref.name.replace('/', "-")),
            )
        };

        let encoder = GzEncoder::new(Vec::new(), Compression::default());
        let mut builder = Builder::new(encoder);

        Self::add_tree_to_archive(&tree, "", &mut builder)?;

        // Flush the tar footer, then finish the gzip stream to get the final compressed bytes.
        let encoder = builder.into_inner()?;
        let bytes = encoder.finish()?;
        Ok((bytes, filename))
    }

    fn add_tree_to_archive(
        tree: &gix::Tree<'_>,
        prefix: &str,
        builder: &mut Builder<GzEncoder<Vec<u8>>>,
    ) -> anyhow::Result<()> {
        for entry in tree.iter() {
            let entry = entry?;
            let mode = entry.mode();
            let name = entry.filename().to_string();
            let path = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };

            if mode.is_tree() {
                let subtree = entry.object()?.into_tree();
                Self::add_tree_to_archive(&subtree, &path, builder)?;
            } else if mode.is_blob() || mode.is_executable() {
                let blob = entry.object()?.into_blob();
                let data = &blob.data;

                let mut header = tar::Header::new_gnu();
                header.set_size(data.len() as u64);
                header.set_mode(if mode.is_executable() { 0o755 } else { 0o644 });
                header.set_cksum();

                builder.append_data(&mut header, &path, &data[..])?;
            }
            // symlinks (mode.is_link()) intentionally skipped here — add
            // handling if you need them (header.set_entry_type(EntryType::Symlink)
            // + set_link_name to the blob content as the target path).
        }
        Ok(())
    }

    /// Resolves a relative repository path on `current_ref` to either a folder listing (`PathView::Tree`)
    /// or a file view (`PathView::Blob`).
    pub fn resolve_path(&self, current_ref: &Ref, path: &str) -> anyhow::Result<PathView> {
        let gix_repo = self.gix();
        let commit = Commit::from_gix(
            gix_repo
                .find_reference(&current_ref.name)?
                .peel_to_commit()?,
        )?;
        let path = path.trim_matches('/');

        if path.is_empty() {
            return Ok(PathView::Tree(commit.tree("")?));
        }

        let root_tree = commit.gix.tree().context("failed to get root tree")?;
        let entry = root_tree
            .lookup_entry_by_path(path)
            .with_context(|| format!("failed lookup for path: {path}"))?
            .with_context(|| format!("path not found: {path}"))?;

        let mode = entry.mode();
        match mode.kind() {
            EntryKind::Tree => {
                let entries = commit.tree(path)?;
                Ok(PathView::Tree(entries))
            }
            EntryKind::Blob | EntryKind::BlobExecutable => {
                let blob = entry
                    .object()
                    .context("failed to load blob object")?
                    .try_into_blob()
                    .map_err(|_| anyhow::anyhow!("entry is not a blob"))?;

                let id = entry.oid().to_string();
                let size = blob.data.len() as u64;

                // Try decoding as UTF-8 text; if it contains non-UTF-8 bytes, treat it as binary (None).
                let lines = std::str::from_utf8(&blob.data)
                    .ok()
                    .map(|s| s.lines().map(str::to_string).collect());

                Ok(PathView::Blob(Blob {
                    id,
                    mode: mode.value() as u32,
                    size,
                    lines,
                }))
            }
            EntryKind::Link => {
                anyhow::bail!("symlinks are not yet supported for direct viewing")
            }
            EntryKind::Commit => {
                anyhow::bail!("path resolves to a submodule, not a viewable file/dir")
            }
        }
    }

    /// Reads the raw bytes of a file (`Blob`) at `path` without splitting lines or cloning buffers.
    /// Returns `Ok(None)` if `path` points to a directory (`Tree`).
    pub fn read_raw_blob(&self, current_ref: &Ref, path: &str) -> anyhow::Result<Option<Vec<u8>>> {
        let path = path.trim_matches('/');
        if path.is_empty() {
            return Ok(None);
        }

        let gix_repo = self.gix();
        let root_tree = gix_repo
            .find_reference(&current_ref.name)?
            .peel_to_tree()
            .context("failed to get root tree")?;

        let entry = root_tree
            .lookup_entry_by_path(path)
            .with_context(|| format!("failed lookup for path: {path}"))?
            .with_context(|| format!("path not found: {path}"))?;

        match entry.mode().kind() {
            EntryKind::Tree => Ok(None),
            EntryKind::Blob | EntryKind::BlobExecutable => {
                let blob = entry
                    .object()
                    .context("failed to load blob object")?
                    .try_into_blob()
                    .map_err(|_| anyhow::anyhow!("entry is not a blob"))?;
                // `.detach().data` moves the underlying Vec<u8> out of the gix Blob without cloning.
                Ok(Some(blob.detach().data))
            }
            EntryKind::Link => anyhow::bail!("symlinks are not yet supported for direct viewing"),
            EntryKind::Commit => {
                anyhow::bail!("path resolves to a submodule, not a viewable file/dir")
            }
        }
    }
}

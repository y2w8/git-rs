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

    // So clients dont see the path
    pub path: PathBuf,
    pub gix: gix::ThreadSafeRepository,
}

impl Repository {
    pub fn open(path: &Path) -> Result<Self, Error> {
        let Some(name) = path.file_name().and_then(OsStr::to_str) else {
            return Err(Error::Failed);
        };
        let description = std::fs::read(path.join("description")).unwrap_or_default();
        let description = match String::from_utf8(description) {
            Ok(v) => {
                if !v.is_empty() {
                    v
                } else {
                    "There is no description!".to_string()
                }
            }
            Err(_) => "Failed to get the description!".to_string(),
        };

        let gix = match gix::ThreadSafeRepository::open(path) {
            Ok(repo) => repo,
            Err(_) => return Err(Error::Failed),
        };

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

    pub fn gix(&self) -> gix::Repository {
        self.gix.to_thread_local()
    }

    pub fn readme(&self) -> Option<String> {
        let gix_repo = self.gix();
        let commit = gix_repo.head().ok()?.peel_to_commit().ok()?;
        let tree = commit.tree().ok()?;

        let entry = tree.iter().find(|entry| {
            entry
                .as_ref()
                .unwrap()
                .filename()
                .to_string()
                .to_lowercase()
                .starts_with("readme")
        })?;

        let blob = entry.unwrap().object().unwrap().into_blob();
        Some(String::from_utf8_lossy(&blob.data).into_owned())
    }

    pub fn archive(
        &self,
        current_ref: &Ref,
        commit: &Option<String>,
    ) -> Result<(Vec<u8>, String), anyhow::Error> {
        let gix_repo = self.gix();
        let (tree, filename) = if let Some(commit) = commit {
            (
                self.find_commit(&gix_repo, commit)?.gix.tree()?,
                format!("{}.tar.gz", commit),
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

        // finish() flushes the tar footer, then finish() on the gz encoder
        // flushes the gzip footer and gives you the final Vec<u8>.
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

    pub fn resolve_path(&self, current_ref: &Ref, path: &str) -> anyhow::Result<PathView> {
        let gix_repo = self.gix();
        let commit = Commit::from_gix(
            gix_repo
                .find_reference(&current_ref.name.to_string())?
                .peel_to_commit()?,
        )?; // however you currently get a Commit from a Ref
        let path = path.trim_matches('/');

        if path.is_empty() {
            // root of the tree
            let entries = commit.tree("")?;
            return Ok(PathView::Tree(entries));
        }

        let tree = commit.gix.tree().context("failed to get root tree")?;

        let entry = tree
            .lookup_entry_by_path(path)
            .with_context(|| format!("failed lookup for path: {path}"))?
            .with_context(|| format!("path not found: {path}"))?;

        let mode = entry.mode();
        let kind = mode.kind();

        match kind {
            EntryKind::Tree => {
                let entries = commit.tree(path)?;
                Ok(PathView::Tree(entries))
            }
            EntryKind::Blob | EntryKind::BlobExecutable => {
                let object = entry.object().context("failed to load blob object")?;
                let blob = object
                    .try_into_blob()
                    .map_err(|_| anyhow::anyhow!("entry is not a blob"))?;

                let id = entry.oid().to_string();
                let size = blob.data.len() as u64;

                // try to decode as UTF-8 text; if it fails, treat as binary
                let lines = std::str::from_utf8(&blob.data)
                    .ok()
                    .map(|s| s.lines().map(|l| l.to_string()).collect());

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
}

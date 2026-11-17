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
        refs::Ref,
        tree::{self, Blob, PathView},
    },
};

#[derive(Clone)]
pub struct Repository {
    pub name: String,
    pub description: String,
    pub owner: Option<String>,
    pub path: PathBuf,
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

    pub fn gix(&self) -> gix::Repository {
        self.gix.to_thread_local()
    }

    pub fn readme(&self) -> Option<String> {
        let gix_repo = self.gix();
        let tree = gix_repo.head().ok()?.peel_to_commit().ok()?.tree().ok()?;

        for entry in tree.iter().flatten() {
            if entry
                .filename()
                .to_string()
                .to_ascii_lowercase()
                .starts_with("readme")
            {
                let blob = entry.object().ok()?.into_blob();
                return Some(String::from_utf8_lossy(&blob.data).into_owned());
            }
        }
        None
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
                format!("{commit}.tar.gz"),
            )
        } else {
            (
                Self::root_tree(&gix_repo, current_ref)?,
                format!("{}.tar.gz", current_ref.name.replace('/', "-")),
            )
        };

        let encoder = GzEncoder::new(Vec::new(), Compression::default());
        let mut builder = Builder::new(encoder);

        Self::add_tree_to_archive(&tree, "", &mut builder)?;

        let bytes = builder.into_inner()?.finish()?;
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
        }
        Ok(())
    }

    pub fn resolve_path(&self, current_ref: &Ref, path: &str) -> anyhow::Result<PathView> {
        let gix_repo = self.gix();
        let root_tree = Self::root_tree(&gix_repo, current_ref)?;
        let path = path.trim_matches('/');

        if path.is_empty() {
            return Ok(PathView::Tree(tree::read_entries(&root_tree, "")?));
        }

        let entry = Self::lookup_entry(&root_tree, path)?;
        let mode = entry.mode();

        match mode.kind() {
            EntryKind::Tree => {
                let subtree = entry.object()?.into_tree();
                Ok(PathView::Tree(tree::read_entries(&subtree, path)?))
            }
            EntryKind::Blob | EntryKind::BlobExecutable => {
                let blob = entry.object()?.into_blob();
                let lines = std::str::from_utf8(&blob.data)
                    .ok()
                    .map(|s| crate::syntax_highlight::highlight_lines(s, path));

                Ok(PathView::Blob(Blob {
                    id: entry.oid().to_string(),
                    mode: mode.value() as u32,
                    size: blob.data.len() as u64,
                    lines,
                }))
            }
            EntryKind::Link => anyhow::bail!("symlinks are not yet supported for direct viewing"),
            EntryKind::Commit => {
                anyhow::bail!("path resolves to a submodule, not a viewable file/dir")
            }
        }
    }

    pub fn read_raw_blob(&self, current_ref: &Ref, path: &str) -> anyhow::Result<Option<Vec<u8>>> {
        let path = path.trim_matches('/');
        if path.is_empty() {
            return Ok(None);
        }

        let gix_repo = self.gix();
        let root_tree = Self::root_tree(&gix_repo, current_ref)?;
        let entry = Self::lookup_entry(&root_tree, path)?;

        match entry.mode().kind() {
            EntryKind::Tree => Ok(None),
            EntryKind::Blob | EntryKind::BlobExecutable => {
                let blob = entry.object()?.into_blob();
                Ok(Some(blob.detach().data))
            }
            EntryKind::Link => anyhow::bail!("symlinks are not yet supported for direct viewing"),
            EntryKind::Commit => {
                anyhow::bail!("path resolves to a submodule, not a viewable file/dir")
            }
        }
    }

    fn root_tree<'a>(
        gix_repo: &'a gix::Repository,
        current_ref: &Ref,
    ) -> anyhow::Result<gix::Tree<'a>> {
        gix_repo
            .find_reference(&current_ref.name)?
            .peel_to_tree()
            .context("failed to get root tree")
    }

    fn lookup_entry<'a>(
        root_tree: &gix::Tree<'a>,
        path: &str,
    ) -> anyhow::Result<gix::object::tree::Entry<'a>> {
        root_tree
            .lookup_entry_by_path(path)
            .with_context(|| format!("failed lookup for path: {path}"))?
            .with_context(|| format!("path not found: {path}"))
    }
}

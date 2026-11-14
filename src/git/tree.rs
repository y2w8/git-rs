use crate::prelude::*;
use anyhow::Context;
use gix::objs::tree::EntryKind;
use serde::Serialize;

#[derive(Clone)]
pub struct TreeEntry {
    pub name: String,
    pub path: String, // full path from repo root, for building links
    pub kind: EntryKind,
    pub size: Option<u64>, // only set for files
    pub mode: u32,
    pub id: String,
}

impl TreeEntry {
    pub fn size_display(&self) -> String {
        match self.size {
            Some(bytes) => format_size(bytes),
            None => "—".to_string(),
        }
    }

    pub fn mode_display(&self) -> String {
        format_mode(self.mode, &self.kind)
    }
}

impl<'repo> Commit<'repo> {
    pub fn tree(&self, dir_path: &str) -> anyhow::Result<Vec<TreeEntry>> {
        let mut entries: Vec<TreeEntry> = Vec::new();

        let base_tree = if dir_path.is_empty() {
            self.gix.tree().context("failed to get root tree")?
        } else {
            self.gix
                .tree()
                .context("failed to get root tree")?
                .lookup_entry_by_path(dir_path)
                .with_context(|| format!("failed lookup for path: {dir_path}"))?
                .with_context(|| format!("path not found: {dir_path}"))?
                .object()
                .context("failed to load object")?
                .try_into_tree()
                .context("path did not resolve to a tree")?
        };

        for entry in base_tree.iter() {
            let entry = entry.context("failed to read tree entry")?;
            let name = entry.filename().to_string();
            let full_path = if dir_path.is_empty() {
                name.clone()
            } else {
                format!("{dir_path}/{name}")
            };
            let id = entry.oid().to_string();
            let mode = entry.mode();
            let kind = mode.kind();

            let size = if matches!(kind, EntryKind::Blob | EntryKind::BlobExecutable) {
                self.gix
                    .repo
                    .find_header(entry.oid())
                    .ok()
                    .map(|h| h.size())
            } else {
                None
            };

            entries.push(TreeEntry {
                name,
                path: full_path,
                kind,
                size,
                mode: mode.value() as u32,
                id,
            });
        }

        Ok(entries)
    }
}

#[derive(Clone, Serialize)]
pub struct Blob {
    pub id: String,
    pub mode: u32,
    pub size: u64,
    pub lines: Option<Vec<String>>, // None => binary, can't show as text
}

impl Blob {
    pub fn mode_display(&self) -> String {
        format_mode(self.mode, &EntryKind::Blob)
    }
}

pub enum PathView {
    Tree(Vec<TreeEntry>),
    Blob(Blob),
}

#[derive(Clone, Serialize)]
pub struct Breadcrumb {
    pub name: String,
    pub path: String, // cumulative path up to and including this segment
}

pub fn build_breadcrumbs(path: &str) -> Vec<Breadcrumb> {
    let mut crumbs = Vec::new();
    let mut acc = String::new();

    for segment in path.split('/').filter(|s| !s.is_empty()) {
        if !acc.is_empty() {
            acc.push('/');
        }
        acc.push_str(segment);
        crumbs.push(Breadcrumb {
            name: segment.to_string(),
            path: acc.clone(),
        });
    }
    crumbs
}

pub fn parent_path(path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    match path.rsplit_once('/') {
        Some((parent, _)) => Some(parent.to_string()),
        None => Some(String::new()), // one level deep -> parent is root
    }
}

pub fn format_mode(mode: u32, kind: &EntryKind) -> String {
    match kind {
        EntryKind::Tree => "drwxr-xr-x".to_string(),
        EntryKind::Link => "lrwxrwxrwx".to_string(),
        EntryKind::Commit => "m---------".to_string(), // submodule / gitlink
        EntryKind::Blob | EntryKind::BlobExecutable => {
            let perm = mode & 0o777;
            let mut s = String::from("-");
            for shift in [6, 3, 0] {
                let bits = (perm >> shift) & 0o7;
                s.push(if bits & 0o4 != 0 { 'r' } else { '-' });
                s.push(if bits & 0o2 != 0 { 'w' } else { '-' });
                s.push(if bits & 0o1 != 0 { 'x' } else { '-' });
            }
            s
        }
    }
}

fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

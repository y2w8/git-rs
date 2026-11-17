use anyhow::Context;
use gix::objs::tree::EntryKind;
use serde::Serialize;

#[derive(Clone)]
pub struct TreeEntry {
    pub name: String,
    pub path: String,
    pub kind: EntryKind,
    pub size: Option<u64>,
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
        format_entry_mode(self.mode, &self.kind)
    }
}

#[derive(Clone, Serialize)]
pub struct Blob {
    pub id: String,
    pub mode: u32,
    pub size: u64,
    pub lines: Option<Vec<String>>,
}

impl Blob {
    pub fn mode_display(&self) -> String {
        format_entry_mode(self.mode, &EntryKind::Blob)
    }
}

pub enum PathView {
    Tree(Vec<TreeEntry>),
    Blob(Blob),
}

#[derive(Clone, Serialize)]
pub struct Breadcrumb {
    pub name: String,
    pub path: String,
}

pub fn read_entries(base_tree: &gix::Tree<'_>, dir_path: &str) -> anyhow::Result<Vec<TreeEntry>> {
    let mut entries = Vec::new();

    for entry in base_tree.iter() {
        let entry = entry.context("failed to read tree entry")?;
        let name = entry.filename().to_string();
        let path = if dir_path.is_empty() {
            name.clone()
        } else {
            format!("{dir_path}/{name}")
        };
        let mode = entry.mode();
        let kind = mode.kind();

        let size = if matches!(kind, EntryKind::Blob | EntryKind::BlobExecutable) {
            base_tree
                .repo
                .find_header(entry.oid())
                .ok()
                .map(|h| h.size())
        } else {
            None
        };

        entries.push(TreeEntry {
            name,
            path,
            kind,
            size,
            mode: mode.value() as u32,
            id: entry.oid().to_string(),
        });
    }

    // Directories first (right under `..`), then alphabetical by name
    entries.sort_by(|a, b| {
        let a_is_dir = matches!(a.kind, EntryKind::Tree);
        let b_is_dir = matches!(b.kind, EntryKind::Tree);
        b_is_dir
            .cmp(&a_is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    });

    Ok(entries)
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
        None => Some(String::new()),
    }
}

pub fn format_entry_mode(mode: u32, kind: &EntryKind) -> String {
    match kind {
        EntryKind::Tree => "drwxr-xr-x".to_string(),
        EntryKind::Link => "lrwxrwxrwx".to_string(),
        EntryKind::Commit => "m---------".to_string(),
        EntryKind::Blob | EntryKind::BlobExecutable => format_rwx('-', mode),
    }
}

pub fn format_mode(mode: u32) -> String {
    let file_type = match mode & 0o170000 {
        0o040000 => 'd',
        0o120000 => 'l',
        0o160000 => 'm',
        0o100000 => '-',
        _ => '?',
    };
    format_rwx(file_type, mode)
}

fn format_rwx(file_type: char, mode: u32) -> String {
    let perms = mode & 0o777;
    let mut out = String::with_capacity(10);
    out.push(file_type);

    for shift in [6, 3, 0] {
        let bits = (perms >> shift) & 0o7;
        out.push(if bits & 0o4 != 0 { 'r' } else { '-' });
        out.push(if bits & 0o2 != 0 { 'w' } else { '-' });
        out.push(if bits & 0o1 != 0 { 'x' } else { '-' });
    }
    out
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

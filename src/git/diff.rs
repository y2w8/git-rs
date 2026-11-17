use std::fmt::{self, Display, Write};

use gix::{
    bstr::{BStr, ByteSlice},
    diff::blob::platform::prepare_diff::Operation,
};

use crate::{git::tree::format_mode, prelude::*};

#[derive(Default)]
pub struct Diff {
    pub file_diffs: Vec<FileDiff>,
    pub output: String,
    pub stats: String,
}

#[derive(Default)]
pub enum FileState {
    Added,
    Removed,
    #[default]
    Changed,
}

impl Display for FileState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Added => write!(f, "diffstat-added"),
            Self::Removed => write!(f, "diffstat-removed"),
            Self::Changed => write!(f, "diffstat-changed"),
        }
    }
}

#[derive(Default)]
pub struct FileDiff {
    pub path: String,
    pub state: FileState,
    pub mode: String,
    pub insertions: usize,
    pub deletions: usize,
    pub total: usize,
    pub insertions_percentage: f64,
    pub deletions_percentage: f64,
    pub total_percentage: f64,
}

#[derive(PartialEq)]
pub enum DiffFormat {
    Plain,
    Patch,
}

impl Diff {
    fn finalize(&mut self, is_patch: bool) -> fmt::Result {
        let files_changed = self.file_diffs.len();
        let insertions: usize = self.file_diffs.iter().map(|f| f.insertions).sum();
        let deletions: usize = self.file_diffs.iter().map(|f| f.deletions).sum();
        let total = deletions.saturating_add(insertions);

        if is_patch {
            self.write_patch_stat_bars(insertions, total)?;
        }
        self.write_summary_line(files_changed, insertions, deletions)?;
        self.compute_percentages(total);
        Ok(())
    }

    fn write_patch_stat_bars(
        &mut self,
        total_insertions: usize,
        total_changes: usize,
    ) -> fmt::Result {
        const WIDTH: usize = 80;

        let (max_name_len, max_change_len) =
            self.file_diffs
                .iter()
                .fold((0, 0), |(max_name, max_change), f| {
                    let digits = ((f.insertions + f.deletions + 1).ilog10() + 1) as usize;
                    (max_name.max(f.path.len()), max_change.max(digits))
                });

        for f in &self.file_diffs {
            let local_changes = f.insertions + f.deletions;
            let width = WIDTH.min(local_changes);

            let add_w = (width * f.insertions) / total_changes;
            let del_w = (width * f.deletions) / total_changes;
            let rem_w = width - (add_w + del_w);

            let plus_str = "+".repeat(add_w + rem_w.min(total_insertions));
            let minus_str = "-".repeat(del_w + (rem_w - rem_w.min(total_insertions)));

            writeln!(
                self.stats,
                " {:max_name_len$} | {:max_change_len$} {plus_str}{minus_str}",
                f.path, local_changes
            )?;
        }
        Ok(())
    }

    fn write_summary_line(
        &mut self,
        files_changed: usize,
        insertions: usize,
        deletions: usize,
    ) -> fmt::Result {
        for (i, (singular, plural, amount)) in [
            ("file changed", "files changed", files_changed),
            ("insertion(+)", "insertions(+)", insertions),
            ("deletion(-)", "deletions(-)", deletions),
        ]
        .into_iter()
        .enumerate()
        {
            if amount == 0 {
                continue;
            }
            let prefix = if i == 0 { "" } else { "," };
            let desc = if amount == 1 { singular } else { plural };
            write!(self.stats, "{prefix} {amount} {desc}")?;
        }
        writeln!(self.stats)
    }

    fn compute_percentages(&mut self, total_changes: usize) {
        for f in &mut self.file_diffs {
            f.total_percentage = if total_changes == 0 {
                0.0
            } else if total_changes < 100 {
                total_changes as f64
            } else {
                (f.total as f64) * 100.0 / (total_changes as f64)
            };

            if f.total > 0 {
                f.insertions_percentage = (f.insertions as f64) * 100.0 / (f.total as f64);
                f.deletions_percentage = (f.deletions as f64) * 100.0 / (f.total as f64);
            }
        }
    }
}

impl<'repo> Commit<'repo> {
    pub fn diff(
        &self,
        repo: &Repository,
        limit_file: Option<&str>,
        format: DiffFormat,
    ) -> Result<Diff, Error> {
        match format {
            DiffFormat::Plain => self.diff_with(repo, limit_file, PlainDiffFormatter),
            DiffFormat::Patch => self.diff_with(repo, limit_file, PatchDiffFormatter),
        }
    }

    fn diff_with<F: DiffFormatter>(
        &self,
        repo: &Repository,
        limit_file: Option<&str>,
        formatter: F,
    ) -> Result<Diff, Error> {
        let mut diff = Diff::default();
        let gix_repo = repo.gix();

        let current_tree = self.gix.tree()?;
        let first_parent = self.gix.parent_ids().next();
        let parent_tree = match first_parent {
            Some(id) => id.object()?.into_commit().tree()?,
            None => gix_repo.empty_tree(),
        };
        let parent_id = first_parent.map(|id| id.to_string());
        let commit_id = self.gix.id.to_string();

        let mut tree_diff_cache = gix_repo.diff_resource_cache_for_tree_diff()?;
        let mut blob_diff_cache = gix_repo.diff_resource_cache_for_tree_diff()?;

        let mut changes = parent_tree.changes()?;
        changes.options(|opts| {
            opts.track_path().track_rewrites(None);
        });
        changes.for_each_to_obtain_tree_with_cache(
            &current_tree,
            &mut tree_diff_cache,
            |change| {
                DiffBuilder {
                    output: &mut diff.output,
                    resource_cache: &mut blob_diff_cache,
                    diffs: &mut diff.file_diffs,
                    formatter: &formatter,
                }
                .handle(change, repo, parent_id.as_deref(), &commit_id, limit_file)
            },
        )?;

        diff.finalize(formatter.format() == DiffFormat::Patch)?;
        Ok(diff)
    }
}

trait DiffFormatter {
    fn file_header(&self, output: &mut String, data: fmt::Arguments<'_>);
    fn write(&self, output: &mut String, class: &str, data: &str);
    fn binary(&self, output: &mut String, left: &str, right: &str);
    fn path_line(
        &self,
        prefix: char,
        is_null: bool,
        state: &FileState,
        repo_name: &str,
        rela_path: &BStr,
        rev_id: &str,
    ) -> String;
    fn format(&self) -> DiffFormat;
}

struct DiffBuilder<'a, F> {
    output: &'a mut String,
    resource_cache: &'a mut gix::diff::blob::Platform,
    diffs: &'a mut Vec<FileDiff>,
    formatter: &'a F,
}

impl<F: DiffFormatter> DiffBuilder<'_, F> {
    fn handle(
        &mut self,
        change: gix::object::tree::diff::Change<'_, '_, '_>,
        repo: &Repository,
        parent_id: Option<&str>,
        commit_id: &str,
        limit_file: Option<&str>,
    ) -> anyhow::Result<gix::object::tree::diff::Action> {
        if !change.entry_mode().is_blob_or_symlink() {
            return Ok(gix::object::tree::diff::Action::Continue(()));
        }

        let path = change.location().to_string();
        if let Some(limit_file) = limit_file
            && !limit_file.is_empty()
            && limit_file != path
        {
            return Ok(gix::object::tree::diff::Action::Continue(()));
        }

        let mut diff = FileDiff {
            path,
            ..Default::default()
        };
        let change = change.diff(self.resource_cache)?;
        let prep = change.resource_cache.prepare_diff()?;

        self.formatter.file_header(
            self.output,
            format_args!(
                "diff --git a/{} b/{}",
                prep.old.rela_path, prep.new.rela_path
            ),
        );

        if prep.old.id.is_null() {
            self.formatter.file_header(
                self.output,
                format_args!("new file mode {}", prep.new.mode.as_octal_str()),
            );
            diff.state = FileState::Added;
        } else if prep.new.id.is_null() {
            self.formatter.file_header(
                self.output,
                format_args!("deleted file mode {}", prep.old.mode.as_octal_str()),
            );
            diff.state = FileState::Removed;
        } else if prep.new.mode != prep.old.mode {
            self.formatter.file_header(
                self.output,
                format_args!("old mode {}", prep.old.mode.as_octal_str()),
            );
            self.formatter.file_header(
                self.output,
                format_args!("new mode {}", prep.new.mode.as_octal_str()),
            );
        }

        let mode_source = if matches!(diff.state, FileState::Removed) {
            prep.old.mode
        } else {
            prep.new.mode
        };
        let raw_mode = u32::from_str_radix(mode_source.as_octal_str().to_str()?, 8)?;
        diff.mode = format_mode(raw_mode);

        let (index_suffix_sep, index_suffix) = if prep.old.mode == prep.new.mode {
            (" ", prep.new.mode.as_octal_str())
        } else {
            ("", BStr::new(&[]))
        };

        let old_path = self.formatter.path_line(
            'a',
            prep.old.id.is_null(),
            &diff.state,
            &repo.name,
            prep.old.rela_path,
            parent_id.unwrap_or_default(),
        );
        let new_path = self.formatter.path_line(
            'b',
            prep.new.id.is_null(),
            &diff.state,
            &repo.name,
            prep.new.rela_path,
            commit_id,
        );

        match prep.operation {
            Operation::InternalDiff { algorithm } => {
                self.formatter.file_header(
                    self.output,
                    format_args!(
                        "index {}..{}{index_suffix_sep}{index_suffix}",
                        prep.old.id.to_hex_with_len(7),
                        prep.new.id.to_hex_with_len(7)
                    ),
                );
                self.formatter
                    .file_header(self.output, format_args!("--- {old_path}"));
                self.formatter
                    .file_header(self.output, format_args!("+++ {new_path}"));

                let old_source = gix::diff::blob::sources::lines(str::from_utf8(
                    prep.old.data.as_slice().unwrap_or_default(),
                )?);
                let new_source = gix::diff::blob::sources::lines(str::from_utf8(
                    prep.new.data.as_slice().unwrap_or_default(),
                )?);
                let input = gix::diff::blob::InternedInput::new(old_source, new_source);
                let output = gix::diff::blob::Diff::compute(algorithm, &input);

                diff.deletions += output.count_removals() as usize;
                diff.insertions += output.count_additions() as usize;
                diff.total = diff.deletions.saturating_add(diff.insertions);

                Self::write_hunks(self.formatter, self.output, &input, &output);
            }
            Operation::ExternalCommand { .. } => {}
            Operation::SourceOrDestinationIsBinary => {
                self.formatter.file_header(
                    self.output,
                    format_args!(
                        "index {}..{}{index_suffix_sep}{index_suffix}",
                        prep.old.id, prep.new.id,
                    ),
                );
                self.formatter.binary(self.output, &old_path, &new_path);
            }
        }

        self.diffs.push(diff);
        self.resource_cache.clear_resource_cache_keep_allocation();
        Ok(gix::object::tree::diff::Action::Continue(()))
    }

    fn write_hunks(
        formatter: &F,
        output: &mut String,
        input: &gix::diff::blob::InternedInput<&str>,
        diff: &gix::diff::blob::Diff,
    ) {
        const CONTEXT_LINES: u32 = 3;

        let hunks: Vec<_> = diff.hunks().collect();
        let mut i = 0;

        while i < hunks.len() {
            let start = i;
            let mut end = i + 1;
            while end < hunks.len()
                && hunks[end]
                    .before
                    .start
                    .saturating_sub(hunks[end - 1].before.end)
                    <= CONTEXT_LINES * 2
            {
                end += 1;
            }
            i = end;

            let first = &hunks[start];
            let last = &hunks[end - 1];

            let before_start = first.before.start.saturating_sub(CONTEXT_LINES);
            let before_end = (last.before.end + CONTEXT_LINES).min(input.before.len() as u32);
            let after_start = first.after.start.saturating_sub(CONTEXT_LINES);
            let after_end = (last.after.end + CONTEXT_LINES).min(input.after.len() as u32);

            formatter.write(
                output,
                "hunk",
                &format!(
                    "@@ -{},{} +{},{} @@\n",
                    before_start + 1,
                    before_end - before_start,
                    after_start + 1,
                    after_end - after_start,
                ),
            );

            let mut before_pos = before_start;

            for hunk in &hunks[start..end] {
                while before_pos < hunk.before.start {
                    let line = input.interner[input.before[before_pos as usize]];
                    formatter.write(output, "context", line);
                    before_pos += 1;
                }

                for pos in hunk.before.clone() {
                    let line = input.interner[input.before[pos as usize]];
                    formatter.write(output, "deletion", line);
                }
                before_pos = hunk.before.end;

                for pos in hunk.after.clone() {
                    let line = input.interner[input.after[pos as usize]];
                    formatter.write(output, "addition", line);
                }
            }

            while before_pos < before_end {
                let line = input.interner[input.before[before_pos as usize]];
                formatter.write(output, "context", line);
                before_pos += 1;
            }
        }
    }
}

struct PlainDiffFormatter;

impl DiffFormatter for PlainDiffFormatter {
    fn file_header(&self, output: &mut String, data: fmt::Arguments<'_>) {
        write!(output, r#"<span class="diff-file-header">{data}</span>"#).unwrap();
        output.push('\n');
    }

    fn write(&self, output: &mut String, class: &str, data: &str) {
        write!(output, r#"<span class="diff-{class}">"#).unwrap();
        v_htmlescape::b_escape(data.as_bytes(), output);
        output.push_str("</span>");
    }

    fn binary(&self, output: &mut String, left: &str, right: &str) {
        writeln!(output, "Binary files {left} and {right} differ").unwrap();
    }

    fn path_line(
        &self,
        prefix: char,
        is_null: bool,
        state: &FileState,
        repo_name: &str,
        rela_path: &BStr,
        rev_id: &str,
    ) -> String {
        if is_null {
            "/dev/null".to_string()
        } else {
            format!(
                "{prefix}/<a class='{state}' href='/{repo_name}/diff/{rela_path}?id={rev_id}'>{rela_path}</a>"
            )
        }
    }

    fn format(&self) -> DiffFormat {
        DiffFormat::Plain
    }
}

struct PatchDiffFormatter;

impl DiffFormatter for PatchDiffFormatter {
    fn file_header(&self, output: &mut String, data: fmt::Arguments<'_>) {
        writeln!(output, "{data}").unwrap();
    }

    fn write(&self, output: &mut String, class: &str, data: &str) {
        let prefix = match class {
            "addition" => "+",
            "deletion" => "-",
            "context" => " ",
            _ => "",
        };
        output.push_str(prefix);
        output.push_str(data);
    }

    fn binary(&self, output: &mut String, left: &str, right: &str) {
        writeln!(output, "Binary files {left} and {right} differ").unwrap();
    }

    fn path_line(
        &self,
        prefix: char,
        is_null: bool,
        _state: &FileState,
        _repo_name: &str,
        rela_path: &BStr,
        _rev_id: &str,
    ) -> String {
        if is_null {
            "/dev/null".to_string()
        } else {
            format!("{prefix}/{rela_path}")
        }
    }

    fn format(&self) -> DiffFormat {
        DiffFormat::Patch
    }
}

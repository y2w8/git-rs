use std::str::FromStr;
use std::{borrow::Cow, collections::HashMap};

use gix::{bstr::BStr, utils::AsBStr};

use crate::{prelude::*, time::Time};

pub struct Commit<'repo> {
    pub id: String,
    pub tree_id: String,
    pub parents: Vec<String>,
    pub author: GitSignature,
    pub committer: GitSignature,
    pub time: Time,

    pub gix: gix::Commit<'repo>,
}

impl<'repo> Commit<'repo> {
    pub fn from_gix(gix: gix::Commit<'_>) -> Result<Commit<'_>, Error> {
        let commit_id = gix.id().to_string();

        let tree_id = gix.tree_id()?.to_string();
        let parents = gix.parent_ids().map(|id| id.to_string()).collect();
        Ok(Commit {
            id: commit_id,
            tree_id,
            parents,
            author: GitSignature::from_gix(gix.author()?),
            committer: GitSignature::from_gix(gix.committer()?),
            time: Time::new(gix.time()?),

            gix,
        })
    }

    pub fn summary(&'repo self) -> Result<Cow<'repo, BStr>, Error> {
        Ok(self.gix.message()?.summary())
    }

    pub fn body(&'repo self) -> Result<&'repo BStr, Error> {
        Ok(self.gix.message()?.body.unwrap_or(BStr::new("")))
    }

    pub fn branch_name(&self, repo: &Repository) -> Result<Option<String>, Error> {
        Ok(self.branch_tips(repo)?.into_iter().next())
    }

    pub fn branch_tips(&self, repo: &Repository) -> Result<Vec<String>, Error> {
        let branch_tips = repo.branch_tips_by_commit()?;
        Ok(branch_tips.get(&self.id).cloned().unwrap_or_default())
    }

    pub fn is_head(&self, repo: &Repository) -> Result<bool, Error> {
        let head_commit_id: Option<String> =
            repo.gix().head_commit().ok().map(|c| c.id().to_string());
        Ok(head_commit_id.as_deref() == Some(&self.id))
    }
}

impl Repository {
    /// Looks up a Commit by its 40-character hex SHA-1 hash.
    pub fn find_commit<'repo>(
        &self,
        gix_repo: &'repo gix::Repository,
        id: &str,
    ) -> Result<Commit<'repo>, Error> {
        let oid = gix::ObjectId::from_str(id)?;
        Commit::from_gix(gix_repo.find_commit(oid)?)
    }

    /// Resolves a Commit either from an explicit `?id=<sha>` query parameter,
    /// or by peeling the active branch/tag reference (`current_ref`) to its latest commit.
    pub fn resolve_commit<'repo>(
        &self,
        gix_repo: &'repo gix::Repository,
        id: Option<&str>,
        current_ref: &Ref,
    ) -> Result<Commit<'repo>, Error> {
        if let Some(id) = id {
            self.find_commit(gix_repo, id)
        } else {
            let mut reference = gix_repo.find_reference(&current_ref.name)?;
            Commit::from_gix(reference.peel_to_commit()?)
        }
    }

    /// Returns the latest commit on `HEAD` (or falls back to the first valid reference in the repo).
    pub fn last_commit<'repo>(
        &self,
        gix_repo: &'repo gix::Repository,
    ) -> Result<Option<Commit<'repo>>, Error> {
        if let Ok(commit) = gix_repo.head_commit() {
            return Ok(Some(Commit::from_gix(commit)?));
        }
        for reference in gix_repo.references()?.all()? {
            if let Ok(commit) = reference?.peel_to_commit() {
                return Ok(Some(Commit::from_gix(commit)?));
            }
        }
        Ok(None)
    }

    /// Builds a map of `commit_sha -> [branch_names]` for every local branch (`refs/heads/*`).
    pub fn branch_tips_by_commit(&self) -> Result<HashMap<String, Vec<String>>, Error> {
        let gix_repo = self.gix();
        let references = gix_repo.references()?;
        let local_branches = references.local_branches()?;

        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for branch in local_branches {
            let mut branch = branch?;
            let commit_id = branch.peel_to_commit()?.id().to_string();
            map.entry(commit_id)
                .or_default()
                .push(branch.name().shorten().to_string());
        }
        Ok(map)
    }

    /// Walks the commit graph backwards starting from `current_ref` (like `git log`),
    /// sorted newest-first by commit timestamp, with optional filtering and pagination.
    pub fn commits<'repo>(
        &self,
        gix_repo: &'repo gix::Repository,
        current_ref: &Ref,
        offset: usize,
        limit: usize,
        filter: Option<&dyn Fn(&Commit) -> bool>,
    ) -> Result<(Vec<Commit<'repo>>, bool), Error> {
        // 1. Parse the branch/tag name (e.g. "main") into its starting commit ObjectId
        let start = gix_repo.rev_parse_single(current_ref.name.as_bstr())?;
        // 2. Walk parent links backwards from `start`, ordered by commit time
        let revwalk = gix_repo
            .rev_walk([start])
            .sorting(gix::revision::walk::Sorting::ByCommitTime(
                Default::default(),
            ))
            .all()?;

        let mut commits: Vec<Commit> = Vec::new();
        let mut has_next = false;
        let mut matched = 0usize;

        for rev in revwalk {
            let rev = rev?;
            let object = rev.object()?;
            let commit = Commit::from_gix(object)?;
            if let Some(f) = filter
                && !f(&commit)
            {
                continue;
            }

            if matched < offset {
                matched += 1;
                continue;
            }

            if commits.len() >= limit {
                has_next = true;
                break;
            }

            commits.push(commit);
        }

        Ok((commits, has_next))
    }
}

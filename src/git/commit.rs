use std::{borrow::Cow, collections::HashMap, str::FromStr};

use gix::{bstr::BStr, utils::AsBStr};

use crate::{prelude::*, time::Time};

pub struct Commit<'repo> {
    pub id: String,
    pub tree_id: String,
    pub parents: Vec<String>,
    pub author: GitSignature,
    pub committer: GitSignature,
    pub time: Time,
    pub is_head: bool,
    pub branch_tips: Vec<String>,
    pub gix: gix::Commit<'repo>,
}

impl<'repo> Commit<'repo> {
    pub fn from_gix(gix: gix::Commit<'_>) -> Result<Commit<'_>, Error> {
        // Decode the raw commit object once instead of re-parsing it 5 times via
        // `.tree_id()`, `.parent_ids()`, `.author()`, `.committer()`, and `.time()`.
        let (tree_id, parents, author, committer) = {
            let decoded = gix.decode()?;
            (
                decoded.tree().to_string(),
                decoded.parents().map(|id| id.to_string()).collect(),
                GitSignature::from_gix(decoded.author()?),
                GitSignature::from_gix(decoded.committer()?),
            )
        };
        let time = committer.time.clone();

        Ok(Commit {
            id: gix.id().to_string(),
            tree_id,
            parents,
            author,
            committer,
            time,
            is_head: false,
            branch_tips: Vec::new(),
            gix,
        })
    }

    pub fn summary(&'repo self) -> Result<Cow<'repo, BStr>, Error> {
        Ok(self.gix.message()?.summary())
    }

    pub fn body(&'repo self) -> Result<&'repo BStr, Error> {
        Ok(self.gix.message()?.body.unwrap_or(BStr::new("")))
    }

    pub fn branch_name(&self, _repo: &Repository) -> Result<Option<&str>, Error> {
        Ok(self.branch_tips.first().map(String::as_str))
    }

    pub fn branch_tips(&self, _repo: &Repository) -> Result<&[String], Error> {
        Ok(&self.branch_tips)
    }

    pub fn is_head(&self, _repo: &Repository) -> Result<bool, Error> {
        Ok(self.is_head)
    }
}

impl Repository {
    pub fn find_commit<'repo>(
        &self,
        gix_repo: &'repo gix::Repository,
        id: &str,
    ) -> Result<Commit<'repo>, Error> {
        let oid = gix::ObjectId::from_str(id)?;
        Commit::from_gix(gix_repo.find_commit(oid)?)
    }

    pub fn resolve_commit<'repo>(
        &self,
        gix_repo: &'repo gix::Repository,
        id: Option<&str>,
        current_ref: &Ref,
    ) -> Result<Commit<'repo>, Error> {
        let mut commit = if let Some(id) = id {
            self.find_commit(gix_repo, id)?
        } else {
            let mut reference = gix_repo.find_reference(&current_ref.name)?;
            Commit::from_gix(reference.peel_to_commit()?)?
        };
        let head_id = gix_repo.head_id().ok().map(|id| id.to_string());
        let mut branch_tips = Self::branch_tips_from_repo(gix_repo)?;
        commit.is_head = head_id.as_deref() == Some(&commit.id);
        commit.branch_tips = branch_tips.remove(&commit.id).unwrap_or_default();
        Ok(commit)
    }

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

    /// Returns the committer `Time` of the repository's latest commit without allocating
    /// a full `Commit` struct.
    pub fn last_commit_time(&self) -> Option<Time> {
        let gix_repo = self.gix();
        if let Ok(commit) = gix_repo.head_commit()
            && let Ok(sig) = commit.committer()
            && let Ok(t) = sig.time()
        {
            return Some(Time::new(t));
        }
        for reference in gix_repo.references().ok()?.all().ok()? {
            if let Ok(commit) = reference.ok()?.peel_to_commit()
                && let Ok(sig) = commit.committer()
                && let Ok(t) = sig.time()
            {
                return Some(Time::new(t));
            }
        }
        None
    }

    /// Returns the committer timestamp (in seconds) of the repository's latest commit,
    /// or `i64::MIN` if the repository has no commits yet.
    pub fn last_commit_timestamp(&self) -> i64 {
        self.last_commit_time()
            .map(|t| t.seconds())
            .unwrap_or(i64::MIN)
    }

    pub fn branch_tips_by_commit(&self) -> Result<HashMap<String, Vec<String>>, Error> {
        let gix_repo = self.gix();
        Self::branch_tips_from_repo(&gix_repo)
    }

    fn branch_tips_from_repo(
        gix_repo: &gix::Repository,
    ) -> Result<HashMap<String, Vec<String>>, Error> {
        let references = gix_repo.references()?;
        let mut map: HashMap<String, Vec<String>> = HashMap::new();

        for branch in references.local_branches()? {
            let mut branch = branch?;
            let commit_id = branch.peel_to_commit()?.id().to_string();
            map.entry(commit_id)
                .or_default()
                .push(branch.name().shorten().to_string());
        }
        Ok(map)
    }

    pub fn commits<'repo>(
        &self,
        gix_repo: &'repo gix::Repository,
        current_ref: &Ref,
        offset: usize,
        limit: usize,
        filter: Option<&dyn Fn(&Commit) -> bool>,
    ) -> Result<(Vec<Commit<'repo>>, bool), Error> {
        let start = gix_repo.rev_parse_single(current_ref.name.as_bstr())?;
        let revwalk = gix_repo
            .rev_walk([start])
            .sorting(gix::revision::walk::Sorting::ByCommitTime(
                Default::default(),
            ))
            .all()?;

        let head_id = gix_repo.head_id().ok().map(|id| id.to_string());
        let mut branch_tips = Self::branch_tips_from_repo(gix_repo)?;

        let mut commits = Vec::with_capacity(limit);
        let mut has_next = false;
        let mut matched = 0usize;

        for rev in revwalk {
            let rev = rev?;
            if filter.is_none() && matched < offset {
                matched += 1;
                continue;
            }

            let mut commit = Commit::from_gix(rev.object()?)?;
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

            commit.is_head = head_id.as_deref() == Some(&commit.id);
            commit.branch_tips = branch_tips.remove(&commit.id).unwrap_or_default();
            commits.push(commit);
        }

        Ok((commits, has_next))
    }
}

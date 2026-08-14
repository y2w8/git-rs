use crate::{git::GitSignature, prelude::*, time::Time};
use gix::Reference;

pub struct Branch<'repo> {
    pub name: String,
    pub last_commit: Commit<'repo>,
    pub author: GitSignature,
    pub time: Time,
    pub gix: Reference<'repo>,
}

impl<'repo> Branch<'repo> {
    pub fn from_gix(mut reference: Reference<'repo>) -> Result<Branch<'repo>, Error> {
        let commit = reference.peel_to_commit()?;

        Ok(Branch {
            name: reference.name().shorten().to_string(),
            last_commit: Commit::from_gix(commit.clone())?,
            author: GitSignature::from_gix(commit.author()?),
            time: Time::new(commit.time()?),
            gix: reference,
        })
    }

    pub fn is_head(&self, repo: &Repository) -> Result<bool, Error> {
        Ok(repo.head_branch_name()? == self.name)
    }
}

impl Repository {
    pub fn head_branch_name(&self) -> Result<String, Error> {
        let gix_repo = self.gix();
        let head = gix_repo.head()?;
        let branch_name = match head.referent_name() {
            Some(b) => b.shorten().to_string(),
            None => head.name().shorten().to_string(),
        };
        Ok(branch_name)
    }

    pub fn branches<'repo>(
        &self,
        gix_repo: &'repo gix::Repository,
        offset: usize,
        limit: usize,
    ) -> Result<(Vec<Branch<'repo>>, bool), Error> {
        let references = gix_repo.references()?;
        let gix_branches = references.local_branches()?;

        let mut branches: Vec<Branch> = Vec::new();
        let mut has_next = false;

        for (i, branch) in gix_branches.skip(offset).enumerate() {
            if i >= limit {
                has_next = true;
                break;
            }

            let branch = branch?;
            branches.push(Branch::from_gix(branch)?);
        }
        Ok((branches, has_next))
    }

    pub fn branch_names(&self) -> Result<Vec<String>, Error> {
        let gix_repo = self.gix();
        let references = gix_repo.references()?;
        let gix_branches = references.local_branches()?;

        let mut names: Vec<String> = Vec::new();

        for branch in gix_branches {
            let branch = branch?;
            names.push(branch.name().shorten().to_string());
        }
        Ok(names)
    }
}

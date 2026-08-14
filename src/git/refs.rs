use crate::{prelude::*, routes::PageQuery};

pub struct Ref {
    pub name: String,
    pub is_head: bool,
}

impl Ref {
    pub fn resolve(repo: &Repository, query: &PageQuery) -> Result<Ref, Error> {
        let head_branch_name = repo.head_branch_name()?;
        match &query.head {
            Some(h) if !h.is_empty() => Ok(Ref {
                name: h.clone(),
                is_head: *h == head_branch_name,
            }),
            _ => Ok(Ref {
                name: head_branch_name,
                is_head: true,
            }),
        }
    }
}

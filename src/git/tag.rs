use anyhow::Context;
use gix::objs::Kind;

use crate::prelude::*;

pub enum TaggedObject {
    Commit(String),
    Tree(String),
}

impl TaggedObject {
    fn from_kind_and_id(kind: Kind, id: String) -> Option<Self> {
        match kind {
            Kind::Commit => Some(Self::Commit(id)),
            Kind::Tree => Some(Self::Tree(id)),
            _ => None,
        }
    }
}

pub struct Tag {
    pub name: String,
    pub id: String,
    pub message: String,
    pub tagger: Option<GitSignature>,
    pub tagged_object: Option<TaggedObject>,
    pub time: Time,
}

impl Tag {
    /// Builds a `Tag` from a `refs/tags/*` Git reference.
    ///
    /// Git has two types of tags:
    /// 1. **Annotated tags** (`git tag -a`): store a dedicated Tag object with a tagger, date, and message.
    /// 2. **Lightweight tags** (`git tag`): just a reference pointing directly at a Commit (no Tag object).
    pub fn from_gix(mut gix: gix::Reference<'_>) -> anyhow::Result<Tag> {
        let name = gix.name().shorten().to_string();

        if let Ok(tag_obj) = gix.peel_to_tag() {
            // Case 1: Annotated tag
            let tag_ref = tag_obj.decode()?;
            let message = tag_ref.message.to_string();
            let tagged_object =
                TaggedObject::from_kind_and_id(tag_ref.target_kind, tag_ref.target.to_string());
            let tagger = tag_ref.tagger()?.map(GitSignature::from_gix);

            let time = match &tagger {
                Some(t) => t.time.clone(),
                None => Self::commit_time(&mut gix)?,
            };

            Ok(Tag {
                name,
                id: gix.id().to_string(),
                message,
                tagger,
                tagged_object,
                time,
            })
        } else {
            // Case 2: Lightweight tag (points directly to a Commit or Tree)
            let target = gix
                .follow_to_object()
                .context("Failed to resolve lightweight tag target")?;
            let target_kind = target.header()?.kind();
            let tagged_object = TaggedObject::from_kind_and_id(target_kind, target.to_string());
            let time = Self::commit_time(&mut gix)?;

            Ok(Tag {
                name,
                id: gix.id().to_string(),
                message: String::new(),
                tagger: None,
                tagged_object,
                time,
            })
        }
    }

    fn commit_time(gix: &mut gix::Reference<'_>) -> anyhow::Result<Time> {
        let commit = gix
            .peel_to_commit()
            .context("Couldn't get to a commit from the given reference!")?;
        Ok(Time::new(commit.time()?))
    }
}

impl Repository {
    pub fn find_tag(&self, name: &str) -> anyhow::Result<Tag> {
        let gix_repo = self.gix();
        let reference = gix_repo.find_reference(&format!("tags/{name}"))?;
        Tag::from_gix(reference)
    }

    pub fn tags(&self, offset: usize, limit: usize) -> anyhow::Result<(Vec<Tag>, bool)> {
        let gix_repo = self.gix();
        let references = gix_repo.references()?;
        let gix_tags = references.tags()?;

        let mut tags: Vec<Tag> = Vec::new();
        let mut has_next = false;

        for (i, rev) in gix_tags.skip(offset).enumerate() {
            if i >= limit {
                has_next = true;
                break;
            }

            let rev = rev.map_err(|e| anyhow::anyhow!(e))?;
            tags.push(Tag::from_gix(rev)?);
        }
        Ok((tags, has_next))
    }
}

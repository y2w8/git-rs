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
    pub fn from_gix(mut gix: gix::Reference<'_>) -> anyhow::Result<Tag> {
        let name = gix.name().shorten().to_string();
        let id = gix.id().to_string();

        let (message, tagger, tagged_object, time) = if let Ok(tag_obj) = gix.peel_to_tag() {
            let tag_ref = tag_obj.decode()?;
            let message = tag_ref.message.to_string();
            let tagged_object =
                TaggedObject::from_kind_and_id(tag_ref.target_kind, tag_ref.target.to_string());
            let tagger = tag_ref.tagger()?.map(GitSignature::from_gix);
            let time = match &tagger {
                Some(t) => t.time.clone(),
                None => Self::commit_time(&mut gix)?,
            };
            (message, tagger, tagged_object, time)
        } else {
            let target = gix
                .follow_to_object()
                .context("Failed to resolve lightweight tag target")?;
            let tagged_object =
                TaggedObject::from_kind_and_id(target.header()?.kind(), target.to_string());
            let time = Self::commit_time(&mut gix)?;
            (String::new(), None, tagged_object, time)
        };

        Ok(Tag {
            name,
            id,
            message,
            tagger,
            tagged_object,
            time,
        })
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
        let mut all_tags: Vec<Tag> = references
            .tags()?
            .map(|rev| {
                let rev = rev.map_err(|e| anyhow::anyhow!(e))?;
                Tag::from_gix(rev)
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        // Sort tags descending (newest / highest version first, e.g. v0.2.0 -> v0.1.1 -> v0.1.0)
        all_tags.sort_by(|a, b| {
            b.time
                .seconds()
                .cmp(&a.time.seconds())
                .then_with(|| b.name.cmp(&a.name))
        });

        let has_next = all_tags.len() > offset.saturating_add(limit);
        let tags = all_tags.into_iter().skip(offset).take(limit).collect();
        Ok((tags, has_next))
    }
}

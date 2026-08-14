use anyhow::Context;
use gix::objs::{Kind, TagRef};

use crate::prelude::*;

pub enum TaggedObject {
    Commit(String),
    Tree(String),
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
        let tag = match gix.peel_to_tag() {
            Ok(tag) => tag.detach().data,
            Err(error) => Self::lightweight_tag_object_data(&mut gix).with_context(|| {
                format!("Couldn't get to a tag from the given reference: {error}")
            })?,
        };
        let tag_ref = TagRef::from_bytes(&tag, gix::hash::Kind::shortest())?;
        let message = tag_ref.message.to_string();
        let tagged_object = match tag_ref.target_kind {
            Kind::Commit => Some(TaggedObject::Commit(tag_ref.target.to_string())),
            Kind::Tree => Some(TaggedObject::Tree(tag_ref.target.to_string())),
            _ => None,
        };

        let time: Time;
        let mut tagger: Option<GitSignature> = None;
        if let Some(sig) = tag_ref.tagger()? {
            tagger = Some(GitSignature::from_gix(sig))
        }

        if let Some(ref tagger) = tagger {
            time = tagger.time.clone()
        } else {
            time = Time::new(
                gix.peel_to_commit()
                    .with_context(|| "Couldn't get to a commit from the given reference!")?
                    .time()?,
            )
        }

        Ok(Tag {
            name: gix.name().shorten().to_string(),
            id: gix.id().to_string(),
            message,
            tagger,
            tagged_object,
            time,
        })
    }

    fn lightweight_tag_object_data(reference: &mut gix::Reference) -> anyhow::Result<Vec<u8>> {
        let target = reference
            .follow_to_object()
            .context("Failed to resolve lightweight tag target")?;
        let tag_name = reference.name().shorten();
        let target_kind = target.header()?.kind();

        if target_kind == Kind::Tag {
            anyhow::bail!("reference resolves to an annotated tag object");
        }

        Ok(format!("object {target}\ntype {target_kind}\ntag {tag_name}\n\n").into_bytes())
    }
}

impl Repository {
    pub fn find_tag(&self, name: &str) -> anyhow::Result<Tag> {
        let gix_repo = self.gix();
        Tag::from_gix(
            gix_repo
                .find_reference(&format!("tags/{}", name))?
                .to_owned(),
        )
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

            let rev = rev.unwrap();
            tags.push(Tag::from_gix(rev)?);
        }
        Ok((tags, has_next))
    }
}

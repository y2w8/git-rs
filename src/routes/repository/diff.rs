use bytes::{BufMut, BytesMut};
use clap::crate_version;
use std::fmt::Write;

use crate::{
    git::diff::{Diff, DiffFormat},
    prelude::*,
    routes::PageQuery,
};

#[derive(Template)]
#[template(path = "repository/diff.html")]
struct DiffPage<'repo> {
    repo: &'repo Repository,
    branch_names: Vec<String>,
    current_ref: Ref,
    file_path: Option<String>,
    diff: Diff,
    query: web::Query<PageQuery>,
}

#[get("/{repo}/diff/{file_path:.*}")]
pub async fn page(
    path: web::Path<(String, Option<String>)>,
    query: web::Query<PageQuery>,
    data: web::Data<App>,
) -> Result<HttpResponse, Error> {
    let (name, file_path) = path.into_inner();
    let file_path = file_path.filter(|p| !p.is_empty());
    let repo = data.get_repo(&name)?;
    let gix_repo = repo.gix();

    let current_ref = Ref::resolve(&repo, &query)?;
    let branch_names = repo.branch_names()?;
    let commit = repo.resolve_commit(&gix_repo, query.id.as_deref(), &current_ref)?;
    let diff = commit.diff(&repo, file_path.as_deref(), DiffFormat::Plain)?;

    render_html(&DiffPage {
        repo: &repo,
        branch_names,
        current_ref,
        file_path,
        diff,
        query,
    })
}

#[get("/{repo}/patch/")]
pub async fn patch(
    path: web::Path<String>,
    query: web::Query<PageQuery>,
    data: web::Data<App>,
) -> Result<HttpResponse, Error> {
    let repo = data.get_repo(&path)?;
    let gix_repo = repo.gix();

    let current_ref = Ref::resolve(&repo, &query)?;
    let commit = repo.resolve_commit(&gix_repo, query.id.as_deref(), &current_ref)?;
    let diff = commit.diff(&repo, None, DiffFormat::Patch)?;

    let mut data = BytesMut::new();
    writeln!(data, "From {} Mon Sep 17 00:00:00 2001", commit.id)?;
    writeln!(
        data,
        "From: {} <{}>",
        commit.author.name, commit.author.email
    )?;

    write!(data, "Date: ")?;
    let mut writer = data.writer();
    commit.author.time.format_rfc(&mut writer);
    let mut data = writer.into_inner();
    writeln!(data)?;

    writeln!(data, "Subject: [PATCH] {}\n", commit.summary()?)?;
    write!(data, "{}", commit.body()?)?;
    writeln!(data, "---")?;

    data.extend_from_slice(diff.stats.as_bytes());
    data.extend_from_slice(b"\n");
    data.extend_from_slice(diff.output.as_bytes());

    writeln!(data, "--\ngit-rs {}", crate_version!())?;

    Ok(HttpResponse::Ok().content_type("text").body(data))
}

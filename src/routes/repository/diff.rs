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
) -> ActixResult<HttpResponse> {
    let (name, file_path) = path.into_inner();
    let repo = match data.find_repo(name) {
        Some(r) => r,
        None => return Ok(HttpResponse::NotFound().body("Repo not found")),
    };
    let gix_repo = repo.gix();

    let current_ref = match Ref::resolve(&repo, &query) {
        Ok(r) => r,
        Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
    };
    let branch_names = match repo.branch_names() {
        Ok(b) => b,
        Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
    };
    let commit: Commit = if let Some(id) = query.clone().into_inner().id {
        match repo.find_commit(&gix_repo, &id) {
            Ok(c) => c,
            Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
        }
    } else {
        match gix_repo.find_reference(&format!("heads/{}", current_ref.name)) {
            Ok(mut r) => match r.peel_to_commit() {
                Ok(c) => Commit::from_gix(c).unwrap(),
                Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
            },
            Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
        }
    };

    let diff: Diff = match commit.diff(&repo, file_path.clone(), DiffFormat::Plain) {
        Ok(d) => d,
        Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
    };

    let rendered = DiffPage {
        repo: &repo,
        branch_names,
        current_ref,
        file_path,
        diff,
        query,
    }
    .render();
    match rendered {
        Ok(html) => Ok(HttpResponse::Ok().content_type("text/html").body(html)),
        Err(e) => Ok(HttpResponse::InternalServerError().body(e.to_string())),
    }
}

#[get("/{repo}/patch/")]
pub async fn patch(
    path: web::Path<String>,
    query: web::Query<PageQuery>,
    data: web::Data<App>,
) -> ActixResult<HttpResponse> {
    let name = path.into_inner();
    let repo = match data.find_repo(name) {
        Some(r) => r,
        None => return Ok(HttpResponse::NotFound().body("Repo not found")),
    };
    let gix_repo = repo.gix();

    let current_ref = match Ref::resolve(&repo, &query) {
        Ok(r) => r,
        Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
    };
    let commit: Commit = if let Some(id) = query.clone().into_inner().id {
        match repo.find_commit(&gix_repo, &id) {
            Ok(c) => c,
            Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
        }
    } else {
        match gix_repo.find_reference(&format!("heads/{}", current_ref.name)) {
            Ok(mut r) => match r.peel_to_commit() {
                Ok(c) => Commit::from_gix(c).unwrap(),
                Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
            },
            Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
        }
    };

    let diff: Diff = match commit.diff(&repo, None, DiffFormat::Patch) {
        Ok(d) => d,
        Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
    };

    let mut data = BytesMut::new();
    writeln!(data, "From {} Mon Sep 17 00:00:00 2001", commit.id).unwrap();
    writeln!(
        data,
        "From: {} <{}>",
        commit.author.name, commit.author.email
    )
    .unwrap();

    write!(data, "Date: ").unwrap();
    let mut writer = data.writer();
    commit.author.time.format_rfc(&mut writer);
    let mut data = writer.into_inner();
    writeln!(data).unwrap();

    writeln!(data, "Subject: [PATCH] {}\n", commit.summary().unwrap()).unwrap();

    write!(data, "{}", commit.body().unwrap()).unwrap();

    writeln!(data, "---").unwrap();

    data.extend_from_slice(diff.stats.as_bytes());
    data.extend_from_slice(b"\n");
    data.extend_from_slice(diff.output.as_bytes());

    writeln!(data, "--\ngit-rs {}", crate_version!()).unwrap();

    Ok(HttpResponse::Ok().content_type("text").body(data))
}

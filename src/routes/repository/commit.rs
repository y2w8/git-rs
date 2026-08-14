use crate::{
    git::diff::{Diff, DiffFormat},
    prelude::*,
    routes::PageQuery,
};

#[derive(Template)]
#[template(path = "repository/commit.html")]
struct CommitPage<'repo> {
    repo: &'repo Repository,
    branch_names: Vec<String>,
    current_ref: Ref,
    commit: Commit<'repo>,
    diff: Diff,
    query: web::Query<PageQuery>,
}

#[get("/{repo}/commit/")]
pub async fn page(
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
        match gix_repo.find_reference(&current_ref.name.to_string()) {
            Ok(mut r) => match r.peel_to_commit() {
                Ok(c) => Commit::from_gix(c).unwrap(),
                Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
            },
            Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
        }
    };

    let diff: Diff = match commit.diff(&repo, None, DiffFormat::Plain) {
        Ok(d) => d,
        Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
    };

    let rendered = CommitPage {
        repo: &repo,
        branch_names,
        current_ref,
        commit,
        diff,
        query,
    }
    .render();
    match rendered {
        Ok(html) => Ok(HttpResponse::Ok().content_type("text/html").body(html)),
        Err(e) => Ok(HttpResponse::InternalServerError().body(e.to_string())),
    }
}

use crate::{
    prelude::*,
    routes::{PAGE_SIZE, PageQuery},
};

const SUMMARY_LIMIT: usize = 10;

#[derive(Template)]
#[template(path = "repository/summary.html")]
struct SummaryPage<'repo> {
    repo: &'repo Repository,
    current_ref: Ref,
    offset: usize,
    branch_names: Vec<String>,
    branches: Vec<Branch<'repo>>,
    branches_has_more: bool,
    tags: Vec<Tag>,
    tags_has_more: bool,
    commits: Vec<Commit<'repo>>,
    commits_has_more: bool,
    query: web::Query<PageQuery>,
}

#[get("/{repo}/")]
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

    let (branches, branches_has_more) = match repo.branches(&gix_repo, 0, SUMMARY_LIMIT) {
        Ok(c) => c,
        Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
    };

    let (tags, tags_has_more) = match repo.tags(0, SUMMARY_LIMIT) {
        Ok(t) => t,
        Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
    };

    let (commits, commits_has_more) =
        match repo.commits(&gix_repo, &current_ref, 0, SUMMARY_LIMIT, None) {
            Ok(c) => c,
            Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
        };

    let rendered = SummaryPage {
        repo: &repo,
        current_ref,
        offset: 0,
        branch_names,
        branches,
        branches_has_more,
        tags,
        tags_has_more,
        commits,
        commits_has_more,
        query,
    }
    .render();

    match rendered {
        Ok(html) => Ok(HttpResponse::Ok().content_type("text/html").body(html)),
        Err(e) => Ok(HttpResponse::InternalServerError().body(e.to_string())),
    }
}

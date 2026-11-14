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
) -> Result<HttpResponse, Error> {
    let repo = data.get_repo(&path)?;
    let gix_repo = repo.gix();

    let current_ref = Ref::resolve(&repo, &query)?;
    let branch_names = repo.branch_names()?;
    let (branches, branches_has_more) = repo.branches(&gix_repo, 0, SUMMARY_LIMIT)?;
    let (tags, tags_has_more) = repo.tags(0, SUMMARY_LIMIT)?;
    let (commits, commits_has_more) =
        repo.commits(&gix_repo, &current_ref, 0, SUMMARY_LIMIT, None)?;

    render_html(&SummaryPage {
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
    })
}

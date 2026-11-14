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
) -> Result<HttpResponse, Error> {
    let repo = data.get_repo(&path)?;
    let gix_repo = repo.gix();

    let current_ref = Ref::resolve(&repo, &query)?;
    let branch_names = repo.branch_names()?;
    let commit = repo.resolve_commit(&gix_repo, query.id.as_deref(), &current_ref)?;
    let diff = commit.diff(&repo, None, DiffFormat::Plain)?;

    render_html(&CommitPage {
        repo: &repo,
        branch_names,
        current_ref,
        commit,
        diff,
        query,
    })
}

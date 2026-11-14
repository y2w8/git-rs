use crate::{
    prelude::*,
    routes::{PAGE_SIZE, PageQuery},
};

#[derive(Template)]
#[template(path = "repository/log.html")]
struct LogPage<'repo> {
    repo: &'repo Repository,
    branch_names: Vec<String>,
    current_ref: Ref,
    commits: Vec<Commit<'repo>>,
    offset: usize,
    has_prev: bool,
    has_next: bool,
    query: PageQuery,
}

#[get("/{repo}/log/")]
pub async fn page(
    path: web::Path<String>,
    query: web::Query<PageQuery>,
    data: web::Data<App>,
) -> Result<HttpResponse, Error> {
    let query = query.into_inner();
    let repo = data.get_repo(&path)?;
    let gix_repo = repo.gix();

    let current_ref = Ref::resolve(&repo, &query)?;
    let branch_names = repo.branch_names()?;

    let offset = query.offset.unwrap_or(0);
    let has_prev = offset >= 1;

    let value_lower = query.value.as_deref().unwrap_or_default().to_lowercase();
    let filter: Option<&dyn Fn(&Commit) -> bool> =
        match (query.query_type.as_deref(), query.value.is_some()) {
            (Some("msg"), true) => Some(&|c| {
                c.summary()
                    .map(|s| s.to_string().to_lowercase().contains(&value_lower))
                    .unwrap_or(false)
            }),
            (Some("author"), true) => {
                Some(&|c| c.author.name.to_lowercase().contains(&value_lower))
            }
            (Some("committer"), true) => {
                Some(&|c| c.committer.name.to_lowercase().contains(&value_lower))
            }
            _ => None,
        };

    let (commits, has_next) = repo.commits(&gix_repo, &current_ref, offset, PAGE_SIZE, filter)?;

    render_html(&LogPage {
        repo: &repo,
        branch_names,
        current_ref,
        commits,
        offset,
        has_prev,
        has_next,
        query,
    })
}

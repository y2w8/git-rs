use crate::{
    git::tag::Tag,
    prelude::*,
    routes::{PAGE_SIZE, PageQuery},
};

#[derive(Template)]
#[template(path = "repository/refs.html")]
struct RefsPage<'repo> {
    repo: &'repo Repository,
    current_ref: Ref,
    offset: usize,
    mode: String,
    branch_names: Vec<String>,
    branches: Vec<Branch<'repo>>,
    tags: Vec<Tag>,
    has_prev: bool,
    branches_has_more: bool,
    tags_has_more: bool,
    query: web::Query<PageQuery>,
}

#[get("/{repo}/refs/{mode:.*}")]
pub async fn page(
    path: web::Path<(String, String)>,
    query: web::Query<PageQuery>,
    data: web::Data<App>,
) -> Result<HttpResponse, Error> {
    let (name, mode) = path.into_inner();
    let repo = data.get_repo(&name)?;
    let gix_repo = repo.gix();

    let current_ref = Ref::resolve(&repo, &query)?;
    let branch_names = repo.branch_names()?;
    let offset = query.offset.unwrap_or(0);

    let (branches, branches_has_more, tags, tags_has_more, has_prev) = match mode.as_str() {
        "heads" => {
            let (branches, has_more) = repo.branches(&gix_repo, offset, PAGE_SIZE)?;
            (branches, has_more, Vec::new(), false, offset >= 1)
        }
        "tags" => {
            let (tags, has_more) = repo.tags(offset, PAGE_SIZE)?;
            (Vec::new(), false, tags, has_more, offset >= 1)
        }
        _ => {
            let (branches, branches_has_more) = repo.branches(&gix_repo, 0, PAGE_SIZE / 2)?;
            let (tags, tags_has_more) = repo.tags(0, PAGE_SIZE / 2)?;
            (branches, branches_has_more, tags, tags_has_more, false)
        }
    };

    render_html(&RefsPage {
        repo: &repo,
        current_ref,
        offset,
        mode,
        branch_names,
        branches,
        tags,
        has_prev,
        branches_has_more,
        tags_has_more,
        query,
    })
}

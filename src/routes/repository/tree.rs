use crate::{
    git::tree::{self, Breadcrumb, PathView},
    prelude::*,
    routes::PageQuery,
};
use gix::objs::tree::EntryKind;

#[derive(Template)]
#[template(path = "repository/tree.html")]
struct TreePage {
    repo: Repository,
    branch_names: Vec<String>,
    current_ref: Ref,
    filepath: String,
    breadcrumbs: Vec<Breadcrumb>,
    parent_path: Option<String>,
    kind: PathView,
    query: web::Query<PageQuery>,
}

#[get("/{repo}/tree/{filepath:.*}")]
pub async fn page(
    path: web::Path<(String, String)>,
    query: web::Query<PageQuery>,
    data: web::Data<App>,
) -> Result<HttpResponse, Error> {
    let (name, filepath) = path.into_inner();
    let filepath = filepath.trim_matches('/').to_string();

    let repo = data.get_repo(&name)?;
    let current_ref = Ref::resolve(&repo, &query)?;
    let branch_names = repo.branch_names()?;
    let kind = repo.resolve_path(&current_ref, &filepath)?;

    render_html(&TreePage {
        repo,
        branch_names,
        current_ref,
        breadcrumbs: tree::build_breadcrumbs(&filepath),
        parent_path: tree::parent_path(&filepath),
        filepath,
        kind,
        query,
    })
}

#[get("/{repo}/raw/{path:.*}")]
pub async fn raw(
    info: web::Path<(String, String)>,
    query: web::Query<PageQuery>,
    data: web::Data<App>,
) -> Result<HttpResponse, Error> {
    let (name, path) = info.into_inner();
    let repo = data.get_repo(&name)?;
    let current_ref = Ref::resolve(&repo, &query)?;

    match repo.read_raw_blob(&current_ref, &path)? {
        Some(bytes) => Ok(HttpResponse::Ok()
            .content_type("text/plain; charset=utf-8")
            .body(bytes)),
        None => Ok(HttpResponse::BadRequest().body("path is a directory")),
    }
}

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
) -> ActixResult<HttpResponse> {
    let (name, filepath) = path.into_inner();
    let filepath = filepath.trim_matches('/').to_string(); // fixes the double-slash bug from before

    let repo = match data.find_repo(name) {
        Some(r) => r,
        None => return Ok(HttpResponse::NotFound().body("Repo not found")),
    };

    let current_ref = match Ref::resolve(&repo, &query) {
        Ok(r) => r,
        Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
    };

    let branch_names = match repo.branch_names() {
        Ok(b) => b,
        Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
    };

    // was `&ref_name` (undefined) — should reference the resolved ref
    let kind = match repo.resolve_path(&current_ref, &filepath) {
        Ok(k) => k,
        Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
    };

    let rendered = TreePage {
        repo,
        branch_names,
        current_ref,
        breadcrumbs: tree::build_breadcrumbs(&filepath),
        parent_path: tree::parent_path(&filepath),
        filepath,
        kind,
        query,
    }
    .render();

    match rendered {
        Ok(html) => Ok(HttpResponse::Ok().content_type("text/html").body(html)),
        Err(e) => Ok(HttpResponse::InternalServerError().body(e.to_string())),
    }
}

#[get("/{repo}/raw/{path:.*}")]
pub async fn raw(
    info: web::Path<(String, String)>,
    query: web::Query<PageQuery>,
    data: web::Data<App>,
) -> ActixResult<HttpResponse> {
    let (name, path) = info.into_inner();
    let path = path.trim_matches('/');

    let repo = match data.find_repo(name) {
        Some(r) => r,
        None => return Ok(HttpResponse::NotFound().body("Repo not found")),
    };
    let gix_repo = repo.gix();

    let current_ref = match Ref::resolve(&repo, &query) {
        Ok(r) => r,
        Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
    };

    match repo.resolve_path(&current_ref, path) {
        Ok(PathView::Blob(blob)) => {
            // Re-read content rather than store it on Blob — keeps Blob
            // light for the tree-view case, which only needs metadata + lines.
            let object_id = match gix::ObjectId::from_hex(blob.id.as_bytes()) {
                Ok(id) => id,
                Err(e) => {
                    return Ok(
                        HttpResponse::InternalServerError().body(format!("invalid object id: {e}"))
                    );
                }
            };

            let object = match gix_repo.find_object(object_id) {
                Ok(o) => o,
                Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
            };

            let blob_obj = match object.try_into_blob() {
                Ok(b) => b,
                Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
            };

            Ok(HttpResponse::Ok()
                .content_type("text/plain; charset=utf-8")
                .body(blob_obj.data.clone()))
        }
        Ok(PathView::Tree(_)) => Ok(HttpResponse::BadRequest().body("path is a directory")),
        Err(e) => Ok(HttpResponse::InternalServerError().body(e.to_string())),
    }
}

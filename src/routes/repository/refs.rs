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
) -> ActixResult<HttpResponse> {
    let (name, mode) = path.into_inner();
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

    let offset = query.offset.unwrap_or(0);
    let has_prev = offset >= 1;

    let rendered = match mode.as_ref() {
        "heads" => {
            let (branches, branches_has_more) = match repo.branches(&gix_repo, offset, PAGE_SIZE) {
                Ok(b) => b,
                Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
            };

            RefsPage {
                repo: &repo,
                current_ref,
                offset,
                mode,
                branch_names,
                branches,
                tags: Vec::new(),
                has_prev,
                branches_has_more,
                tags_has_more: false,
                query,
            }
            .render()
        }

        "tags" => {
            let (tags, tags_has_more) = match repo.tags(offset, PAGE_SIZE) {
                Ok(t) => t,
                Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
            };

            RefsPage {
                repo: &repo,
                current_ref,
                offset,
                mode,
                branch_names,
                branches: Vec::new(),
                tags,
                has_prev,
                branches_has_more: false,
                tags_has_more,
                query,
            }
            .render()
        }
        &_ => {
            let (branches, branches_has_more) = match repo.branches(&gix_repo, 0, PAGE_SIZE / 2) {
                Ok(b) => b,
                Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
            };

            let (tags, tags_has_more) = match repo.tags(0, PAGE_SIZE / 2) {
                Ok(t) => t,
                Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
            };

            RefsPage {
                repo: &repo,
                current_ref,
                offset,
                mode,
                branch_names,
                branches,
                tags,
                has_prev: false,
                branches_has_more,
                tags_has_more,
                query,
            }
            .render()
        }
    };
    match rendered {
        Ok(html) => Ok(HttpResponse::Ok().content_type("text/html").body(html)),
        Err(e) => Ok(HttpResponse::InternalServerError().body(e.to_string())),
    }
}

// #[get("/{repo}/archive/{tag}.tar.gz")]
// pub async fn archive_tar_gz(
//     path: web::Path<(String, String)>,
//     data: web::Data<App>,
// ) -> ActixResult<HttpResponse> {
//     let (name, tag) = path.into_inner();
//     let repo = match data.find_repo(name) {
//         Some(r) => r,
//         None => return Ok(HttpResponse::NotFound().body("Repo not found")),
//     };
//
//     match repo.archive_tar_gz(&tag) {
//         Ok(bytes) => Ok(HttpResponse::Ok()
//             .content_type("application/gzip")
//             .append_header((
//                 "Content-Disposition",
//                 format!("attachment; filename=\"{}-{}.tar.gz\"", repo.name, tag),
//             ))
//             .body(bytes)),
//         // Most failures here are "no such tag" rather than a real server
//         // fault — 404 reads truer than 500 for a download endpoint.
//         Err(e) => Ok(HttpResponse::NotFound().body(e.to_string())),
//     }
// }
//
// #[get("/{repo}/archive/{tag}.zip")]
// pub async fn archive_zip(
//     path: web::Path<(String, String)>,
//     data: web::Data<App>,
// ) -> ActixResult<HttpResponse> {
//     let (name, tag) = path.into_inner();
//     let repo = match data.find_repo(&name) {
//         Some(r) => r,
//         None => return Ok(HttpResponse::NotFound().body("Repo not found")),
//     };
//
//     match repo.archive_zip(&tag) {
//         Ok(bytes) => Ok(HttpResponse::Ok()
//             .content_type("application/zip")
//             .append_header((
//                 "Content-Disposition",
//                 format!("attachment; filename=\"{}-{}.zip\"", repo.name, tag),
//             ))
//             .body(bytes)),
//         Err(e) => Ok(HttpResponse::NotFound().body(e.to_string())),
//     }
// }

use crate::{prelude::*, routes::PageQuery};

#[get("/{repo}/snapshot/")]
pub async fn download(
    path: web::Path<String>,
    query: web::Query<PageQuery>,
    data: web::Data<App>,
) -> ActixResult<HttpResponse> {
    let name = path.into_inner();
    let repo = match data.find_repo(name) {
        Some(r) => r,
        None => return Ok(HttpResponse::NotFound().body("Repo not found")),
    };

    let current_ref = match Ref::resolve(&repo, &query) {
        Ok(r) => r,
        Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
    };

    let (bytes, filename) = match repo.archive(&current_ref, &query.id) {
        Ok(result) => (result.0, result.1),
        Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
    };

    Ok(HttpResponse::Ok()
        .content_type("application/gzip")
        .append_header((
            "Content-Disposition",
            format!(r#"attachment; filename="{filename}""#),
        ))
        .body(bytes))
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

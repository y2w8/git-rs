use crate::{git::tag::TaggedObject, prelude::*, routes::PageQuery};

#[derive(Template)]
#[template(path = "repository/tag.html")]
struct TagPage<'repo> {
    repo: &'repo Repository,
    branch_names: Vec<String>,
    current_ref: Ref,
    tag: Tag,
    query: web::Query<PageQuery>,
}

#[get("/{repo}/tag/")]
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
    let tag: Tag = if let Some(head) = query.clone().into_inner().head {
        match repo.find_tag(&head) {
            Ok(c) => c,
            Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
        }
    } else {
        match gix_repo.find_reference(&format!("heads/{}", current_ref.name)) {
            Ok(r) => Tag::from_gix(r).unwrap(),
            Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
        }
    };

    let rendered = TagPage {
        repo: &repo,
        branch_names,
        current_ref,
        tag,
        query,
    }
    .render();
    match rendered {
        Ok(html) => Ok(HttpResponse::Ok().content_type("text/html").body(html)),
        Err(e) => Ok(HttpResponse::InternalServerError().body(e.to_string())),
    }
}

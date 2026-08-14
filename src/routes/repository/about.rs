use crate::{markdown, prelude::*, routes::PageQuery};

#[derive(Template)]
#[template(path = "repository/about.html")]
struct AboutPage {
    repo: Repository,
    current_ref: Ref,
    branch_names: Vec<String>,
    readme_html: Option<String>,
    query: web::Query<PageQuery>,
}

#[get("/{repo}/about/")]
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

    let current_ref = match Ref::resolve(&repo, &query) {
        Ok(r) => r,
        Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
    };
    let branch_names = match repo.branch_names() {
        Ok(b) => b,
        Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
    };

    let readme_html = repo.readme().map(|raw| markdown::render(&raw));

    let rendered = AboutPage {
        repo,
        current_ref,
        branch_names,
        readme_html,
        query,
    }
    .render();

    match rendered {
        Ok(html) => Ok(HttpResponse::Ok().content_type("text/html").body(html)),
        Err(e) => Ok(HttpResponse::InternalServerError().body(e.to_string())),
    }
}

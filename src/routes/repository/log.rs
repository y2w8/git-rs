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
) -> ActixResult<HttpResponse> {
    let name = path.into_inner();
    let query = query.into_inner();
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

    let (mut commits, mut has_next): (Vec<Commit<'_>>, bool) = (Vec::new(), false);

    if let Some(ref qt) = query.query_type {
        match qt.as_str() {
            "msg" => {
                if let Some(ref v) = query.value {
                    (commits, has_next) = match repo.commits(
                        &gix_repo,
                        &current_ref,
                        offset,
                        PAGE_SIZE,
                        Some(&|c| {
                            c.summary()
                                .unwrap()
                                .to_string()
                                .to_lowercase()
                                .contains(&v.to_lowercase())
                        }),
                    ) {
                        Ok(c) => c,
                        Err(e) => {
                            return Ok(HttpResponse::InternalServerError().body(e.to_string()));
                        }
                    };
                }
            }

            "author" => {
                if let Some(ref v) = query.value {
                    (commits, has_next) = match repo.commits(
                        &gix_repo,
                        &current_ref,
                        offset,
                        PAGE_SIZE,
                        Some(&|c| c.author.name.to_lowercase().contains(&v.to_lowercase())),
                    ) {
                        Ok(c) => c,
                        Err(e) => {
                            return Ok(HttpResponse::InternalServerError().body(e.to_string()));
                        }
                    };
                }
            }

            "committer" => {
                if let Some(ref v) = query.value {
                    (commits, has_next) = match repo.commits(
                        &gix_repo,
                        &current_ref,
                        offset,
                        PAGE_SIZE,
                        Some(&|c| c.committer.name.to_lowercase().contains(&v.to_lowercase())),
                    ) {
                        Ok(c) => c,
                        Err(e) => {
                            return Ok(HttpResponse::InternalServerError().body(e.to_string()));
                        }
                    };
                }
            }
            _ => {
                (commits, has_next) =
                    match repo.commits(&gix_repo, &current_ref, offset, PAGE_SIZE, None) {
                        Ok(c) => c,
                        Err(e) => {
                            return Ok(HttpResponse::InternalServerError().body(e.to_string()));
                        }
                    }
            }
        };
    } else {
        (commits, has_next) = match repo.commits(&gix_repo, &current_ref, offset, PAGE_SIZE, None) {
            Ok(c) => c,
            Err(e) => return Ok(HttpResponse::InternalServerError().body(e.to_string())),
        };
    }

    let rendered = LogPage {
        repo: &repo,
        branch_names,
        current_ref,
        commits,
        offset,
        has_prev,
        has_next,
        query,
    }
    .render();
    match rendered {
        Ok(html) => Ok(HttpResponse::Ok().content_type("text/html").body(html)),
        Err(e) => Ok(HttpResponse::InternalServerError().body(e.to_string())),
    }
}

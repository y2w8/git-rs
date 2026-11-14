use crate::{prelude::*, routes::PageQuery};

#[get("/{repo}/snapshot/")]
pub async fn download(
    path: web::Path<String>,
    query: web::Query<PageQuery>,
    data: web::Data<App>,
) -> Result<HttpResponse, Error> {
    let repo = data.get_repo(&path)?;
    let current_ref = Ref::resolve(&repo, &query)?;
    let (bytes, filename) = repo.archive(&current_ref, &query.id)?;

    Ok(HttpResponse::Ok()
        .content_type("application/gzip")
        .append_header((
            "Content-Disposition",
            format!(r#"attachment; filename="{filename}""#),
        ))
        .body(bytes))
}

use rust_embed::RustEmbed;

use crate::prelude::*;

#[derive(RustEmbed)]
#[folder = "static/"]
struct Static;

#[get("/static/{filename:.*}")]
pub async fn files(req: HttpRequest, path: web::Path<String>) -> ActixResult<HttpResponse> {
    let path = path.into_inner();

    if path.is_empty() {
        return Ok(HttpResponse::NotFound().body("Not Found"));
    }

    match Static::get(&path) {
        Some(content) => {
            let hash = content.metadata.sha256_hash();
            let etag = format!(
                "\"{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}\"",
                hash[0], hash[1], hash[2], hash[3], hash[4], hash[5], hash[6], hash[7]
            );

            if let Some(if_none_match) = req.headers().get("If-None-Match")
                && if_none_match.to_str().ok() == Some(&etag)
            {
                return Ok(HttpResponse::NotModified()
                    .insert_header(("ETag", etag))
                    .finish());
            }

            let mime_type = mime_guess::from_path(&path).first_or_octet_stream();
            Ok(HttpResponse::Ok()
                .content_type(mime_type.as_ref())
                .insert_header(("ETag", etag))
                .insert_header(("Cache-Control", "public, max-age=3600, must-revalidate"))
                .body(content.data))
        }
        None => Ok(HttpResponse::NotFound().body("Not Found")),
    }
}

use rust_embed::RustEmbed;

use crate::prelude::*;

#[derive(RustEmbed)]
#[folder = "static/"]
struct Static;

#[get("/static/{filename:.*}")]
pub async fn files(path: web::Path<String>) -> ActixResult<HttpResponse> {
    let path = path.into_inner();

    if path.is_empty() {
        return Ok(HttpResponse::NotFound().body("Not Found"));
    }

    match Static::get(&path) {
        Some(content) => {
            let mime_type = mime_guess::from_path(&path).first_or_octet_stream();
            Ok(HttpResponse::Ok()
                .content_type(mime_type.as_ref())
                .insert_header(("Cache-Control", "no-cache, must-revalidate"))
                .body(content.data.into_owned()))
        }
        None => Ok(HttpResponse::NotFound().body("Not Found")),
    }
}

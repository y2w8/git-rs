use crate::prelude::*;

#[derive(Template)]
#[template(path = "about.html")]
struct About {}

#[get("/about")]
pub async fn page() -> ActixResult<HttpResponse> {
    let rendered = About {}.render();

    match rendered {
        Ok(html) => Ok(HttpResponse::Ok().content_type("text/html").body(html)),
        Err(e) => Ok(HttpResponse::InternalServerError().body(e.to_string())),
    }
}

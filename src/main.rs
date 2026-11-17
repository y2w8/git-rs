use crate::prelude::*;
use actix_web::{App as ActixApp, HttpServer, web};
use clap::Parser;
use std::io::Error;

use crate::cli::Args;

pub mod app;
pub mod cli;
pub mod error;
pub mod git;
pub mod markdown;
pub mod prelude;
pub mod routes;
pub mod syntax_highlight;
pub mod time;

#[actix_web::main]
async fn main() -> Result<(), Error> {
    let args = Args::parse();
    let app = web::Data::new(App::new(args.repos_dir.to_string()).await);

    HttpServer::new(move || {
        ActixApp::new()
            .app_data(app.clone())
            .service(routes::r#static::files)
            .service(routes::git::git_info_refs)
            .service(routes::git::git_upload_pack)
            .service(routes::repository::diff::patch)
            .service(routes::repository::snapshot::download)
            .service(routes::page)
            .service(routes::about::page)
            .service(routes::repository::about::page)
            .service(routes::repository::summary::page)
            .service(routes::repository::refs::page)
            .service(routes::repository::tag::page)
            .service(routes::repository::log::page)
            .service(routes::repository::tree::page)
            .service(routes::repository::tree::raw)
            .service(routes::repository::commit::page)
            .service(routes::repository::diff::page)
    })
    .bind((args.url, args.port))?
    .run()
    .await
}

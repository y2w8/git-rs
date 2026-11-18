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

    if args.install_service {
        return install_systemd_service(&args);
    }

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

fn install_systemd_service(args: &Args) -> Result<(), Error> {
    let exe = std::env::current_exe()?;
    let service = format!(
        "[Unit]\n\
         Description=git-rs — Lightweight Git web frontend\n\
         After=network-online.target\n\
         Wants=network-online.target\n\
         \n\
         [Service]\n\
         Type=simple\n\
         User=root\n\
         WorkingDirectory={repos_dir}\n\
         ExecStart={exe} --url {url} --port {port} --repos-dir {repos_dir}\n\
         Restart=on-failure\n\
         RestartSec=5\n\
         AmbientCapabilities=CAP_NET_BIND_SERVICE\n\
         \n\
         [Install]\n\
         WantedBy=multi-user.target\n",
        exe = exe.display(),
        url = args.url,
        port = args.port,
        repos_dir = args.repos_dir,
    );

    std::fs::write("/etc/systemd/system/git-rs.service", service)?;
    println!("Installed /etc/systemd/system/git-rs.service");
    Ok(())
}

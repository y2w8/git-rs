use std::{
    collections::HashMap,
    io::Write,
    process::{Command, Stdio},
};

use actix_web::web::Bytes;

use crate::prelude::*;

#[get("/{repo}/info/refs")]
pub async fn git_info_refs(
    path: web::Path<String>,
    req_query: web::Query<HashMap<String, String>>,
    data: web::Data<App>,
) -> ActixResult<HttpResponse> {
    let service = match req_query.get("service") {
        Some(s) => s,
        None => return Ok(HttpResponse::BadRequest().body("Missing service parameter")),
    };

    if service != "git-upload-pack" && service != "git-receive-pack" {
        return Ok(HttpResponse::BadRequest().body("Unsupported service"));
    }

    let name = path.into_inner();
    let repo = match data.find_repo(&name) {
        Some(r) => r,
        None => return Ok(HttpResponse::NotFound().body("Repo not found")),
    };

    let first_line = format!("# service={}\n", service);
    let pkt_line = format!("{:04x}{}", first_line.len() + 4, first_line);
    let zero_line = "0000";

    let output = Command::new("git")
        .arg(service.strip_prefix("git-").unwrap())
        .arg("--stateless-rpc")
        .arg("--advertise-refs")
        .arg(&repo.path)
        .output();

    match output {
        Ok(out) => {
            let mut body = Vec::new();
            body.extend_from_slice(pkt_line.as_bytes());
            body.extend_from_slice(zero_line.as_bytes());
            body.extend_from_slice(&out.stdout);

            Ok(HttpResponse::Ok()
                .content_type(format!("application/x-{}-advertisement", service))
                .body(body))
        }
        Err(_) => Ok(HttpResponse::InternalServerError().body("Git command failed")),
    }
}

#[post("/{repo}/git-upload-pack")]
pub async fn git_upload_pack(
    path: web::Path<String>,
    data: web::Data<App>,
    body: Bytes,
) -> ActixResult<HttpResponse> {
    let name = path.into_inner();
    let repo = match data.find_repo(&name) {
        Some(r) => r,
        None => return Ok(HttpResponse::NotFound().body("Repo not found")),
    };

    Ok(handle_git_rpc("upload-pack", repo, body).await)
}

#[post("/{repo}/git-receive-pack")]
pub async fn git_receive_pack(
    path: web::Path<String>,
    data: web::Data<App>,
    body: Bytes,
) -> ActixResult<HttpResponse> {
    let name = path.into_inner();
    let repo = match data.find_repo(&name) {
        Some(r) => r,
        None => return Ok(HttpResponse::NotFound().body("Repo not found")),
    };

    Ok(handle_git_rpc("receive-pack", repo, body).await)
}

async fn handle_git_rpc(service: &str, repo: Repository, body: Bytes) -> HttpResponse {
    let mut child = Command::new("git")
        .arg(service)
        .arg("--stateless-rpc")
        .arg(&repo.path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&body).unwrap();
    }

    let output = child.wait_with_output().unwrap();

    HttpResponse::Ok()
        .content_type(format!("application/x-{}-result", service))
        .body(output.stdout)
}

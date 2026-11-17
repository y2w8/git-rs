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

    // Only allow `git-upload-pack` (read-only clone/fetch/pull).
    // `git-receive-pack` (push) is disabled over HTTP so pushes are only possible over SSH.
    if service != "git-upload-pack" {
        return Ok(HttpResponse::Forbidden().body("Push over HTTP is disabled; use SSH"));
    }

    let repo = data.get_repo(&path)?;

    let first_line = "# service=git-upload-pack\n";
    let pkt_line = format!("{:04x}{first_line}", first_line.len() + 4);
    let zero_line = "0000";

    let output = Command::new("git")
        .arg("upload-pack")
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
                .content_type("application/x-git-upload-pack-advertisement")
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
) -> Result<HttpResponse, Error> {
    let repo = data.get_repo(&path)?;

    let mut child = Command::new("git")
        .arg("upload-pack")
        .arg("--stateless-rpc")
        .arg(&repo.path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!(e))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&body).map_err(|e| anyhow::anyhow!(e))?;
    }

    let output = child.wait_with_output().map_err(|e| anyhow::anyhow!(e))?;

    Ok(HttpResponse::Ok()
        .content_type("application/x-git-upload-pack-result")
        .body(output.stdout))
}

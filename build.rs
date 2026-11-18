fn main() {
    // Cargo watches these directories recursively; any file added/changed/removed
    // under them triggers a rebuild on the next `cargo build`/`cargo run`.
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=templates");
    println!("cargo:rerun-if-changed=static");
    println!("cargo:rerun-if-changed=git-rs.service");

    // When running `cargo install` (release profile) with permissions to write to
    // /etc/systemd/system, automatically install git-rs.service.
    if std::env::var("PROFILE").as_deref() == Ok("release") {
        let _ = std::fs::copy("git-rs.service", "/etc/systemd/system/git-rs.service");
    }
}

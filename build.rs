fn main() {
    // Cargo watches these directories recursively; any file added/changed/removed
    // under them triggers a rebuild on the next `cargo build`/`cargo run`.
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=templates");
    println!("cargo:rerun-if-changed=static");
}

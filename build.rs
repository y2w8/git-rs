fn main() {
    // Cargo watches this directory recursively; any file added/changed/removed
    // under it triggers a rebuild on the next `cargo build`/`cargo run`.
    println!("cargo:rerun-if-changed=templates");
}

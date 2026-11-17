arm_target := "aarch64-unknown-linux-gnu"

default:
    @just --list

# Build debug binary
build *args:
    cargo build {{args}}

# Build optimized release binary (x86_64)
build-release:
    cargo build --release

# Cross-compile static ARM64 binary for Orange Pi
build-arm:
    cargo build --release --target {{arm_target}}

# Build and deploy ARM64 binary + systemd service to Orange Pi
deploy host="pi": build-arm
    ssh {{host}} "systemctl stop git-rs || true"
    scp target/{{arm_target}}/release/git-rs {{host}}:/usr/bin/git-rs
    # scp git-rs.service {{host}}:/etc/systemd/system/git-rs.service
    ssh {{host}} "systemctl daemon-reload && systemctl start git-rs"

# Run locally (e.g. `just run -p 8080 -r ./repos`)
run *args:
    cargo run -- {{args}}

# Fast type and borrow check
check:
    cargo check --all-targets

# Format all Rust code
fmt:
    cargo fmt --all

# Run clippy with warnings denied
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Run format and lint checks
ci: fmt lint

# Remove build artifacts
clean:
    cargo clean

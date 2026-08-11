#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_dir"

docker info >/dev/null
command -v kind >/dev/null
command -v kubectl >/dev/null
command -v cargo-audit >/dev/null
command -v cargo-deny >/dev/null
docker image inspect postgres:18-alpine quay.io/minio/minio:latest minio/mc:RELEASE.2025-08-13T08-35-41Z redis:8.8.0-alpine registry:2 debian:bookworm-slim sha256:3489c7674813ba5d8b1a9977baea8a6e553784dab7b84759d1014dbd78f7ebd5 >/dev/null

cargo fmt --all --check
cargo clippy --all-targets --all-features --locked --offline -- -D warnings
cargo test --all-targets --all-features --locked --offline
cargo doc --no-deps --all-features --locked --offline
cargo metadata --locked --offline --no-deps --format-version 1 >/dev/null
cargo audit --no-fetch --stale --file Cargo.lock
cargo deny --offline --locked check advisories bans sources
cargo run --quiet --locked --offline --bin repo_check
git diff --check

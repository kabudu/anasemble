#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_dir"

docker info >/dev/null
docker image inspect postgres:18-alpine quay.io/minio/minio:latest minio/mc:RELEASE.2025-08-13T08-35-41Z redis:8.8.0-alpine >/dev/null

cargo fmt --all --check
cargo clippy --all-targets --all-features --locked --offline -- -D warnings
cargo test --all-targets --all-features --locked --offline
cargo doc --no-deps --all-features --locked --offline
cargo metadata --locked --offline --no-deps --format-version 1 >/dev/null
cargo run --quiet --locked --offline --bin repo_check
git diff --check

#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_dir"

cargo fmt --all --check
cargo clippy --all-targets --all-features --locked --offline -- -D warnings
cargo test --all-targets --all-features --locked --offline
cargo doc --no-deps --all-features --locked --offline
cargo metadata --locked --offline --no-deps --format-version 1 >/dev/null
cargo run --quiet --locked --offline --bin repo_check
git diff --check

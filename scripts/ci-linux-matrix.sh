#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_dir"

git diff --quiet
git diff --cached --quiet
test -z "$(git status --short)"
docker info >/dev/null

image='rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97'
run_id="$(date +%s)-$$"
volumes=''

cleanup() {
  for volume in $volumes; do
    case "$volume" in
      anasemble-linux-matrix-"$run_id"-*) docker volume rm "$volume" >/dev/null ;;
      *) echo "refusing unexpected matrix volume cleanup: $volume" >&2; exit 1 ;;
    esac
  done
}
trap cleanup EXIT HUP INT TERM

run_profile() {
  platform=$1
  expected_arch=$2
  suffix=$3
  volume="anasemble-linux-matrix-$run_id-$suffix"
  docker volume create "$volume" >/dev/null
  volumes="$volumes $volume"

  docker run --rm \
    --platform "$platform" \
    --env CARGO_HOME=/workspace/cargo \
    --env CARGO_TARGET_DIR=/workspace/target \
    --env EXPECTED_ARCH="$expected_arch" \
    --volume "$project_dir:/source:ro" \
    --volume "$volume:/workspace" \
    "$image" \
    sh -euc '
      git clone --no-local /source /workspace/repo
      cd /workspace/repo
      test "$(uname -m)" = "$EXPECTED_ARCH"
      rustc --version --verbose
      cargo fetch --locked
    '

  docker run --rm \
    --network none \
    --platform "$platform" \
    --env CARGO_HOME=/workspace/cargo \
    --env CARGO_TARGET_DIR=/workspace/target \
    --env EXPECTED_ARCH="$expected_arch" \
    --volume "$volume:/workspace" \
    "$image" \
    sh -euc '
      cd /workspace/repo
      test -z "$(git status --short)"
      test "$(uname -m)" = "$EXPECTED_ARCH"
      cargo fmt --all --check
      cargo clippy --lib --bins --tests --locked --offline -- -D warnings
      cargo test --locked --offline \
        --lib --bins \
        --test cli_e2e \
        --test m0_contract \
        --test m1_loop \
        --test m2_campaign \
        --test p1_evidence_plane \
        --test p4_product_readiness \
        --test production_foundations
      cargo doc --no-deps --all-features --locked --offline
      cargo build --release --locked --offline
      cargo run --quiet --locked --offline --bin repo_check
    '
}

run_profile linux/arm64 aarch64 arm64
run_profile linux/amd64 x86_64 x86_64

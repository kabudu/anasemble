#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_dir"

docker info >/dev/null
command -v kind >/dev/null
command -v kubectl >/dev/null
command -v cargo-audit >/dev/null
command -v cargo-deny >/dev/null
docker_version=$(docker version --format '{{.Server.Version}}')
case "$docker_version" in
  29.*) ;;
  *) echo "Docker Engine 29 is required; found $docker_version" >&2; exit 1 ;;
esac
kind_version=$(kind version)
case "$kind_version" in
  "kind v0.32.0 "*) ;;
  *) echo "kind 0.32.0 is required; found $kind_version" >&2; exit 1 ;;
esac
kubectl_version=$(kubectl version --client -o json)
case "$kubectl_version" in
  *'"gitVersion": "v1.36.'*) ;;
  *) echo "kubectl 1.36 is required" >&2; exit 1 ;;
esac
docker image inspect \
  postgres@sha256:9a8afca54e7861fd90fab5fdf4c42477a6b1cb7d293595148e674e0a3181de15 \
  quay.io/minio/minio@sha256:14cea493d9a34af32f524e538b8346cf79f3321eff8e708c1e2960462bd8936e \
  minio/mc@sha256:a7fe349ef4bd8521fb8497f55c6042871b2ae640607cf99d9bede5e9bdf11727 \
  redis@sha256:9d317178eceac8454a2284a9e6df2466b93c745529947f0cd42a0fa9609d7005 \
  registry@sha256:a3d8aaa63ed8681a604f1dea0aa03f100d5895b6a58ace528858a7b332415373 \
  debian@sha256:7b140f374b289a7c2befc338f42ebe6441b7ea838a042bbd5acbfca6ec875818 \
  sha256:3489c7674813ba5d8b1a9977baea8a6e553784dab7b84759d1014dbd78f7ebd5 \
  >/dev/null

cargo fmt --all --check
cargo clippy --all-targets --all-features --locked --offline -- -D warnings
cargo test --all-targets --all-features --locked --offline
cargo doc --no-deps --all-features --locked --offline
cargo metadata --locked --offline --no-deps --format-version 1 >/dev/null
cargo audit --no-fetch --stale --file Cargo.lock
cargo deny --offline --locked check advisories bans licenses sources
cargo run --quiet --locked --offline --bin repo_check
(cd website && npm ci --ignore-scripts --offline && npm run build)
git diff --exit-code -- website/styles.css
git diff --check

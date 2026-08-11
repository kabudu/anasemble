# Linux Clean-Clone Matrix

## Result and claim boundary

On 2026-08-11, `./scripts/ci-linux-matrix.sh` completed successfully for
`linux/arm64` and `linux/amd64` from a clean committed tree. The arm64 container
ran on the host's arm64 instruction set. The amd64 container ran through Docker
emulation on that arm64 host, so this is x86_64 build-and-execution evidence but
not native x86_64 hardware evidence.

Both profiles used the digest-pinned multi-architecture Rust image
`rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97`
and the repository-pinned Rust 1.97.0 toolchain. Each profile cloned with
`git clone --no-local` into a new named Docker volume, asserted an empty Git
status and expected `uname -m`, fetched locked dependencies once, then disabled
network access for formatting, Clippy, bounded non-Docker tests, documentation,
release build, and repository validation. The script removes only its exact
validated volumes on exit.

## Reproduction

From a clean committed checkout with Docker running:

```text
./scripts/ci-linux-matrix.sh
```

The matrix intentionally excludes Docker-backed PostgreSQL, S3, Redis, OCI and
Kubernetes integration tests because the validator itself runs inside Docker.
Those destructive integrations remain in `./scripts/ci-local.sh`. Hosted CI is
prohibited while the repository is private.

## Remaining evidence gaps

- Run the same clean-clone gate on native x86_64 Linux hardware before describing
  that profile as fully tested.
- Validate named production Linux distributions, kernels, container runtimes and
  Kubernetes CNIs before promoting their combinations from experimental.
- Retain exact machine, kernel, Docker and commit metadata for each future
  compatibility promotion.

# Integrated Evaluation Quickstart

This workflow demonstrates the bounded Anasemble path from prepared semantic and
state evidence through reconstruction, state restoration, immutable artifact
publication, operator-approved Kubernetes activation, health gating and rollback.
It is destructive and intended for disposable evaluation infrastructure.

## Prerequisites and trust boundary

Build the Rust CLI with the pinned toolchain and run `./scripts/ci-local.sh` first.
Provide reachable PostgreSQL 18, S3-compatible and Redis 8 fixtures, an OCI
registry, Docker, kubectl and a Kubernetes context. Database writers must be
quiesced during capture. The Docker daemon, Kubernetes control plane, credential
files and operator signing key are trusted. Secret and connection files must be
non-empty owner-only regular files no larger than 64 KiB.

Copy `examples/reference-recovery-config-v1.json` outside the repository and
replace every environment-specific path, resource name, immutable base-image
digest and approval time. The configuration contains references, never secret
values. The recovery workspace must already contain the surviving fragments and
must pass the artifact-loss oracle.

## Prepare before loss

```text
target/release/anasemble prepare-reference-recovery \
  /secure/anasemble/reference-config.json \
  /secure/anasemble/reference-state-bundle.json
```

Store the owner-only bundle independently of deployable artifacts. It binds the
service manifest and exact PostgreSQL, S3 and Redis snapshots.

## Recover and activate after declared loss

```text
target/release/anasemble recover-activate-reference \
  /secure/anasemble/reference-config.json \
  /secure/anasemble/reference-state-bundle.json \
  /secure/anasemble/reference-recovery-receipt.json
```

The command refuses uncertified reconstruction, mismatched evidence, invalid
credentials or approval, failed restoration, publication, health or activation.
After state mutation begins, failures trigger reverse-order backend rollback.
The receipt is created exclusively with owner-only permissions and binds the
activation plan, three backend rollback receipts, immutable OCI artifact and
Kubernetes activation.

## Roll back

Retain the receipt until operator acceptance. Rollback first restores the prior
Kubernetes selector, then Redis, S3 and PostgreSQL in reverse activation order:

```text
target/release/anasemble rollback-reference-recovery \
  /secure/anasemble/reference-config.json \
  /secure/anasemble/reference-recovery-receipt.json
```

If Kubernetes rollback fails, state rollback does not begin, preserving the
currently active service-to-state relationship. Backend rollback errors are
reported together after every remaining rollback has been attempted. There is no
automatic acceptance or deletion of retained rollback resources.

## What this proves

The repository drill deliberately deletes the original component plus the source
PostgreSQL schema, S3 object and Redis stream, then verifies recovery, activation
and rollback through these public commands. The packaged finite-state candidate
is an inspectable certified artifact; the current bounded DSL does not generate
a live HTTP server. Production Kubernetes egress isolation additionally requires
a NetworkPolicy-enforcing CNI, which the disposable kind drill does not prove.

# Repository Agent Policy

## Engineering mode

Apply Lazarus Mode to every implementation and review. Prefer the simplest
architecture that satisfies proven requirements and keep correctness, security,
resource, compatibility, and failure boundaries explicit.

Rust is the project-wide implementation language. Use the pinned stable toolchain
in `rust-toolchain.toml`. A different technology requires a concrete necessity
that Rust cannot reasonably satisfy, an ADR recording the exception and its trust
boundary, and explicit user approval. A project website may justify such an
exception; convenience alone does not.

## Private repository delivery

Hosted CI is prohibited while this repository is private. The only authoritative
gate is `./scripts/ci-local.sh`; absent hosted checks are policy-compliant, not
passing checks.

For every milestone:

1. Start from a clean, current `master` branch.
2. Create a scoped `codex/<item>` branch.
3. Implement code, tests, evidence, documentation, traceability, and roadmap state together.
4. Run `./scripts/ci-local.sh` and perform a Lazarus Mode self-review.
5. Commit intended files, push, and open a reviewed pull request against `master`.
6. Record the exact local CI command and result in the pull request.
7. Squash-merge only a mergeable, reviewed, locally green head.
8. Fast-forward local `master`, verify it, and delete the merged local branch.

Do not publish a package, release, deployment, or public repository without the
separate gates and explicit authority documented in `docs/RELEASE.md`.

## Public transition

Repository preparation does not authorize publication. Follow
`docs/PUBLIC_OPENING.md` for the visibility transition. Add hosted CI only after
public visibility is explicitly approved and verified, in a separate reviewed
pull request with pinned actions, least permissions, bounded execution and safe
fork behavior. Do not enable package publishing, deployments or release secrets
as part of source opening.

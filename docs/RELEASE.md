# Release Strategy

`./scripts/ci-local.sh` remains the authoritative release gate. After the
repository became public, the owner-authorized GitHub Actions workflow added a
least-privilege, non-destructive contributor gate; it does not replace the
Docker, Kubernetes, security-tooling, or cross-architecture local evidence.

No product release is authorized by repository preparation alone. An evaluation tag requires a frozen DSL and loss
oracle, reproducible artifact-absence proof, complete baselines, adversarial
results, sandbox and soundness review, refreshed novelty/name diligence, and
published limitations.

The M3 decision authorizes production engineering, not release. P4 completes the implementation roadmap for the profiles in `COMPATIBILITY.md`, but no tag, GitHub Release, package, hosted CI, visibility change, or deployment is authorized without a new explicit user decision. Independent reproduction and external security review are optional post-release assurance rather than release blockers.

`v0.1.0-rc.1` is the prepared first public source candidate. The owner has authorized publication of the existing `anasemble` Rust package from the exact release tag through the protected release workflow. Hosted recovery, real service traces, container publication, production deployment and commercial claims each require a separate gate.

The first release artifact set is defined in `docs/RELEASE_ARTIFACTS.md`: the
annotated source tag, GitHub source archives, the `anasemble` crates.io package,
native Amazon Linux 2023 arm64 and x86_64 binary archives, checksums, and
per-target build provenance. Container images, macOS binary distribution, hosted
recovery, SBOM assertions, and detached artifact signing remain separate future
gates.

The source and project-authored assets are licensed under Apache License 2.0, subject to `NOTICE` and the trademark boundary in `TRADEMARKS.md`. Dependency licences must pass `cargo deny --locked check licenses`. Community, support, governance and security policies are repository-owned release surfaces.

## Release presentation contract

Every public release requires repository-owned, versioned curated release notes
that are distinct from `CHANGELOG.md`. The release title must contain Anasemble,
the version, and a short human-readable theme. The body must open with the
user-visible outcome, list three to five material changes, state claim and
compatibility boundaries, provide one primary installation path, and link to
supporting evidence and the detailed changelog.

Release prose must use one physical source line per paragraph or list item. Do not hard-wrap it at a fixed column. It must not repeat the title, dump the raw
changelog, use an unexplained internal experiment inventory, or contain Unicode
U+2014. Release automation must fail closed if the curated title or body is
missing or mismatched to the tag.

Before publication, inspect a rendered preview at desktop and narrow widths for
heading hierarchy, wrapping, lists, code fences, links, and placeholder text.
After publication, inspect the canonical release URL and immediately correct any
metadata that differs from the approved preview.

## Prepared release-candidate evidence

`release/0.1.0-rc.1.title` and `release/0.1.0-rc.1.md` are the repository-owned curated preview. Apache License 2.0 and the source candidate version are recorded, but the files are intentionally unpublished. Before release authority can be exercised, refresh legal/name diligence, approve the exact version and title, complete every private gate in `PUBLIC_OPENING.md`, render the exact notes at desktop and narrow widths, run both local validation entry points, test install/upgrade/rollback/uninstall from the clean candidate source and verify the exact crates.io package inventory.

Hosted CI is prohibited while the repository is private. The public workflow is
therefore added only after public visibility, in its own reviewed pull request.
It uses no secrets on pull requests and remains non-required until its fork-safe
behaviour is proven.

Release rollback preserves the prior exact-prefix installation, configuration, job store, backend rollback receipts, OCI digest, and Kubernetes prior deployment until acceptance. Roll back the executable reference first, runtime activation second, and backend state in reverse receipt order. A release must not delete an operations root or rewrite job records during downgrade.

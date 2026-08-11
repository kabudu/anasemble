# Public opening runbook

## Prepared state

The repository is prepared for an Apache-2.0 source release but remains private.
Preparation does not authorize a visibility change, tag, GitHub Release, package
publication, container publication, hosted CI activation or deployment.

The intended first public source candidate is `v0.1.0-rc.1`, titled “Anasemble
v0.1.0-rc.1: Evidence-bound recovery.” Opening the source does not itself
publish the crates.io package. Registry publication occurs only from the exact
tag after package inspection and release approval.

The static GitHub Pages foundation under `website/` is a presentation-only
exception governed by ADR 0003. It is not deployed while the repository is
private and has no JavaScript, telemetry, data collection, external resources,
or product trust-boundary role.

The repository landing page must lead with the recovery outcome, supported
profiles, runnable evaluation path, and safety boundaries. Before opening,
verify that the GitHub About description remains product-oriented and that the
topic set covers disaster recovery, service recovery, business continuity,
Rust, resilience, Kubernetes, PostgreSQL, Redis, S3, and OCI without implying
support beyond `docs/COMPATIBILITY.md`.

## Final private gates

- [x] Refresh exact and similar name and trademark review at the publication date.
- [x] Confirm no confidential agreement, third-party code, customer data or private evidence is present.
- [x] Run `gitleaks git --redact` against all reachable history and review every finding.
- [x] Run `./scripts/ci-local.sh` on the exact release commit.
- [x] Run `./scripts/ci-linux-matrix.sh` on the exact release commit.
- [x] Build the release binary from a clean source archive and exercise install, upgrade, rollback and uninstall.
- [x] Render the exact release notes at desktop and narrow widths and verify every link.
- [x] Inspect the rendered README, About description and repository topics on GitHub.
- [x] Obtain explicit owner approval for visibility change, hosted CI activation, tag and GitHub Release as separate actions.

The transition completed on 2026-08-11. Public CI remains non-required until a
true unprivileged fork run can be performed from an identity other than the
repository owner; its workflow exposes no pull-request secrets and does not use
`pull_request_target`.

## Visibility transition

Perform these steps in order and stop on any mismatch:

1. Record the approved release commit and verify local `master` equals `origin/master`.
2. Change repository visibility to public and independently verify owner, name, default branch and `PUBLIC` visibility.
3. Enable private vulnerability reporting, discussions if desired, and branch protection without weakening the reviewed merge policy.
4. Add the repository-reviewed public GitHub Actions workflow in a new pull request. It must use pinned action commit SHAs, least permissions, dependency caching without secret exposure, bounded timeouts and the non-destructive public subset. Local CI remains the release authority until the public workflow is proven.
5. Run the public workflow from an unprivileged fork scenario before making it required. Never expose repository or environment secrets to pull-request code.
6. Create and verify the annotated tag only after the final commit and release title/body match.
7. Create the GitHub Release from the exact committed title and body, attach only approved artifacts and inspect the live page immediately.

## Rollback

If confidential content, licensing uncertainty, unsafe workflow behavior or
misleading release metadata is found before tagging, stop and keep the repository
private. If found after opening, disable affected automation, preserve evidence,
rotate any exposed credential, remove public release artifacts when necessary and
follow GitHub's sensitive-data remediation guidance. Visibility changes do not
erase cloned history, so history-rewrite decisions require a separate incident
plan and explicit owner approval.

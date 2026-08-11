# Anasemble

**Regrow function from surviving meaning.**

Anasemble is a research project for reconstructing a lost service component when
no source, binary, container, or identical replica survives. It combines
separately distributed executable contracts, protocol traces, and a bounded typed
grammar to synthesize a non-identical replacement, then subjects it to an
independent checker before deployment.

The first experiment uses a finite service-component DSL. It does not claim
arbitrary program recovery or general autonomous software creation.

## Candidate contribution

A disaster-recovery protocol in which independently placed semantic fragments are
sufficient to construct and certify a behaviorally compatible replacement after
total artifact loss, with explicit refusal when evidence is insufficient.

## Status

M0 through M2 establish the bounded reconstruction kernel. P0 through P3 add real-service contracts, authenticated and encrypted evidence, filesystem plus PostgreSQL/S3/Redis state recovery, and isolated operator-approved OCI activation. P4 adds restart-safe recovery jobs, queue backpressure, audit chains, metrics, diagnostics, redacted support bundles, configuration migration, exact-prefix installation and removal, compatibility contracts, destructive local drills, sustained scheduler evidence, dependency checks, and release-candidate presentation. Anasemble is implementation-complete for the explicitly supported profiles in [COMPATIBILITY](docs/COMPATIBILITY.md), not for arbitrary service reconstruction. Independent reproduction and external security review are optional post-release assurance, not implementation gates. No public release, production deployment, package publication, or repository visibility change is authorized. Claim boundaries are in [NOVELTY](docs/NOVELTY.md), and operational limits are in [P4 operations](docs/P4_OPERATIONS_AND_READINESS.md).

Install the pinned Rust toolchain with `rustup show`, fetch dependencies once with
`cargo fetch --locked`, then run the authoritative private-repository CI with
`./scripts/ci-local.sh`.

The supported Rust-native installation and disaster procedures are in [INSTALLATION](docs/INSTALLATION.md) and [DISASTER_RUNBOOK](docs/DISASTER_RUNBOOK.md).

## Repository policy

This is a private local repository. Its required remote is a private repository in
the GitHub account `kabudu`. The default branch is `master`.

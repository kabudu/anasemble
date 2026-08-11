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

M0 through M2 establish a viable bounded reconstruction kernel. P0 adds real-service contracts and durable local-file state; P1 adds Ed25519 identity lifecycle, signed multi-domain stores, bounded quorum retrieval, encrypted retention, provenance, and deletion workflows. P2 adds bounded PostgreSQL, S3-compatible object, and Redis Stream recovery with stable snapshots or refusal, verified migration, rollback, and certificate-bound activation plans. P3 and P4 still require isolated runtime activation and operational control before Anasemble is called production-complete. Independent reproduction and external security review are optional post-release assurance, not implementation gates. Anasemble is not yet approved for public release, production recovery, or arbitrary service reconstruction claims. Claim boundaries are in [NOVELTY](docs/NOVELTY.md), and the production roadmap is in [IMPLEMENTATION_PLAN](docs/IMPLEMENTATION_PLAN.md).

Install the pinned Rust toolchain with `rustup show`, fetch dependencies once with
`cargo fetch --locked`, then run the authoritative private-repository CI with
`./scripts/ci-local.sh`.

## Repository policy

This is a private local repository. Its required remote is a private repository in
the GitHub account `kabudu`. The default branch is `master`.

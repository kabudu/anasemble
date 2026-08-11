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

M0 through M2 establish the bounded reconstruction kernel. P0 through P3 add real-service contracts, authenticated and encrypted evidence, filesystem plus PostgreSQL/S3/Redis state recovery, and isolated operator-approved OCI activation. P4 adds restart-safe recovery jobs, queue backpressure, audit chains, metrics, diagnostics, redacted support bundles, configuration migration, exact-prefix installation and removal, compatibility contracts, destructive local drills, sustained scheduler evidence, dependency checks, and release-candidate presentation. Anasemble is implementation-complete for the explicitly supported profiles in [COMPATIBILITY](docs/COMPATIBILITY.md), not for arbitrary service reconstruction. Independent reproduction and external security review are optional post-release assurance, not implementation gates. The repository is prepared for an Apache-2.0 public source candidate but remains private; no public release, production deployment, package publication or visibility change is authorized by preparation alone. Claim boundaries are in [NOVELTY](docs/NOVELTY.md), and operational limits are in [P4 operations](docs/P4_OPERATIONS_AND_READINESS.md).

Install the pinned Rust toolchain with `rustup show`, fetch dependencies once with
`cargo fetch --locked`, then run the authoritative private-repository CI with
`./scripts/ci-local.sh`.

The supported Rust-native installation and disaster procedures are in [INSTALLATION](docs/INSTALLATION.md) and [DISASTER_RUNBOOK](docs/DISASTER_RUNBOOK.md).
Evaluators can follow the bounded integrated flow in [QUICKSTART](docs/QUICKSTART.md)
and inspect the clean-clone Linux evidence in [LINUX_MATRIX](docs/LINUX_MATRIX.md).
The private implementation productisation boundary is recorded in
[PRODUCTISATION](docs/PRODUCTISATION.md), and the final internal Lazarus security
sweep is retained in [FINAL_SECURITY_SWEEP](docs/FINAL_SECURITY_SWEEP.md).

The enduring Semantic Fit visual and verbal system is defined in
[BRAND_IDENTITY](docs/BRAND_IDENTITY.md). Canonical sources, deterministic SVG
exports, accessible tokens, templates, provenance and the digest manifest live
under `assets/brand`; release maturity remains a separate overlay.

## Repository policy

The default branch is `master`. While the remote is private, local CI is the only
authoritative gate and hosted CI is prohibited. The exact public transition is in
[PUBLIC_OPENING](docs/PUBLIC_OPENING.md).

## Contributing, security and licence

Anasemble is licensed under [Apache License 2.0](LICENSE). The licence does not
grant trademark rights in the Anasemble name or Semantic Fit identity. See
[CONTRIBUTING](CONTRIBUTING.md), [SECURITY](SECURITY.md), [SUPPORT](SUPPORT.md),
[GOVERNANCE](GOVERNANCE.md), [CODE_OF_CONDUCT](CODE_OF_CONDUCT.md) and
[TRADEMARKS](TRADEMARKS.md) before participating or redistributing the project.

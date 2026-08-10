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

M2 provides a bounded state migration and transactional deployment path, rollback,
matched baseline execution, registered metric observations, freshness policy, and
a retained adversarial campaign for the frozen finite-state DSL. It remains a
synthetic research harness, not a production recovery system or a claim about
arbitrary service reconstruction.
Claim boundaries are in [NOVELTY](docs/NOVELTY.md).

Install the pinned Rust toolchain with `rustup show`, fetch dependencies once with
`cargo fetch --locked`, then run the authoritative private-repository CI with
`./scripts/ci-local.sh`.

## Repository policy

This is a private local repository. Its required remote is a private repository in
the GitHub account `kabudu`. The default branch is `master`.

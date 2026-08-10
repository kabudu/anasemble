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

M1 provides a generalized deterministic reconstruction loop for the frozen
finite-state DSL, checker-private candidate parsing, capability-empty and
fuel-limited WebAssembly execution, an immutable evidence ledger, and a registered
two-component stateless corpus. It does not provide modeled external state,
deployment, comparative baseline results, or production recovery.
Claim boundaries are in [NOVELTY](docs/NOVELTY.md).

Install the pinned Rust toolchain with `rustup show`, fetch dependencies once with
`cargo fetch --locked`, then run the authoritative private-repository CI with
`./scripts/ci-local.sh`.

## Repository policy

This is a private local repository. Its required remote is a private repository in
the GitHub account `kabudu`. The default branch is `master`.

# Architecture

## Technology boundary

The control plane and all project implementation use Rust under
[ADR 0002](DECISIONS/0002-rust-control-plane.md). This is a single-language
system unless a separately approved ADR proves another technology is absolutely
necessary. Future reconstructed executable components are expected to cross a
capability-scoped WebAssembly boundary; M0 reconstructs data-defined finite-state
machines and does not execute WebAssembly.

## Components

1. **Fragment distributor** places contracts, schemas, and observations in
   independent failure domains before loss.
2. **Evidence collector** authenticates survivors and builds a provenance graph.
3. **Canonicalizer** converts valid fragments into typed constraints.
4. **Synthesizer** searches only a finite, resource-bounded component DSL.
5. **Candidate sandbox** executes generated components without ambient authority.
6. **Independent conformance checker** evaluates contracts, negative cases,
   metamorphic properties, and held-out traces using a separate interpreter.
7. **State migration planner** transforms only explicitly modeled state.
8. **Transactional deployer** installs a certified component or refuses.
9. **Evidence ledger** preserves inputs, search decisions, and outcomes.

## Trust boundaries

Surviving nodes, traces, and candidates are untrusted. The initial TCB includes
fragment identity policy, canonical schemas, resource monitor, independent
checker, deployment transaction, and ledger integrity. Synthesizer and checker
must not share an interpreter.

## Safety posture

Candidates receive capability-scoped I/O. Search, execution, memory, and output
are bounded. Missing side-effect semantics, contradictory evidence, checker
disagreement, state ambiguity, or budget exhaustion causes refusal.

## M0 implementation boundary

M0 implements the canonicalizer, a narrow bounded synthesizer, and an independent
checker as separate Rust modules. The CLI consumes only file-based canonical
JSON. It produces a candidate and certificate but does not install either.
Capability isolation, transactional deployment, a durable ledger, and generalized
state migration remain outside M0.

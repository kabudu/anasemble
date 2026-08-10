# Soundness Case

## Conditional claim

If the surviving contracts are mutually consistent and behaviorally complete for
the declared interface, the actual component lies within the finite DSL's
behavioral envelope, the independent checker implements those semantics
correctly, and side effects/state are fully modeled, then a certified candidate
satisfies the declared observable behavior.

## Obligations

- Total artifact loss is verified, not simulated by accidentally retaining code.
- Evidence provenance and failure-domain independence are authenticated.
- Training traces and held-out checks are separated.
- Candidate search cannot influence or weaken the checker.
- All effects require explicit capabilities and modeled state transitions.
- Deployment is atomic and reversible.

M0 separates synthesis and checking semantics but shares Rust protocol types and
Serde JSON parsing. Its certificate names this limitation. Full parser and model
independence remains an M1 obligation and is required before a research release.

## Known limits

Finite observations do not identify arbitrary programs. Contracts may be
incomplete or poisoned; survivors may share a fault; equivalent visible behavior
may hide unacceptable timing, security, or side effects. For these cases the only
sound output may be refusal. The project makes no general semantic-recovery proof.

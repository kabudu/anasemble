# Validation

## Registered hypothesis

For components inside the finite DSL, separately distributed executable contracts
plus traces will recover declared behavior after verified total artifact loss more
often than trace-only synthesis, while independent checking prevents unsafe
deployment under the registered adversarial cases.

Baselines are: restore from backup/replica (expected to fail under stipulated
total loss), architecture reconstruction without synthesis, trace-only synthesis,
and contract synthesis from a centralized specification.

Primary metrics are certified correct recoveries and unsafe certifications.
Secondary metrics are refusal rate, authoring/storage cost, search time, generated
complexity, held-out conformance, and state-loss cost. One unsafe certification
inside the stated model blocks productisation.

## Falsifiers

- centralized contracts provide equal resilience;
- traces add no recovery value or induce unsafe overfitting;
- independence cannot be maintained;
- artifact-absence cannot be credibly demonstrated;
- authoring or search cost dominates ordinary redundant backup;
- bounded results do not generalize even within the declared DSL.

## M0 evidence

The registered M0 fixture is the two-state turnstile in `tests/common/mod.rs`, with
seed `20260729`. The fixture records backup/replica, trace-only, and centralized
contract baselines and the registered primary and secondary metrics. Comparative
baseline execution remains M2 work; M0 only freezes the experiment contract.

`./scripts/ci-local.sh` runs Rust formatting, Clippy with warnings denied, the
deletion-attested fresh-process recovery,
deterministic replay, independent-checker mutation, evidence omission and tamper,
forged-domain, surviving-artifact, symlink, canonicalization, compilation, and
repository-text checks. This local command is the authoritative CI for the
private repository.

## M1 evidence

The registered M1 corpus contains stateless identity and inversion components
under `fsm-v1`. The public corpus workflow reconstructs both after controlled
artifact deletion. Focused tests cover unique and ambiguous enumeration, negative
and metamorphic obligations, independent candidate-wire rejection, generated
WebAssembly equivalence, denied imports, fuel exhaustion, atomic ledger creation,
and stable replay. This establishes engineering feasibility only; M2 executes the
matched baseline and broader adversarial campaigns.

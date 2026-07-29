# Implementation Plan

## M0 — executable research contract (unchecked)

- [ ] Specify the finite typed component DSL and observable semantics.
- [ ] Define fragment envelopes, loss oracle, certification rules, and refusal codes.
- [ ] Register components, seeds, baselines, metrics, and artifact-deletion checks.
- [ ] Exit: reconstruct one pure finite-state component and prove the original
  artifact is inaccessible to the recovery process.

## M1 — independent reconstruction loop

Implement fragment collection, bounded enumerative synthesis, separate checker
interpreter, sandbox, and certificate generation.

## M2 — state and adversarial evaluation

Add modeled state transformation; test poisoned/omitted/contradictory fragments,
shared-fault domains, trace overfitting, resource exhaustion, and hidden effects.

## M3 — productisation decision

Proceed only if semantic distribution adds measurable recovery capability over
replication/backups and trace-only synthesis at an acceptable authoring cost.

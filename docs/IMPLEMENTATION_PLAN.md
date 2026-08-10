# Implementation Plan

## M0 - executable research contract (complete)

- [x] Specify the finite typed component DSL and observable semantics.
- [x] Define fragment envelopes, loss oracle, certification rules, and refusal codes.
- [x] Register components, seeds, baselines, metrics, and artifact-deletion checks.
- [x] Exit: reconstruct one pure finite-state component and prove, within the
  controlled M0 loss oracle, that the original artifact is inaccessible to the
  recovery process.

Evidence and M0 claim limits are recorded in
[M0_EXECUTABLE_CONTRACT](M0_EXECUTABLE_CONTRACT.md). The authoritative local CI
executes the deletion-attested recovery in a fresh process.

## M1 - independent reconstruction loop (complete)

- [x] Generalize fragment collection beyond the registered M0 component.
- [x] Implement bounded enumerative synthesis across the complete frozen DSL.
- [x] Add a capability-denied-by-default WebAssembly candidate sandbox.
- [x] Strengthen parser and semantic independence between synthesizer and checker.
- [x] Persist reproducible certificates and evidence-ledger entries.
- [x] Exit: reconstruct the registered stateless corpus without artifact access,
  sandbox escape, checker disagreement, or unbounded work.

Evidence, invariants, and limitations are recorded in
[M1_RECONSTRUCTION_LOOP](M1_RECONSTRUCTION_LOOP.md). The authoritative local CI
executes both registered stateless components through the public corpus workflow.

## M2 - state and adversarial evaluation (complete)

- [x] Implement explicitly modeled state transformation and rollback.
- [x] Execute matched baseline and registered metric campaigns.
- [x] Test poisoned, omitted, contradictory, replayed, and stale fragments.
- [x] Test shared-fault domains, forged provenance, and trace overfitting.
- [x] Test resource exhaustion, hidden effects, sandbox escapes, and partial deploy.
- [x] Exit: publish retained positive, refusal, timeout, disagreement, and negative
  results for the pre-registered corpus.

Evidence, failure bounds, units, and claim limits are recorded in
[M2_STATE_AND_ADVERSARIAL](M2_STATE_AND_ADVERSARIAL.md). The authoritative local
CI executes the public deployment and campaign boundaries. M3 remains the first
unchecked milestone.

## M3 - productisation decision

- [ ] Refresh systematic novelty, product, name, standards, and patent diligence.
- [ ] Complete independent reproduction, security, sandbox, and soundness review.
- [ ] Compare semantic distribution with replication/backups, trace-only
  synthesis, and centralized contracts at matched failure scope.
- [ ] Quantify authoring, storage, compute, operational, and state-loss costs.
- [ ] Make and record the stop, continue-as-research, or productise decision.
- [ ] If productisation is approved, complete the separately gated brand,
  adoption, packaging, release-presentation, and public-release work.

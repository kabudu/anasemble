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

## M1 - independent reconstruction loop

- [ ] Generalize fragment collection beyond the registered M0 component.
- [ ] Implement bounded enumerative synthesis across the complete frozen DSL.
- [ ] Add a capability-denied-by-default WebAssembly candidate sandbox.
- [ ] Strengthen parser and semantic independence between synthesizer and checker.
- [ ] Persist reproducible certificates and evidence-ledger entries.
- [ ] Exit: reconstruct the registered stateless corpus without artifact access,
  sandbox escape, checker disagreement, or unbounded work.

## M2 - state and adversarial evaluation

- [ ] Implement explicitly modeled state transformation and rollback.
- [ ] Execute matched baseline and registered metric campaigns.
- [ ] Test poisoned, omitted, contradictory, replayed, and stale fragments.
- [ ] Test shared-fault domains, forged provenance, and trace overfitting.
- [ ] Test resource exhaustion, hidden effects, sandbox escapes, and partial deploy.
- [ ] Exit: publish retained positive, refusal, timeout, disagreement, and negative
  results for the pre-registered corpus.

## M3 - productisation decision

- [ ] Refresh systematic novelty, product, name, standards, and patent diligence.
- [ ] Complete independent reproduction, security, sandbox, and soundness review.
- [ ] Compare semantic distribution with replication/backups, trace-only
  synthesis, and centralized contracts at matched failure scope.
- [ ] Quantify authoring, storage, compute, operational, and state-loss costs.
- [ ] Make and record the stop, continue-as-research, or productise decision.
- [ ] If productisation is approved, complete the separately gated brand,
  adoption, packaging, release-presentation, and public-release work.

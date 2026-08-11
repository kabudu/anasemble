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

## M3 - production engineering decision (in progress)

- [x] Refresh systematic novelty, product, name, standards, and patent diligence.
- [x] Publish a clean-room reproduction packet and complete the internal security,
  sandbox, trusted-computing-base, and soundness review.
- [x] Classify independent reproduction and external security/soundness review as optional post-release assurance, without representing either as completed.
- [x] Compare semantic distribution with replication/backups, trace-only
  synthesis, and centralized contracts at matched failure scope.
- [x] Quantify authoring, storage, compute, operational, and state-loss costs.
- [x] Decide to continue from the proven bounded kernel into production engineering.
- [ ] Complete the production roadmap below before claiming implementation completeness or product readiness.

The decision and evidence are recorded in [M3_DECISION](M3_DECISION.md). External reproduction and review may strengthen later claims but do not block completion or productisation. No release, tag, public opening, hosted CI, package publication, or deployment is authorized by this decision.

## Production-complete roadmap

“Production complete” means an operator can reconstruct, verify, stage, activate, observe, and roll back a supported real service and its declared state after the registered artifact-loss event. It does not mean arbitrary-program synthesis or recovery of undeclared effects. Every supported boundary must fail closed, remain resource-bounded, and have an adversarial end-to-end test.

### P0 - production contracts and durable state foundations

- [x] **P0.1 Service manifest:** bind a versioned HTTP interface, declared effects, state dependencies, and resource limits into recovery certificates; reject unknown, duplicate, ambiguous, or unbounded declarations.
- [x] **P0.2 Filesystem state adapter:** snapshot bounded regular-file state content-addressably and restore it atomically with integrity checking, exclusion locking, rollback, and crash-safe directory synchronization.
- [x] Exit: the public CLI validates a real-service contract and round-trips declared filesystem state without data loss under injected corruption, stale staging, concurrency, and partial-restore failures.

### P1 - production identity and evidence distribution

- [x] **P1.1 Production signatures:** add Ed25519 issuer identities and restrictive operator key files while retaining HMAC only for legacy research fixtures.
- [x] **P1.2 Identity lifecycle:** enforce bounded key rotation sets, validity intervals, revocation, issuer replay floors, equivocation rejection, and structured verification audit events.
- [x] **P1.3 Fragment stores:** add signed local-directory and HTTPS bundle adapters with distinct administrative domains, generation floors, bounded parallel reads, TLS, timeouts, retry budgets, quorum, and provenance receipts.
- [x] **P1.4 Evidence protection:** seal evidence with XChaCha20-Poly1305, support recovery-key rotation, enforce retention, protect key-file permissions, and provide exact materialization and deletion workflows.
- [x] **P1.5 Adversarial drill:** survive one registered store loss and reject a compromised store signature, revoked key, replayed sequence, issuer equivocation, tampered ciphertext, expired evidence, and insecure remote transport.
- [x] Exit: a multi-domain drill survives registered store loss and rejects compromised, revoked, replayed, or equivocated issuers.

### P2 - stateful service recovery

- [ ] Define the transactional state-adapter contract and implement PostgreSQL, S3-compatible object, and durable queue adapters with consistent snapshots or explicit refusal.
- [ ] Add schema discovery, migration planning, referential and consistency invariants, restore verification, and backend-native rollback.
- [ ] Bind reconstructed behavior, state schema, data snapshot, and migration evidence into one activation plan.
- [ ] Exit: end-to-end drills recover representative HTTP services with database, object, and queue state while proving rollback and no acknowledged-data loss within each declared consistency model.

### P3 - isolated runtime and deployment control plane

- [ ] Execute candidates in an OS-level isolation boundary with capability policy, network egress allowlists, filesystem isolation, CPU, memory, process, and wall-time quotas.
- [ ] Add adapters for at least one production orchestrator and artifact registry, using staged immutable artifacts, health gates, operator approval, idempotency, leases, and transactional rollback.
- [ ] Add secrets references that never expose secret values to synthesis, logs, fragments, or certificates.
- [ ] Exit: adversarial candidates cannot exceed declared capabilities, and interrupted or concurrent recovery jobs converge safely without split-brain activation.

### P4 - operations, compatibility, and product readiness

- [ ] Add durable job state, restart recovery, bounded scheduling and backpressure, structured audit events, metrics, diagnostics, and privacy-safe support bundles.
- [ ] Define compatibility, upgrade, configuration migration, backup interoperability, installation, uninstallation, and disaster-runbook contracts for supported platforms.
- [ ] Execute destructive staging-environment drills, sustained performance tests, dependency and supply-chain checks, and operator usability trials across every supported adapter combination.
- [ ] Complete bounded positioning, brand, adoption, packaging, curated release presentation, and release rollback evidence.
- [ ] Exit: every production claim maps to retained executable evidence, all supported failure paths have an operator response, local CI is green, and no unresolved critical or high internal review finding remains.

## Optional post-release assurance

- [ ] Independent clean-clone reproduction attestation.
- [ ] External security and soundness assessment.
- [ ] Broader third-party deployments and academic replication.

These stages are optional evidence amplifiers. They are not milestone or release blockers unless a later regulated deployment contract explicitly makes them mandatory.

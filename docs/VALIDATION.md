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
contract baselines and the registered primary and secondary metrics. M0 freezes
the experiment contract; M2 executes it.

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

## M2 evidence

The M2 campaign runs five retained outcome classes against the pre-registered
turnstile experiment: positive certification, ordinary refusal, search timeout,
checker disagreement, and hostile negative evidence. Every case executes the
same loss-scoped workspace through the registered backup/replica, trace-only, and
centralized-contract baselines.

Metric values use these units: certified recoveries and unsafe certifications are
counts; refusal rate is basis points; search time is elapsed microseconds for the
normal recovery path; candidate complexity is the total certified transition
count; authoring cost is registry plus fragment bytes. Timing is environmental
and is not expected to replay byte-for-byte.

The retained result at `experiments/m2-results.json` records one correct
certification, four refusals, one timeout, one checker disagreement, one hostile
negative result, and zero unsafe certifications. The executable campaign test
also covers freshness, replay, omission, contradiction, shared domains, forged
provenance, overfitting, atomic activation failure, and rollback. M1 sandbox tests
continue to cover imports, memory, and fuel because that boundary is unchanged.

## M3 evidence and decision

`experiments/m3-comparison.json` retains matched aggregate outcomes for Anasemble,
backup/replica, trace-only, and centralized-contract methods. The executable M2
campaign test verifies the retained certification and availability counts.
Centralized contracts match Anasemble's one certification and four refusals, so
the current corpus does not establish differentiated recovery value.

`experiments/m3-costs.json` records the 2026-08-11 single-fixture measurement:
594 artifact bytes versus 4,011 semantic input bytes, six signed fragments, and a
3.22-second warm-cache local CI run on arm64 macOS with Rust 1.97.0. These are
bounded engineering observations, not production estimates.

The clean-room protocol is in `INDEPENDENT_REPRODUCTION.md`. It has been checked for completeness and rerun locally, but no independent party has supplied an attestation. The project proceeds into production engineering because independent reproduction and external security review are optional post-release assurance, not implementation gates. Their absence remains explicit.

P0 verification adds module and public-CLI tests for a certificate-bound service manifest and a bounded content-addressed filesystem state adapter. The state tests cover integrity corruption, exclusion locking, stale staging, round-trip restore, commit, rollback, and injected failure after rollback preparation. These foundations do not verify HTTP runtime generation or non-filesystem backends.

## P1 identity and evidence validation

The P1 suite executes the real key-generation, retrieval, materialization, and deletion CLI boundary plus focused cryptographic and policy functions. It proves that two signed administrative stores recover six independently signed and encrypted fragments when a third store is lost; every fragment must meet copy quorum. A store with invalid ciphertext is excluded without blocking recovery when two complete stores survive.

Negative paths cover a compromised store signature, insufficient store quorum, insufficient per-fragment copies, issuer revocation, replay floors, equivocation, verification-time key expiry, ciphertext tampering, retention expiry, permissive secret-file exposure, and cleartext remote URLs. HTTPS success is implemented through Rustls/WebPKI but does not yet have a repository-owned live TLS fixture; the local signed-store drill is the retained P1 end-to-end evidence.

## P2 stateful recovery validation

The P2 drill starts dedicated disposable PostgreSQL 18, MinIO, and Redis 8 containers. It snapshots a relational schema with referential constraints, destroys the source schema, restores into a staged schema, verifies rows and foreign-key enforcement, and rolls back to the prior target. It also restores and rolls back exact object bytes and Redis Stream entries while preserving a consumer-group cursor. A pending Redis delivery is refused.

The same drill reconstructs a certified `service-v1` HTTP service and binds its certificate, service manifest, three schemas, snapshots, and migration plans into one canonical activation plan. A mismatched service digest is rejected. This is backend recovery evidence, not OS-isolated runtime activation or cross-backend atomicity; those remain P3 work.

## P3 isolated activation validation

The P3 suite executes a digest-pinned Debian candidate with no network, no Linux
capabilities, a read-only root, bounded tmpfs, CPU, memory, PID, output, and wall
time. The candidate observes the PID and capability limits, cannot write the root
filesystem, has no default route, and is forcibly stopped at its deadline.

A disposable OCI registry drill refuses an unlabeled artifact, publishes a
plan/candidate-labelled image, verifies its immutable digest, and activates it
only with a matching Ed25519 operator approval. Injected interruption after the
active-name transition leaves an exclusive same-plan lease and is reconciled by
retry; a competing plan is refused. The test verifies secret-value absence,
disabled runtime logging, zero external egress, idempotency, and restoration of
the prior container.

A disposable kind cluster receives its image through a local Docker export. The
Kubernetes drill stages immutable Deployments, uses Secret references, disables
service-account token mounting, installs a zero-egress NetworkPolicy, waits for
readiness, switches one Service selector, retains a Lease across interruption,
refuses a competing plan, resumes idempotently, and rolls back to the prior
selector. The drill validates Kubernetes control objects; production egress
enforcement additionally trusts a NetworkPolicy-enforcing CNI.

## P4 operations and readiness validation

The P4 restart test admits two jobs into a capacity-two store, durably interrupts one after claim, verifies backpressure, executes the other, advances beyond the lease, and proves the interrupted job is recovered exactly once. Metrics retain two refusals, three total attempts, and one restart recovery. The support bundle excludes both the canonical workspace path and private refusal messages.

The sustained test admits 128 separately durable records, proves the 129th is
refused, drains exactly two 64-job batches, and requires completion within 20
seconds on the local arm64 development profile. A separate contention test
proves that a transient lock succeeds within the 495 ms retry-delay budget and a
persistent lock still refuses. This is a regression ceiling for the supported
file store, not a universal throughput claim.

The public operator trial migrates `operations-config-v0`, initializes a store, enqueues a real workspace only after artifact deletion, executes normal reconstruction, observes certification through `operations-status`, creates a redacted support bundle, installs the running Rust binary to a staged exact prefix, and removes it through its verified manifest.

The retained staging inventory is the union of the P2 PostgreSQL 18, MinIO S3
API, and Redis 8 destructive drill; the P3 Docker sandbox, OCI registry, Docker
activation, and disposable Kubernetes drill; and the P4 public operations
lifecycle. `COMPATIBILITY.md` classifies these profiles independently and does
not infer support for their Cartesian product. CI also runs Rustfmt, Clippy with
warnings denied, all tests and docs offline, RustSec audit against the
pre-fetched local database, Cargo Deny advisory/ban/source checks,
release-presentation checks, text-policy checks, and diff checks.

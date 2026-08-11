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

## M1 implementation boundary

M1 generalizes deterministic full-table enumeration under compiled and
registry-selected bounds. Candidates cross a checker-private binary wire format,
then compile to import-free WebAssembly and execute with an empty linker, fuel,
instance, table, and memory limits. A content-addressed ledger atomically preserves
inputs and outcomes. External state, deployment, production key identity, and a
separate checker process remain outside M1.

## M2 implementation boundary

M2 represents external FSM state as a versioned state symbol and monotonic
revision. A total, operator-supplied mapping converts that symbol into the
certified candidate schema. Candidate and transformed state are encoded in one
bounded deployment bundle and activated by synchronized atomic rename. The prior
bundle is independently persisted for rollback before replacement.

The campaign runner executes each registered workspace through the normal
recovery path and the backup/replica, trace-only, and centralized-contract
baselines. It retains typed decisions and registered metric observations. This is
local file transactionality, not a distributed deployment protocol.

## Production architecture direction

The M0 through M2 implementation is the reconstruction and certification kernel, not the complete product. Production usefulness requires four additional bounded planes around it:

1. **Contract plane:** versioned service interfaces, effects, state dependencies, compatibility, and resource policies become certificate-bound inputs.
2. **Evidence plane:** independently administered, authenticated, encrypted fragment stores operate under bounded concurrency, timeout, retry, rotation, revocation, and retention policies.
3. **State plane:** backend-specific adapters acquire consistent snapshots, verify integrity and invariants, plan migrations, restore transactionally, and retain native rollback evidence.
4. **Activation plane:** an isolated runtime and operator-controlled deployment coordinator stage immutable artifacts, enforce capabilities, evaluate health, serialize activation, and roll back without exposing secrets to synthesis.

The Rust CLI remains the initial control plane and the file protocol remains the compatibility boundary. New adapters implement narrow traits inside the same process until measured scaling or isolation requirements justify a service split. No network daemon, scheduler, database, or plugin runtime is introduced merely for architectural symmetry.

[TCB_LEDGER](TCB_LEDGER.md) records each trusted element and its failure consequence. Each production adapter expands the TCB and therefore needs explicit invariants, resource bounds, adversarial tests, and removal behavior before it becomes supported.

## P1 evidence plane

P1 adds an in-process evidence plane rather than a daemon or plugin runtime. Issuer envelopes and store bundles have separate Ed25519 identities. Store workers fetch one bounded signed bundle each through a local-directory or HTTPS transport, then return to a deterministic coordinator that enforces store quorum, decrypts retained evidence, deduplicates exact replicas, and invokes the existing collector. XChaCha20-Poly1305 protects each signed envelope independently, so a store cannot read or silently alter semantic evidence.

Concurrency is bounded by configured batches rather than one thread per store. HTTPS performs one request per attempt and has a global timeout and retry budget. The output boundary is an explicitly temporary owner-only directory compatible with the existing kernel; deletion is a separate fail-closed operator action.

## P3 activation plane

P3 remains a Rust library boundary and delegates established mechanisms to Docker,
an OCI Distribution registry, and Kubernetes through bounded command adapters. A
validated activation plan is carried into OCI labels and a canonical registry
receipt. Operator approval signs the plan and artifact binding before any traffic
switch.

Docker provides the directly exercised candidate sandbox and a single-host
activation drill. Kubernetes is the production orchestrator adapter: immutable
Deployments are staged behind a zero-egress NetworkPolicy, a Lease serializes each
service, and one Service selector patch switches traffic. Both adapters retain the
prior workload until commit and reconcile a same-plan interruption idempotently.
The exact supported profile and trust boundary are in
[P3_ISOLATED_ACTIVATION](P3_ISOLATED_ACTIVATION.md).

## P4 operations plane

P4 adds a file-backed Rust operations store rather than a daemon or control-plane database. Owner-only digest-sealed job records, atomic replacement, immutable results, hash-chained events, one store lock, and expiring execution leases make each CLI invocation restart-safe. Queue capacity, batch size, attempts, leases, event count, record size, result size, and total job count are bounded.

The scheduler claims under the store lock, releases it during reconstruction, then reacquires it to finalize the same running record. This keeps producer and status latency independent of candidate execution while preventing two runners from mutating the store concurrently. Expired work returns to pending; exhausted work becomes a retained failure. Aggregate metrics and diagnostics are derived from validated records rather than a second mutable database.

Installation uses a sibling staging prefix, syncs the binary and machine-readable compatibility/configuration files, then atomically renames the complete prefix. Uninstallation is manifest-driven and refuses changed files or unexpected topology. See [P4_OPERATIONS_AND_READINESS](P4_OPERATIONS_AND_READINESS.md).

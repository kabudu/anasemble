# End-to-End Testing

Each test builds a component, distributes fragments, records artifact digests,
removes all source/binary/container/identical replicas from the isolated recovery
environment, and then runs reconstruction.

Suites cover pure functions, finite-state components, version negotiation,
state-schema migration, poisoned and omitted fragments, contradictory contracts,
trace overfitting, forged failure-domain labels, interpreter differential,
synthesis timeout, sandbox escape attempts, checker failure, and partial deploy.

Assertions include artifact-absence attestation, mandatory contract satisfaction,
held-out behavior, negative cases, resource bounds, provenance quorum, refusal
reason, reproducibility, and rollback. A byte-identical output is permitted but
not required; access to the lost artifact is forbidden.

M0 implements the pure finite-state subset in `tests/cli_e2e.rs`. The original
artifact is deleted before a newly built Rust CLI subprocess starts; the
subprocess receives the recovery workspace and a cleared deterministic
environment. The oracle checks registered paths, rejects
symlinks and non-regular files, and performs a bounded streaming digest scan.
This is a controlled local absence proof, not a kernel sandbox or secure-erasure
claim. The remaining suites belong to M1 and M2.

M1 adds the public `recover-corpus` and `recover --ledger` workflows, two distinct
stateless components, full generated-Wasm/table equivalence, import denial, fuel
exhaustion, checker-wire truncation and trailing-data rejection, enumerative
ambiguity refusal, executable negative and metamorphic obligations, and immutable
ledger replay.

M2 adds the public `deploy`, `rollback`, and `evaluate-campaign` workflows.
Focused suites exercise mapped state, monotonic revision, activation failure,
rollback restoration, freshness rejection, poisoned, omitted, contradictory and
replayed fragments, shared-domain quorum collapse, forged provenance, held-out
trace disagreement, matched baselines, and retained outcome classes. The M1
import, memory and fuel tests remain the executable hidden-effect, sandbox escape,
and resource-exhaustion evidence for the unchanged sandbox boundary.

M3 binds the retained comparison to the public M2 campaign assertions and publishes `INDEPENDENT_REPRODUCTION.md` for an optional external clean-clone run. P0 then adds the public `validate-service`, `snapshot-state`, `restore-state`, `rollback-state`, and `commit-state` workflows. Tests bind a service manifest digest into a recovery certificate and exercise state integrity, locking, staging, activation failure, rollback, and commit through both module and CLI boundaries.

P4 adds the public operations lifecycle: configuration migration, store creation,
deleted-artifact job admission, durable claim, restart recovery, bounded
execution, status and metrics, redacted support generation, installation, and
exact uninstallation. A sustained test processes 128 durable jobs in two fixed
batches. The authoritative CI combines this with the destructive P2
PostgreSQL/S3/Redis drill and P3 Docker/OCI/Kubernetes drill. The compatibility
matrix classifies each resulting profile independently; their union is not a
supported Cartesian product.

The integrated reference suite composes the public preparation, recovery and
rollback commands. It captures PostgreSQL, S3 and Redis state, deletes all three
sources and the original component artifact, reconstructs a certified candidate,
restores the three targets, packages and publishes a plan-bound OCI image,
activates it in a disposable Kubernetes cluster, and then verifies reverse-order
rollback of Kubernetes and every state backend. The image contains and checks
the certified finite-state candidate; it is not evidence of generated HTTP server
code. The separate Linux matrix validates clean-clone control-plane builds and
non-Docker tests for arm64 and emulated x86_64.

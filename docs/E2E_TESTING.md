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
ledger replay. M2 retains poisoned provenance campaigns, state migration,
arbitrary sandbox attacks, partial deployment, and matched baselines.

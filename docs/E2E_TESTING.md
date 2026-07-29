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

# P0 Production Foundations

P0 establishes the first two dependency-ordered production contracts around the proven reconstruction kernel. It does not make Anasemble production-complete.

## P0.1 Service manifest

`service-v1` declares one component and interface version, a bounded HTTP endpoint set, SHA-256 request and response schema identities, explicit effect access, named state dependencies, consistency expectations, and request, response, wall-time, and concurrency maxima. Unknown JSON fields, duplicate routes, unsafe paths or identifiers, invalid digests, unmatched state effects, duplicate dependencies, and excessive bounds fail closed.

`examples/service-v1.json` is the validated operator example. Its repeated schema digests are illustrative identifiers and must be replaced with the SHA-256 digests of the operator's canonical schemas before use.

The recovery registry may embed this manifest. When present, its identity must exactly match the registry and its canonical digest is included in the recovery certificate. Legacy M0 through M2 fixtures may omit it for protocol compatibility; a future production activation gate must require it.

The manifest describes and binds a real-service surface. The current FSM generator does not yet synthesize general HTTP codecs or effect implementations, so validation and certificate binding must not be reported as runtime support.

## P0.2 Filesystem state adapter

The adapter snapshots one regular file up to 64 MiB into an immutable SHA-256-addressed object and atomically replaces a versioned current manifest. Later snapshots must preserve component identity and strictly increase revision. Restore verifies manifest shape, payload length, and payload digest before staging any destination mutation. Store and destination locks serialize operations. The restored file is synced before activation, the prior destination is retained as a rollback sidecar, directory entries are synced, and an injected activation failure restores the prior bytes.

An operator must choose `rollback-state` or `commit-state` before another restore. Restore records an activation marker bound to the new payload; commit rehashes the active file and refuses to discard rollback if the bytes changed. Stale lock, stage, rollback, or activation files are evidence of an interrupted or concurrent operation and fail closed. Direct symbolic-link boundaries are rejected. Ancestor paths and processes with equal host filesystem authority remain trusted, so OS-level isolation is still required by P3.

## Resource and performance bounds

Service manifests allow at most 256 endpoints, 64 effects, 64 state dependencies, 64 MiB request or response bodies, 300 seconds wall time, and 10,000 concurrent requests. These are validation ceilings, not recommended operating values. Filesystem state operations use at most one 64 MiB payload buffer plus encoded metadata and perform linear hashing and I/O. There is no unbounded retry, traversal, background worker, or network operation.

## Verification

`tests/production_foundations.rs` exercises the public manifest and state CLI, certificate binding, identity mismatch, content corruption, exclusion locks, stale staging, restore rollback and commit, and an injected partial-activation failure. Module tests cover duplicate routes, unsafe traversal, unknown fields, and excessive resource bounds. The authoritative gate remains `./scripts/ci-local.sh`.

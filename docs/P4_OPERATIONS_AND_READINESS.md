# P4 Operations and Product Readiness

P4 completes the implementation boundary for the explicitly supported Anasemble profiles. It does not authorize publication, deployment, a package registry upload, or a general service-reconstruction claim.

## Durable operations

`operations-config-v1` bounds queue capacity to 1,024, work per invocation to 64 jobs, attempts to 10, and leases to 10 seconds through 24 hours. The default is 256 queued jobs, eight jobs per run, three attempts, and a five-minute lease. Every job is an owner-only, atomically replaced, digest-sealed JSON record capped at 64 KiB with a maximum of 64 hash-chained state events. Results are separate immutable files capped at 16 MiB.

Claiming is durable before execution. A heartbeat renews the runner lease from
the system clock while the shorter store lock protects each transition. The
store lock retries transient heartbeat contention for at most 500 ms before
refusing a persistent or stale lock. Process death stops the heartbeat and
allows a new runner after the bounded lease. An expired running job lease is
returned to pending until its attempt budget is exhausted. Queue admission
counts pending and running records and fails closed at capacity. Workspaces are
content-digested across at most 1,024 regular files and 64 MiB at admission and
again before execution. Scheduling is deliberately single-worker per CLI
invocation; operators scale by bounded repeated invocations, not concurrent
access to one store.

Terminal records remain until the operator retains their audit evidence and invokes `prune-jobs`; each call removes at most 256 verified terminal records and immutable results and returns their digests. Pending or running work is never pruned.

`operations-status` derives counts and diagnostic codes from validated records. `job-result` verifies and returns one immutable terminal result through the public boundary. `create-support-bundle` emits only job IDs, workspace-reference digests, states, attempt counts, refusal codes, audit-event digests, configuration digest, and aggregate metrics. It excludes workspace paths, results, refusal messages, fragments, candidate bytes, keys, credentials, approvals, and secret values.

## Evidence and usability

The public CLI test migrates legacy configuration, creates a store, enqueues and executes a real deleted-artifact recovery, reads certified status, generates a redacted bundle, installs the executable to a new prefix, and removes it through the verified manifest. Restart, backpressure, tamper, and sustained 128-job tests exercise the library boundary.

The authoritative local CI also executes the destructive P2 PostgreSQL/S3/Redis
state drill and P3 Docker/OCI/Kubernetes activation drills. P2 local state and
Docker activation are supported profiles; the kind drill provides partial
evidence for the experimental Kubernetes profile. Arbitrary Cartesian
combinations, remote PostgreSQL or Redis, x86 hosts, and policy-ignoring
Kubernetes CNIs are unsupported.

Dependency checks use the current local RustSec database with `cargo audit --no-fetch` and Cargo Deny in locked offline mode for advisories, bans, and sources. Cargo Deny reports warning-level duplicate transitive versions from the supported dependency graph; there are no advisory, ban, or source failures. License policy is not asserted because the private repository has no user-approved product licence; selecting one is a legal and release-authority decision.

## Release boundary

The curated release-candidate presentation is retained under `release/`, while `publish = false` remains in `Cargo.toml`. A public release still requires explicit user authority, name and legal clearance, an approved product licence, version selection, rendered release-note inspection, and the release procedure in [RELEASE](RELEASE.md). Optional independent reproduction and external security review remain post-release assurance rather than implementation gates.

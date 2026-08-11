# P3 Isolated Activation

P3 turns a certificate-bound `activation-plan-v1` into an operator-approved,
immutable runtime activation. It does not widen the reconstruction language or
allow arbitrary workloads. The supported profile is deliberately narrow:

- Linux OCI images identified by a `sha256` manifest digest;
- Docker Engine for adversarial candidate execution and single-host drills;
- an OCI Distribution registry for immutable publication;
- Kubernetes Deployments, Services, NetworkPolicies, Secrets, and Leases for the
  production orchestrator adapter;
- zero network egress, a read-only root filesystem, bounded writable `/tmp`, and
  explicit CPU, memory, PID, wall-time, output, and Linux-capability limits.

Non-empty egress allowlists are refused. This is an intentional supported-profile
boundary, not an unimplemented permissive fallback.

## Activation invariants

An image can be published only when its `anasemble.plan` and
`anasemble.candidate` labels exactly match the validated activation plan. The OCI
manifest digest, plan digest, candidate digest, and their canonical binding form
the registry receipt. An Ed25519 operator approval signs both plan and artifact
binding within a configured trust window.

Activation stages a digest-addressed workload, waits for a bounded health probe,
and changes the active name or Service selector only after health succeeds. A
lease binds one service to one plan. The same plan reconciles idempotently after
interruption; a competing plan is refused. The previous workload remains as the
rollback target until an explicit commit.

Secrets enter the control plane only as owner-only file references for Docker or
Kubernetes Secret name/key references. Values are not serialized into deployment
specifications, receipts, approvals, labels, certificates, or health errors.
Docker runtime logging is disabled for activated workloads. Applications remain
responsible for not disclosing mounted secrets through their own external effects.

## Trust and failure boundaries

The Docker daemon, OCI registry, `kubectl`, Kubernetes API server, scheduler,
container runtime, admission configuration, and cluster network-policy
implementation are trusted. A production Kubernetes cluster must use a CNI that
enforces NetworkPolicy; creating a policy is not proof that the CNI enforces it.
The kind drill validates object construction and switching, while the Docker
sandbox test directly demonstrates denied egress and capability, filesystem, PID,
and wall-time bounds.

The Docker adapter is a single-host recovery profile, not a highly available
orchestrator. The Kubernetes Service selector update is the atomic traffic switch;
individual endpoint propagation remains Kubernetes behavior. A host or cluster
administrator can bypass every control. Side channels, kernel or runtime escapes,
registry compromise, admission mutation, and faults after explicit commit remain
outside P3 guarantees.

## Resource and performance bounds

Policies accept 10 to 4,000 CPU millicores, 16 MiB to 2 GiB memory, 1 to 512
processes, 10 ms to 300 seconds wall time, at most 1 MiB captured output, and 1 to
256 MiB writable temporary storage. Commands contain at most 128 arguments and
16 KiB total argument bytes. Kubernetes reads and Docker command output are
bounded to 1 MiB. Registry publication has twenty attempts with 100 ms spacing;
health probes use an explicit bounded attempt count and interval.

## Retained evidence

`tests/p3_activation.rs` executes three destructive local drills. It proves the
Docker sandbox boundary, OCI label and immutable-digest binding, secret-reference
handling, interruption reconciliation, lease conflict refusal, idempotency,
health-gated switching, and rollback. A disposable kind cluster validates the
Kubernetes Lease, Deployment, Secret reference, NetworkPolicy, Service selector,
security context, interruption recovery, conflict refusal, and rollback paths.
All images are pre-cached and kind imports the workload image locally, so the
authoritative private-repository CI remains local and offline.

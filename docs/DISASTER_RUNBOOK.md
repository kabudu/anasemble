# Disaster Recovery Runbook

## Preconditions

Confirm the incident matches the registered component, interface, effects, state backends, consistency model, platform, adapter versions, and total-artifact-loss boundary. Freeze writers for every state backend. Prefer a healthy native backup when it meets the objective. Preserve logs, locks, leases, failed staging resources, prior installation prefix, and all rollback receipts.

## Recovery sequence

1. Verify the pinned binary and compatibility manifest from the approved installation prefix.
2. Retrieve and authenticate evidence from the required independently administered stores; stop if quorum, freshness, identity, or retention checks fail.
3. Enqueue the workspace with `enqueue-recovery` and inspect `operations-status`. Queue saturation is a refusal to accept more work.
4. Run one bounded batch with `run-jobs`. A process interrupted before its lease expires must not be started concurrently. After expiry, the same store requeues it until the configured attempt limit.
5. Inspect the certified result and activation plan. A refusal is terminal for that evidence set; do not override it.
6. Restore PostgreSQL, object, Redis, and filesystem state according to their receipts while writers remain frozen. On any failure, roll back successful restores in reverse order.
7. Publish the exactly labelled OCI image and obtain operator approval over the plan and artifact binding.
8. Stage through the supported Docker profile or an explicitly accepted
   experimental Kubernetes profile, pass the bounded health gate, and switch
   traffic. Never delete or bypass an activation lease.
9. Exercise representative reads and writes through the customer-facing endpoint. If acceptance fails, roll back activation first, then state receipts in reverse order.
10. Commit activation and state only after acceptance. Delete temporary plaintext evidence and retain audit, result, approval, receipt, metrics, and support-bundle digests under the incident retention policy.
11. Only after retained evidence is verified, prune terminal operations records in bounded batches and preserve each prune receipt.

## Failure responses

| Condition | Operator response |
|---|---|
| Evidence invalid, stale, contradictory, or below quorum | Refuse; repair evidence custody outside the incident workspace |
| Queue saturated | Stop producers; drain bounded batches; do not enlarge limits during the incident |
| Store lock present | Establish process ownership; preserve it for incident review; never delete blindly |
| Running job lease unexpired | Wait for the owner or lease deadline; do not run concurrently |
| Attempts exhausted | Preserve the failed record and diagnostics; create a new job only after root cause correction |
| Snapshot unstable or Redis pending work present | Keep writers frozen and obtain a consistent native snapshot; do not force restore |
| Health gate fails | Leave active traffic unchanged; inspect the staged workload and retain evidence |
| Activation lease conflict | Determine the owning plan and operation; retry only the exact approved operation or roll back |
| NetworkPolicy enforcement unknown | Refuse Kubernetes activation and use a verified cluster |
| Support bundle requested | Generate `support-bundle-v1`; review it before transfer; never attach job results, paths, fragments, keys, or secret values |
| Install or uninstall digest mismatch | Stop; preserve the prefix and compare it with the approved artifact |

## Recovery objective boundary

No wall-clock recovery-time objective is promised. Work is bounded by configured job batches, backend sizes, candidate search limits, health attempts, and activation timeouts. State-loss guarantees are exactly those in the individual backend receipts; there is no distributed transaction spanning all state systems and runtime activation.

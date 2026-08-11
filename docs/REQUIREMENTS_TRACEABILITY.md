# Requirements Traceability

| ID | Requirement | Design | Verification |
|---|---|---|---|
| SEM-1 | Recover after total artifact loss | isolated recovery harness | `test_fresh_process_is_deterministic_and_receives_only_workspace` |
| REL-1 | Use independent semantic fragments | distributor/provenance graph | domain-loss tests |
| SEM-2 | Generate non-identical replacement | bounded DSL synthesizer | forbidden/candidate digest comparison in certificate |
| SEM-3 | Independently certify behavior | separate checker interpreter | `test_checker_rejects_mutated_candidate` |
| SEC-1 | Contain generated code | import-free Wasmi sandbox with empty linker and fuel | import-denial, fuel-exhaustion, and transition-equivalence tests |
| SEM-4 | Handle modeled state | versioned mapping plus atomic deployment bundle | `state_transform_deploy_partial_failure_and_rollback_are_atomic` |
| SEC-2 | Refuse insufficient evidence | typed protocol refusals | omission, tamper, domain-forgery, and artifact-presence tests |
| REL-2 | Reproduce claims | canonical output, fixed seed, and retained campaign schema | replay and public campaign tests |
| EXP-1 | Execute matched baselines and metrics | bounded campaign runner | `campaign_retains_positive_refusal_timeout_disagreement_and_negative_results` |
| SEC-3 | Refuse stale or adversarial evidence | freshness window and fail-closed collector | M2 evidence and trust campaign tests |
| REL-3 | Revert failed deployment | synchronized active and rollback bundles | partial failure and rollback test |
| NOV-1 | Maintain a falsifiable contribution boundary | dated diligence and prior-art matrix | M3 search log and optional external challenge track |
| REL-4 | Enable independent reproduction without overstating it | clean-room packet and authoritative local CI | local packet verification; independent attestation remains open |
| PERF-1 | Quantify bounded research cost | retained M3 cost record | fixture byte counts and timed local CI |
| BRD-1 | Gate product and brand work on evidence and approval | explicit production-engineering decision with separate release authority | M3 decision and unchanged private-release policy |
| PROD-1 | Bind supported service behavior and resource policy to certification | `service-v1` HTTP manifest in the recovery registry and certificate digest | service unit tests and `service_manifest_is_validated_by_cli_and_bound_to_certificate` |
| STATE-1 | Preserve bounded filesystem state through recovery | content-addressed immutable object, versioned manifest, atomic restore, rollback sidecar, and commit | production-foundation corruption, lock, failure-injection, round-trip, and public CLI tests |
| ID-1 | Authenticate production issuers without shared verifier secrets | Ed25519 key IDs, bounded rotation policy, validity, revocation, replay floor, and audit events | P1 rotation, revoked-key, replay, equivocation, and audit tests |
| EVID-1 | Retrieve evidence from independent stores under bounded failure | signed generation bundles, unique administrative domains, local/HTTPS transports, batched workers, timeouts, retries, quorum, and provenance | P1 loss, compromised-store, quorum, and insecure-transport tests |
| SEC-4 | Protect semantic evidence and make retention explicit | XChaCha20-Poly1305 seals, restrictive recovery-key files, authenticated retention deadline, temporary materialization, and exact deletion commands | P1 tamper, expiry, materialization, and deletion tests |
| STATE-2 | Recover bounded relational state without the source database | PostgreSQL schema evidence, binary table snapshots, repeatable-read capture, constraint recreation, verified transactional schema swap, and retained rollback schema | P2 destroys the source schema before restore and verifies rows, foreign keys, and rollback |
| STATE-3 | Recover bounded object state without accepting a torn snapshot | two-pass S3 key, ETag, and byte capture; disjoint rollback prefix; exact key-set verification; fail-closed bounds | P2 MinIO replacement and rollback drill |
| STATE-4 | Recover acknowledged durable queue state within its declared model | stable Redis Stream entry and consumer-group capture, preserved IDs and group cursor, pending-entry refusal, atomic key swap, and rollback key | P2 Redis entries, cursor, rollback, and pending-work refusal drill |
| PROD-2 | Bind behaviour, service contract, schemas, snapshots, and migrations before activation | canonical `activation-plan-v1` with certificate-bound service digest and unique backend resources | P2 three-backend activation binding and mismatched-manifest refusal |
| SEC-5 | Execute a certified candidate within declared OS and resource capabilities | digest-pinned Docker sandbox with zero egress, read-only root, tmpfs, capability allowlist, CPU, memory, PID, output, and wall bounds | `docker_sandbox_enforces_supported_os_capability_boundary` |
| PROD-3 | Publish and activate only plan-bound immutable artifacts with explicit operator authority | OCI labels and receipt binding, Ed25519 approval, health-gated Docker and Kubernetes staging | P3 registry and orchestrator drills |
| REL-5 | Converge safely after interrupted or concurrent activation | per-service lease, same-plan reconciliation, atomic name or Service-selector switch, retained rollback target | Docker and Kubernetes interruption, competing-plan, idempotency, and rollback tests |
| SEC-6 | Keep runtime secret values outside reconstruction and activation evidence | owner-only Docker file references, Kubernetes Secret name/key references, disabled Docker workload logging | P3 receipt, deployment-object, mount, and log-driver assertions |
| OPS-1 | Preserve recovery work across process interruption | atomic digest-sealed job records, expiring leases, bounded attempts, and restart requeue | P4 injected-after-claim restart test and public CLI lifecycle |
| OPS-2 | Bound admission and execution while exposing actionable operations state | queue backpressure, fixed batch size, single store lock, derived metrics, and diagnostic codes | P4 saturation, 128-job sustained, metrics, and status tests |
| SEC-7 | Produce useful support evidence without exposing recovery inputs or secrets | allowlisted support schema containing identifiers, digests, states, counts, and refusal codes only | P4 path and private-diagnostic exclusion assertions |
| COMP-1 | Install, migrate, upgrade, roll back, and remove without implicit mutation or data deletion | out-of-place config migration, atomic exact-prefix installation, digest-verified manifest removal, preserved operations root | P4 public CLI migration/install/uninstall test and lifecycle contracts |
| COMP-2 | Distinguish implemented, tested, supported, experimental, and unsupported combinations without implying a Cartesian product | versioned evidence-linked compatibility profiles with independent implementation, validation, and support status | installed-manifest profile uniqueness, evidence existence, and supported-status invariant assertions in the P4 public lifecycle test |
| PROD-4 | Map every supported production claim and failure path to retained evidence and operator action | compatibility manifest, staging matrix, disaster runbook, curated preview, roadmap and traceability audit | repository checker plus complete local CI |
| EVAL-1 | Provide one public workflow from preparation through reconstruction, three-backend restoration, immutable publication, Kubernetes activation, health gating, and rollback | `reference-recovery-config-v1`, sealed state bundle, plan-bound OCI package, operator approval, and retained composite receipt | `reference_workflow_prepares_recovers_activates_and_rolls_back` through three public CLI commands |
| EVAL-2 | Make Linux portability evidence reproducible without hosted CI | digest-pinned clean clones with one dependency-fetch phase and a network-disabled build/test phase | `scripts/ci-linux-matrix.sh` on arm64 host ISA and emulated x86_64, with exact limits in `docs/LINUX_MATRIX.md` |
| SEC-8 | Prevent corrupted recovery receipts or uncertain Kubernetes outcomes from directing inconsistent destructive state changes | sealed composite receipt with plan/artifact/backend cross-validation and explicit external-state uncertainty | tampered-receipt refusal plus integrated activation, rollback and acceptance drill |
| PROD-5 | Let an operator accept a verified integrated recovery and retire rollback resources safely | read-only preflight across PostgreSQL, S3 and Redis followed by idempotent backend and Kubernetes commit | public `commit-reference-recovery` path in `tests/reference_workflow.rs` |

M2 satisfies SEM-4 only for explicitly enumerated FSM state. M0 through M2
authenticate issuer-to-domain policy but do not claim hardware or organizational
independence.

M1 implements the bounded SEC-1 candidate ABI. M2 tests the generated ABI and
denial paths, not general third-party WebAssembly or process isolation.

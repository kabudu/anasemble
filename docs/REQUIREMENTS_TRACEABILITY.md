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

M2 satisfies SEM-4 only for explicitly enumerated FSM state. M0 through M2
authenticate issuer-to-domain policy but do not claim hardware or organizational
independence.

M1 implements the bounded SEC-1 candidate ABI. M2 tests the generated ABI and
denial paths, not general third-party WebAssembly or process isolation.

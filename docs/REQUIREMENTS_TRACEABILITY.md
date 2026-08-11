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
| NOV-1 | Maintain a falsifiable contribution boundary | dated diligence and prior-art matrix | M3 search log and external challenge gate |
| REL-4 | Enable independent reproduction without overstating it | clean-room packet and authoritative local CI | local packet verification; independent attestation remains open |
| PERF-1 | Quantify bounded research cost | retained M3 cost record | fixture byte counts and timed local CI |
| BRD-1 | Gate product and brand work on evidence and approval | explicit continue-as-research decision | M3 decision and unchanged private-release policy |

M2 satisfies SEM-4 only for explicitly enumerated FSM state. M0 through M2
authenticate issuer-to-domain policy but do not claim hardware or organizational
independence.

M1 implements the bounded SEC-1 candidate ABI. M2 tests the generated ABI and
denial paths, not general third-party WebAssembly or process isolation.

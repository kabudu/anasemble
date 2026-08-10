# Requirements Traceability

| ID | Requirement | Design | Verification |
|---|---|---|---|
| SEM-1 | Recover after total artifact loss | isolated recovery harness | `test_fresh_process_is_deterministic_and_receives_only_workspace` |
| REL-1 | Use independent semantic fragments | distributor/provenance graph | domain-loss tests |
| SEM-2 | Generate non-identical replacement | bounded DSL synthesizer | forbidden/candidate digest comparison in certificate |
| SEM-3 | Independently certify behavior | separate checker interpreter | `test_checker_rejects_mutated_candidate` |
| SEC-1 | Contain generated code | capability sandbox | escape/resource E2E |
| SEM-4 | Handle modeled state | migration planner | stateful component suite |
| SEC-2 | Refuse insufficient evidence | typed protocol refusals | omission, tamper, domain-forgery, and artifact-presence tests |
| REL-2 | Reproduce claims | canonical output and fixed seed | two-process byte-for-byte replay test |

SEC-1, SEM-4, and the full REL-1 domain-loss campaign remain later-milestone
requirements. M0 authenticates issuer-to-domain policy but does not claim
hardware or organizational independence.

# Requirements Traceability

| ID | Requirement | Design | Verification |
|---|---|---|---|
| SEM-1 | Recover after total artifact loss | isolated recovery harness | `test_fresh_process_is_deterministic_and_receives_only_workspace` |
| REL-1 | Use independent semantic fragments | distributor/provenance graph | domain-loss tests |
| SEM-2 | Generate non-identical replacement | bounded DSL synthesizer | forbidden/candidate digest comparison in certificate |
| SEM-3 | Independently certify behavior | separate checker interpreter | `test_checker_rejects_mutated_candidate` |
| SEC-1 | Contain generated code | import-free Wasmi sandbox with empty linker and fuel | import-denial, fuel-exhaustion, and transition-equivalence tests |
| SEM-4 | Handle modeled state | migration planner | stateful component suite |
| SEC-2 | Refuse insufficient evidence | typed protocol refusals | omission, tamper, domain-forgery, and artifact-presence tests |
| REL-2 | Reproduce claims | canonical output and fixed seed | two-process byte-for-byte replay test |

SEM-4 and the full REL-1 domain-loss campaign remain later-milestone
requirements. M0 and M1 authenticate issuer-to-domain policy but do not claim
hardware or organizational independence.

M1 implements the bounded SEC-1 candidate ABI. General third-party WebAssembly,
process isolation, external state, and the full adversarial campaign remain M2
requirements; SEM-4 remains unimplemented.

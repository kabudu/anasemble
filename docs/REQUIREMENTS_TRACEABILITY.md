# Requirements Traceability

| ID | Requirement | Design | Verification |
|---|---|---|---|
| SEM-1 | Recover after total artifact loss | isolated recovery harness | deletion-attested E2E |
| REL-1 | Use independent semantic fragments | distributor/provenance graph | domain-loss tests |
| SEM-2 | Generate non-identical replacement | bounded DSL synthesizer | candidate digest/source audit |
| SEM-3 | Independently certify behavior | separate checker interpreter | differential and mutation tests |
| SEC-1 | Contain generated code | capability sandbox | escape/resource E2E |
| SEM-4 | Handle modeled state | migration planner | stateful component suite |
| SEC-2 | Refuse insufficient evidence | protocol rules | omission/contradiction suite |
| REL-2 | Reproduce claims | ledger and fixed seeds | clean replay |

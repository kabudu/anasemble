# Risk Register

| ID | Risk | Likelihood | Impact | Mitigation / trigger |
|---|---|---:|---:|---|
| R1 | Contribution collides with synthesis/self-construction work | Medium | High | Refresh claim-level matrix before publication |
| R2 | Oracle/contract incompleteness | High | Critical | Finite DSL, mandatory negatives, refusal |
| R3 | Shared survivor fault | Medium | Critical | Attested failure domains; adversarial test |
| R4 | Trace poisoning/overfitting | High | High | Held-out and metamorphic checks |
| R5 | Synthesizer/checker common bug | Medium | Critical | Separate interpreters and differential testing |
| R6 | Hidden state or side effects | High | Critical | Explicit capabilities; reject unsupported effects |
| R7 | Reconstruction costs exceed backups | High | High | Measure authoring/storage/compute total |
| R8 | Name/package collision | Low | Medium | Repeat registries/domains/trademark search |

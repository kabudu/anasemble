# Threat Model

Assets are contract integrity, survivor independence, service behavior, state,
candidate isolation, checker integrity, and recovery evidence.

Attackers may poison a survivor, forge traces, replay old fragments, falsely claim
failure-domain independence, craft contradictory constraints, exploit parser
differences, trigger search exhaustion, induce a sandbox escape, or smuggle hidden
effects into a candidate.

Excluded initially: compromised checker host, cryptographic breaks, components
outside the DSL, unmodeled external state, and faults exceeding the declared
survivor threshold.

Controls include signed canonical envelopes, domain attestations, explicit
dependencies, separate interpreters, held-out tests, capability isolation,
resource limits, deterministic search, transactional deployment, and fail-closed
certification.

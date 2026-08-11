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

M3's internal review confirms that the host, Rust compiler, dependency supply
chain, Wasmi engine, configured HMAC keys and issuer-domain policy, loss-oracle
scope, and shared checker obligation types remain trusted. The W3C WebAssembly
security model supports import-denied embedding but assigns capability policy to
the embedder and does not eliminate hardware side channels. No external security
review or sandbox penetration test has occurred. See [TCB_LEDGER](TCB_LEDGER.md).

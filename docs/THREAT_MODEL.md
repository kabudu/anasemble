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

P0.1 treats the service manifest as untrusted input. Unknown fields, unsafe identifiers and paths, duplicate routes, undeclared state relationships, invalid schema digests, and zero or excessive resource limits fail before recovery. A manifest digest is bound into the certificate, but the current kernel does not yet prove that a generated FSM implements arbitrary HTTP schemas or effects.

P0.2 treats state source files, manifests, content-addressed objects, destination files, locks, staging files, and rollback files as untrusted local inputs. It rejects symbolic links at each direct file boundary, bounds state at 64 MiB, verifies SHA-256 and length before mutation, serializes store and destination operations, and restores the prior destination after an injected activation failure. Ancestor-directory substitution, a compromised host, storage firmware faults, and malicious processes with equal filesystem authority remain outside this local adapter's guarantees. Production isolation and remote state backends remain unchecked roadmap work.

Controls include signed canonical envelopes, domain attestations, explicit
dependencies, separate interpreters, held-out tests, capability isolation,
resource limits, deterministic search, transactional deployment, and fail-closed
certification.

M3's internal review confirms that the host, Rust compiler, dependency supply
chain, Wasmi engine, configured HMAC keys and issuer-domain policy, loss-oracle
scope, and shared checker obligation types remain trusted. The W3C WebAssembly
security model supports import-denied embedding but assigns capability policy to
the embedder and does not eliminate hardware side channels. No external security
review or sandbox penetration test has occurred. Those are optional post-release assurance and are not represented as completed. See [TCB_LEDGER](TCB_LEDGER.md).

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

P1 assumes stores can be unavailable, stale, corrupted, malicious, or mutually replicated. Store identity, administrative-domain uniqueness, signed generation, authenticated encryption, issuer policy, store quorum, and per-fragment copy quorum are checked independently. HTTPS prevents cleartext remote transport but does not make a store trustworthy. Bounded workers, one-request bundles, timeouts, retry ceilings, body limits, fragment limits, and store limits constrain resource exhaustion.

P3 treats candidate images, commands, health probes, artifact metadata, approvals,
secret references, stale staging resources, and concurrent activation attempts as
untrusted. Immutable digests, exact plan/candidate labels, signature-bound operator
approval, zero egress, capability dropping, read-only filesystems, resource bounds,
exclusive leases, health gates, atomic traffic switching, idempotent reconciliation,
and retained rollback targets constrain them. Docker, Kubernetes, the host kernel,
the OCI registry, admission control, and NetworkPolicy enforcement remain trusted.
A cluster administrator, runtime escape, policy-ignoring CNI, malicious registry,
or application that deliberately exfiltrates a mounted secret through an allowed
effect is outside this boundary.

Issuer signing keys and recovery keys are high-impact secrets. The CLI creates owner-only files, refuses permissive key files, and excludes secret bytes from receipts. Equal-authority host compromise, copied secrets, weak administrator-domain assertions, TLS-root compromise, rollback of the configuration itself, memory disclosure, filesystem snapshots, and secure erasure remain outside P1 guarantees. Revocation rejects all evidence under that key; recovery floors and store generation floors must be advanced after a replay incident.

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

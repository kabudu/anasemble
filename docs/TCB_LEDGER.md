# Trusted Computing Base Ledger

| Element | Owned invariant | Failure consequence | Current evidence | Residual boundary |
|---|---|---|---|---|
| Registry and grammar parser | only bounded, declared FSM work enters recovery | invalid admission or resource exhaustion | strict Serde schemas, grammar and registry bounds | same Rust process as recovery |
| Fragment collector | signatures, issuer-domain binding, freshness, replay, dependencies, and quorum fail closed | poisoned or correlated evidence accepted | hostile M0 and M2 evidence suites | synthetic HMAC keys and configured domains |
| P1 issuer policy | Ed25519 identity, rotation, validity, revocation, replay floors, equivocation, and audit bind every production fragment | compromised or stale issuer evidence accepted | rotation, revocation, replay, equivocation, and audit tests | configured public keys, clock, and domain assertions |
| P1 store coordinator | only authenticated, current, independently configured store quorum reaches collection under fixed work bounds | malicious store accepted or unavailable store stalls recovery | signed bundle, loss, compromised store, generation, quorum, and HTTPS-only tests | configured store keys/domains and no remote HTTPS success fixture |
| P1 evidence cipher and recovery keys | semantic evidence remains confidential and authenticated until explicit materialization and expiry | evidence disclosure, undetected mutation, or unrecoverable evidence | XChaCha tamper, retention, permission, materialization, and deletion tests | host memory, filesystem snapshots, key custody, and secure erasure |
| Loss oracle | registered artifacts are absent from the controlled workspace | experiment accidentally restores rather than reconstructs | fresh-process deletion and bounded digest traversal | no secure erasure or kernel isolation |
| Synthesizer | search is deterministic, bounded, and returns one satisfying candidate | wrong or ambiguous candidate | full-table enumeration and ambiguity/budget tests | exponential and shares host/toolchain |
| Checker | candidate identity and every mandatory obligation are checked independently of synthesis semantics | unsafe certification | private binary candidate parser, mutation and disagreement tests | shared obligation types, process, compiler, and dependencies |
| Wasmi embedder | no imports, bounded module, memory, instances, tables, and fuel | capability access, host compromise, or denial of service | import, memory, infinite-loop, and equivalence tests | engine defects, host compromise, and side channels |
| Evidence ledger | immutable inputs and outcomes are replay-verifiable | lost or rewritten research evidence | content addressing, create-new lock, fsync, atomic rename tests | local filesystem and stale-lock governance |
| Deployment transaction | certified candidate and mapped state activate as one validated bundle; prior bundle remains recoverable | partial activation or state corruption | digest binding, lock, atomic rename, injected failure, rollback tests | one local file, no distributed effects |
| Docker isolation and activation adapter | candidate receives only declared process, filesystem, network, capability, and resource authority; one plan owns activation | host escape, excess authority, split brain, or secret disclosure | direct adversarial sandbox, interruption, conflict, secret, idempotency, and rollback tests | Docker daemon, host kernel, root-equivalent daemon authority, and single host |
| OCI registry adapter | published manifest is immutable and bound to the certified plan and candidate | unrelated or mutable artifact activated | required OCI labels, digest receipt, approval binding, and disposable registry drill | registry availability, storage integrity, authentication, and TLS configuration |
| Kubernetes activation adapter | one approved immutable deployment receives traffic and the prior deployment remains recoverable | split brain, unapproved activation, secret disclosure, or failed rollback | disposable kind Lease, Deployment, Secret-ref, NetworkPolicy, Service switch, same-plan lease takeover, interruption, rollback and commit drill | API server, scheduler, runtime, admission, CNI policy enforcement, and cluster administrators |
| Composite reference receipt | only the exact configuration, plan, artifact, activation and backend state can authorize rollback or acceptance | tampered receipt redirects destructive state operations or crosses an uncertain activation | owner-only bounded file, canonical seal, cross-binding validation, tamper refusal and public rollback/commit drill | trusted local owner and filesystem integrity |
| P4 operations store | job state, attempts, leases, audit history, and results survive interruption without unbounded admission or concurrent mutation | lost work, duplicate work, false status, or resource exhaustion | injected restart, backpressure, audit-chain, sustained queue, and public CLI tests | host filesystem durability, wall-clock input, and one trusted local operator |
| P4 lifecycle adapter | installed executable and manifests are complete, attributable, removable, and never remove operator data | partial install, wrong binary, destructive uninstall, or failed rollback | staged-prefix install, digest verification, modification refusal, and exact removal tests | filesystem administrator, current executable provenance, and platform loader |
| Support-bundle projection | support output contains only allowlisted non-secret operational fields | path, evidence, candidate, credential, or secret disclosure | path and private-message exclusion plus typed closed schema | identifiers and digests may still be sensitive metadata |
| Cargo and Rust supply chain | pinned dependency graph and compiler produce the reviewed behavior | compiler or dependency compromise | `Cargo.lock`, Rust 1.97 pin, offline CI, `cargo audit` | no reproducible-build attestation or third-party provenance review |
| Host and operator | filesystem, clock inputs, keys, and commands reflect the experiment | arbitrary false result | documented procedure and clean-worktree checks | fully trusted local host and operator |

## Internal review conclusion

No known M0 through P3 path converts malformed input, timeout, unsupported work,
checker disagreement, sandbox rejection, ledger failure, or partial deployment
into certification. That conclusion is internal and test-backed, not an external
security assurance. Compromise of the host, compiler, checker dependencies,
configured issuer policy, Wasmi engine, Docker, OCI registry, Kubernetes, local filesystem, or operator clock remains inside the TCB.

The final internal sweep additionally verifies that unsupported remote
PostgreSQL and Redis endpoints fail closed and that S3 uses HTTPS except for the
explicit loopback evaluation fixture. Its result and residual boundaries are in
`FINAL_SECURITY_SWEEP.md`.

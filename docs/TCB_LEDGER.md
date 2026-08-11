# Trusted Computing Base Ledger

| Element | Owned invariant | Failure consequence | Current evidence | Residual boundary |
|---|---|---|---|---|
| Registry and grammar parser | only bounded, declared FSM work enters recovery | invalid admission or resource exhaustion | strict Serde schemas, grammar and registry bounds | same Rust process as recovery |
| Fragment collector | signatures, issuer-domain binding, freshness, replay, dependencies, and quorum fail closed | poisoned or correlated evidence accepted | hostile M0 and M2 evidence suites | synthetic HMAC keys and configured domains |
| Loss oracle | registered artifacts are absent from the controlled workspace | experiment accidentally restores rather than reconstructs | fresh-process deletion and bounded digest traversal | no secure erasure or kernel isolation |
| Synthesizer | search is deterministic, bounded, and returns one satisfying candidate | wrong or ambiguous candidate | full-table enumeration and ambiguity/budget tests | exponential and shares host/toolchain |
| Checker | candidate identity and every mandatory obligation are checked independently of synthesis semantics | unsafe certification | private binary candidate parser, mutation and disagreement tests | shared obligation types, process, compiler, and dependencies |
| Wasmi embedder | no imports, bounded module, memory, instances, tables, and fuel | capability access, host compromise, or denial of service | import, memory, infinite-loop, and equivalence tests | engine defects, host compromise, and side channels |
| Evidence ledger | immutable inputs and outcomes are replay-verifiable | lost or rewritten research evidence | content addressing, create-new lock, fsync, atomic rename tests | local filesystem and stale-lock governance |
| Deployment transaction | certified candidate and mapped state activate as one validated bundle; prior bundle remains recoverable | partial activation or state corruption | digest binding, lock, atomic rename, injected failure, rollback tests | one local file, no distributed effects |
| Cargo and Rust supply chain | pinned dependency graph and compiler produce the reviewed behavior | compiler or dependency compromise | `Cargo.lock`, Rust 1.97 pin, offline CI, `cargo audit` | no reproducible-build attestation or third-party provenance review |
| Host and operator | filesystem, clock inputs, keys, and commands reflect the experiment | arbitrary false result | documented procedure and clean-worktree checks | fully trusted local host and operator |

## Internal review conclusion

No known M0 through M2 path converts malformed input, timeout, unsupported work,
checker disagreement, sandbox rejection, ledger failure, or partial deployment
into certification. That conclusion is internal and test-backed, not an external
security assurance. Compromise of the host, compiler, checker dependencies,
configured issuer policy, or Wasmi engine remains inside the TCB.

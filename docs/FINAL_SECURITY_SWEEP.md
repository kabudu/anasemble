# Final Internal Security Sweep

## Scope and method

The 2026-08-11 Lazarus sweep covers the complete merged Rust implementation,
public CLI, state and activation adapters, file lifecycle, tests, scripts,
dependency graph, compatibility claims and operator documentation. It inventories
trust boundaries, destructive operations, secrets, remote transports, process
execution, parsing, replay, resource exhaustion, concurrency, commit ambiguity,
rollback ordering, cleanup and supply-chain exposure.

The sweep includes source review; searches for unsafe Rust, panic paths, shell
execution, credential material, insecure URL handling, unbounded files and
processes, symlink traversal and destructive filesystem operations; Rustfmt and
Clippy with warnings denied; RustSec and Cargo Deny advisory, ban and source
checks; adversarial unit and destructive integration tests; the authoritative
private local CI; and the clean-clone Linux matrix. The project contains no owned
unsafe Rust block and invokes Docker, kubectl and Git through argument arrays,
not a command shell.

## Material findings resolved

1. The composite recovery receipt was owner-only but was not sealed or fully
   cross-validated before rollback. It now carries a canonical digest and binds
   the configuration, activation plan, immutable artifact, activation result and
   every backend receipt. A mutated receipt refuses before external mutation.
2. A Kubernetes Service update can succeed even when the client observes an
   error. The activation adapter now confirms the selector when possible and
   returns an explicit uncertain-external-state error when it cannot. The
   reference workflow never rolls restored state back across that uncertainty.
   Lease cleanup occurs after the Service commit point and cannot invert a
   successful activation into backend rollback.
3. PostgreSQL and Redis were documented as loopback-only but constructors did
   not enforce that boundary. They now reject remote hosts and PostgreSQL host
   overrides. S3 now accepts only HTTPS or explicit loopback HTTP evaluation.
4. Successful integrated recovery retained rollback resources indefinitely and
   could block later work. The public acceptance command verifies all active
   state before deleting any rollback resource, then commits every backend and
   Kubernetes. Tampering, drift and unavailable verification fail closed.

## Result and residual trust

No unresolved critical or high internal finding remains after the final gates.
Cargo Deny reports warning-level duplicate transitive versions but no advisory,
ban or untrusted source failure. The host, compiler, locked dependencies, local
filesystem, operator and clock, Wasmi, Docker daemon and kernel, registry,
Kubernetes control plane and enforcing CNI remain trusted as recorded in
`TCB_LEDGER.md`. This internal result is not an external penetration test,
independent reproduction, formal proof, secure-erasure claim or native x86_64
attestation.

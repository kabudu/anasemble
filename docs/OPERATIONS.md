# Operations

The harness is local, deterministic, and offline by default. Every run records
commit, dependency locks, DSL/checker versions, fragment digests, deletion
attestation, search bounds, candidate digest, certificate, and outcome.

Alerts cover insufficient survivor quorum, equivocation, schema mismatch, search
budget exhaustion, sandbox violation, checker differential, state migration
failure, ledger failure, and deployment rollback.

Recovery evidence may contain sensitive protocol values; fixtures must be
synthetic until retention, redaction, and access policies exist. Test signing keys
must not be reused outside research.

M1 ledger publication uses an exclusive content-addressed lock and atomic rename.
A pre-existing lock fails closed; automated stale-lock removal is intentionally
absent because process liveness and ownership are not yet attestable. Operators
must preserve a stuck lock for incident analysis rather than deleting it blindly.

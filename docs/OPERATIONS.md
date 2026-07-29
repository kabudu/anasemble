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

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

M2 deployment accepts only a certified result and a bounded explicit state map.
It takes a create-new `.deploy.lock`, writes `rollback.json` before atomically
replacing `active.json`; either image
is capped at 1 MiB and symlinks are rejected. A stale `active.tmp` or
`rollback.tmp` fails closed and requires incident review. `rollback` restores the
last complete image. This transaction covers one local file, not multiple hosts,
databases, queues, or external side effects.

`evaluate-campaign <root>` reads at most 256 safe single-component workspaces.
Exit code 0 means every retained expectation matched and unsafe certifications
were zero; exit code 2 means the campaign completed with a mismatch.

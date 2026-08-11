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

For reproduction, preserve the exact commit, full local-CI output, platform,
toolchain, dependency acquisition, exit codes, refusals, timeouts, disagreements,
and warnings. Do not normalize timing or discard negative rows. A mismatch opens
an incident against the experiment and blocks tags or stronger claims until it is
classified. The procedure and attestation fields are in
`INDEPENDENT_REPRODUCTION.md`.

## P0 production foundations

`validate-service <manifest>` validates a bounded `service-v1` HTTP contract and prints its canonical digest. When the same manifest is embedded in `registry.json`, recovery checks the component and interface identity and binds that digest into the certificate. Validation does not claim that the FSM kernel can yet implement arbitrary HTTP bodies or declared effects.

`snapshot-state <store> <source> <component> <schema> <revision>` captures one regular file up to 64 MiB as an immutable SHA-256-addressed object and atomically advances `current.json`. `restore-state <store> <destination>` verifies that object, stages and syncs it beside the destination, preserves an existing destination as `<name>.anasemble-rollback`, records an activation digest, and atomically activates the recovered bytes. Run `rollback-state <destination>` to restore the prior bytes or `commit-state <destination>` to re-verify and accept the recovered bytes before removing rollback.

Store and destination locks use exclusive create. Corruption, symbolic links at direct boundaries, stale locks, staging files, or rollback files fail closed and require operator inspection. Do not delete a lock until process ownership is established. A successful state restore covers one local regular file only; it is not a database, object-store, queue, distributed-transaction, or crash-consistent multi-file guarantee.

## P1 identity and evidence operations

Create Ed25519 and recovery keys with `create-signing-key <path> <key-id> <created-at>` and `create-recovery-key <path> <key-id> <created-at>`. Back up recovery keys separately from evidence stores, restrict access to the recovery role, test restoration before relying on them, and retain an old recovery key only until every required sealed record has expired or been re-encrypted. Losing the last mapped recovery key makes the affected evidence unrecoverable.

Use `sign-fragment <input> <signing-key> <output>` before `seal-evidence <signed-fragment> <recovery-key> <created-at> <delete-after> <output>`. Assemble sealed records into a `fragment-store-v1` bundle and run `sign-store-bundle <input> <store-signing-key> <output>`. Store signing keys and issuer signing keys should be administered separately.

Use `retrieve-evidence <config> <output-directory>` to enforce store and fragment quorum and create a verified temporary fragment set. Preserve `receipt.json` with the recovery audit evidence, but treat the other files as sensitive plaintext. Run `delete-evidence <output-directory>` immediately after use. To delete one local encrypted store generation, run `delete-store-bundle <bundle.json>` and retain the returned digest in the operator audit record. These commands do not securely erase lower filesystem or provider layers.

A remote store URL must use HTTPS. Bearer credentials are named by environment-variable reference, never embedded in the store configuration. A failed or invalid store is recorded by ID; recovery proceeds only if authenticated quorum remains. Stale generations, invalid signatures, expired seals, unavailable recovery keys, and revoked or replayed issuer keys fail closed.

## P2 stateful recovery operations

P2 adapters are Rust library interfaces and are not yet exposed as unattended production commands. Snapshot writers must be quiesced for the full snapshot and activation window. PostgreSQL refuses schema changes across capture, S3 refuses unequal two-pass reads, and Redis refuses unequal reads or any pending consumer delivery. Never bypass these refusals.

Preserve staging and rollback resources until activation is accepted. PostgreSQL uses `<target>_anasemble_stage` and `<target>_anasemble_rollback` schemas. Redis uses corresponding staging and rollback keys. S3 rollback objects live under a digest-addressed top-level `anasemble-rollback/` prefix that is deliberately outside the target prefix. A stale resource requires operator investigation; do not delete it automatically.

No transaction spans all backends. If activation fails, invoke rollback for every successful receipt in reverse activation order and retain errors and backend state for incident handling. Local PostgreSQL and Redis transports are trusted-loopback profiles only. Remote production profiles require authenticated TLS and credential references in P4.

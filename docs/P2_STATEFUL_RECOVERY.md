# P2 Stateful Recovery

P2 implements bounded state recovery for one supported relational, object, and durable-queue combination. It does not claim arbitrary PostgreSQL types, cross-backend atomicity, or recovery of in-flight queue deliveries.

## Contract and bounds

`TransactionalStateAdapter` requires schema discovery, snapshot, migration planning, restore, verification, and rollback. Every snapshot binds canonical schema and data digests. The common hard limits are 10,000 items and 64 MiB. Identifiers, prefixes, backend responses, stale staging resources, integrity mismatches, and unsupported types fail closed.

The PostgreSQL adapter supports tables composed of `bigint`, `integer`, `text`, `boolean`, and `bytea`, with primary-key, unique, and foreign-key constraints. It captures binary `COPY` data in a repeatable-read, read-only transaction, rejects concurrent schema migration, reconstructs from retained schema evidence after the source schema is destroyed, validates constraints, and swaps schemas transactionally. The previous schema is retained for rollback.

The S3-compatible adapter records a sorted key, ETag, and byte snapshot from two identical passes. A mutation between passes is refused. Restore retains previous objects in a disjoint rollback prefix, replaces the exact target key set, rereads all bytes, and can copy the prior set back. S3 has no atomic multi-object rename, so writers must remain quiesced through snapshot and activation.

The durable queue adapter supports Redis Streams. It retains ordered entry IDs, binary fields, consumer-group names, and last-delivered IDs. Any pending consumer delivery is refused because recreating ownership and delivery counters would be unsound. Two identical reads are required. Restore builds a staging stream and atomically renames keys, retaining the prior stream for rollback.

`activation-plan-v1` binds the certified candidate, recovery certificate, certificate-bound HTTP service manifest, state schemas, snapshots, and migrations. Duplicate backend resources and mismatched service digests are rejected.

## Trust and failure boundaries

Backend credentials and TLS termination remain operator responsibilities. PostgreSQL currently uses `NoTls`, so version 0.0.1 supports it only across a trusted local transport. The MinIO and Redis drills use isolated loopback transports, and Redis remains local-only. Production remote object endpoints must use HTTPS. A later compatibility revision, not P4 completion itself, is required before remote PostgreSQL or Redis can become supported.

There is no distributed transaction across PostgreSQL, S3, and Redis. P3 must orchestrate their prepared receipts and rollbacks around runtime activation. Verification failure triggers rollback where a prior target exists; a rollback failure is surfaced for operator action and retained backend state must not be deleted.

## Evidence

`tests/p2_stateful.rs` starts dedicated disposable PostgreSQL 18, MinIO, and Redis 8 containers, reconstructs a certified HTTP service, destroys the PostgreSQL source schema, restores all three state classes, verifies relational constraints, object bytes, ordered queue entries and group cursor, performs rollback, rejects pending queue work, and binds all evidence into one activation plan. The containers have unique `anasemble-p2-*` names and the test removes only those names.

The authoritative command is `./scripts/ci-local.sh`. Docker and the pinned local images are explicit prerequisites. Hosted CI remains prohibited while the repository is private.

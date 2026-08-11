# Compatibility Contract

## Supported platforms

Anasemble `0.0.1` supports the Rust 1.97.0 control plane on arm64 macOS and arm64 Linux. The macOS profile supports local operations, reconstruction, evidence, filesystem state, Docker, and Kubernetes control. PostgreSQL 18, S3-compatible HTTPS object storage, Redis 8 Streams, OCI Distribution v2, Docker Engine 29, Kubernetes 1.36, `kubectl` 1.36, and kind 0.32 are the verified adapter versions. Other versions are unsupported until the local matrix is rerun.

The machine-readable manifest installed at `share/compatibility-v1.json` is authoritative for protocol and adapter identifiers. Support requires the exact pinned Rust dependency graph in `Cargo.lock`.

## Protocol and configuration compatibility

`fsm-v1`, `service-v1`, `activation-plan-v1`, `operations-config-v1`, `recovery-job-v1`, and `support-bundle-v1` reject unknown fields. Legacy `operations-config-v0` is accepted only by `migrate-operations-config`, which maps all five fields without inference and writes a new file exclusively. No in-place configuration mutation occurs.

Upgrades must preserve the operations root, create a separate migrated configuration, validate it with `init-operations` in a disposable root, stop job runners, install to a new prefix, execute the disaster drill, and then move the operator-controlled executable reference. Downgrade is replacement of that reference with the preserved prior prefix. Job records are not downgraded or rewritten.

## Backup interoperability

Ordinary backups remain mandatory. Anasemble does not replace backup, replication, database point-in-time recovery, object versioning, or queue durability. Snapshot and evidence exports must be captured through the backend contracts before catastrophe, stored independently of deployable artifacts, and tested alongside native restore. If a native backup survives and satisfies the recovery objective, operators should prefer it. Anasemble is the bounded path for the declared total-artifact-loss case.

PostgreSQL and Redis remote transports are not supported in this version. S3-compatible production endpoints require HTTPS. Kubernetes requires a CNI that enforces NetworkPolicy. Unsupported architecture, protocol, backend version, transport, CNI, or configuration version must be treated as a refusal, not presumed compatible.

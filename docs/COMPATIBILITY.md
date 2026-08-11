# Compatibility Contract

## Status contract

Compatibility is recorded across three independent dimensions. `Implemented`
means the bounded code path exists; `Partial` means only part of the named
profile exists; `Not implemented` means it does not exist. `Tested` requires
retained evidence for the exact profile; `Partially tested` covers only the
named boundaries; `Untested` has no retained execution evidence. `Supported`
means maintainers accept defects for that exact profile; `Experimental` permits
evaluation without that commitment; `Unsupported` must be refused.

An entry is not supported merely because its components appear in separate
rows. The matrix does not imply a Cartesian product. Evidence paths name the
authoritative repository drills and documentation for each tested claim.

## Compatibility matrix

| Profile | Platform and transport | Components | Implementation | Validation | Support | Retained evidence and limits |
| --- | --- | --- | --- | --- | --- | --- |
| macOS arm64 control plane | macOS, aarch64, local filesystem | Rust 1.97.0 control plane, `filesystem-v1`, operations lifecycle | Implemented | Tested | Supported | `scripts/ci-local.sh`; `tests/p4_product_readiness.rs`; exact locked dependency graph required |
| macOS arm64 local state | macOS, aarch64, trusted loopback | PostgreSQL 18, MinIO S3 API, Redis Streams 8.8 | Implemented | Tested | Supported | `tests/p2_stateful.rs`; writers must be quiesced; no cross-backend transaction |
| macOS arm64 Docker activation | macOS, aarch64, local Docker daemon | OCI Distribution v2, Docker Engine 29 | Implemented | Tested | Supported | `tests/p3_activation.rs`; single host; Docker daemon and kernel are trusted |
| macOS arm64 kind activation | macOS, aarch64, local kind cluster | Kubernetes 1.36, kubectl 1.36, kind 0.32 | Implemented | Partially tested | Experimental | Control objects, leases, switching, and rollback are tested; NetworkPolicy enforcement is not |
| Linux arm64 control plane | Linux GNU, aarch64, clean local container | Rust 1.97.0 control plane | Implemented | Tested | Experimental | `scripts/ci-linux-matrix.sh`; native host ISA, clean clone, offline build/test/doc/repository gate; production distributions and kernels remain unverified |
| Linux x86_64 control plane | Linux GNU, x86_64, emulated local container | Rust 1.97.0 control plane | Implemented | Partially tested | Experimental | `scripts/ci-linux-matrix.sh`; clean clone and offline build/test/doc/repository gate under amd64 emulation; native x86_64 hardware remains unverified |
| AWS AL2023 arm64 control plane | Amazon Linux 2023, native aarch64 EC2 | Rust 1.97.0 control plane | Implemented | Tested | Supported | `docs/AWS_COMPATIBILITY.md`; AMI `ami-053d8df569ac57bbb`, `t4g.medium`, kernel `6.1.177-224.371.amzn2023.aarch64`; no other AMI, kernel or family implied |
| AWS AL2023 x86_64 control plane | Amazon Linux 2023, native x86_64 EC2 | Rust 1.97.0 control plane | Implemented | Tested | Supported | `docs/AWS_COMPATIBILITY.md`; AMI `ami-062a8901a5ddcf280`, `t3.medium`, kernel `6.1.177-224.371.amzn2023.x86_64`; no other AMI, kernel or family implied |
| AWS managed state in `eu-west-1` | Private provider endpoints, arm64 AL2023 runner | RDS PostgreSQL 18.3, S3, ElastiCache Redis 7.1 | Implemented | Tested | Supported | `tests/aws_compatibility.rs`; certificate-verified PostgreSQL TLS, authenticated Redis TLS, temporary-role S3 HTTPS; writers quiesced and no cross-backend transaction |
| Amazon EKS strict-policy activation | EKS 1.36/`eks.9`, one AL2023 arm64 `t4g.medium` node | VPC CNI `v1.22.4-eksbuild.3`, immutable deployment, approval, switch, rollback | Implemented | Tested | Supported | `tests/aws_compatibility.rs`; strict zero-egress probe denied; EKS, VPC CNI, admission, IAM and administrators remain trusted |
| Generic S3-compatible HTTPS | Provider-managed HTTPS | S3-compatible object adapter | Implemented | Partially tested | Experimental | MinIO exercises the S3 API; named provider behavior remains unverified |
| Other production Kubernetes with enforcing CNI | Linux, provider-dependent Kubernetes API | Kubernetes 1.36 and NetworkPolicy-enforcing CNI | Implemented | Partially tested | Experimental | The exact EKS/VPC CNI row is validated; no other provider or CNI is implied |
| Integrated recovery through activation | macOS, aarch64, mixed local transports | Reconstruction, PostgreSQL 18, MinIO S3 API, Redis Streams 8.8, OCI Distribution v2, Kubernetes 1.36, public reference CLI | Implemented | Tested | Experimental | `tests/reference_workflow.rs`; restores after deliberate source deletion, activates, rolls back, and separately accepts while retiring rollback resources; candidate artifact is not a generated HTTP server and kind does not prove NetworkPolicy enforcement |
| Other remote PostgreSQL or Redis | Provider-managed remote network | PostgreSQL or Redis Streams | Implemented | Partially tested | Experimental | Fail-closed authenticated TLS boundaries exist, but only the exact AWS managed-state combination above is supported |

The installed `share/compatibility-v2.json` manifest carries the same status
definitions, exact profiles, evidence paths, and limitations. Version 2 replaces
the earlier list-shaped manifest because independent status and combination
semantics are a schema change. Support also requires the exact dependency graph
in `Cargo.lock` and the digest-pinned test images in `scripts/ci-local.sh`.
The Linux execution record and its native-versus-emulated boundary are retained
in `docs/LINUX_MATRIX.md`; an emulated execution is never treated as native
hardware evidence.

## Verified local fixture identities

The authoritative macOS arm64 drill uses PostgreSQL 18, Redis 8.8.0, OCI
Distribution v2, Docker Engine 29, Kubernetes 1.36, kubectl 1.36, and kind 0.32.
PostgreSQL, MinIO, Redis, registry, client, Debian, and kind node images are
selected by immutable digest in local CI and the destructive tests. A tag or
newer compatible-looking version is not automatically supported.

## Protocol and configuration compatibility

`fsm-v1`, `service-v1`, `activation-plan-v1`, `operations-config-v1`,
`recovery-job-v1`, and `support-bundle-v1` reject unknown fields. Legacy
`operations-config-v0` is accepted only by `migrate-operations-config`, which
maps all five fields without inference and writes a new file exclusively. No
in-place configuration mutation occurs.

Upgrades must preserve the operations root, create a separate migrated
configuration, validate it with `init-operations` in a disposable root, stop job
runners, install to a new prefix, execute the disaster drill, and then move the
operator-controlled executable reference. Downgrade replaces that reference
with the preserved prior prefix. Job records are not downgraded or rewritten.

## Backup interoperability

Ordinary backups remain mandatory. Anasemble does not replace backup,
replication, database point-in-time recovery, object versioning, or queue
durability. Snapshot and evidence exports must be captured through the backend
contracts before catastrophe, stored independently of deployable artifacts, and
tested alongside native restore. If a native backup survives and satisfies the
recovery objective, operators should prefer it. Anasemble is the bounded path
for the declared total-artifact-loss case.

Remote PostgreSQL and Redis require authenticated TLS and DNS identity;
S3-compatible production endpoints require HTTPS. The exact AWS managed-state
and EKS profiles above are supported. Other providers, versions, architectures,
instance families, certificate policies, authentication modes and CNIs remain
experimental until their own retained drills pass. An unlisted combination must
never be inferred from independently supported rows.

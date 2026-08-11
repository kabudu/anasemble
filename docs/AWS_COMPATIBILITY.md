# AWS compatibility evidence

## Scope and cost boundary

The P5 matrix is restricted to tagged, ephemeral resources in `eu-west-1`.
Every run uses a unique `RunId`, `Project=anasemble`,
`Purpose=compatibility`, and an absolute `ExpiresAt`. The account monthly budget
is USD 10; the stricter incremental ceiling for this matrix is USD 2. The run
must stop before provisioning another fixture if elapsed-resource estimates can
cross that ceiling.

The intended exact profiles are Amazon Linux 2023 on native arm64 and x86_64,
Amazon RDS for PostgreSQL with certificate and hostname verification,
ElastiCache for Redis with authentication and in-transit encryption, Amazon S3
over HTTPS, and Amazon EKS with the VPC CNI network-policy engine in strict mode.
Evidence for one profile never promotes another provider, version, architecture,
distribution, CNI, or Cartesian combination.

## Security and cleanup contract

Fixtures have no public database or cache endpoint. Native validation instances
have no inbound security-group rules and require IMDSv2. Source bundles are held
in a private, encrypted S3 bucket and downloaded through a two-hour presigned
URL or a run-scoped instance role. Credentials are generated per run, kept out
of repository evidence, and deleted with the fixture.

Completion requires deletion of every run-owned instance, volume, snapshot,
database, cache, cluster, node group, load balancer, elastic IP, security group,
IAM role and policy, instance profile, parameter, log group, bucket and object.
Deletion is followed by a tag-based residual scan. A run with any residual
resource is incomplete regardless of test outcome.

## Retained result: 2026-08-11

The successful native run `anasemble-aws-20260811162029-2a75fc06` exercised the
same locked tree on two real EC2 architectures. Both ran formatting, Clippy,
bounded tests, documentation, the repository policy checker, and an optimised
release build with Rust 1.97.0:

| Architecture | Exact profile | Retained final marker |
| --- | --- | --- |
| arm64 | AL2023 AMI `ami-053d8df569ac57bbb`, `t4g.medium`, kernel `6.1.177-224.371.amzn2023.aarch64` | `ANASEMBLE_RESULT status=pass arch=arm64 uname=aarch64` at 16:33 UTC |
| x86_64 | AL2023 AMI `ami-062a8901a5ddcf280`, `t3.medium`, kernel `6.1.177-224.371.amzn2023.x86_64` | `ANASEMBLE_RESULT status=pass arch=x86_64 uname=x86_64` at 16:35 UTC |

The managed-state run `anasemble-p5-20260811154128-8ef49a` used a private,
encrypted, Single-AZ RDS PostgreSQL 18.3 `db.t4g.micro` with
`rds-ca-rsa2048-g1`, encrypted and AUTH-enabled ElastiCache Redis 7.1.0 on one
`cache.t4g.micro`, Amazon S3 over HTTPS, and an IMDSv2-required AL2023 arm64
`t4g.medium` runner. The final provider test passed in 1.22 seconds after exact
snapshot, restore, verification and rollback for all three backends. RDS and
ElastiCache had no public endpoint; only the runner security group could reach
their state ports. S3 used the runner's temporary role credentials and session
token. The earlier test-only crypto-provider ambiguity was fixed by explicitly
selecting ring before any Redis TLS client was constructed.

The orchestration run `anasemble-eks-20260811161512-2c158e` used EKS 1.36 on
platform `eks.9`, VPC CNI `v1.22.4-eksbuild.3` with
`enableNetworkPolicy=true` and `NETWORK_POLICY_ENFORCING_MODE=strict`, and one
AL2023 arm64 `t4g.medium` node with kubelet `v1.36.2-eks-254016e` and kernel
`6.18.38-76.139.amzn2023.aarch64`. The ignored provider drill passed signed
approval, secret-reference mounting, health gating, immutable deployment,
Service switching, an outbound TCP denial from the active pod, and rollback to
the prior selector in 78.86 seconds.

## Cost and teardown result

Cost Explorer showed USD 0.0631726408 for the month before the runs but had not
yet ingested same-hour usage at teardown. The run inventory stayed below the USD
2 ceiling: short-lived EC2 runners, one RDS micro instance for less than an hour,
one billable ElastiCache micro node-hour, and one EKS control plane plus one node
for less than an hour. No NAT gateway, load balancer, elastic IP, snapshot,
Multi-AZ database, or retained log service was created.

Every successful and failed attempt was explicitly torn down. Final
service-specific scans returned no live EC2 instance or volume, RDS database,
ElastiCache group, EKS cluster or node group, network interface, security group,
subnet group, SSM parameter, IAM role or instance profile, S3 bucket or object.
The EKS run's tag-index scan was empty. EC2's eventually consistent tag index
temporarily returned only already-terminated instance and deleted-volume
tombstones for the native and managed runs; direct EC2 scans returned empty.
There are no billable or operator-recoverable run resources left. Secrets,
account identifiers, private endpoints, presigned URLs and credential material
are not retained.

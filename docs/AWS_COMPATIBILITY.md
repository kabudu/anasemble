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

## Retained result

The exact resource versions, native architecture and kernel output, test
outcomes, elapsed time, bounded cost and zero-resource teardown result are
recorded here only after the corresponding drill and cleanup complete. Secrets,
account identifiers, private endpoints, presigned URLs and credential material
must never be retained.

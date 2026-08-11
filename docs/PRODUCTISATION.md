# Productisation Completion

## Status

Anasemble is productised at the private implementation boundary for the exact
profiles in `COMPATIBILITY.md`. Supported profiles have operator lifecycle,
bounded failure behavior, retained executable evidence, installation and removal,
configuration migration, diagnostics, compatibility policy and disaster
procedures. The integrated reference profile is implemented and tested but
remains experimental because the candidate is not a generated HTTP server and
the kind fixture does not prove production CNI enforcement.

## Operator lifecycle

The product lifecycle is preparation, declared loss, certified reconstruction,
backend restoration, immutable OCI publication, approved Kubernetes activation,
observation, and an explicit choice between rollback and acceptance. Rollback
restores Kubernetes before state. Acceptance verifies all active state before it
retires rollback resources. Composite receipts are owner-only, bounded, sealed
and cross-validated before destructive use.

Operations jobs are durable and bounded; queue admission, attempts, leases,
audit history, results and support output have explicit ceilings. Installation is
to a new exact prefix, configuration migration is out of place, uninstallation
verifies digests and never removes the operations root, and unsupported platform,
transport, backend and orchestrator combinations fail closed or remain clearly
experimental.

## Separation from release preparation

Productisation completion does not choose a public version or licence, perform
legal/name clearance, render public release notes, make the repository public,
enable hosted CI, publish packages or images, create a tag or GitHub Release, or
authorize production deployment. Those activities begin only when the user
separately authorizes release preparation.

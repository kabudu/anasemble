# Independent Reproduction Packet

## Status

The packet is complete, but no independent party has attested a result. Local
reruns by the project author or implementation agent are reproducibility checks,
not independent reproduction.

## Reviewer boundary

The reviewer must not reuse an existing build directory, Cargo cache supplied by
the project author, generated fixture workspace, unpublished patch, or verbal
interpretation of expected outcomes. The reviewer may use the public Rust toolchain
distribution and crates.io dependency sources identified by `Cargo.lock`.

## Procedure

1. Receive read-only access to the private repository and record the repository
   URL, commit SHA, operating system, architecture, Rust version, and UTC time.
2. Clone into a new directory and verify `git status --short` is empty.
3. Inspect `AGENTS.md`, `rust-toolchain.toml`, `Cargo.lock`,
   `docs/QUICKSTART.md`, `docs/COMPATIBILITY.md`, and the M0 through P4 evidence
   documents before executing code.
4. Bootstrap dependencies once with `cargo fetch --locked` on an unprivileged
   machine or disposable VM with no project secrets.
5. Disconnect network access if practical, then run exactly
   `./scripts/ci-local.sh`.
6. Run `cargo test --release --all-targets --all-features --locked --offline`.
7. Run `cargo audit --file Cargo.lock` with a freshly updated advisory database
   and record the advisory database commit or timestamp.
8. Confirm the test inventory includes the public fresh-process loss workflow,
   two-component corpus workflow, campaign workflow, state deployment and
   rollback, hostile evidence, checker disagreement, import denial, memory bound,
   fuel exhaustion, operations lifecycle, and integrated reference workflow.
9. Compare `experiments/m2-results.json`, `experiments/m3-comparison.json`, and
   `experiments/m3-costs.json` with the observed result categories. Timing may
   differ and must be reported rather than normalized away.
10. Preserve the full stdout, stderr, exit codes, environment description, and
    any crash, refusal, timeout, warning, or disagreement.
11. On Linux with Docker available, run `./scripts/ci-linux-matrix.sh` and state
    whether each architecture ran on native hardware or through emulation. Do not
    convert emulated evidence into a native-platform attestation.

The integrated reference drill requires destructive disposable PostgreSQL, S3,
Redis, registry and Kubernetes fixtures. A reviewer unable to provide them must
report that portion as not executed, not passed. The packet is an optional
post-release assurance activity and does not authorize publication or deployment.

## Required attestation

The reviewer should publish or return a signed statement containing:

- reviewer identity and independence relationship;
- repository and exact commit;
- environment and dependency acquisition method;
- commands and exit codes;
- passed, failed, refused, timeout, and disagreement counts;
- deviations from this packet;
- security or soundness findings;
- whether the claim was reproduced, contradicted, or remains inconclusive.

An attestation is invalid if it reports only “tests passed,” omits the exact
commit, suppresses negative rows, or changes the loss scope or baselines.

## Failure handling

Any mismatch is retained as research evidence. Do not update expected files to
fit an unexplained result. First classify environment, nondeterminism, dependency,
oracle, implementation, and claim errors. A candidate or checker disagreement is
a stop-ship event; an unavailable platform is an inconclusive reproduction, not a
pass.

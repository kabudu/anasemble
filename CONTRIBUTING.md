# Contributing to Anasemble

Thank you for helping improve Anasemble. The project welcomes focused bug
reports, documentation corrections, compatibility evidence, security hardening
and narrowly scoped implementation changes.

## Before proposing a change

Read `README.md`, `AGENTS.md`, `docs/COMPATIBILITY.md`, `docs/THREAT_MODEL.md` and
the relevant design document. Open an issue before undertaking a large change,
new backend, protocol revision or trust-boundary expansion. Do not submit real
customer data, credentials, proprietary recovery evidence or undisclosed
vulnerability details.

## Development

Use Rust 1.97.0 through the pinned `rust-toolchain.toml`. Fork the repository,
create a focused branch, keep commits reviewable and add behavioral tests for
changed behavior. Rust remains the implementation language unless an approved ADR
and maintainer decision establish a concrete necessity.

The authoritative pre-submission command is:

```sh
./scripts/ci-local.sh
```

The full gate requires the documented Docker, kind and kubectl versions and runs
destructive disposable-container tests. Never point tests at production systems.
For documentation-only changes, contributors may initially run formatting,
`cargo run --locked --offline --bin repo_check`, brand checks and `git diff
--check`, but maintainers run the complete gate before merge.

## Pull requests

Explain the requirement, user impact, trust or compatibility change, tests and
remaining limitations. Update documentation, traceability and changelog entries
with implementation changes. Keep unsupported combinations explicit. A pull
request certifies that you have the right to contribute its contents under
Apache License 2.0 and agree that intentional contributions are provided under
that licence, as described in section 5.

Maintainers may close changes that weaken refusal boundaries, add unbounded work,
overstate evidence, introduce hosted services without approval or cannot be
maintained safely.

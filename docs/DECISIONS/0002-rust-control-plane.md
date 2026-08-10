# ADR 0002: Use Rust throughout the Anasemble system

**Status:** Accepted, 2026-08-10

## Context

Anasemble's trusted control plane parses hostile evidence, enforces resource
bounds, performs deterministic search, certifies generated components, and may
eventually coordinate sandboxed WebAssembly execution and transactional
deployment. Memory safety, explicit error handling, predictable performance, and
a small runtime footprint are correctness concerns at this boundary.

## Decision

Rust is the required implementation language for the entire project. The
repository pins the stable Rust toolchain and commits `Cargo.lock`. Core logic,
CLI tools, test harnesses, experiment orchestration, protocol implementations,
and future control-plane services must use Rust.

A different technology is allowed only where it is absolutely necessary and
Rust is not a reasonable solution. A project website is the expected example.
Every exception requires explicit user approval and a separate ADR defining why
Rust is insufficient, the minimal exception boundary, data and trust crossings,
build and supply-chain ownership, and removal or migration options.

## Alternatives

Python would shorten an early prototype but adds a dynamic runtime and weaker
compile-time guarantees at the hostile-input boundary. Go offers a simpler
runtime model but does not provide Rust's ownership-based memory and concurrency
safety. A mixed-language control plane increases supply-chain, interoperability,
and review burden without an M0 requirement.

## Consequences

Contributors need the pinned Rust toolchain. Local CI runs formatting, Clippy,
tests, documentation, metadata validation, and bootstrap checks. WebAssembly is
the preferred future boundary for reconstructed components, but M0 does not yet
execute generated code or claim sandbox isolation.

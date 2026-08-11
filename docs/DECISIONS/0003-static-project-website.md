# ADR 0003: Static project website exception

## Status

Accepted for the public source opening on 2026-08-11.

## Context

Anasemble is implemented in Rust by policy. The public project also needs a
small GitHub Pages entry point that explains the product, supported profiles,
evaluation path, and claim boundaries without adding an application runtime or
another package ecosystem.

HTML and CSS are browser platform formats rather than a replacement control
plane. TailwindCSS is used only as a pinned build-time CSS compiler. Implementing
their presentation layer in Rust would add machinery without improving
correctness, security, or operator value.

## Decision

The project website may use static HTML, TailwindCSS, and generated CSS. This is
the user-approved exception to the Rust-only implementation policy. It must:

- remain under `website/` and outside the Rust control-plane trust boundary;
- ship no browser JavaScript, analytics, cookies, forms, external fonts, remote
  resources, or executable third-party content;
- reuse repository-owned Semantic Fit assets and claim language;
- link authoritative compatibility and security details back to versioned
  repository documents;
- build through a bounded repository script into an ignored output directory;
- deploy only through the separately approved GitHub Pages workflow after the
  repository is public.

The pinned npm dependency graph is confined to CSS compilation and checked in as
a lockfile. GitHub workflow YAML and the minimal shell required to assemble a
static Pages artifact are delivery configuration, not product implementation.
Any future browser application logic, hosted service, telemetry, or data
collection requires a new ADR and explicit owner approval.

## Consequences

The deployed site works without JavaScript and has no user-data trust boundary.
Its build adds a bounded Node package supply chain that is isolated from Rust
product compilation and runtime. It does not provide hosted recovery, documentation versioning,
search, telemetry, or an interactive product demonstration. Those omissions are
intentional until concrete requirements justify additional machinery.

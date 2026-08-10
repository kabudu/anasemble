# M0 Executable Research Contract

## Scope

M0 supports deterministic Mealy-style finite-state request/response components.
Inputs, outputs, and states are finite string enumerations. Every pair of current
state and input has exactly one next state and observable output. The initial
state is declared. Networking, clocks, randomness, native code, reflection,
unbounded data, hidden effects, and external state are outside the grammar.

The M0 implementation is a narrow executable vertical, not the general M1
reconstruction loop. It reconstructs a total transition table from exhaustive
transition obligations using bounded deterministic enumeration.

## Observable semantics

For a request in state `s` with input `i`, lookup of `(s, i)` atomically returns
the declared output and replaces `s` with the declared next state. The observable
behavior is the ordered output sequence for an input sequence. Termination is
guaranteed because one table lookup handles each request.

The grammar rejects unknown or duplicate values, undeclared states, inputs or
outputs, partial tables, and search budgets outside `1..1000000`.

M0 also caps a run at 10,000 fragments, 1 MiB per fragment, 100,000 workspace
entries, and 1 GiB of scanned workspace data. A registry may select lower bounds
but cannot raise these compiled safety maxima.

## Fragment envelope

Every fragment has exactly these fields: `kind`, `component`,
`interface_version`, `issuer`, `failure_domain`, `issued_at`, `sequence`,
`content_digest`, `dependencies`, `content`, and `signature`. Canonical JSON uses
sorted keys, UTF-8, ASCII escapes, and no insignificant whitespace. Digests use
SHA-256. M0 signatures use HMAC-SHA-256 with synthetic per-issuer test keys.

The trusted registry pins each issuer to one failure domain, so a valid issuer
cannot self-assert a second domain to satisfy quorum. HMAC is sufficient only for
the local research harness. A production protocol
requires asymmetric issuer identity, key rotation, revocation, freshness policy,
and independently attestable failure-domain ownership.

M0 accepts transition content only in `contract` envelopes, held-out sequences
only in `trace` envelopes, and state policies only in `state_schema` envelopes.
The registered `metamorphic_property` and `negative_case` kinds fail closed until
their executable M1 schemas and checker obligations are frozen.

The collector rejects unknown issuers or kinds, kind/content mismatch, signature or digest mismatch,
issuer/sequence replay, duplicate content, absent or cyclic dependencies,
interface mismatch, and insufficient distinct domain labels.

## Loss oracle

The original component artifact is created outside the recovery workspace.
Fragments and the registry are written, the origin directory is deleted, and a
fresh Rust CLI subprocess with a cleared environment receives only the
recovery-workspace path.
Before reconstruction the oracle verifies every registered original path is
absent, rejects symlinks, and scans every recovery file for forbidden SHA-256
digests.

This demonstrates artifact absence for the controlled local experiment. It does
not prove deletion from storage outside the experiment, erase filesystem history,
or create a kernel security boundary. Those stronger claims require a disposable
VM or container with a mounted evidence-only volume in a later milestone.

## Certification and refusal

The synthesizer and checker are separate Rust modules with different internal
table representations. The checker reparses the serialized candidate and
independently requires a total transition table and every mandatory transition
obligation. They share Serde JSON syntax handling and protocol data types, so M0
claims semantic-interpreter separation rather than full implementation
independence; M1 must reduce and differentially test that common-mode parser risk.
Coverage cannot override a failure. At least one held-out trace is mandatory and
is evaluated only by the checker.

Certification binds complete survivor-envelope digests, normalized constraints,
domain labels, grammar and checker
identities, search bounds and actual work, candidate digest, obligation coverage,
loss attestation, and deployment preconditions. M0 generates evidence only; it
does not deploy a candidate.

Refusal codes are `INVALID_REGISTRY`, `INVALID_EVIDENCE`,
`INSUFFICIENT_EVIDENCE`, `CONTRADICTORY_EVIDENCE`, `ARTIFACT_PRESENT`,
`SEARCH_EXHAUSTED`, and `CHECKER_REJECTED`. `INTERNAL_ERROR` is reserved for a
future fail-closed CLI boundary; unexpected defects currently terminate the CLI
and must not be interpreted as certification.

## Registered experiment

The deterministic fixture is a two-state turnstile with `coin` and `push`
inputs. Its seed is `20260729`; M0 records backup/replica, trace-only, and
centralized-contract baselines. Primary metrics are certified correct recoveries
and unsafe certifications. Secondary metrics include refusal rate, search time,
candidate complexity, and authoring cost. M0 registers these measurements; M2
performs comparative evaluation.

Run the authoritative private-repository validation with:

```sh
rustup show
cargo fetch --locked
./scripts/ci-local.sh
```

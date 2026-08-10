# Engineering Review

The project is ready only for an executable research contract. The strongest
design choice is bounding synthesis and making refusal first-class. The weakest
point is oracle completeness: the system may faithfully satisfy an incomplete
contract while producing an unsafe service.

Before M1, review must verify the loss oracle, canonical fragments, failure-domain
semantics, interpreter separation, sandbox denial-by-default, deterministic
search, state/effect boundaries, and honest baseline accounting.

Reject any shortcut that leaks the original artifact, uses the same interpreter
for generation and certification, lets a score override failed contracts, or
describes bounded DSL results as arbitrary service recovery.

## M0 implementation review, 2026-08-10

Scope reviewed: all four M0 requirements, Rust stack policy, private local CI,
finite-state semantics, canonical envelopes, provenance and quorum, loss oracle,
bounded synthesis, separate semantic checker, certificate, refusal behavior,
tests, packaging, and documentation claims.

Material findings resolved before delivery:

- Replaced the initial uncommitted Python prototype with the required Rust
  control plane and pinned Rust 1.97 toolchain.
- Bound each trusted issuer to one configured failure domain so a signed fragment
  cannot forge quorum by self-asserting another domain.
- Replaced wording-dependent refusal mapping with typed Rust error variants.
- Added file-count, byte, fragment-count, fragment-size, and synthesis-work
  bounds; rejected symlinks and non-regular evidence files.
- Classified malformed fragment JSON as hostile evidence rather than registry
  failure and enforced fragment kind/content schemas.
- Used the HMAC implementation's constant-time verification path.
- Corrected canonical JSON to sort nested object keys rather than relying on Rust
  declaration order.
- Moved fragment and workspace entry limits inside enumeration so an attacker
  cannot force unbounded path collection before refusal.
- Made the checker independently bind candidate component, interface, and state
  policy rather than checking transition behavior alone.
- Qualified checker independence: synthesis and checking semantics are separate,
  but Serde parsing and protocol types are common-mode M0 dependencies.
- Replaced hosted-CI assumptions with the repository-owned offline
  `./scripts/ci-local.sh` gate and added the required delivery policy.

Residual M0 risks and limitations:

- The loss oracle proves absence only inside a controlled local experiment. It
  does not provide secure erasure or a kernel isolation boundary.
- HMAC keys and issuer-domain policy are synthetic TCB inputs. Production use
  requires asymmetric identity, rotation, revocation, and domain attestation.
- The checker shares serialization code and protocol types with the collector.
  M1 must reduce and differentially test this common-mode risk.
- M0 produces data-defined candidates and certificates only. It has no
  WebAssembly sandbox, deployment transaction, durable ledger, or rollback path.
- Comparative baselines and adversarial campaigns are registered but not run;
  they remain explicitly unchecked M2 work.

No M0 limitation weakens a certification obligation into success. Unsupported,
ambiguous, contradictory, malformed, over-budget, or artifact-present cases fail
closed. No product release or research tag is authorized by M0 completion.

## M1 implementation review, 2026-08-10

Scope reviewed: every M1 checkbox, `fsm-v1` enumeration, training/held-out
separation, executable negative and metamorphic obligations, checker wire parser,
generated WebAssembly, Wasmi configuration, corpus runner, evidence ledger, CLI,
tests, documentation, and dependency graph.

Material findings resolved:

- Upgraded Wasmi to the latest stable 1.1.0 line and reran all sandbox evidence.
- Capped grammar symbols, cells, checker-wire allocations, ledger snapshots,
  corpus size, WebAssembly bytes, memory, instances, tables, and execution fuel.
- Kept held-out traces outside synthesis and proved that only training traces can
  select a candidate.
- Added ambiguity refusal after detecting a second satisfying candidate and
  budget refusal before work exceeds the registered limit.
- Removed Serde JSON from candidate checking through a strict bounds-checked
  binary wire decoder; malformed, truncated, oversized, and trailing data refuse.
- Made generated modules import-free and compared every sandbox transition with
  the independently checked table.
- Added import-denial and infinite-loop fuel tests at the sandbox public boundary.
- Made ledger entries content-addressed, create-new, synchronized, atomically
  published, no-clobber, input-snapshotted, and replay-verified.
- Rejected corpus path traversal and symlinked workspaces.

Residual M1 limitations:

- Enumeration is exponential by design. Candidate and grammar limits make work
  finite, but larger supported grammars will often refuse on budget.
- The checker has a private candidate parser and separate semantics but still
  shares obligation types, dependencies, process, compiler, and host with the
  synthesizer. Independent process/toolchain reproduction remains a release gate.
- Wasmi provides a capability-empty VM boundary, not OS process isolation,
  side-channel resistance, or proof against every engine defect.
- Ledger fixtures contain synthetic HMAC keys. Real evidence needs encryption,
  redaction, retention, access policy, key separation, and stale-lock governance.
- The M1 corpus is intentionally small and synthetic. It establishes the loop,
  not comparative value, generality, or production safety.

Every unsupported, ambiguous, contradictory, malformed, capability-requesting,
over-budget, checker-disagreeing, or artifact-present path refuses. M2 remains the
first unchecked milestone. No release or tag is authorized by M1 completion.

## M2 implementation review, 2026-08-10

Scope reviewed: every M2 checkbox, state schema transformation, activation and
rollback ordering, baseline semantics, metric registration, retained result
classes, freshness, evidence adversaries, trust domains, trace separation,
sandbox bounds, public CLI behavior, and documentation claims.

Material findings resolved:

- Encoded candidate and state in one synchronized deployment bundle so activation
  cannot expose a mixed candidate/state pair.
- Persisted the prior complete bundle before replacement and proved injected
  pre-activation failure leaves the active image unchanged.
- Required certified campaign expectations to pin a candidate digest and
  revalidated candidate/state integrity before accepting rollback images.
- Bound deployment, campaign manifest, case count, evidence count, individual
  evidence bytes, aggregate byte and candidate-work counters, state inputs, and
  path components.
- Made backup/replica execution inspect the registered loss paths rather than
  reporting the experimental assumption as an observation.
- Required every case to share one baseline and metric registration and refused
  unsupported registered measures instead of silently omitting them.
- Added an explicit freshness window and refused stale but otherwise valid signed
  fragments.
- Retained typed positive, refusal, timeout, checker-disagreement, and negative
  results with zero unsafe certifications in the synthetic campaign.

Residual M2 limitations:

- State is one finite FSM symbol. Databases, queues, files, clocks, remote effects,
  and distributed transactions are not modeled or reversible.
- Atomic rename and a create-new lock protect one local deployment file. The lock
  is not a cross-host lease or consensus protocol and has no stale-owner recovery.
- Search timing is environment-dependent. The retained stable result records
  categories and counts, while the executable report records measured timing.
- Baselines are matched to the same registered local loss scope but do not model
  production backup systems or independent engineering teams.
- The checker, Wasmi engine, HMAC policy, fixtures, and host remain synthetic TCB
  elements. Independent reproduction and security review remain M3 gates.

No M2 result authorizes productisation, a public release, or a general recovery
claim. M3 is the first unchecked milestone.

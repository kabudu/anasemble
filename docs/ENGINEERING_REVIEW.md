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

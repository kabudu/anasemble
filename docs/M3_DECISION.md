# M3 Research and Productisation Decision

## Decision

**Continue into production engineering. Do not claim production completeness yet.**

The bounded mechanism is technically viable inside the frozen FSM model, and the searched material did not reveal an exact match for the complete catastrophe protocol. The proven kernel justifies production engineering, not a general disaster-recovery claim. The current corpus is synthetic and small, centralized surviving contracts match the successful Anasemble outcome, and semantic evidence costs more bytes and authoring effort than the disposable fixture artifact.

This decision authorizes private implementation of the production-complete roadmap. It does not authorize a tag, package, GitHub Release, public repository, hosted CI, website, production deployment, trademark filing, or customer claim.

## Requirement audit

| M3 requirement | State | Evidence |
|---|---|---|
| Current novelty, product, name, standards, and patent diligence | Completed internally | `NOVELTY.md`, `PRIOR_ART_MATRIX.md`, `M3_DILIGENCE_LOG.md` |
| Internal security, sandbox, TCB, and soundness review | Completed | `TCB_LEDGER.md`, `ENGINEERING_REVIEW.md`, existing adversarial suites |
| Clean-room reproduction packet | Published and locally verified | `INDEPENDENT_REPRODUCTION.md`, `./scripts/ci-local.sh` |
| Independent reproduction and expert review | Optional post-release | not represented as completed and not a productisation gate |
| Matched comparison | Completed for the synthetic registered scope | `experiments/m3-comparison.json` and M2 campaign |
| Cost quantification | Completed for the synthetic registered scope | `experiments/m3-costs.json` |
| Productisation decision | Completed | continue into production engineering under bounded claims |
| Production-complete implementation | In progress | dependency-ordered P0 through P4 roadmap |

## Matched comparison

All methods receive the same registered turnstile loss scope. Conventional
backup/replica recovery is unavailable only because the experiment stipulates
loss of every deployable artifact. Trace-only synthesis certifies no case.
Centralized surviving contracts certify the same positive case as Anasemble and
refuse the other four cases. Therefore, the present experiment demonstrates no
recovery-rate advantage over centralized contracts. Anasemble's remaining
hypothesis is organizational and fault-domain resilience from distributing
semantic evidence, which this local corpus does not independently establish.

## Cost findings

The measured turnstile fixture has a 594-byte disposable artifact. Its six signed
semantic fragments occupy 2,816 bytes and the registry occupies 1,195 bytes in
the measured temporary workspace: 4,011 bytes total, about 6.75 times the artifact
size before ledger duplication. Authoring also requires four transition
contracts, one state policy, one held-out trace, two issuer identities, two
failure-domain assignments, a grammar, resource bounds, loss-oracle inputs,
baselines, and metrics.

On an arm64 macOS host with Rust 1.97.0 and a warm build cache,
`./scripts/ci-local.sh` completed in 3.22 seconds on 2026-08-11. This is a local
engineering measurement, not a recovery-time claim. Search remains exponential
within explicit bounds. Operationally, recovery additionally requires semantic
distribution, key and freshness governance, catastrophe drills, evidence-ledger
retention, state mapping, deployment locking, and rollback handling.

The retained cost record counts eight minimum operational activity classes in
that list. It does not convert them to labour hours because no observed operator
study exists; inventing a duration would be less accurate than retaining the
count and explicitly leaving human time unmeasured.

The modeled state-loss cost is one finite state symbol plus schema version and
revision. The test restores that modeled value exactly. Database, queue, file,
clock, remote, and hidden-effect loss is unmeasured and unsupported, so it cannot
be reported as zero.

## Diligence refresh, 2026-08-11

Search classes covered exact and adjacent academic work, commercial disaster
recovery, standards, patents, GitHub, npm, Cargo, general web, and official
trademark search portals. Queries included total artifact loss, executable
specification recovery, distributed semantic evidence, program synthesis for
disaster recovery, service reconstruction, and exact or similar Anasemble names.

Key findings:

- Program synthesis from specifications and examples is established, including
  Microsoft PROSE and the Gulwani, Polozov, and Singh survey.
- AWS Elastic Disaster Recovery continuously replicates source-server blocks and
  launches recovery instances; it restores retained artifacts rather than
  synthesizing a replacement after their stipulated total loss.
- PASE synthesizes and verifies cloud recovery plans, a closer 2026 adjacency,
  but its subject is remediation planning rather than reconstructing a lost
  component from independently distributed semantic fragments.
- WebAssembly's standard security model supports no ambient host access and makes
  imported capabilities an embedder responsibility, while explicitly retaining
  side-channel and embedder risks. This supports but does not prove Anasemble's
  Wasmi boundary.
- NIST SP 800-34 treats backup, alternate storage, alternate processing, and
  contingency planning as established recovery practice.
- Patent results cover program synthesis from examples, executable specification
  generation, recovery workflow generation, metadata-based application recovery,
  and data-fragment reconstruction. No legal conclusion or freedom-to-operate
  opinion follows from this search.
- Exact-name web searches returned no third-party result. GitHub exact-name search
  returned only `kabudu/anasemble`; npm returned not found; `cargo search` returned
  no exact package. The crates.io HTTP API denied the automated request, so the
  Cargo check is index-search evidence, not registry reservation.
- Official USPTO and EUIPO/TMview search facilities were identified. No
  professional confusing-similarity or goods-and-services search was completed.

Primary and direct sources:

- <https://www.microsoft.com/en-us/research/publication/program-synthesis/>
- <https://www.microsoft.com/en-us/research/project/prose-framework/usage/>
- <https://arxiv.org/abs/2607.01595>
- <https://docs.aws.amazon.com/drs/latest/userguide/what-is-drs.html>
- <https://www.w3.org/TR/wasm-core/>
- <https://csrc.nist.gov/pubs/sp/800/34/r1/upd1/final>
- <https://patents.google.com/patent/US10817552B2/en>
- <https://patents.google.com/patent/US10423780B1/en>
- <https://patents.google.com/patent/US7039898B2/en>
- <https://patents.google.com/patent/EP3230865B1/en>
- <https://patents.google.com/patent/US9965358B2/en>
- <https://www.uspto.gov/trademarks/search>
- <https://www.euipo.europa.eu/en/search>

## Production gates

Implementation completeness requires the internal P0 through P4 roadmap: supported real-service contracts, production identity and evidence distribution, stateful backend recovery, isolated runtime activation, durable operations, compatibility, destructive staging drills, and release readiness. Claims remain limited to supported and tested combinations, and ordinary backups remain a required comparison and interoperability path.

Independent clean-clone reproduction, external security and soundness review, and broader third-party deployment are optional post-release assurance. Their absence must not be described as completion, but it does not block implementation or productisation. Legal review remains a separate release or commercialisation decision when proportionate.

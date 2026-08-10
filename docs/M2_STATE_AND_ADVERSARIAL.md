# M2 State and Adversarial Evaluation

## Scope

M2 extends the frozen finite-state research harness. It does not broaden the DSL.
The state model is one declared FSM state symbol, a schema version, and a monotonic
revision. A transform is a bounded explicit map from source symbols to candidate
symbols. Missing mappings, schema mismatch, revision overflow, or undeclared
target states refuse.

## Deployment transaction

`deploy` reconstructs and certifies first. It then persists the prior complete
deployment as `rollback.json`, builds one candidate-and-state bundle, synchronizes
it, and atomically renames it to `active.json`. Injected failures before activation
leave the previous active image unchanged. `rollback` atomically restores the
prior complete image. Files are regular, symlink-free, and at most 1 MiB.
Candidate digest, candidate grammar, state membership, schema binding, and audit
digest fields are revalidated before an active or rollback image is accepted.

This transaction cannot roll back databases, remote services, queues, network
effects, or other hidden state. Such effects remain outside the grammar and must
cause refusal rather than be inferred.

## Matched campaign

`evaluate-campaign` accepts at most 256 cases with safe relative workspace names.
Each case runs the normal recovery path plus every registered baseline:

- backup/replica checks whether any registered lost artifact path survives;
- trace-only removes transition contracts before bounded synthesis;
- centralized-contract removes trace evidence before bounded synthesis.

All cases must share one baseline and metric registration. Unsupported baseline
or metric names refuse, preventing a report from silently omitting a pre-registered
measure. A certified expectation must pin the candidate digest; refusal classes
must not. Outcomes retain the typed refusal category and candidate digest.

## Adversarial coverage

The executable suites cover poisoned content, omission, contradiction, issuer
sequence replay, freshness-window rejection, shared-domain quorum collapse,
forged provenance, held-out trace disagreement, resource exhaustion, capability
imports, excessive memory, infinite execution, and partial activation. Positive,
refusal, timeout, disagreement, and negative outcomes are retained separately.

## Bounds and residual risk

Campaign manifests are 128 KiB or less. Evidence enumeration is capped at 10,000
regular files of 1 MiB each and uses checked byte counters. Search and sandbox
bounds remain those certified in M1, with at most four million registered
candidate evaluations across all normal and synthesis-baseline runs in one
campaign. Deployment uses a create-new local lock;
stale-lock ownership and distributed commit require a later production design.
Results use synthetic evidence and keys and do not establish
general recovery value.

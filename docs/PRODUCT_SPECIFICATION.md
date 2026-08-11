# Product Specification

## Problem

Backups and replicas preserve artifacts. They do not help when every deployable
artifact for one component is lost or untrusted but other nodes retain partial
knowledge of its protocol and behavior. Anasemble tests whether deliberately
distributed semantic evidence can support construction of a bounded replacement.

## Research user and job

A resilience researcher defines a component in a finite typed DSL, distributes
contracts and observations across failure domains, destroys all original
artifacts, and asks the system to produce either a certified non-identical
replacement or a precise refusal.

Inputs are signed executable contracts, typed protocol traces, state-schema
fragments, a synthesis grammar, resource bounds, and survivor provenance. Outputs
are a candidate component, proof/test certificate, confidence-independent
coverage report, and deployment/refusal decision.

## First vertical

Stateless and explicitly stateful request/response components over finite data
types. No unrestricted networking, reflection, native code, hidden clocks, or
unbounded storage.

Success requires restoring declared behavior after total artifact deletion,
rejecting poisoned or ambiguous evidence, and outperforming trace-only synthesis
and backup/replica baselines on the stated failure model.

## M3 status

The implementation satisfies the bounded technical behaviors and outperforms the
unavailable backup and trace-only baselines under the stipulated total-loss scope.
It does not outperform a surviving centralized contract on the synthetic corpus.
That result, the narrow state model, and absent independent reproduction block
productisation. The supported user remains a resilience researcher running an
offline disposable experiment.

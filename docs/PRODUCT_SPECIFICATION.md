# Product Specification

## Problem

Backups and replicas preserve artifacts. They do not help when every deployable
artifact for one component is lost or untrusted but other nodes retain partial
knowledge of its protocol and behavior. Anasemble tests whether deliberately
distributed semantic evidence can support construction of a bounded replacement.

## Product user and job

A resilience engineer registers a supported service interface, declared effects, state dependencies, consistency policy, recovery bounds, contracts, and observations across independent failure domains. After a declared artifact-loss event, the operator asks Anasemble to produce either a staged, certified, state-bound replacement with rollback evidence or a precise refusal.

Inputs are signed executable contracts, typed protocol traces, state-schema
fragments, a synthesis grammar, resource bounds, and survivor provenance. Outputs
are a candidate component, proof/test certificate, confidence-independent
coverage report, and deployment/refusal decision.

## Proven kernel

Stateless and explicitly stateful request/response components over finite data
types. No unrestricted networking, reflection, native code, hidden clocks, or
unbounded storage.

The finite-state vertical proves bounded reconstruction, certification, refusal, and local transactional deployment. It remains the reference semantics and regression kernel.

## Production-complete acceptance boundary

The product is implementation-complete only when it supports at least one real HTTP service runtime, production identity and independently administered evidence stores, filesystem plus database/object/queue state, OS-level candidate isolation, a production orchestrator and artifact registry, operator-approved activation, durable recovery jobs, health observation, audit, rollback, installation, upgrade, and removal. Each supported combination must pass a destructive staging drill and publish exact compatibility and loss bounds.

Unsupported protocols, effects, state backends, consistency models, or deployment targets must refuse before synthesis or mutation. Anasemble never infers that undeclared state or side effects are safe to discard.

## Current status

The implementation is complete for the exact profiles marked `Supported` in
`COMPATIBILITY.md`. Other implemented paths are explicitly experimental until
their named evidence gap is closed. Anasemble reconstructs and certifies the
bounded kernel, restores each supported state model, activates immutable
artifacts under the supported Docker isolation profile, and provides
restart-safe operator jobs, audit, diagnostics, installation, removal, and
runbooks. It still does not outperform a surviving centralized contract on the
synthetic corpus and does not support arbitrary services, hidden effects,
undeclared state, remote providers outside the exact compatibility profiles, or
a cross-backend distributed transaction.

Implementation completion is not release authority or a claim of universal production suitability. Public release, deployment, package publication, legal clearance, and visibility remain separate explicit decisions. Independent reproduction and external review are optional post-release assurance.

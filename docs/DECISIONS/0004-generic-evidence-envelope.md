# ADR 0004: Generic evidence envelope and reconstruction events

Status: accepted on 2026-08-24.

## Context

Threniq needs a provider-neutral signed evidence envelope and an append-only reconstruction event log. Those capabilities are generic to evidence-bound reconstruction, not to Threniq's world model. Implementing them only in Threniq would fork Anasemble.

## Decision

Publish `anasemble-core`, `anasemble-evidence`, and `anasemble-events` as workspace crates. Canonical CBOR is the signing and digest representation for the generic envelope and events. Existing fragment `Envelope` files keep canonical JSON, HMAC or Ed25519 signatures, and the current CLI receipts. The root `anasemble` crate re-exports the new types without changing file formats.

## Alternatives

- Implement the envelope only in Threniq: rejected because it creates a shadow protocol.
- Replace fragment envelopes with EvidenceEnvelopeV1 immediately: rejected because it would break existing recovery fixtures.

## Consequences

Threniq may consume the tagged Anasemble APIs after this lands. Old JSON certificate and fragment digests must remain stable. Generic envelope payloads never persist forbidden secret classes.

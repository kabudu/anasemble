# ADR 0001: Bound Anasemble to a finite typed component DSL

**Status:** Accepted — 2026-07-29

We will test semantic reconstruction only for finite typed request/response
components with explicit state and effects. We will not start with arbitrary
languages, opaque production traces, native binaries, or LLM-generated code.

This permits deterministic search, independent interpretation, meaningful
artifact-absence checks, and falsifiable safety claims. Applicability is narrow by
design; expansion requires evidence.

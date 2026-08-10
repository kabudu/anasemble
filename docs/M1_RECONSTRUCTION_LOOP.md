# M1 Independent Reconstruction Loop

## Scope and frozen grammar

M1 freezes `fsm-v1`, a deterministic finite Mealy-machine DSL. A grammar contains
at most 64 states, inputs, and outputs, no more than 256 state/input cells, symbols
of at most 256 bytes, and a candidate budget no greater than 1,000,000. Search
enumerates complete transition tables in deterministic lexical order. It certifies
only one satisfying candidate; zero candidates means contradictory evidence, more
than one means insufficient evidence, and reaching the registered budget means
`SEARCH_EXHAUSTED`.

`fsm-v0` remains available for M0 replay. `fsm-v1` accepts transition contracts,
training and held-out traces, negative cases, idempotence properties, and an exact
state policy. Held-out traces do not influence synthesis.

## Checker separation

The synthesizer sends candidates through the versioned `ANCKM1` binary wire
format. The checker uses a handwritten, bounds-checked decoder and its own nested
transition-table interpreter. It does not use Serde JSON to parse candidates and
does not call synthesizer evaluation logic. Protocol obligation values are still
shared Rust types, so M1 reduces rather than eliminates common-mode implementation
risk. Differential parser and semantic mutation tests remain mandatory.

## WebAssembly sandbox

Every certified table is compiled into a WebAssembly module exporting
`step(i32, i32) -> i64`. The module has no imports, memory, table, start function,
clock, randomness, filesystem, network, environment, or host callback. Wasmi
1.1.0 validates and executes it with an empty linker, one instance, no tables,
at most one 64 KiB memory, and 10,000 fuel per call. Certification executes every
transition through WebAssembly and compares it with the checked table.

Import attempts fail before instantiation. Infinite execution stops at the fuel
bound. This is capability isolation for the generated M1 ABI, not a general
production sandbox, process boundary, side-channel defense, or permission to run
arbitrary third-party WebAssembly.

## Evidence ledger

`anasemble recover <workspace> --ledger <root>` snapshots the canonical outcome,
registry, and fragments into an immutable content-addressed entry. It uses an
exclusive entry lock, create-new writes, file synchronization, directory
synchronization, and atomic directory rename. Replaying identical evidence
returns the existing entry only after outcome verification; different content
cannot overwrite it.

M1 fixtures contain synthetic keys and values. Real evidence needs encryption,
redaction, retention, access control, and crash-lock recovery policy before use.

## Registered corpus and limits

`experiments/m1-corpus.json` registers two distinct stateless components:
`identity` and `inverter`. Tests generate their signed evidence and lost artifacts,
delete the artifacts, and invoke `anasemble recover-corpus <root>` in a fresh
process. Both must certify with no imports, checker disagreement, ambiguity,
artifact access, or unbounded work.

M1 does not execute matched baselines, model external state, deploy candidates,
or claim production readiness. M2 addresses the bounded research cases for the
first three; production readiness remains an M3 decision gate.

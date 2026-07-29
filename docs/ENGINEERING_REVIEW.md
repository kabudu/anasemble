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

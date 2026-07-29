# Adoption and Integration

Initial adoption is an offline catastrophe drill. A team translates one small
component into the typed DSL, exports signed semantic fragments to distinct local
stores, destroys a disposable build artifact, and compares recovery outcomes.

The integration contract is file-based JSON plus a deterministic CLI. Adapters for
service meshes or schema registries remain outside the trusted core. A future
shadow mode may generate candidates without deployment.

Brownfield cost is the central adoption risk: contracts, effects, state, and
failure-domain provenance must be explicit. Anasemble must quantify this authoring
cost rather than treating it as free.

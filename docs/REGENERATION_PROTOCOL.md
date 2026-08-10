# Regeneration Protocol v0

A fragment envelope contains `kind`, `component`, `interface_version`, `issuer`,
`failure_domain`, `issued_at`, `sequence`, `content_digest`, `dependencies`, and
`signature`. Fragment kinds are `contract`, `trace`, `state_schema`,
`metamorphic_property`, and `negative_case`.

Recovery requires the configured quorum of independent domains and at least one
contract plus state policy. The M0 collector rejects equivocation, unknown
interfaces, and dependency cycles.

In M0 the registry pins every trusted issuer to one domain; labels supplied by a
fragment cannot expand that issuer's domain identity. Timestamps are parsed but
expiry policy is not yet implemented, so the broader expired-schema rule remains
an M1 requirement. M0 uses synthetic HMAC keys; asymmetric identity, rotation,
and revocation are required beyond the local harness.

A certificate binds the survivor set, normalized constraints, grammar version,
search bounds, candidate digest, checker identity, passed and uncovered
obligations, state transform, and deployment preconditions. Certification requires
all mandatory contracts and negative cases, held-out trace conformance, resource
bounds, and no unresolved ambiguity. Coverage scores can inform diagnosis but
cannot override a failed obligation.

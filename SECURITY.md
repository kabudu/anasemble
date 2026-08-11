# Security policy

## Supported versions

Before the first public release, only the current `master` branch receives
security fixes. After release, `docs/RELEASE.md` records supported versions and
backport policy. Experimental compatibility profiles do not carry a production
security-support claim.

## Reporting a vulnerability

Do not open a public issue for a suspected vulnerability. After the repository
becomes public, use GitHub private vulnerability reporting. If that channel is
not available, contact the repository owner through the private method on their
GitHub profile and share only enough information to establish a secure channel.

Include affected commit or version, prerequisites, impact, reproducible steps
and any known mitigation. Never include live credentials, customer evidence or
unnecessary personal data. Maintainers will acknowledge a credible report within
five working days, provide a triage decision when evidence permits and coordinate
disclosure based on severity and fix readiness. These are response targets, not
warranties.

## Scope

High-priority areas include evidence authenticity, replay, receipt integrity,
secret exposure, sandbox escape, activation authority, path traversal, rollback,
ambiguous external state and resource exhaustion. Dependency-only reports should
show reachability or plausible impact where possible.

The project cannot promise rewards, embargo duration or acceptance of every
report. Good-faith research that avoids privacy violations, service disruption
and data destruction will be handled constructively.

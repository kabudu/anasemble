# Release Strategy

While the repository is private, `./scripts/ci-local.sh` is the only
authoritative local CI gate. Hosted CI, including GitHub Actions, is disabled by policy
and requires explicit user approval at a documented public-opening or release
gate. Absent hosted checks must never be described as passing checks.

No product release is authorized. A research tag requires a frozen DSL and loss
oracle, reproducible artifact-absence proof, complete baselines, adversarial
results, sandbox and soundness review, refreshed novelty/name diligence, and
published limitations.

`0.1.0-research` may identify the first reproducible artifact. Public packages,
hosted recovery, real service traces, production deployment, and commercial
claims each require a separate gate.

## Release presentation contract

Every public release requires repository-owned, versioned curated release notes
that are distinct from `CHANGELOG.md`. The release title must contain Anasemble,
the version, and a short human-readable theme. The body must open with the
user-visible outcome, list three to five material changes, state claim and
compatibility boundaries, provide one primary installation path, and link to
supporting evidence and the detailed changelog.

Release prose must use one physical source line per paragraph or list item. Do not hard-wrap it at a fixed column. It must not repeat the title, dump the raw
changelog, use an unexplained internal experiment inventory, or contain Unicode
U+2014. Release automation must fail closed if the curated title or body is
missing or mismatched to the tag.

Before publication, inspect a rendered preview at desktop and narrow widths for
heading hierarchy, wrapping, lists, code fences, links, and placeholder text.
After publication, inspect the canonical release URL and immediately correct any
metadata that differs from the approved preview.

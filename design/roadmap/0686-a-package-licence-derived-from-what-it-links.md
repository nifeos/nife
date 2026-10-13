---
status: NOT-STARTED
raised: 2026-09-27
promoted_from: a-package-licence-derived-from-what-it-links
milestone_dependencies: 611
decision_dependencies: 135
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 686. A package's license is derived from what it links, and a lint checks it

*Title and slug are drafts.*
Owed to `design/decisions/0135-running-gpl-software.md`'s amendment (calef, 2026-09-27T15:11Z: the
project will serve GPL packages). Not built by that amendment; filed here so the work has a home.

## The shape

- A per-program SPDX license, computed from the crates that program statically links, read from
  Cargo metadata rather than hand-entered.
- A lint fails when a package's declared license disagrees with what its programs actually link,
  or when a `base` package (§239 (four package kinds, and TOML for package declarations and
  recipes)) carries copyleft.
- A package's own license is derived as the AND of its programs' licences, never hand-edited: the
  same discipline `helpers/packages.py` already applies to a package's other declared fields.

## Why this is owed rather than optional

§135 (running GPL software is aggregation, the capability boundary is what makes it so, and
packages are how it arrives)'s base-and-packages split depends on the image staying free of
copyleft. Today that rests on
a person noticing; this proposal is the mechanism rung above noticing, per AGENTS.md's ladder ("a
gate that fails loudly").

## Index row

DECISIONS §135's amendment says the project serves GPL packages. Proposed: derive each package's license from the crates its programs link, and lint that a declared license agrees and that a base package carries no copyleft.

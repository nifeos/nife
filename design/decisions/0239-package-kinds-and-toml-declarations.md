---
status: DECIDED
raised: 2026-09-27
decided: 2026-09-27
ratified_by: calef
---

# 239. Four package kinds, and TOML for package declarations and recipes

*Section number provisional until the merge queue lands it.*

Raised on pull request #1396, milestone 611 (every program and crate belongs to a package, and
every package has a home). It built the package-boundary lint against the existing `.package` and
`.recipe` formats, and left two questions open: what format a package declaration and a recipe are
written in, and how many kinds of package there are.

calef ruled both in comments on #1396, the same session. He also ruled a third thing about #1396
itself, with no PR comment to cite.

## Land the PR now

calef said "Yes" to landing #1396 now, at approximately 2026-09-27T06:30Z. There is no PR comment
recording this: it was said in the maintainer session that briefed this lane. This section is
where it is written down (CLAUDE.md, "a note, a report, or a comment on a pull request" is the
floor rung for a fact that would otherwise exist only in that session).

## The format: TOML

calef ruled at 2026-09-27T07:00Z: package declarations and recipes both move to TOML. His words:
*"Move both to toml."* JSON was refused for having no comments. YAML was refused for its ambiguity,
and for how it handles dependencies: a YAML anchor/alias graph reads as a dependency list without
being one.

This PR (#1396) lands as it already was. A follow-up (built in #1405) converts `packages/*.package`
to `packages/<name>.package.toml`, `packages/homes` to `packages/homes.toml`, and
`packages/*.recipe` to `packages/*.recipe.toml`. It also gives `xtask` the `toml` crate as a
host-only dependency, which §46 (thin primitives or whole subsystems) governs and §240 (`toml` in
`xtask`) records. The file extensions are provisional.

## The four kinds, and what each buys

calef ratified the four package kinds at 2026-09-27T07:23Z: `base`, `optional`, `sdk` and `test`.
The rule: **a package's kind says where it ends up.**

| Kind | Destination |
|---|---|
| `base` | every image |
| `optional` | installable on a device |
| `sdk` | a developer's machine (the S2 archive) |
| `test` | nowhere outside CI |

A new kind has to name a new destination. A kind that does not change where a package lands is not
a new kind: it is a `base` or `optional` package that happens to be used differently. The S2
archive is the slot layout of §235 (the OS is built and updated from packages, and its source
divides by purpose).

## Reversibility

The format change is mechanical and reversible while nothing outside the tree reads these files.
`helpers/packages.py`'s selftest plants violations and checks that the parser refuses a misspelt
key, which is the gate that would catch a bad migration. The four kinds are closer to the naming
category (CLAUDE.md, "move fast on what can be undone"): cheap to add a fifth kind later, expensive
to rename one that packages already declare themselves against.

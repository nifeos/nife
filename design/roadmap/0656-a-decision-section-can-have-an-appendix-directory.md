---
status: NOT-STARTED
raised: 2026-09-26
promoted_from: a-decision-section-can-have-an-appendix-directory
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 656. A decision section can have an appendix directory

*Title and slug are drafts.*
Raised by the `proposal/signed-builds` lane (#1325). It was writing
§220 (a vendor signs, a developer self-signs, and trusting a key is scoped) and had been told to put
overflow in an appendix directory named after the section.

A change to one regular expression in `script/decisions` and to what it treats as a
decision file. It touches no syscall surface, wire format or dependency.

## The finding

§212 (a prose budget: 3,000 words of main body, with appendices under the same cap) ratified where
an appendix lives: a parent-named sibling directory, `X.md` beside `X/`. `helpers/prose_ratchet.py`
enforces that siting with its orphan check. `script/decisions` refuses it for the one directory
where sections are longest.

It lists `design/decisions/` and matches every entry against `FNAME =
re.compile(r"(\d+)-[a-z0-9][a-z0-9-]*\.md")`. An entry that does not match is a problem. Reproduced
on 2026-09-26 by creating an empty
`design/decisions/0220-signed-builds-and-scoped-key-trust/`:

```
PROBLEM: 0220-signed-builds-and-scoped-key-trust: not the index and not a decision file (expected <number>-<slug>.md); nothing else lives here
```

`script/decisions --check`, which `script/lint` runs, exits 1. So a decision section over the
3,000-word cap cannot split the way §212 says to. It has to be cut instead, and §220 was cut. A
section whose evidence cannot be cut has no legal shape.

## The fix

Accept a directory `<N>-<slug>/` when `<N>-<slug>.md` exists beside it, and skip it as a decision.
The orphan check already makes sure every file inside is linked from the section. Keep refusing a
directory with no section beside it, because that one really is a stray.

## Index row

DECISIONS §212 sites an appendix in a directory beside its section, and `script/decisions` refuses that directory. Proposed: change one regular expression and what counts as a decision file.

---
status: NOT-STARTED
raised: 2026-11-10
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1067. The interface check reads a reserved row

*(The number, title and slug are provisional.)*

Reuse: `helpers/interface_declarations_check.py` already reads the one table of ids in milestone 956
(the linked interfaces carry checkable versions); this adds one row form to its reader. Nothing
outside the tree does this job.

A ruling sometimes fixes a wire before its crate exists, and the wire's interface id waits for the crate. The first such
case is the JH7110 clock and reset wire ruling, which gives its contract interface id 24 before
`crates/` holds the contract. The table needs a row for it, and the check cannot read that row yet.

## Today

`TABLE` is milestone 956's "Interface ids" section. `table_rows` reads a row only when it matches
`ROW`: a number, an optional `retired,` and a backticked path. `id_problems` then fails any
assigned path that carries no `INTERFACE_ID`, with "the table assigns N to PATH, which carries no
INTERFACE_ID".

So a row for a crate that does not exist cannot take that form. The row for id 24 is prose
without a backticked path, which `ROW` skips. The check never sees the number, and nothing stops
another interface from declaring `INTERFACE_ID` 24 in the meantime. `id_problems` finds an id with
no row (`has no row in`), but it cannot object to this one: the table appears not to hold 24 at all.

## The shape

1. A reserved row form: `| 24 | reserved, for the JH7110 clock and reset wire |`. It names
   no crate and does name the ruling or milestone that holds the number. `table_rows` returns it
   as a third kind beside an assigned row and a retired one.
2. `id_problems` fails when any interface in the tree declares a reserved number, and the message
   names the reserved row and its holder: `INTERFACE_ID 24 is reserved for the JH7110 clock and reset wire; the crate that
   takes it replaces the reserved row in the same pull request`.
3. `id_problems` fails when a reserved number appears twice in the table, or in two kinds of row,
   the way it fails a number assigned twice today.
4. A crate's pull request replaces the reserved row with its normal row (`| 24 | `crate` |`) in
   the same change that adds the `INTERFACE_ID`. The check needs no other rule for this: with the
   reserved row gone, the normal row and the declaration agree, and while the reserved row stands,
   the declaration is refused.
5. The `--selftest` fixtures cover a reserved row read, a declaration of a reserved number
   refused, a reserved number listed twice refused, and a reserved row replaced by a normal one
   passing.
6. Milestone 956's "Interface ids" section states the form in the sentence that now describes the
   reserved row, and the checker's docstring lists the rule beside the five it holds.

A reserved row has no retirement and no expiry. It stays until its crate lands or a person strikes
it, and striking it is a reviewed edit to the table like any other.

## What the milestone does not do

- It does not give a reserved row a version. The version is `1` at assignment, as for every other
  row (milestone 956), and the crate states it.
- It does not decide which interfaces get ids. That stays with the ruling that holds the number.

## Effort and dependencies

Effort: S. One regex and one row kind in a script of about 330 lines, one rule in `id_problems`,
four fixtures and a sentence in 956's block.

Dependencies: none. Pull request #3422 (the clock and reset wire ruling) is the first user: its reserved row for id 24 is the
case this makes checkable. Once this lands, that row is the first one the check reads.

## Done when

1. `helpers/interface_declarations_check.py` reads the reserved row form, and `--selftest` holds
   the four fixtures of the shape above.
2. On the tree with id 24 reserved, the check fails if any interface declares `INTERFACE_ID` 24,
   and it passes with no declaration. A trial change that adds the declaration shows the failure.
3. Milestone 956's table holds the clock and reset row in the reserved form, and the check reads it.
4. The check's docstring and milestone 956's text state the rule, and the crate that takes id 24
   later needs no change to the check.

## Index row

The table of interface ids has no row form for a number held before its crate exists, so the
JH7110 clock and reset wire's id 24 is invisible to the check, and another interface could take it.
This adds a reserved row, fails any declaration of a reserved number, and has the crate's pull
request replace the row. Effort S, no dependencies.

# Architects

*calef asked for this file on 2026-09-25 (UTC) so the tree stops assuming it has one architect. It
is spelled `ARCHITECTS.md` rather than the `architects.md` he wrote because
[design/naming.md](https://github.com/nifeos/record/blob/main/design/naming.md) spells repo-root markdown in capitals, beside `AGENTS.md` and
`CONTRIBUTING.md`.*

## Who holds the role

| GitHub username | Since |
|---|---|
| calef | 2026-07-12, the first commit |

A contributor is listed by GitHub username only; legal names belong in legal and authorship strings
(`AGENTS.md`, the naming section's last paragraph).

## What the role holds

[AGENTS.md](AGENTS.md) defines it, with
[the three roles](notes/skills/maintainer/SKILL.md) and the other notes/skills/ files AGENTS.md links, wherever
they say "an architect". This file does not restate it, so there is one place to change.

## Adding an architect

An existing architect adds a row to the table above, in a pull request that says so in its title,
and merges it. The row is the grant. Nothing else should need editing: any place in the tree that
still treats one person as the architect is a bug, and the fix is to name the role there. Give the
new architect whatever GitHub access the role needs at the same time, and record the date in UTC.

## Every architect holds the whole role

calef, 2026-09-25 (UTC):

> "A second architect will have all of the authority of the first one. Same with the third, fourth,
> fifth, etc. Which means disagreements should make for interesting conversations."

So any one listed architect may rule on anything the role covers, with no domain split and no
quorum.
[§217 (every architect holds the whole role)](https://github.com/nifeos/record/blob/main/design/decisions/0217-every-architect-holds-the-whole-role.md)
records it. What happens when two architects disagree is not specified by that ruling, and nothing
here specifies it either.

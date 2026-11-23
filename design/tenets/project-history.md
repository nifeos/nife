# What a reader meets in an old record: two renames and one pivot

<!-- history: exception. Reason: this page exists to decode the old names and framing a reader meets in older records and git log, so the renames are its subject (2026-10-13 UTC). -->

*Appendix to `nifeos/nife:AGENTS.md`, which keeps the operational fact that `CLAUDE.md` is a
symlink to it. This file carries the three pieces of history a reader needs exactly once, on the day
they open a record that uses a name or a framing the project has left behind. Moved here 2026-09-24
(UTC) on calef's ruling, unchanged in substance.*

*Name: ratified 2026-09-24 (calef, approving #1189). The directory was ratified first; this
fourteenth file arrived after that ruling, shipped with a provisional name, and was ratified on its
own the same day.*

## The architect was Chris and is calef

The architect is calef (GitHub username; Chris Alef): older records and commits may say Chris,
and both are the same person, renamed 2026-08-15 at his request. Contributors are referred to by
their GitHub username in prose, attributions, records and lane reports. A grep for a contributor
then finds them rather than everyone sharing a first name. That rule is in `AGENTS.md` and this is
the case that prompted it.

Since 2026-09-25, "the architect" also names a role, listed in
`nifeos/nife:ARCHITECTS.md`, that more than one person can hold
([§217 (every architect holds the whole role)](../decisions/0217-every-architect-holds-the-whole-role.md)).
A record from before that date
means calef, since he was the only one; a reader meeting "an architect" or "the architect" in a
record after it should check the list rather than assume it still does.

## The OS was cricker-os and is nife

The OS was renamed the same day: nife, formerly cricker-os, by milestone 120 (the rename: the OS
becomes `nife`). Older records, commits, and quoted transcripts keep the old name where they
describe the past, which is the general rule for a rename here: a record says what was true when it
was written.

The GitHub organization was renamed from crickertech to nifeos on 2026-10-03 (UTC), to match the domain nifeos.org, which calef holds; crickertech was a placeholder. Records that name the old organization in prose, keep it, and old URLs redirect. Links, `gh` arguments and scripts name nifeos.

## The file is AGENTS.md and the citations still say CLAUDE.md

`AGENTS.md` is the cross-tool convention and `CLAUDE.md` is a symlink to it, decided 2026-08-14, so
Claude Code keeps finding it. It addresses any competent agent, which is what it always did.

**The in-tree citations of "CLAUDE.md" keep resolving through the symlink and were deliberately not
rewritten**, per this file's own blind-sed scar. A sweep that rewrote a name across the tree also
rewrote the row recording that the name had been *refused*: a cheap edit destroying an expensive
record. A citation that still works is not worth a sweep that might not.

## It was a learning project and is a demonstrator

This began as a learning project and pivoted to a demonstrator, deliberately and on the record
(2026-07-26). If you find the old "understanding is the goal, explain every line as we build it
together" framing anywhere, it is stale; `AGENTS.md` is the current word.

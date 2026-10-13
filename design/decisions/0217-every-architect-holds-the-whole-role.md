---
status: DECIDED
raised: 2026-09-25
decided: 2026-09-25
ratified_by: calef
---

# 217. Every architect holds the whole role

calef, 2026-09-25 (UTC), in the conversation that asked for [`ARCHITECTS.md`](../../ARCHITECTS.md):

> "A second architect will have all of the authority of the first one. Same with the third, fourth,
> fifth, etc. Which means disagreements should make for interesting conversations."

## The ruling

Every architect listed in `ARCHITECTS.md` holds the full authority of the role that
[`AGENTS.md`](../../AGENTS.md) describes. Any one of them may rule on anything the role covers:
a design fork, a name, the syscall surface, a dependency, a `PROPOSED` decision. No domain split, no
quorum and no senior architect. Adding an architect grants the whole role, and there is no partial
form of it.

## What it does not decide

What happens when two architects disagree. The ruling expects disagreements and does not give a
procedure for settling one, so this section gives none. A procedure would be a new decision, made by
the architects when one is needed.

## Why it was needed at all

Until this date the tree was written as if the role and one person were the same thing: "names are
calef's", "his attention is the scarcest thing", `ratified_by: calef` described as the only possible
value. The request that produced `ARCHITECTS.md` was to strip that assumption out so a second
architect does not mean a scour of the tree. That only works if the tree knows what a second
architect *is*, and this section is that answer.

## What it touches

- `ARCHITECTS.md` lists the architects and states this ruling beside the list.
- `AGENTS.md` refers to "the architect" or "an architect" wherever it meant the role, and keeps
  calef's name only in quotations and dated provenance, which record who said what.
- `ratified_by` in the [frontmatter](README.md#frontmatter-which-is-where-the-status-lives) was
  already a GitHub username rather than a fixed value. Any listed architect's username is valid
  there.

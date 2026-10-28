---
status: DECIDED
raised: 2026-10-26
decided: 2026-10-26
ratified_by: calef
---

# 315. A crate's documentation leaves with it

*Section number provisional, and the name a lane's coinage; the merge queue mints the number at
merge. Ruled 2026-10-26 (UTC) in the maintainer's session, and recorded 2026-10-28 on pull request
#2851.*

A crate that leaves this repository takes its documentation with it. A stranger who clones the
crate's new repository reads that repository alone, so what the crate is, why it takes the shape it
has, and what its hardware does, belong in that repository. What nife does with the crate stays
here.

## The rule

Before a crate leaves this repository, a lane lists every note and every decision in nife that
mentions the crate, and sorts each passage into one of three piles:

- moves: the passage is about the crate alone, such as a hardware quirk, an invariant, or why the
  crate takes the shape it has. It goes into the crate's README, its rustdoc, or a `docs/` folder
  inside the crate.
- stays: the passage is about nife's use of the crate. It stays in this repository.
- gone: the passage narrates the code. [§310 (read a note against its
  code)](0310-read-a-note-against-its-code.md) settles those.

The lane finishes the moves pile before the split. A passage's home follows the side of the
boundary it describes: after the split, the crate's reasons and its reader sit in the new
repository, and nothing here can speak for them.

## How it sits beside the notes rule

[§310 (read a note against its code, and send a crate's documentation with the
crate)](0310-read-a-note-against-its-code.md) sorts one notes area against its code inside this
tree, in four piles. This section holds the ruling for the moment the boundary crosses: the sort
runs over every note and decision in nife that mentions the crate, and the piles are the ruling's
three. Its rule 4 states the same obligation for a crate that leaves. This section is that
obligation's own record, and the Follow-on below holds the step that enforces it.

[§201 (one roadmap until a citation has to
cross)](0201-one-roadmap-until-a-citation-has-to-cross.md) names this sort in its section on the
citation identity: when code moves out, the sort converts citations to rulings into reasoning
carried in the crate's own documentation.

## Follow-on

- **Recorded.** The ruling orders a step in `script/crate-standalone`: refuse a crate whose moves
  list is not done. The tree owes that step; nobody has built it. Until it exists, review checks
  the moves pile.

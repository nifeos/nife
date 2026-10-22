---
status: DECIDED
raised: 2026-10-09
decided: 2026-10-07
ratified_by: calef
---

# 267. A comment states the constraint as it is now

calef ruled the forks of `notes/comment-cost-2026-10-07.md` on
[PR #1800](https://github.com/nifeos/nife/pull/1800) (2026-10-07, UTC) and ratified fork (c) in the
sharper form, *"ratify #1800 Fork (c) in the sharper form"*. This section is the tree's copy of
that ruling. Every script and file name below is provisional.

The build is [milestone 860 (comments state the constraint as it is
now)](../roadmap/0860-comments-state-the-constraint-as-it-is-now.md): the gate, and the first
milestone's worth of the sweep.

## The ruling, as made

calef, 2026-10-07 (UTC), on what a Rust comment is for, in the sharper form he chose:

1. A comment states the constraint as it is now, in a sentence or two.
2. History ("this used to...", "found by milestone N...") goes in the commit message, where
   `git blame` finds it.
3. Semantics, wire layouts and rulings go in `design/decisions/`, cited by `§N`.
4. A cap of about 40 lines per comment block, held by a ratchet that only falls.

Points 1 through 3 say where each kind of sentence lives. The comment keeps the constraint; the
commit message keeps the history; a section keeps the semantics and the rulings, and the comment
cites it. A comment that cannot shrink to its constraint without losing a fact nobody has
recorded is a bug in where the fact lives. The sweep records the fact, in a note, a `BUGS` entry or a
section, rather than deleting it.

**Amended 2026-10-09 (UTC), calef, in session**: *"We do not need to capture the full history in
the tree. That's what source control is for."* and *"Lets remove the refusal records. They're in
history. We don't need to repeat them in document form. If we need them, which we are unlikely
to, then we can get them from the commit history."* History that the sweep lifts out of a comment
goes to the commit message and to git, not to a new document: the sweep does not transplant
narratives, refusal accounts or correction stories into notes or appendices. What still earns a
home in the tree is the rule, the ruling, the standing measurement and the current constraint.
The naming record's refusal archive keeps its own convention (§155 (the naming conventions move
out of the constitution); design/naming.md says where
it lives); this amendment ends the sweep adding to it.

A comment explains a constraint the code can't show, and never restates the next line. The
measurement that asked the question found that rule followed: a uniform sample of 30 blocks kept
24, cut 4 and moved 2. What the ruling adds is the cap on the long tail, and the placement rule for
the sentences that accumulate in place.

## Comments in the Linux style

calef, 2026-10-22 00:53 UTC, on [milestone 929 (comments in the Linux style across first-party
Rust)](../roadmap/0929-kernel-comments-in-the-linux-style.md) in the scope
[PR #2531](https://github.com/nifeos/nife/pull/2531) set: *"Consider 929 ruled."* The rule binds
all first-party Rust, every non-`vendor/` directory:

1. An item's doc comment opens with a summary sentence, then gives its arguments, what it holds and
   what it returns or refuses, and links the note that carries its design. A design argument goes
   in the note, not the doc comment. A `Context:` line (the locks it takes, whether it may run in
   interrupt context) appears only on code that holds locks or runs in interrupt context.
2. A comment inside a function body states a constraint the code cannot show: an ordering, a
   `SAFETY` argument, a hardware quirk. Narrative, worked examples and "how this works" go in the
   item's doc comment or the note.
3. An invariant a comment states as "the caller must..." becomes an assertion or a type where one
   fits.

Rustdoc takes the place of Linux's kernel-doc, and `missing_docs` and the `// SAFETY:` discipline
stay as they are. The newcomer test is met by the note a doc comment links to.

A name's provenance comment stays (calef, 2026-10-22 03:35 UTC: *"Approved assuming naming
provenance comments remain."*). The `Name:` block, with its dates, rulings and refusals, is where
[design/naming.md](../naming.md) ("Where a name's provenance lives") keeps a name's record. It is
not history for the commit message, and the paydown leaves it in place.

New code meets the rule everywhere now. The existing comments are paid down in stages, ordered by
effort and the merge queue rather than by exemption: `kernel/`; then `crates/` and `components/`;
then `system_tests/`, `xtask/` and every other directory. In the same order a directory's block cap
steps from 40 to 20 to 10, and a ceiling on plain comment lines inside function bodies starts for
it. The cap and ceiling in force for each directory are the ones
`helpers/comment_block_ratchet.py` holds.

## The cap: 40 lines

"A cap of about 40 lines" is calef's wording. The exact number is set from the measured
distribution, as the milestone's block promised, not chosen before it. Measured 2026-10-09 (UTC)
at `2d59ddde1`, with the same lexer and block definition the note measured with
(`notes/comment-cost-2026-10-07.md` section 4): 28,950 blocks, median 2 lines, mean 4.77. Blocks
over 40 lines hold 19.8% of comment lines (the note, one day earlier, measured 20%); over 30 holds
24.5%, over 50 holds 17.3%. Forty is where the long tail's bulk sits, and it is the number calef
named. A block of 40 lines is allowed; 41 is over.

The block is the note's unit: a run of adjacent comment lines of one kind, doc (`///`, `//!`) or
plain (`//`, `/* */`). A trailing comment, one that begins after code on its line, labels that line
and is not part of any block; its code line ends the run around it. Comment density itself is not capped and not the target; this tree comments
on purpose. The cost the note measured is the long tail of essays, findings and corrections that
accrete in place, not the median two-line explanation.

## The gate: a ratchet in script/lint

`helpers/comment_block_ratchet.py` holds the cap, and `design/comment-block-baseline.tsv` is its
list. Both copy §266 (a Rust source file stays under 2,000 lines)'s file-length ratchet, which
copies §212 (a prose budget)'s prose ratchet:

- No block outside the list may exceed 40 lines. A new file's blocks meet the cap outright.
- The blocks over the cap when the gate lands go on the list, one row each, with the size of that
  day. A listed block may not grow against the merge base and may not exceed its row.
- A row that no longer has an over-cap block behind it fails, so the list cannot go stale.
  `--bank` lowers rows to the tree and never raises one.
- A block that must stay over the cap needs this section amended, by an architect.

**Amended 2026-10-11 (UTC), calef, on [PR #1915](https://github.com/nifeos/nife/pull/1915)**:
*"Amend §267"*. He was ruling on the comment-block fork of milestone 845 (the filesystem-protocol
crate root is split along its seams). A row moves with its block when a file splits, at the same
ceiling, the way a rename already moves rows. No ceiling rises, the list still only falls, and a
moved block that grows still fails. The gate finds the move by content, the way git finds a
rename, rather than from a declared list. A listed block carries its row when it is gone from its
file and turns up in a file that had no rows at the merge base. Its lines must be unchanged or only
deleted; indentation is ignored. A grown or edited block, a copy that leaves the original behind,
and a second copy all fail as unlisted. `--bank` writes the carried rows. A split is a move and
nothing else, and this keeps it checkable as one.

Why a ceiling that only falls rather than exact match: §266 section 3's measurement (164 of 664
merges in a fortnight touching a listed file) argues it there, and holds one level down. The sweep
banks its shrinks rather than editing numbers by hand.

## The sweep

The list held 338 rows when the gate landed (2026-10-09, UTC). The sweep works through them per
file, worst first, one milestone's worth at a time. Each block over the cap either shrinks to its
constraint, with history to the commit message, findings to a note and semantics to a `§N`
citation, or stays listed until a later milestone's worth reaches it. Milestone 860 is the first
worth: the five blocks the note's samples judged "move" or tutorial.

## BUGS

- "About 40" is calef's number read off one measurement; 30 and 50 were not put to him, and either
  could be argued from the same distribution.
- A split carries a row only into a file with no rows at the merge base. A block moved into a
  file that already has listed blocks fails as unlisted; amend this section again if a split needs
  it. A block whose kind changes in the move (`///` to `//!`) is not the same block and does not
  carry.
- Rows pair with blocks in line order within a file. A listed block that leaves the middle of its
  file without being carried (cut to the cap in place, or moved and cut to it) shifts that pairing.
  The gate can then report a later, larger block as grown when it is not. This was found on
  2026-10-11 (UTC) while building the split carry, and it was on main before it. The fix is to pair
  by content within a file, as the carry does across files.
- The lexer is the note's hand-written one, adapted. It decides comment spans well enough for a
  gate, and a file it mis-lexes would be mis-measured; none has been found, and the note's own
  caveats apply.

---
status: BUILT
raised: 2026-10-08
built: 2026-10-09
promoted_from: comments-state-the-constraint-as-it-is-now
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 860. Comments state the constraint as it is now

*(The title and slug are provisional.)*

calef ruled the shape on 2026-10-07 (UTC), fork (c) of
`notes/comment-cost-2026-10-07.md` (PR #1800). This block is the sweep that ruling wants, which
nothing yet builds.

## The ruling, as made

calef, 2026-10-07 (UTC), on what a Rust comment is for, in the sharper form he chose:

1. A comment states the constraint as it is now, in a sentence or two.
2. History ("this used to...", "found by milestone N...") goes in the commit message, where
   `git blame` finds it.
3. Semantics, wire layouts and rulings go in `design/decisions/`, cited by `§N`.
4. A cap of about 40 lines per comment block, held by a ratchet that only falls.

The ruling is recorded on #1800 and nowhere else yet; minting its `design/decisions/` section is
part of this work. Its companion ruling (fork (b): code comments cite stable names, never a
`proposals/` path) already has its gate from that same pull request's thread.

Reuse: the block cap copies `helpers/prose_ratchet.py`'s pattern in-tree, as §266 (a Rust source
file stays under 2,000 lines)'s file-length ratchet does. No existing comment-block cap was found
outside the tree (rustfmt and clippy document none as of 2026-10-08), and the sweep itself is
prose movement, not code to take.

## Why a sweep is wanted

`notes/comment-cost-2026-10-07.md` measured the tree's comments on 2026-10-07 (UTC):

- 28,536 comment blocks, median 2 lines, mean 4.8. A uniform sample of 30, judged against
  `AGENTS.md`'s comment rule, kept 24, cut 4 and moved 2: the rule is being followed.
- The cost is the long tail, not the median. A length-weighted draw of 15 found blocks that are a
  legitimate explanation plus history and findings that accumulate in place, which is what
  "correct yourself loudly" produces when the correction is written at the site and never retired.
- Comment is 2.5 million tokens of a 4.3 million token tree.
- Comments went stale and were re-edited in 55 merged pull requests' worth of incidents: 24 where
  a fact moved under the comment, 13 of new content accreting, 8 of record status changes, 6 rename
  sweeps, 4 moved path references.

So the ordinary two-line comment stays. What moves is the block that tells a story: the history of
a removed `Unsupported`, a tutorial whose last sentence is its only constraint, a correction with
its motivation still attached after the code it corrected is gone.

## What the sweep does

Per file, worst first, one milestone's worth at a time:

1. Move history and findings out of comment blocks. The fact goes in a note if it is still a
   finding (`notes/`, cited by name), in the commit message if it is history, or is deleted if it
   restates a record that already exists.
2. Move semantics and rulings to a `§N` citation where a section exists; where the ruling is only
   in the comment, this sweep records it first, exactly as it stands, before deleting the comment.
3. Build the block cap as a lint ratchet that mirrors §212 (a prose budget)'s
   `helpers/prose_ratchet.py`: a committed list of blocks over the cap that only falls, a
   comparison against the merge base, and a selftest. Its shape is the same as §266 (a Rust source
   file stays under 2,000 lines)'s file-length ratchet, milestone 841 (a ratchet on Rust file
   length). The cap is about 40 lines; the exact number is set when the ratchet is built, from the
   measured distribution, not chosen before it.

A comment that cannot shrink to its constraint without losing a fact nobody has recorded is a bug
in where the fact lives, and the sweep records the fact rather than deleting it.

## What it refuses

- No rewrite of the 80% that the sample kept. The rule for ordinary comments does not change.
- No split of files along the way; that is milestone 840 (the scheduler file is split along its
  seams), not this.
- No new prose beyond the notes the findings move into; §213 (writing standards) governs them and
  the prose ratchet holds them from the day they are written.

## Evidence

Everything measured above is in `notes/comment-cost-2026-10-07.md`, including the scripts that
took each number (`notes/comment-cost-2026-10-07/`). The kept/moved judgments of the thirty-block
sample are listed block by block there.

## Follow-on

- **Decision.** `design/decisions/0267-a-comment-states-the-constraint-as-it-is-now.md` records
  the ruling this block files, including the cap and the ratchet; the number is provisional until
  the merge queue lands it.
- **Milestone 862.** Milestone 862 (the comment-block sweep's next worth): the 337
  over-cap blocks the baseline still holds (login.rs's and rmle.rs's shrunken docs among them,
  at 178 and 64), swept a milestone's worth at a time, worst first.

## Index row

Comments state the constraint as it is now: the sweep moves history to commit messages and findings to notes, cites rulings by section number, and a ratchet caps comment blocks at about 40 lines. The cost being paid is measured: comment is 2.5 million tokens of a 4.3 million token tree.

## Built, 2026-10-09 (UTC)

One milestone's worth, by lane `milestone/860-comments-state-the-constraint-as-it-is-now`:

- **§267 (a comment states the constraint as it is now)**, the decisions section recording calef's
  #1800 fork (c) ruling in the sharper form, with the cap and the ratchet as this milestone's
  gate. The section number is provisional until the merge queue lands it.
- **The ratchet**: `helpers/comment_block_ratchet.py` (name provisional), mirroring §266's
  file-length ratchet. Same verbs: a baseline that only falls, a merge-base comparison, a
  selftest that runs before the check, a one-time `--init`. The note's own lexer is adapted in,
  so the gate and the measurement read the same tree. Baseline
  `design/comment-block-baseline.tsv` (provisional). Wired into `script/lint` beside the
  file-length ratchet, text-only, no-cargo.
- **The cap, measured** at base `2d59ddde1`: 28,950 blocks, median 2 lines, mean 4.77; blocks
  over 40 hold 19.8% of comment lines (the note measured 20% a day earlier), over 30 holds
  24.5%, over 50 holds 17.3%. 40 is where the tail's bulk sits, and the number calef named.
  338 rows at `--init`; 337 after this worth's sweep.
- **The sweep, the five blocks the note's samples judged "move" or tutorial:**
  - `components/src/login.rs:1`, 689 lines to 178. The design argument and history moved to
    `notes/login.md` and two new appendices, `notes/login/teardown-and-channels-history.md` and
    `notes/login/boot-wiring-history.md` (beside a new `notes/login/README.md`), marked with
    dated sections; the module doc keeps the constraints, the contract table, the Name block and
    the living BUGS. `notes/login.md` was also corrected: its subtree, example and name-status
    sections predated §117 (a principal's subtree is named by its identity string), channel-per-client and the 2026-09-15 ratification.
  - `components/src/rmle.rs:1`: the naming search moved to
    `design/naming/rmle-name-search.md` (a new appendix, indexed from `design/naming.md`); the
    Name block keeps the ruling and a pointer.
  - `components/src/uptime.rs:1`: the finding moved to `notes/process-view.md`; the doc keeps
    the constraint and a pointer.
  - `kernel/src/arch/x86_64/exceptions.rs:472`: the "TWO ways into the kernel" banner cut to its
    constraint; the sentences that carry it already live on the items below.
  - `std_exerciser/src/main.rs:1016`: the `remove_dir_all` history dropped (it restates records
    in `design/roadmap/0122-a-directory-handle-std-can-hold.md` and
    `notes/std/fs-descent.md`, and the history is in the lane's commit message); the living
    constraint stays.

What remains for later milestones' worth: 337 over-cap blocks the baseline holds, swept a
milestone's worth at a time, worst first. The next worst after this worth:
`kernel/src/testing.rs:240` (468), `crates/system_initializer/src/lib.rs:6` (406),
`components/src/timetable.rs:1` (224), `crates/jh7110_entropy/src/lib.rs:2` (209).

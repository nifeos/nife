---
status: NOT-STARTED
raised: 2026-09-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 333. The same stale claim, in the code and the notes, where no roadmap gate reads it

Filed 2026-09-03 as an unnumbered proposal by the milestone 252 sweep;
numbered 2026-09-19 by milestone 433. **Premise re-checked item by item on 2026-09-19: eight of the
ten still stand, one is closed, and one could not be confirmed.** Still wrong today:
`crates/capability/src/lib.rs` (the `ENUMERATE` rustdoc's "when `pmap` is built", and `crates/pmap`
and `components/src/pmap.rs` both exist). `notes/crates-io-on-nife.md` rows 19 and 28 (against
`filesystem_contract`'s `GETMTIME`, `SETMTIME` and `SETMTIME_AT`, opcodes 20 to 22).
`design/decisions/0102-frame-names-a-run.md` line 7 ("this decision is still unbuilt").
`notes/live-replacement.md` line 261. `notes/register-of-measures.md` line 253.
`close_cycle_counter_to_el0` in `kernel/src/arch/aarch64/timer.rs`. `components/src/session_reviver.rs` (eight surviving mentions
of the deleted durable-session type). And milestone 142 (a text display good enough that people use
it)'s duplicated paragraphs. Closed: the
`crates/abi` start invocation, whose doc now says `START` spends all three argument registers rather
than ignoring them. Unconfirmed: the display-name item, because milestone 126's block still
carries it under "Fork: a process display name" with a recommendation rather than a ruling, so
`notes/process-view.md` may be right and the sweep's reading of 126 may be the error. A lane taking
this re-reads that one rather than trusting either record.

Every item is a comment or a note, each one verifiable by reading the file beside
it.

In brief. Milestone 252 swept 22 `PARTIAL` blocks and corrected them. The blocks were rarely
the only place the wrong sentence lived. A stale claim that had been copied into a rustdoc, a
module header or a note stayed wrong after the block was fixed, and nothing in the tree reads those.
This is the list, so that fixing the roadmap does not leave the tree quietly disagreeing with it.

## The list

- `crates/capability`'s `ENUMERATE` rustdoc says only the rendezvous object consults the right
  and that the address space will "when `pmap` is built". `pmap` was built 2026-08-23 and
  `kernel/src/syscall.rs`'s address-space list arm checks the right today. The same sentence sits in
  a comment on that file's retype arm.
- `notes/process-view.md` still records the display-name authority question as undecided, which
  milestone 126's block answered on 2026-08-26.
- `notes/crates-io-on-nife.md` rows 19 and 28 still say no verb reports or sets an mtime.
  `crates/filesystem_contract` has carried all three verbs since §112 (`touch`'s two behaviors need two rights: write covers "now", a separate right covers "arbitrary") was decided on 2026-08-23, and the
  PAL's own comments under `patches/std-nife/overlay/` repeat the dead claim a third time.
- `crates/abi`'s start invocation is documented as ignoring its three arguments, which both the
  kernel's own arm and the builder's call contradict. Found by milestone 139's round 7 and left for
  whoever next touched the file.
- `design/decisions/0102-frame-names-a-run.md` says the decision is still unbuilt and quotes
  milestone 142's block saying nobody is building it. It is built: the page-frame object carries a
  count and the compositor sizes a scanout over 311 frames.
- `notes/live-replacement.md` says the interactive stack is not running under the test harness,
  which three kernel test files disprove, and milestone 177's block quotes the stale sentence
  approvingly.
- `notes/register-of-measures.md` still names milestone 75's authority question as open, after
  §139 decided it and milestones 229 and 237 built it.
- `kernel/src/arch/aarch64/timer.rs` asserts that nothing in this tree reads the cycle counter
  from EL0, which a test in `kernel/src/user/tests.rs` does.
- `components/src/session_reviver.rs`'s module doc and six inline comments cite a durable-session type
  that was deleted with the SMB implementation on 2026-08-30.
- **`design/roadmap/0142-a-text-display-worth-living-in.md`** carries sixty lines of verbatim
  duplicated text, three paragraphs pasted twice during its 2026-08-27 edit.

## Why this matters

`script/roadmap --check` reads paths cited in a `## Follow-on` bullet and nothing else, so a claim
in a rustdoc is invisible to every gate this project has. Milestone 247's note already records the
same shape: a `Recorded.` bullet citing a path is the only reason the `crates/regions` rename rot was
ever found. These are the claims a reader meets first, because a reader in the code does not open the
roadmap.

## BUGS

- A list like this is a snapshot and it starts rotting the day it is written. It is a worklist,
  not a mechanism, and the mechanism question (what would have caught these) is genuinely open: none
  of them is greppable in the way a path citation is.
- **It is not exhaustive.** The sweep read 22 blocks and the code each one pointed at. A claim in a
  file no `PARTIAL` block happens to cite was never looked at.

## Index row

Milestone 252 swept 22 `PARTIAL` blocks and corrected them, and the blocks were rarely the only
place the wrong sentence lived. A stale claim copied into a rustdoc, a module header or a note stayed
wrong after the block was fixed, and nothing in the tree reads those. `script/roadmap --check` reads
paths cited in a `## Follow-on` bullet and nothing else, so a claim in a rustdoc is invisible to every
gate this project has. These are the claims a reader meets first, because a reader in the code
does not open the roadmap. This is the list, verified again on 2026-09-19, so that fixing the roadmap
does not leave the tree quietly disagreeing with it. It is a worklist rather than a mechanism, it
starts rotting the day it is written, and it is not exhaustive: a claim in a file no `PARTIAL` block
happens to cite was never looked at.

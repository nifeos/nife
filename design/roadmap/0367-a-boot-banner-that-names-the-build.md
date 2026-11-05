---
status: NOT-STARTED
raised: 2026-09-04
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 367. A boot banner that names the build, so two cards cannot be confused for one

Checked against the tree on 2026-09-19, both halves hold. `script/board-image` still echoes
`features: ...` at build time (line 253), which is the rung-four mitigation this file describes.
`print_machine_description` in `kernel/src/main.rs` still prints exception level, stack top, memory,
initrd and the rest with no line naming the feature set, so nothing about which build booted reaches
the capture.

Small, and it makes a class of wasted bench session impossible rather than unlikely.

## In brief

E3 is a comparison of two kernels that differ in exactly one Cargo feature. Both are written to a
microSD card as the same three filenames, and neither prints its feature set. Nothing on the card or
in the capture says which one booted. A session that writes the second card and forgets to relabel
its log has compared a build against itself, and **the capture is indistinguishable from a correct
one**.

The same shape applies to every card this tree writes: `--soak`, `--job-mix`, `--reboot`, `--bench`,
and the plain tour all produce `nife-vf2.img`.

## The mitigation today, and why it is not enough

`script/board-image` echoes `features: board,bench,single_hart` at build time (added by this lane),
and the operator names the log file. That is **rung four** of AGENTS.md's ladder, a note in a place
someone has to have read, and it is guarding a decision that is made twenty minutes and one power
cycle later.

## What it would be

The kernel prints its own feature set, once, in the same breath as the banner it already prints, so
the fact lives in the capture rather than beside it. Rung three: a written record at the thing
itself. Roughly `env!` over the enabled features at compile time, or a small `const` list assembled
from `cfg!`s; the choice between them is exactly the sort of thing that is cheaper to decide with
the code open than in a proposal.

Two properties worth holding it to. It should cost an ordinary boot nothing but one line. And it
should name the whole feature set rather than a curated list, because the next feature somebody
adds is the one that will be missing from a curated one.

## What it does not fix

A card written from an uncommitted working tree still says nothing about the commit. The stronger
version prints a build identity as well, which is a bigger decision (a reproducibility claim rather
than a debugging aid) and should not ride along on this one.

## Index row

E3 compares two kernels that differ in one Cargo feature. Both are written to a microSD card under
the same filenames, neither prints its feature set, and nothing in the capture says which booted.
A session that forgets to relabel its log has compared a build against itself, and the capture looks
correct. The same holds for every card this tree writes: `--soak`, `--job-mix`, `--reboot`,
`--bench` and the plain tour all produce `nife-vf2.img`. The fix is the kernel printing its whole
feature set once, beside the banner it already prints, so the fact lives in the capture. Hold it to
one line on an ordinary boot, and to the whole set rather than a curated list. A build identity is
the stronger version and a different decision.

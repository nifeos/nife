---
status: BUILT
raised: 2026-09-17
built: 2026-10-15
promoted_from: the-documentation-sweep-the-worklist-already-ranks
---
# 427. The documentation sweep the worklist already ranks

The documentation audit `script/audits` had reported due on every count trigger, run 2026-10-15
(UTC) at base `8fad33799` under the staleness worklist's top-six lens. The report is
[design/audit-reports/2026-10-15-the-worklists-top-six.md](../audit-reports/2026-10-15-the-worklists-top-six.md);
this block is the account of building it, not a second copy of it.

## What it found

Nine findings, all fixed in the lane; nothing minted, nothing accepted.

Four documents said today what a base held. The comment-cost note's density figures, re-measured
with its own script: 54.6% of Rust bytes are comment, not 57%, and the kernel runs 0.86 comment lines
per code line, not 0.92. Its 481 KB `sched.rs` exhibit, the file no agent could read whole, is
an index now, split into `kernel/src/sched/`. The shared-page audit's bold sentence claimed a live
privilege-escalation absence "on `main` today"; it holds its own day, with the four security
audits since named as the current word, and its absent inbound socket half has since landed. The
0407 proposal's counter-example moved crates with the system tests and still re-declares the three
role numbers, so the argument stands at its new address. And the core-harnesses appendix held
counts from an older tree.

Two documents pointed where the scheduler split had moved the thing: `IRQ_ROUTES` and the
work-steal slot live in `sched/interrupt_route.rs` and `sched/placement.rs`, and the IPC
functions the verification appendix describes are in `sched/rendezvous.rs`. The 0902 helpers
appendix lost four of its own rows' subjects to basalt three days after its snapshot, and says so
with a dated note.

Two mechanisms came out of it. Five of the core-harnesses appendix's nine harness counts were
wrong (8 to 14, 2 to 4, 8 to 37, 6 to 15, 3 to 4). All nine counts are corrected and are now
counted claims, with one derivation per crate in `script/lint`'s registry, so the next harness
lands with its number or not at all. And the ABI-constants trigger had gone blind to most of the
ABI: the derivation read only `crates/moho_abi/src/lib.rs`, where 6 of the crate's 95 constants now
live, so `script/audits` walks the crate's `src/`. The drop that fired this sweep's own trigger
was that blindness, not a shrinking surface.

## What it cost

Reading: six documents whole, and the greps and re-measures behind the findings. The only code
changed is `script/audits`' derivation and `script/lint`'s registry, both host-side; no boots.

## Follow-on

- **Milestone 622.** The fence re-adjudication, more owed than when it was minted: the gate counts
  23 fences against the 13 `notes/memory-ordering.md`'s table adjudicates.
- **Milestone 383.** The backticked-path gate; the nine dead proposal-path citations this sweep
  re-counted are its exact population.
- **Milestone 407.** Its mechanical sweep is still unstarted; this sweep re-verified its premise's
  counter-example at the address it moved to.
- **Recorded.** The baseline discontinuity finding 9 leaves: this audit's ABI cell is the first
  taken under the crate-walking derivation, and the report records it where the next delta's
  reader meets it.
- **Done.** The report, both index rows in `design/audit-reports/README.md`, and this block, by
  this lane. `script/audits --due` stops naming `documentation` on its own.

## Index row

The documentation audit every count trigger had fired on, run under the worklist's top-six lens at
base `8fad33799`. Nine findings, nine fixes, in the lane. Four documents claimed the present for a
base: density figures re-measured with the note's own script, a security sentence held to its own
day, a socket half noted landed, a counter-example followed into the crate it moved to. Two
documents were pointed at files the scheduler split had moved. A tooling appendix whose rows lost
their subjects to basalt three days after its snapshot got a dated note. Five wrong harness counts
in the verification appendix were corrected and all nine converted into counted claims with
derivations in `script/lint`'s registry, so the class cannot rot again. The ABI-constants
trigger's derivation, which read only `crates/moho_abi/src/lib.rs` while the crate grew per-object
modules and held 95 constants where the counter saw 6, was made to walk the crate; this audit's
baseline row is the first taken under it. What it left: milestone 622 (the fence inventory is
re-adjudicated) owes more than when it was minted, 23 fences against the 13 adjudicated.
Milestone 383 (a gate for a backticked in-tree path that does not resolve) is the exact
population the nine dead proposal-path citations keep feeding. Milestone 407 (one definition
of the numbers a kernel test and its program agree on) is still unstarted.

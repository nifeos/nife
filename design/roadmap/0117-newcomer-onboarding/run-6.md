# Milestone 117 appendix: run 6 and the three items it closed

*An appendix to [milestone 117 (the stranger test)](../0117-newcomer-onboarding.md), moved out of
that page on 2026-10-12 (UTC) under [§212 (a prose
budget)](../../decisions/0212-a-prose-budget-for-every-document.md). The main page is complete
without it, and carries why run 6 made the block `BUILT`. Run 6's full record is
[notes/stranger-test/run-6.md](../../../notes/stranger-test/run-6.md); this page is the lane's
account of it, written 2026-09-19.*

## Run 6's headline results

- Milestone 150 (adding a program should not need eight hand-maintained lists) worked on first
  contact. The stranger added a program through the one-place declaration the day it landed. Every
  edit the page names was demanded by the compiler or a named host test, and nobody nominated
  "eight hand-maintained lists" for the first time in four runs.
- The largest cost was a confident wrong diagnosis the tree invited. The stranger saw an
  intermittent `uefi-test` red after a passing suite, two expected VT-d fault lines printed just
  before it, and a documented QEMU version gap. Those led it to build and install the pinned QEMU
  into an account-wide prefix, which would have changed the emulator under two other lanes. The
  operator's re-run passed on the same QEMU. Homed in notes/x86-uefi-boot.md's and notes/qemu.md's
  `BUGS`, and in `design/roadmap/0516-uefi-test-exits-one-after-a-passing-suite.md`.
- The instrument changed under it. The CLI's default model refused the task three times out
  of three, so run 6 ran on `claude-sonnet-5`. The stranger ended its session waiting on
  background work and wrote no write-up. And `script/lint` in the clone announced the withheld
  answer key. All in `script/stranger-test`'s `BUGS`; the harness now stops rather than debrief a
  refused session.
- M2 is absent for the fourth run in six, `notes/net.md` still unopened, and `script/apropos`
  still never run. That was the tree it was handed: the placement below is the change, and run 7
  is the measurement.

## The three items the block's follow-on carried, closed

1. Run 6: conducted, scored, recorded, every finding given a home (fixed here, a `BUGS` entry
   at the feature, or a proposal). Four were fixed here because each was a sentence a stranger
   believed. They were `CONTRIBUTING.md`'s two-ISA claim in five places, `README.md`'s claim that
   QEMU is pinned exactly on every machine, and the missing warning that a bare `cargo build` fails
   on the host. The fourth was `script/stranger-test` debriefing a refused session.
2. `script/apropos` placed, in `README.md` directly after the start-here order and in
   `CONTRIBUTING.md` beside its pointer to that order. Each carries an invocation checked against
   the tree (`system_call` reaches `crates/moho_abi/src/lib.rs`, `socket` reaches `notes/net.md`). Placing
   it found that it exited 1 on every search, because six filenames had grown past the guest
   record's 72-byte path; it now fails only when a result it printed is truncated. Its `BUGS` also
   said ranking was occurrence count, and it is density.
3. `script/names` counts one worklist. The gate line said `recorded + unrecorded` (28) beside
   a pointer to `--unratified`, which lists 69; the census lines left `provisional` out and so did
   not sum. The three-category number is right, because `--unratified` sorts provisional names
   first as the shortest conversation calef can have, and design/naming.md defines the worklist as
   that listing. Refusals stay at 273. design/naming.md's `BUGS` entry is gone and its captures say
   what changed.

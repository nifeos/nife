---
status: BUILT
raised: 2026-10-04
built: 2026-10-04
milestone_dependencies: 191
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 741. Does a standing proof notice a regression?

Raised 2026-10-04 (UTC) by calef for fatal risk 2 (the proofs prove trivia, and the real bugs live
where Kani cannot reach). Milestone number provisional; the maintainer mints it at merge. *(Title and
slug are drafts.)*

The red half of risk 2's amber is that no standing proof has caught a regression: every catch in
milestone 191 (did the proofs catch the bugs?) happened while a harness was being written. The
defects that escaped most recently were tests and gates that could not fail, so this asks whether
the proofs are in the same class, by measurement rather than by reading.

**The question:** of the code a standing Kani harness covers, which cargo-mutants mutants does a
proof kill, which do only the tests kill, and which does nothing kill? Answered per harness as
killed / total mutants in the functions it reaches.

## Done when

- `helpers/kani_reach.py` (name provisional) plans, proves and reports one package, and
  `.github/workflows/kani-reach.yml` runs it sharded in CI.
- A pilot (nifefs, elf, and the kernel's aarch64 harnesses) is reported before scaling.
- Every crate in `script/verify`'s table is measured, or the measured cost says why not.
- The numbers, per harness, live in `notes/kani-reach-2026-10-04.md`. The line for risk 2's section
  goes to the maintainer, who owns `design/fatal-risks/`.

## Result

Built 2026-10-04 (UTC). 189 harnesses in 26 packages were measured, for 30.0 Kani CPU-hours over
runs 37168513701, 37179003058 and 37219544157. 178 of the 184 harnesses that reach a viable mutant
kill at least one. Together they kill 1,668 of 2,626 (64%), where the census tests catch 91%. Six
harnesses reach mutants and kill none. Five reach no mutant at all; four of those prove `unsafe fn`s,
which cargo-mutants never mutates. 17 mutants are killed by a proof alone, and 162 by nothing.
glob and calendar were left out on a measured cost estimate. Everything is in
[`notes/kani-reach-2026-10-04.md`](../../notes/kani-reach-2026-10-04.md).

## Index row

Mutation-tests the proofs themselves: every cargo-mutants mutant in a standing Kani harness's reach
is proved, so a harness no regression could turn red gets named. Of the 184 harnesses that reach a
mutant, 178 can fail and 6 cannot. That puts a number per harness on the survivorship half of fatal
risk 2's amber.

## Follow-on

- **Milestone 782.** Milestone 782 (measure the reach of the glob proofs). `design/roadmap/0782-measure-the-reach-of-the-glob-and-calendar-proofs.md`.
- **Milestone 780.** Milestone 780 (give the zero-kill harnesses a property a wrong value breaks). `design/roadmap/0780-give-the-zero-kill-harnesses-a-property-a-wrong-value-breaks.md`.
- **Milestone 776.** Milestone 776 (a weekly check that every proof can still fail). `design/roadmap/0776-a-weekly-check-that-every-proof-can-still-fail.md`.
- **Recorded.** The `unsafe fn` blind spot, the reach approximation and the unpinned Kani, in
  `helpers/kani_reach.py`'s BUGS.

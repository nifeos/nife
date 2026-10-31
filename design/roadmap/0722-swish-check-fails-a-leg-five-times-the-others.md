---
status: BUILT
raised: 2026-10-03
built: 2026-10-03
promoted_from: swish-check-fails-a-leg-five-times-the-others
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 722. swish-check fails a leg that costs five times the others per line

Raised 2026-10-03 (UTC) by the maintainer session writing
the merge-rate correction (`notes/coes/2026-10-03-the-merge-rate.md`, on PR #1513's branch until it lands). Name provisional,
this file's alone.

## Why

Behavior across architectures is gated by §19 (architectural parity is a tenet); nothing gates parity of cost. The
x86_64 leg of swish-check cost 7.7 s a line against aarch64's 0.19 s, about 40 times, for two weeks.
The cause was a kernel scheduling defect, fixed by #1487, milestone 628 (the x86_64 swish-check leg
costs what the others do). A per-line ratio is a better tripwire than the job's wall time for this
class, because it does not move when lines are added and it points at the architecture.

## What to build

- swish-check already times each line. Each leg reports its median seconds per line.
- The check fails when one leg's figure is over five times the median of all legs.
- Compare like with like. After #1487 the x86_64 leg runs under KVM and the others under TCG, and a
  local x86_64 TCG run still costs 2.85 s a line. So the rule compares legs under the same
  accelerator, or each leg against its own committed baseline when no peer shares it.

## What shipped

`cargo xtask swish-check` prints each boot's median seconds a line next to a baseline row for its
architecture and accelerator, and fails the leg when the median is over five times that row. The
rule is `leg_cost_verdict` in `xtask/src/swish_check.rs`, pure and tested on the host. The 628
defect under KVM (7.7 s against 0.26 s) fails. One slow line among 142 does not move a median, and a
boot of fewer than 20 lines is reported and not judged.

One departure from the proposal, with its reason. It asked for five times the median of all legs
under the same accelerator, or a committed baseline when no peer shares one. In CI no two legs share
KVM, and the legs that share TCG differ by design: x86_64 under TCG costs about fourteen times
aarch64 under TCG, so a peer rule fails it on every Mac. A leg is therefore held to its own row, which
is the proposal's fallback made the rule. Each row cites the CI run it was read from.

## BUGS

- Under TCG the 628 defect would pass: it ran 2.7 times the x86_64 TCG row. Only the 45 s per-line
  bound sees it there. KVM, where CI runs the leg, is where it fails.
- The rows are means read from two CI runs, because the job printed a mean until this change. The
  first runs that print medians should be compared with the rows and the rows tightened if they sit
  far below.
- A faster or slower runner pool moves every row. The failure message says to measure and add a row.
- The check runs inside the boot, so a leg that fails it reports a cost defect and stops the rest of
  that boot's transcript checks. That is deliberate, since a cost fault needs no further evidence.
- **An echo assertion failed once on the riscv64 leg and passed on a fresh run.** Seen on #2945
  on 2026-10-31 (UTC); GitHub refused a rerun of the failed run, so the failure was not reproduced.
  It may be the echo-wait signature that `notes/swish-check-flake.md`
  measures, or a new one; nothing ties it to either. A recurrence names the line, the run and the
  leg in that note's table. No consumer waits on this; it is recorded so a second sighting counts
  against a first.

## Follow-on

- **Recorded.** Tightening the rows once medians exist, in this block's BUGS; the numbers live in
  `LEG_COST_BASELINE`.

## Index row

BUILT on `milestone/722-leg-cost` (PR #1535). swish-check fails any architecture leg whose per-line time is over five times the median leg's under the same accelerator; x86_64 ran at about 40 times for two weeks unnoticed.

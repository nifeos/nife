# The confirmation run, 2026-08-22

*(An appendix of [notes/load-sensitive-assertions.md](../load-sensitive-assertions.md), which holds
the register and the rules. Moved here on 2026-09-24 and tightened; pull request #1211's first
commit has the text verbatim.)*

## The confirmation run, 2026-08-22: 45 of 45 green, the block closes

Milestone 62 (tests that assert on time) named exactly one thing left after
[the 2026-08-18 disposition](timer-assertion-disposition.md). It needed a repeat count under load of
the tree as it now stands, at the 45-run standard [the first acceptance run](first-loaded-acceptance-run.md)
set. The block's own BUGS section says a green run cannot show a flake fixed, and the same rule
applies to the change that removed one. This is that run.

`script/repeat-under-load -n 45 -s 8`: the same recipe and the same host as 2026-08-17, on the tree
with both the assertion disposition and [the migration-drain fix](migration-drain-tick-budget.md)
in place.

- Host: Mac15,3, 8 cores, Darwin 25.6.0 arm64. Tree `50a0e7cb`.
- 45 runs, 112 minutes of wall clock, 2026-08-23T00:57:46Z to 2026-08-23T02:49:34Z.
- One-minute load average across the whole loop: 4.8 low, 90.1 peak (684 samples at 10 s cadence).
  Both ends are outside the 2026-08-17 run's 26.1 to 63.0 band. The low end catches a run before its
  spinners had ramped the average up. The high end is harsher than anything the first acceptance
  run saw, with three separate runs touching 90.
- A few runs shared the host with a neighboring QEMU (peak QEMUs seen reached 3 in run 14, and 2 in
  five others); most ran alone. That is recorded rather than smoothed over, per the first run's
  convention.

The result: 45 of 45 green. `ticks_arrive_at_the_configured_rate` ran on both ISA legs in every run
(90 legs total) and never printed `UNMEASURED`. The retry budget found a miss-free window on its
first attempt every time at this load. `the_handler_keeps_up_when_no_lock_is_held` is gone from the
suite, so it contributed neither a red nor a false pass. `a_migrated_kernel_thread_keeps_its_hart_pointer`,
the migration-drain fix's own target, was green in all 45 runs. No other assertion went red.

This is stronger than the 2026-08-18 interim run's numbers predicted, and the honest reading is that
18 runs was too small a sample to bet on. That run saw `UNMEASURED` on 9 of 36 legs (25%). It
flagged the rate itself as the thing that wanted a bigger count before anyone acted on it. Forty-five
runs, 90 legs of the same assertion, found zero. Both are true statements about different sample
sizes. The second is the one the block's acceptance standard asks for.

What this closes: the one remaining item of design/roadmap/0062-time-sensitive-tests.md. The repeat
count under load, at the block's own standard, on the current tree, is 45 of 45. The block moves to
BUILT.

What it does not close. Both caveats were on record before this run.

- One host is one host. Nothing here says anything about a GitHub Actions runner, a different QEMU
  build, or a load shape this recipe does not produce. That was true of the 2026-08-17 run and stays
  true of this one. It is the standard the block set, not a wider claim.
- Zero red in a finite sample bounds a rate; it does not prove one impossible. By the rule of three,
  0 failures in 45 full-suite runs is consistent with a true failure rate as high as roughly 3/45
  (about 6.7%) at 95% confidence. Zero `UNMEASURED` results in 90 legs of the specific assertion
  bounds that narrower quantity at roughly 3/90 (about 3.3%). Both are compatible with the
  2026-08-17 run's observed 9-of-90-leg rate, if the disposition lowered the rate rather than
  removing the failure mode. Both are also compatible with a true rate near zero. Only more runs, or
  a different host, would narrow it. Nothing in the milestone's acceptance standard asks for that: 45
  was the number the first run set, and this run matched it.

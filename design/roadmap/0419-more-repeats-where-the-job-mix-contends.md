---
status: BUILT
raised: 2026-09-16
built: 2026-09-19
promoted_from: more-repeats-where-the-job-mix-contends
---
# 419. `tasks=4` needs more repeats, not more power cycles, and `REPEATS` is one constant for the whole sweep

Built 2026-09-19, by milestone 168's own lane, in a different session and on the same
day this block was numbered and its decision written up. What landed is options 1 and 3 together
from the list below: a uniform `REPEATS` of 21 for the whole sweep, and the median reported with the
minimum and maximum beside it. Option 2, a per-point table, was refused on measured board time:
at 21 repeats the timed windows are a small share of a boot, so varying the count per point buys
nothing it costs. Promoted from the proposal `more-repeats-where-the-job-mix-contends`, filed
2026-09-16 by the maintainer from the five-boot job-mix session on radon, where the fourth and fifth
boots each landed outside the range the first three had established. *(Number provisional until the
merge queue lands it.)*

Its `DECISION` gate was answered by a build rather than by calef, and that is worth saying
plainly. The gate was right: `job_mix::REPEATS` decides how long every bench evening takes on
every board, and the `job-mix-repeat:` line is output that two programs read (`script/job-mix`'s
rehearsal and `crates/serial_console`'s recognizer). The lane that built it had the change assigned
in its maintainer's brief, noted that both readers are in-tree, and recorded the choice in milestone
168 so it is visible rather than implied. So the question is settled in the tree, and it was not
settled by the person whose call it was.
[§191](../decisions/0191-job-mix-repeats-and-what-the-line-reports.md) stays `PROPOSED` for exactly
that reason: it is now a ratification or an overrule rather than an open fork, and the difference
between those two is not a lane's to erase.

*(Updated 2026-09-26.)* calef ratified what shipped on 2026-09-26 (about 01:30 UTC): *"Yes, ratify
§191 as shipped."* §191 (whether the job mix reports the spread rather than the best) is `DECIDED`, so the paragraph above describes a question that is now
closed, and it was closed by the person whose call it was.

Premise re-checked 2026-09-19 and still true. *(That check predates commit `ab6ff0fa0` the
same day; `REPEATS` has been 21 since, at `crates/job_mix/src/lib.rs:465`.)* `job_mix::REPEATS` is still `3`, one constant for
the whole sweep, and milestone 168 is still `PARTIAL` for the reason this block names. Its status
line says it does not turn `BUILT` until `tasks=4` has a number, and it still does not have one. The
interim this block recommends, option 4, is what 168's block now does.

In brief. Milestone 168's sweep reports the best of three repeats per sweep point. At
`tasks=4` the underlying distribution is wide enough that the best of three is itself a coin flip.
Across five boots of an identical image the reported figure ranged from 766,361 to 991,671 jobs per
minute, a 29.4% spread. Boot 3's three repeats *on their own* spanned 132,148 to 181,408
ticks, which contains the whole boot-to-boot range. The variance is within a boot, so power cycling
does not reduce it.

`tasks=4` is where `job_mix::ECHO_SERVERS = 2` first produces contention (2:1) with too few samples
to average it. That contention is deliberate and correct; what is missing is enough samples at the
point it bites.

## Why it is a decision rather than a patch

**The cost is not symmetric across sweep points.** `tasks=32` already takes about 930,000 ticks per
repeat and is stable at 2.7%; tripling its repeats buys nothing and lengthens every board session
noticeably. `tasks=4` takes about 150,000 ticks and is the one that needs them. So the obvious fix
is not "raise `REPEATS`" but "vary repeats by sweep point", and that turns one constant into a
table, which is a different thing for a reader to hold and for `board_console` to recognize.

**And a wide distribution may not want a minimum at all.** `REPEATS`' doc states the rule this tree
uses: the minimum is the least host-contended sample and everything above it is somebody else's
load. That reasoning is sound for a micro-benchmark on a busy host. It is questionable for a
workload whose *whole subject* is contention between its own tasks: there, the spread is the signal
rather than noise to be minimized away, and reporting only the best discards it.

## The options

1. Raise `REPEATS` uniformly, to 5 or 7. Simplest, one constant, no format change. Costs the
   most board time, and spends it mostly where it is not needed.
2. A per-sweep-point repeat table. Cheap in board time and targets the problem. Costs a
   constant becoming a table, and `board_console`'s recognizer has to stop assuming a fixed count.
3. Keep three repeats and report the spread rather than the best, so the line carries min, max
   and median. No extra board time at all, and it makes the instability visible rather than
   averaged away, which is arguably what a multi-tasking benchmark should publish. Costs a wire
   format change to the `job-mix:` line, which is the most expensive thing on this list by
   AGENTS.md's *move fast on what can be undone* test.
4. **Do nothing and record `tasks=4` as a range.** Free, honest, and leaves milestone 168 `PARTIAL`
   for a reason nobody can close without one of the above.

**Recommendation: 3, with 4 as the interim**, and the reason is the one this proposal opened with.
The job mix exists to measure what contention costs; a report that keeps only the least-contended
sample is answering a different question than the one §96 asked. But option 3 changes a line two
programs read, so it is an architect's rather than a lane's. Until it is decided the honest
thing is option 4, which is what the block of milestone 168 (a multi-tasking workload benchmark) now
does.

**Blocked until it is answered:** milestone 168 turning `BUILT`. Its own status line says it does
not, until a number exists, and `tasks=4` does not yet have one.

## Follow-on

- **Milestone 168.** Where the work landed: *"What changed on 2026-09-19"*, now the appendix
  [`0168-multitasking-benchmark/holes-closed-2026-09-19.md`](0168-multitasking-benchmark/holes-closed-2026-09-19.md), with the resampling
  evidence for 21 and the old-and-new line formats in `notes/job-mix.md`.
- **Decision.** [`design/decisions/0191-job-mix-repeats-and-what-the-line-reports.md`](../decisions/0191-job-mix-repeats-and-what-the-line-reports.md),
  still `PROPOSED`, and now asking calef to ratify or overrule what shipped rather than to choose
  from four options.
- **Refused.** *Option 2, a per-point repeat table.* Refused on measured board time rather than on
  taste: the proposal assumed board time was the cost and it is not.
- **Recorded.** *Every `job-mix:` line produced before 2026-09-19 is incomparable to one after it*,
  because the line gained `repeats=`, `ticks_min=`, `ticks_median=` and `ticks_max=` and the
  statistic changed from best-of-three to median-of-21. Recorded in milestone 168 and in
  `notes/job-mix.md`, which carries the old-and-new table.

## Index row

Milestone 168's sweep reports the best of three repeats per sweep point, and at `tasks=4` the
underlying distribution is wide enough that the best of three is itself a coin flip. Five boots of
an identical image ranged from 766,361 to 991,671 jobs per minute, a 29.4% spread. One boot's
three repeats on their own spanned a range containing the whole boot-to-boot spread. The variance is
within a boot, so power cycling does not reduce it. `tasks=4` is where `ECHO_SERVERS = 2` first
produces contention with too few samples to average it, and `tasks=32` is already stable at 2.7% and
would pay for repeats it does not need. So the obvious fix turns one constant into a table that
`board_console`'s recognizer has to stop assuming is fixed. The recommendation is to report the
spread rather than the best, because a benchmark whose subject is contention should not keep only
the least-contended sample, and that changes a line two programs read, which makes it calef's.

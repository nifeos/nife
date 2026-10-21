---
status: PARTIAL
raised: 2026-08-31
milestone_dependencies: 225
decision_dependencies: none
machine_requirements: aarch64, riscv64 and x86_64 silicon
specific_machine: none
needs_person: yes
---
# 201. Is multicore reliability converging? A defect-discovery curve, not a stress run

This scopes `design/fatal-risks/README.md`'s risk 5. The part that needs no hardware is done: the
seeds are derived below and the curve has a home and a data format,
[`notes/multicore-defect-curve.md`](../../notes/multicore-defect-curve.md). The run is what remains.

Three boards, and the sense of the gate that means somebody has to sit at them.

**In brief.** Risk 5 is *"it cannot be made reliable on multicore, and the bugs appear only on
silicon."* The experiment written against it was "sustained multi-core stress on all three boards",
which is a plan and not a test: it can never come back green, because you either stop finding bugs
or you run out of time.

**So measure the rate rather than the absence.** Track defects found per hour of stress. A curve that
flattens is evidence that the concurrency is converging. A curve that stays linear is the red
result, and it is the only framing of this risk in which "we cannot make this reliable" is a
possible outcome rather than an admission.

## Why this risk needs the reframe more than the others, and what its seeds are

Three candidate seeds for the curve do not hold up: the VisionFive 2's "receiver woken with nothing
delivered", and x86_64's `ap_boot` bugs from milestone 161 (the x86_64 kernel port: bring up the
HAL's third architecture).

The VisionFive 2 reading is void. `notes/visionfive2.md`'s fifth bench stop (2026-08-15)
overturned it with five independent identifications: the dumps the fourth stop read as a stranded
receiver were the terminal state of a *completed* boot tour. `notes/scheduler.md` ("The
undelivered-wake gate") records the consequence plainly: the wake-refusal gate built against this
reading "has never fired on a field failure," and `refuse:` has never appeared on a board ring. It
is hardening against a reachable state, proved by a loom harness, not a repair of an observed one.
There was no silicon-only, QEMU-invisible defect here. It never happened, and treating it as risk
5's fired instance is exactly what risk 5 needs not to have.

That matters more than it looks, because it was risk 5's only candidate for the risk's own central
claim. Risk 5 is specifically that "the bugs appear only on silicon" (QEMU cannot show them), and it
has zero confirmed instances of that claim, not one.

**The `ap_boot` bugs are real defects, and closed.** The boot-core-identity bug
(`arch::x86_64::ap_boot`'s `BUGS` #3) was fixed by milestone 316 (which core booted: making
`NIFE_SMP=2` mean something on x86_64) on 2026-09-18 UTC, and the third-or-later-secondary race
(`BUGS` #1) was fixed by milestone 161's own closing work (2026-09-19). A third x86_64 SMP bug from
the same bring-up, a missing cross-core TLB shootdown (`BUGS` #2), is fixed too.

None of the three was found on silicon. All three were found and reproduced under QEMU TCG, never
on real x86_64 hardware (xenon). They are genuine concurrency defects and belong in a defect history,
but they do not speak to risk 5's specific claim either, since that claim is about defects QEMU
cannot show, and QEMU showed all three of these.

So the seed set is:

1. x86_64: a missing cross-core TLB shootdown (`ap_boot`'s `BUGS` #2). A dead thread's kernel
   stack, unmapped and remapped onto a different frame for reuse, was still translated by another
   core through a stale TLB entry, so a recycled thread's context read back as stack paint.
   Reproduced 10 of 10 runs at `NIFE_SMP=2`; found and fixed 2026-08-26 (UTC; the commits are dated
   the evening of 2026-08-25 Pacific).
2. x86_64: a boot-core-identity mixup (`ap_boot`'s `BUGS` #3). `boot_cpu_id` answered "which core
   am I" instead of "which core booted," so a test body DECISIONS §28 (SMP placement: two random
   choices at spawn, message-shaped stealing, local wakes)'s placement migrated onto a secondary
   mistook that secondary for the boot core. Found 2026-08-26 UTC (while verifying #2 above); fixed
   by milestone 316, 2026-09-18 UTC.
3. x86_64: a lost-checkin race in secondary bring-up (`ap_boot`'s `BUGS` #1). `cpu_start`
   re-read the online count after issuing `STARTUP` IPIs and waited for it to move again, so a core
   that checked in during the settle delay was counted absent. 26 of 40 plain boots failed at
   `-smp 4` before the fix; fixed by milestone 161's own closing work, 2026-09-19.

**All three are QEMU-only findings, dated and closed, not the silicon-only defect risk 5 exists to
catch.** The curve therefore has three real data points for ordinary multicore concurrency bugs, and
zero for the specific failure mode this risk names. Presenting the three above without saying
that would be exactly the overclaim this milestone's own first `BUGS` entry warns against ("a
flattening curve is a confidence, not a verdict").

## The seeds, re-derived from primary sources (2026-09-24)

The correction above (2026-09-23) was right about what happened and still left three things wrong,
found by re-reading each seed against the commit that recorded it rather than against the notes that
cite it. Its dates were Pacific rather than UTC (fixed in place above), and two more:

| Seed as minted (2026-08-31) | Verdict | What it is now |
|---|---|---|
| The VisionFive 2's receiver woken with nothing delivered, on radon | **Falls** | A retracted reading, row D6 of the curve's history, class `retracted`. `notes/visionfive2.md`'s fifth bench stop (2026-08-15) stands on re-reading: five identifications, each checkable from the tree. It is kept as a row so nobody who read the old version counts it again |
| x86_64 `ap_boot` bug, "a third or later secondary" | **Stands, changed** | Row D10, a `race`, fixed 2026-09-19 UTC by milestone 161. **Its minted description was wrong as well as stale**: no core was ever failing to start, and "third or later" was never the boundary (the first secondary took turns failing too). `cpu_start` re-read the online count after the `STARTUP` IPIs, so a core that checked in during the settle delay was counted absent. It is a race between two cores, in this port's code, shown by QEMU |
| x86_64 `ap_boot` bug, which core booted | **Stands, changed** | Row D9, class **`multicore` rather than `race`**. `boot_cpu_id` answered "which core am I"; given where §28's placement put a test body, the wrong answer was deterministic. It needs two cores and does not need an interleaving, and 201's own "every defect classified" rule is where the difference counts. Fixed 2026-09-18 UTC, milestone 316 |

None of the three can be a point on this curve, and that was true on the day it was minted. A
point needs a denominator: hours, crossings or boots of measured stress on a board. D9 and D10 were
found by `script/test` under QEMU TCG, and D6 by a boot tour, not a soak. So they belong to the
curve's history, beside ten other rows the sweep for this section found (six QEMU finds, one loom
find, one audit later reproduced under QEMU, one with no recorded instrument, and one HVF hang still
unclassified). The curve starts empty of defects.

Its first real points already exist, and nobody had entered them. radon ran `soak-test` three
times on 2026-09-03 and 2026-09-04 (UTC), one of them for 2 h 59 m and 5,507 crossings, all with
`refused=0 mismatch=0 stalled=0` (`notes/soak.md`). Those are exposure rows E1 to E3: roughly three
and a half hours of riscv64 silicon with zero defects. That is one architecture, one workload and
thin records (no build, no committed log), and it is still more than the three seeds ever were.

What this does to risk 5. Of thirteen history rows, every real defect whose instrument is
recorded was shown by QEMU, loom or an audit. The one silicon reading is retracted. The only row seen
on physical cores and not under TCG is the HVF listener hang, and its evidence points at the test
harness. It stays `unclassified` because an unexplained row is a candidate for exactly the class
this risk names.

## radon's slope, read 2026-10-21

Read from milestone 225's rows E1 to E50 in `notes/multicore-defect-curve/radon-rows.md`, as they
stand, by the note's own rule: a running total of crossings against a running count of `race` or
`multicore` defects.

| what | value |
|---|---|
| boots on the curve | 39 (E2, E4 to E15, E17 to E21, E23 to E26, E29 to E34, E37 to E40, E42 to E44, E46 to E49) |
| crossings | 10,930,204 |
| hours | 26.3 |
| defects with a radon exposure row | 0 |
| slope | 0 defects per million crossings, from the first point to the last |

Left out: E1 and E3, which have no final crossing count, and the nine rows marked as not crossing
(E16, E22, E27, E28, E35, E36, E41, E45, E50). E32 holds 24 crossings at its last beat and is not
marked, so it is on the curve; it moves no total.

What the number means. It is a curve with no points above zero, which is not the flattening this
milestone looks for: a curve can only flatten after it has risen. What zero in 10.9 million
crossings gives is a bound. Read as independent trials, the 95% upper bound is about 3 in
10.9 million, or 0.27 per million crossings. Crossings within one boot share a placement and are
not independent, so that bound is optimistic. Per boot, it is zero in 39, an upper bound near 3 in
39.

Where it is not one experiment:

- Two boots carry most of it. E4 and E6 are 9,677,908 crossings, 89% of the total; 35 of the 39
  boots are two-minute reboot draws adding 635,892 between them.
- Five recorded builds and two workloads (`soak-test` and `soak-test-reboot`) are pooled, which the
  note allows within one architecture and which 201's `BUGS` warns can flatten a curve for free.
- E27 reset itself with no kernel reset lines. It reads `clean` by the row rule and is unclassified
  in milestone 225's `BUGS`; classified as `multicore`, it would be radon's first point.

**argon (aarch64) and xenon (x86_64) have no rows, so their slopes do not exist yet.** riscv64 meets
§259's bar (E4 + E6 + E7, 10,288,805 crossings over three plain boots); the other two architectures
have zero crossings toward it. Risk 5 is three per-architecture curves and this is one of them.

## What the run needs

- The load-sensitive assertions live (`notes/load-sensitive-assertions.md`), plus
  `script/repeat-under-load` and `script/interleaving-check`, since a defect nothing asserts on is a
  defect nobody counts.
- Hours logged per board, because the denominator is the whole measurement and is the thing most
  likely to be recorded badly. The format is fixed now: one exposure row per boot, read off the last
  `soak-test:` beat, in `notes/multicore-defect-curve.md`. The curve is judged on crossings (calef,
  2026-09-25 UTC), because `notes/soak.md` argues clock time is the wrong axis; hours and boots are
  still recorded in every row.
- **Every defect classified** as concurrency or not. A curve polluted with unrelated failures answers
  a different question.

## BUGS

- **A flattening curve is a confidence, not a verdict.** It cannot prove the concurrency is right,
  only that this project is finding fewer problems per hour than it used to, which is also what
  running out of imagination looks like.
- **Three boards is not three samples of the same thing.** aarch64, riscv64 and x86_64 have different
  memory models, so a per-board curve is three measurements, and pooling them would hide the
  architecture where the trend is worst.
- The denominator is manipulable without anyone meaning to. Hours of stress on an idle workload
  find nothing and flatten the curve for free, so the workload has to be stated with the number.
- This is expensive and slow, and its own answer arrives over weeks. It is on the fatal-risk list
  because it could be fatal, not because it is efficient.

## Follow-on

- **Outstanding.** The run: milestone 225 (run the soak on radon, argon and xenon) appends one
  exposure row per boot to `notes/multicore-defect-curve.md`, and this milestone reads the slope per
  architecture. Checked 2026-10-21: radon's slope is read above from E1 to E50; argon and xenon
  have no rows.
- **Done.** The stated duration `design/fatal-risks/multicore-reliability.md` asks for: calef ruled
  on 2026-10-07 (UTC), recorded in §259 (a multicore soak counts toward risk 5 at ten million
  crossings over three boots). At least 10 million crossings per architecture across at least
  3 boots, stated before the run. Its sibling question was answered earlier: calef ruled on 2026-09-25 (UTC) that the curve is judged on crossings, with hours and
  boots still recorded, carried by pull request #1282 into the run section above and
  `notes/multicore-defect-curve.md`'s *Which denominator*.
- **Recorded.** The history table is one lane's sweep rather than a census, D5's instrument is
  unrecorded, and nothing computes the curve; all three are in
  `notes/multicore-defect-curve.md`'s `BUGS`.

## Index row

Scopes `design/fatal-risks/README.md`'s risk 5, which its own BUGS recorded as unowned. "Sustained
stress" is a plan, not a test: it can never come back green. Measuring defects found per hour can:
a flattening curve is evidence, a linear one is the red result. Its original three seeds were
wrong (found 2026-09-23): the VisionFive 2 reading was retracted 2026-08-15, before this milestone
was even minted, and it never happened; the two "unresolved" x86_64 `ap_boot` bugs are now both fixed
(milestones 316 and 161). Re-derived 2026-09-24: none of the seeds can be a point on the curve,
since none has a stress denominator. They are history rows in `notes/multicore-defect-curve.md`,
which also holds the curve's data format and its first real points (radon, about three and a half
hours of soak, zero defects). Risk 5's own defining claim, a defect QEMU cannot show, has no
confirmed instance.
Gate: HARDWARE, in the sense that somebody must sit at three boards.

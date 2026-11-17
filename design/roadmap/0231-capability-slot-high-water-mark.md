---
status: BUILT
raised: 2026-09-02
built: 2026-09-02
---
# 231. Nothing counts how many capability slots a boot actually uses, so the wall is always a surprise

Minted the same day by calef, from milestone 230's
(`script/shell-check` is red on `main`, on both architectures, and nothing says so) own `BUGS`.
*(Number provisional until the merge queue lands it.)*

It was minted with no gate and needed none. What it produces is itself a check. Every boot now prints
`capability slots: 21 of 24 at peak`, `script/swish-check` echoes the last such line on both
architectures, and that check fails if the kernel flagged the boot as having gone past the peak
recorded beside the constant.

**In brief.** `CAPABILITY_TABLE_SLOTS` has been raised three times, and **every raise was reactive,
after a silent failure that named something else.**

- **16 to 17**, milestone 49 (users and attribution). The symptom was that the first login against a
  freshly built service answered `login_proto::DENIED` instead of `OK`, **on a correct password**.
  Nothing said "out of slots".
- 17 to 24, milestone 230, today. The symptom was that init trapped with no message and `main`
  could not boot interactively for five days. `MemoryRegion::RETYPE` answers `OutOfMemory` for both
  "region out of pages" and "table full", so even the error did not distinguish them.

Between those, the constant spent time at `28 // TEMP: generous bisection value`, was restored to 17
on two true observations, and the restoration shipped a system that could not boot. That episode is
milestone 230's account and is worth reading before this one is designed.

The measured high-water mark is 21, in init, during `build_child` for `credentialer`. Nobody knew
that until somebody instrumented four boots to find it.

## What it needs

A boot that reports the peak it reached, against the ceiling it had. That is the whole idea. A
line saying the boot used 21 of 24 turns a cliff into a gauge, and it is the difference between
milestone 230's four instrumented boots and one ordinary one.

Three things this block did not decide, and how they were decided.

Where the count lives: in the table, unconditionally. `capability_table::CapabilityTable` carries a
`used` and a `peak`, and every path that can occupy an empty slot goes through one private `grew()`.
That is rung one of AGENTS.md's ladder rather than a hook somebody has to remember at each of the
kernel's seven insert sites. The lookups (`get`, `get_with`) are untouched, which is what keeps the
counting off the read path.

The feature-gate question was settled by measurement rather than preference, which is what the block
asked for. `script/fastpath-footprint`, against its 5% bound: +1.1% aarch64, +0.5% riscv64, +0.7%
x86_64. Milestone 221's soak counters needed a gate at 5.7%; this does not need one, so `soak` and
`fastpath_pad`'s precedent is noted and not followed.

What it does as it approaches: nothing, and that is deliberate. The gauge is passive. What is
checked is not the boot's distance from the ceiling but whether a recorded measurement has gone
stale. `kernel::cap::CAPABILITY_TABLE_PEAK_MEASURED` is 21, the number milestone 230 (shell-check is
red on main) found by
instrumenting four boots, and a boot that goes past it prints `ABOVE` and fails the gate. That is the
same shape as this file's `size_of::<Cap>() == 32` assertion, which its own comment calls "the fact,
not a target". A margin picked from one boot would have been the fourth deleted check; a fact that
stopped being true is not a margin.

Whether `script/swish-check` asserts on it: yes, on two things. That the line is printed at all,
because a gauge that quietly stopped printing is one nobody misses until the wall arrives again,
which is exactly how the constant came to be raised three times reactively. And that it does not say
`ABOVE`. It also echoes the line on success, so the number is in every CI log a person reads
rather than only in a failure.

## What it reports, and how one line instead of twenty-one

The kernel prints from the scheduler's idle loop (`kernel::cap::report_peak`), which is the one place
reached after every phase of a boot with nothing else to do. The mark climbs once per grant, so the
naive version printed six lines on an aarch64 boot (4, 5, 12, 14, 16, 21 of 24). Init blocks on IPC
several times while building the login stack, and each pause looked like an ending. Waiting sixteen
consecutive idle passes for the number to hold still reduces it to two, ending in the one that
matters. That window is a coalescing constant rather than a threshold on the measurement, and getting
it wrong can only cost an extra line or a later one; the peak itself never decreases.

## What it measured

21 of 24, on both architectures, which is exactly the figure milestone 230 arrived at by hand.
That agreement is the point: the number had been true all along and cost four instrumented boots to
learn, and it now costs a boot.

It also priced milestone 233 (`login` dies on every boot, and the boot says it is ready) before that
milestone spent anything. Handing `login` the two blobs it needs was expected to cost slots at the
peak; the gauge says the peak after the change is still 21, because `supervision_proto`'s
`fill_and_map` holds one frame capability at a time. That is the whole reason these two milestones
were one lane, and it is the first time this tree has been able to answer "what will this cost in
slots" before merging rather than after.

## Why the headroom is not the answer

Milestone 230 left three slots of margin deliberately, because both previous raises set the number to
exactly what that day's boot needed and both times the next addition hit the wall in the same
silence. Three slots is a guess standing in for a mechanism, and its own block says so. This
milestone is what would replace it.

## BUGS

- **This does not make the wall impossible, only visible.** A boot that needs a twenty-fifth slot
  still fails; it just fails saying so.
- **The line does not say which thread.** `capability_table::highest_seen` is one atomic for every table in
  the binary, because a `static` cannot be keyed by a const generic. Finding the owner means
  walking every thread under the scheduler lock, which is the scan the atomic exists to avoid. A
  reader who needs the owner is back to instrumenting, which is what this milestone was written
  against; closing it is a scan at print time and nothing else, and it was left out rather than
  designed away.
- The gauge is only read by `script/swish-check`. `script/test` never boots the real init, so the
  suite's own peak is never checked against anything. The recorded-measurement arm is compiled
  out of the test kernel on purpose (the guest suite runs a much larger workload through the same
  kernel, so a test going past 21 would be true and misleading). Every other boot mode prints the
  gauge and nothing reads it.
- Two lines rather than one on a normal boot. Init blocks early enough that the mark holds still
  at 5 for long enough to be believed. Harmless, and the cure is a longer window that would delay the
  real line.
- It says nothing about the other fixed-size tables. `MAX_THREADS`, `MAX_REGIONS` and
  `nifefs::NAME_LEN` are the same shape, and whether one mechanism should serve all of them is a
  question this block leaves open rather than answers by scope creep.
- The peak is workload-dependent. The number a boot reports is the number *that* boot reached,
  and a richer initrd reaches a different one, so a single green figure is not a guarantee about
  every configuration.

## Follow-on

- **Recorded.** `design/roadmap/0231-capability-slot-high-water-mark.md`. The gauge does not make the
  wall impossible, only visible: a boot that needs a twenty-fifth slot still fails, it just fails
  saying so.
- **Recorded.** `design/roadmap/0231-capability-slot-high-water-mark.md`. The line does not say which
  thread. `capability_table::highest_seen` is one atomic for every table in the binary, because a `static`
  cannot be keyed by a const generic. Naming the owner means walking every thread under the
  scheduler lock, which is the scan the atomic exists to avoid. Closing it is a scan at print time
  and nothing else; it was left out rather than designed away.
- **Recorded.** `design/roadmap/0231-capability-slot-high-water-mark.md`. Only `script/swish-check`
  reads the gauge. `script/test` never boots the real init, so the suite's own peak is checked
  against nothing. The recorded-measurement arm is compiled out of the test kernel on purpose:
  the guest suite runs a much larger workload through the same kernel, so a test going past 21 would
  be true and misleading.
- **Recorded.** `design/roadmap/0231-capability-slot-high-water-mark.md`. A normal boot prints two
  lines rather than one, because init blocks early enough that the mark holds still at 5 long enough
  to be believed. Harmless, and the cure is a longer coalescing window that would delay the real
  line.
- **Recorded.** `design/roadmap/0231-capability-slot-high-water-mark.md`. The peak is
  workload-dependent: the number a boot reports is the number *that* boot reached, and a richer
  initrd reaches a different one, so a single green figure is not a guarantee about every
  configuration.
- **Milestone 338.** Build the same
  high-water gauge for the other fixed-size tables, or decide that one mechanism should serve all of
  them. `MAX_REGIONS` and `nifefs::NAME_LEN` have no gauge at all and `sched::MAX_THREADS` has its
  own separate `PEAK_THREADS`, so the shape is being solved once per table by hand and every
  ungauged constant is still raised only after it has failed silently.

## Index row

the table counts its own peak, the boot prints `capability slots: 21 of 24 at peak`, and `script/swish-check` fails when the recorded measurement goes stale

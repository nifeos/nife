---
status: BUILT
raised: 2026-09-21
built: 2026-09-21
---
# 571. The x86 boot calibrates the TSC once, and can be wrong by 4x

Built 2026-09-21. The boot times several 10 ms PIT windows instead of one and keeps the
smallest. It stops as soon as two of them agree, and prints the worst beside the chosen one so a
calibration the host fought is visible rather than silently confident.
*(Renumbered from the lane's provisional 526 on 2026-09-22: a concurrently merged lane had taken
526 for a riscv64 ACPI host, and this proposal had meanwhile been promoted to 571 on main. 524 onward was contested between in-flight
branches.)*

Inside `kernel/src/arch/x86_64/timer.rs`. No system call surface, no wire format, no dependency.

**x86_64 only by nature, and not a parity gap.** Under §19 (architectural parity is a tenet; the
targets are aarch64, riscv64, and x86_64): aarch64 reads `CNTFRQ_EL0` and riscv64 reads the
device tree. Both are architected statements of the rate, so neither has a calibration that could
be wrong. This is the one architecture that has to *measure* the clock everything else is measured
against. `timer.rs`'s header has said so since the port landed.

## The defect, which another lane found while looking for something else

Milestone 524 (the three x86_64 boot gates: NX, SYSTEM CALL, and the invariant TSC) had just argued
that a non-invariant TSC "cannot be caught by measuring harder". The `tscdrift` lane then went to
check whether TCG's TSC drifts at all. It built an RTC-referenced probe and found that it does not.
The counter ticks at exactly 1000.000 MHz, constant to within 42 ppm whatever the host is doing.

What it found instead was that the number the boot writes down had almost nothing to do with
that. Twenty-two boots of one binary stored 1001 MHz to 4330 MHz. This lane widened that to 490
boots at three host loads, and the shape held without a single exception. Every boot was high and
none ever low.

`init_frequency` armed PIT channel 2 for a 10 ms one-shot. It polled port 0x61 until the output
line went high, reading the TSC either side. Polling can only ever notice the terminal count late,
and the entire elapsed TSC delta was then attributed to exactly 10 ms. One descheduling of the QEMU
thread inside that window inflates the answer without bound. A 4330 MHz reading is a 43 ms window
reported as a 10 ms one, and the worst this lane saw, +1153%, is a 125 ms one.

The error being one-sided is the whole of the fix. Every window is an *upper bound* on the true
rate, so the minimum of several is the tightest bound taken. It converges on the truth from above.
An average would be a biased estimator for precisely the reason the minimum is an unbiased one. It
carries every descheduling into the answer instead of discarding it.

## calef's ruling named the shape; the count was measured

> "Fix the calibration with min-of-five windows." (2026-09-21)

The brief for this lane said not to take five as given, and that was the right instruction. Five
would have been the wrong number to ship. The table below comes from two hundred boots on a host
deliberately saturated to load 30 on eight cores. Each boot timed sixteen windows and reported
every one, which gives the error of the min over the first k of them:

| Windows | Median error | 99th percentile | Worst of 200 | Boots wrong by >1% |
|---|---|---|---|---|
| 1 (what shipped) | +0.36% | +884% | **+1153%** | 56 / 200 |
| 3 | +0.00% | +120% | +131% | 24 / 200 |
| 5 | +0.00% | +20.6% | +51.4% | 11 / 200 |
| 9 | +0.00% | +0.49% | +7.1% | 2 / 200 |
| 16 | +0.00% | +0.01% | **+0.47%** | **0 / 200** |

Five still leaves one boot in eighteen wrong by more than a per cent and a worst case of +51%. The
tail only closes at sixteen. The median is +0.00% at every count from three upward. That is why
reporting the worst case rather than the mean was the instruction that mattered: by the mean, the
defect was fixed at three windows and had never been very bad at one.

The load matters and is stated rather than hidden. At ordinary session load one window's worst of
30 boots was +773%, and min-of-9 got to +0.01%. At load 22 the same estimators gave +318% and
+0.34%. The table above is the hardest condition measured, which is the one a cap should be chosen
against.

## Sixteen windows would be a 160 ms boot tax, so it is a cap and not a count

Ten milliseconds per window is real time on every boot, including every CI test boot, and the suite
boots repeatedly. Paying it sixteen times when three would do is a cost that gets noticed later.
Someone with less context then reverts it.

So the loop stops as soon as two windows agree to within one part in a thousand. The reasoning
is the one-sidedness used the other way round. A window is inflated by a descheduling, an event of
essentially arbitrary size. For two windows to land within a thousandth of each other they must
both have been left alone. Two independent deschedulings agreeing to three decimal places is not a
thing that happens. Agreement is evidence of cleanliness, and once
there is evidence the remaining windows buy nothing but boot time.

Measured over the same 490 boots, this reaches exactly the accuracy of taking the full cap every
time. It matches boot for boot, at a mean of:

| Host | Mean windows timed | Mean calibration cost |
|---|---|---|
| Ordinary session load | 3.53 | 35 ms |
| Busy (load 22) | 4.77 | 48 ms |
| Saturated (load 30) | 5.20 | 52 ms |

calef's five is what this costs; sixteen is only what it is willing to spend when the host is
fighting. That the measured mean on a saturated host came out at 5.20 is a coincidence and is
recorded as one.

A tighter tolerance (one part in 2000) moved the mean by 0.07 windows and the error distribution
not at all. So the choice is not delicate. Clean windows agree to within the PIT's own quantization
and dirty ones are nowhere near.

### The measured boot cost, stated plainly

`script/test --arch x86_64` boots the kernel four times, and those four boots took 3, 3, 4 and
3 windows. So the suite pays 90 ms more than the one-window design did, against a 3m38s
runtime. That is **0.04%**, an order of magnitude below the suite's own run-to-run variance. It is
a computed figure and deliberately not a measured delta. There is nothing to measure a 90 ms change
against.

## Was there a better reference than the PIT? Asked, and the answer is no, twice

`CPUID` leaf 0x15 states the TSC's ratio to a core crystal whose frequency leaf 0x16 may give.
Where it exists it is strictly better than any measurement, because it is the part stating its own
rate. `arch::x86_64::isa::tsc_crystal_hz` already reads it and `init_frequency` already prefers
it. That path is milestone 161 (the x86_64 kernel port: bring up the HAL's third architecture)'s
work, and this lane changed nothing about it. It is empirically unavailable here. QEMU's TCG does
not offer the leaf under any invocation tried, so every boot in this tree takes the calibrated
path. **Nothing to do; the better reference is already
preferred and the machine declines to provide it.**

The CMOS RTC is the harder question, because it is the reference `tsc_probe` uses. It is also
genuinely independent (QEMU drives it from `QEMU_CLOCK_HOST`, host wall time, rather than from the
vCPU). It is a better *check* and a worse *calibration source*, and the reason is resolution. Its
seconds register has one-second granularity. Calibrating against it means either waiting for a
seconds edge (up to one second of boot, a hundred times this fix's cost) or reading it as a time.
The second accepts a one-second error. `tsc_probe` gets its precision by watching for an **edge**
and then spanning thirty-two seconds. That is fine for an instrument that runs once on demand and
absurd for every boot. Worse, the edge wait has the same one-sided polling error this whole section
is about. It would need the same min-of-N treatment on top of costing a hundred times more.

So the PIT stays. It is the only fixed-rate device on a PC that can be timed in ten
milliseconds. The RTC's role is the one it already has, as the independent reference
that says whether the PIT calibration was right.

## What is in the diff

- `kernel/src/arch/x86_64/timer.rs`: the loop, the cap and the stopping rule. It adds a
  `Calibration` record (**provisional name**) that keeps every window rather than discarding them,
  and a rewritten `BUGS` section. The old "one calibration, no averaging" bullet is gone. Two
  bullets that are true now replace it: what this still does not bound, and what it costs in boot
  time.
- `kernel/src/main.rs`: the boot line prints `best of N windows, worst M MHz`. This is the part
  that is rung three of AGENTS.md's ladder rather than rung one. Nothing can *make* a host leave
  the vCPU alone, so the next best thing is that a calibration the host fought stops being
  invisible.
- `kernel/src/arch/x86_64/timer_calibration_tests.rs`, new: five tests of the stopping rule fed
  sequences by hand. It also has two boot assertions (several windows were timed; the stored rate
  is the smallest of them). Its header says why they run under QEMU rather than on the host.
  `timer.rs` compiles only for `x86_64-unknown-none`, and there is no second consumer to justify
  moving the rule into a crate.
- `kernel/src/arch/x86_64/tsc_probe.rs`: prints every calibration window, which is what turned the
  probe from "is the calibration right" into "is the estimator right" and is how the tables above
  were measured.
- `helpers/qemu-runner-x86_64.sh`: forwards SIGTERM to QEMU. See below; this is a separate bug that
  this lane's own sweep tripped over several hundred times.
- `notes/benchmarks.md` (since split: the table is in `notes/benchmarks/x86-tss-iomap.md` and the
  dated section in `notes/benchmarks/counter-frequency-and-calibration.md`): the 2026-08-24 x86
  `ns/iter` table is marked in place. A new dated section records what the defect reaches and what
  it does not.

## The bug this lane tripped over on its way, and fixed

**`helpers/qemu-bounded.sh`'s bound did nothing on x86_64**, and the reason is one word in a
comment. `qemu-runner-x86_64.sh` is the only one of the three runners that does not `exec` QEMU.
It has to translate `isa-debug-exit`'s status afterwards, and it says so. But the wrapper bounds a
run by sending SIGTERM to the child it started. On aarch64 and riscv64 that is QEMU; here it was
the runner shell. The shell died; QEMU was reparented to pid 1 and ran forever.

This lane's calibration sweep orphaned an emulator on every boot until someone looked.
`qemu-bounded.sh`'s own `BUGS` section named a different cause ("a SIGKILL to the killer defeats
all of it") than the one operating. The runner now runs QEMU in the background, traps TERM and HUP,
forwards them, and reaps. It also needed `qemu-bounded.sh`'s own fd-3 dance. Backgrounding a
command gives it `/dev/null` on stdin under dash, which would have broken every piped-input run in
CI.

That is a general hazard rather than this lane's. Any bounded x86_64 run in this tree, in any
lane, leaked its emulator at the bound. It is worth reading as evidence for AGENTS.md's ladder.
The property that failed was recorded in a comment (rung three), in a file that was correct about
everything except what another file would do to it.

## BUGS

- **This is a probabilistic filter, not a bound.** The minimum is only as good as the best window
  the host allowed. A host that never leaves the vCPU alone for ten unbroken milliseconds produces
  an inflated rate however many windows are taken. 0 of 200 boots at load 30 were wrong by more
  than 1%. That is a measured distribution and not a guarantee. The boot line's worst-window
  figure is what makes the remaining failures visible rather than silent.
- The cap was chosen against one machine. Every number here is patagonia, an eight-core Apple
  Silicon host running TCG. An x86_64 CI runner with KVM, or xenon, has a different descheduling
  distribution and might want a different cap. Nothing here measures that. The promotion trigger
  is the first time an x86 wall-clock number from another machine looks wrong.
- **Nothing gates the accuracy.** The two boot tests assert the estimator is the one intended, not
  that the answer is right. Checking the answer needs an independent clock. That clock is
  `tsc_probe`, behind an off-by-default feature costing thirty-five seconds a boot. A gate would
  mean a thirty-five-second suite leg to catch a defect that fails safe. That is not a trade worth
  making today. If x86 wall-clock figures ever become something the project publishes rather than
  reads, that changes.
- **The local APIC timer's rate gets the same treatment and has no `CPUID` escape.** It is the
  minimum over the same windows, taken independently of the TSC's. Unlike the TSC there is no path
  that skips the measurement. On a part that reports leaf 0x15 the windows are still timed.

## Follow-on

- **Recorded.** This is a probabilistic filter rather than a bound, and a host which never leaves
  the vCPU alone still defeats it. That limitation is stated where a reader meets the feature:
  `kernel/src/arch/x86_64/timer.rs`'s `BUGS`, beside the constant that sets the cap.
- **Recorded.** The cap was chosen against one machine (patagonia, eight-core Apple Silicon, TCG).
  It may want a different value on a KVM runner or on xenon. Both facts are in this block's `BUGS`
  and in `kernel/src/arch/x86_64/timer.rs`'s. The promotion trigger is the first x86 wall-clock
  figure from another machine that looks wrong.
- **Recorded.** Nothing gates the calibration's *accuracy*, only its estimator. That is in
  `kernel/src/arch/x86_64/timer_calibration_tests.rs`'s own `BUGS`, with the reason (an accuracy
  gate costs a thirty-five-second suite leg to catch a defect that fails safe).
- **Done.** The proposal this promotes,
  `design/roadmap/proposals/the-x86-boot-calibrates-once-and-can-be-wrong-by-4x.md`, is removed by
  this milestone. Its two open questions are both answered above. The boot does print its
  confidence, and the local APIC timer is derived from the same windows.
- **Recorded.** `helpers/qemu-bounded.sh`'s `BUGS` section still describes SIGKILL as the way its
  bound is defeated. It does not mention that a runner which does not `exec` defeats it with an
  ordinary TERM. The x86_64 runner is fixed here and now says so at the code. The wrapper's own
  `BUGS` is the place a reader would look first, and is left for whoever next touches that file.

## Index row

The x86 boot measured the TSC against a single 10 ms PIT window and stored the result for the life
of the boot. That number was wrong by up to **+1153%** against a counter known to tick at
1000.000 MHz. It was high on every one of 490 boots and never low, because a poll can only notice
the PIT's terminal count late. Everything time-derived on x86 read it: `bench --x86 --real`'s
ns/iter, `Instant`, `uptime`, `coremark`'s self-reported rate. This milestone takes the smallest of
several windows instead. That is the right estimator precisely because the error is one-sided. It
stops as soon as two windows agree to one part in a thousand. So the mean cost is 3.5 windows on a
quiet host and 5.2 on a saturated one, against a cap of sixteen. calef's ruling said five. Five was
measured and found to leave a +51% worst case, which is why the cap is sixteen and the five is what
it spends. The worst window is now printed beside the chosen one, so a calibration the host fought
is visible rather than silently confident.

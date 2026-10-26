# The swish-check x86_64 leg: what its seconds were

*An appendix to [`notes/benchmarks.md`](../benchmarks.md), from milestone 628 (provisional) on
2026-10-03 UTC. Name: provisional (the 628 lane). It follows
[icount tick scales](icount-tick-scales.md), which attributed the leg's cost to the paint path.
Milestone 624 (the x86_64 paint path stops repainting the world) then halved it.*

## The finding: the leg was paced by the timer, not by the emulator

After milestone 624 the x86_64 leg still cost 7.7 s a line on CI's arm64 runner against
aarch64's 0.2 s.
That is run 37083676625: 128 lines in 985.3 s. Most of that time the guest was doing
nothing. Sampling the vCPU through the QEMU monitor while three `caps` lines ran (patagonia, TCG,
1,378 samples over 9.5 s) found it halted (`HLT=1`) in 1,034 of them, 75 percent.

Three measurements say what it was waiting for.

- The second boot's eight lines cost the same everywhere: 23.8 s under TCG on patagonia,
  24.3 s under TCG on CI's arm64 runner, 23.9 s under KVM on CI's x86_64 runner. Work that cost CPU
  would differ by the hosts' speed; this did not move with a 10x faster CPU.
- Byte timestamps show a quantum. Typing `echo hello world` echoed two characters per 0.10 s,
  and every write of a line's answer arrived 0.08 s after the last.
- A faster tick shrank it. `TICK_HZ` at 1000 instead of 100 (temporary, not committed) took the
  first boot from 488 s to 282 s and the second from 23.8 s to 4.5 s.

The cause is the last line of the x86_64 boot. After `x86_hand_over` the boot thread called
`arch::halt()`, `hlt` in a loop, while still a runnable thread on the run queue. Each time
round-robin reached it, the core stopped until the next 10 ms tick, with the shell, the console and
the input driver ready behind it. x86_64's input driver polls COM1 and yields (no COM1 interrupt
reaches userspace yet; `components/src/input.rs`), so the rotation reached the boot thread
constantly. The fix is that thread calling `sched::exit()` instead; the idle thread, which halts
only when nothing else can run, takes the core.

Milestone 720 (the boot thread cannot halt while it is runnable) settles why aarch64 and riscv64
never showed it. Their boot paths also ended in `arch::halt()`, but their input
is interrupt-driven, so no thread keeps the rotation turning while the shell waits. Milestone 720
ended both in `sched::exit()`, made `arch::halt` take a token an ordinary boot
cannot make, and measured both legs before and after; its roadmap block has the numbers.

## Per-line seconds, one change at a time (patagonia, TCG, local)

The x86_64 leg alone (`script/swish-check --arch x86_64`, `NIFE_SHOW_LINE_TIMES=1`), on milestone
624's branch. Local runs skip the nine `std_exerciser` lines because the exerciser is not built
here, so they type 118 or 119 lines where CI types 128.

| change | first boot | per line | median | p90 | slowest | second boot (8 lines) |
|---|---|---|---|---|---|---|
| none (624's branch) | 486.4 s / 119 | 4.09 s | 2.72 s | 7.93 s | 20.2 s | not reached (`wc < args.txt` failed) |
| `-cpu Skylake-Client` for `max` | 488.1 s / 118 | 4.14 s | 2.93 s | 7.68 s | 17.7 s | 23.8 s |
| `TICK_HZ` 1000 (diagnostic) | 282.0 s / 118 | 2.39 s | 1.23 s | 5.38 s | 14.2 s | 4.5 s |
| boot thread yields forever | 115.6 s / 118 | 0.98 s | 0.42 s | 2.39 s | 10.4 s | 1.1 s |
| boot thread exits (shipped) | 125.7 s / 118 | 1.07 s | 0.42 s | 2.29 s | 10.8 s | 0.9 s |
| exits, and no `intel-iommu` | 112.9 s / 118 | 0.96 s | 0.44 s | 2.16 s | 8.5 s | 1.0 s |

Medians and percentiles include the second boot's lines where it ran. One run each, so differences
of 10 percent are inside run-to-run noise: the yield and exit rows are the same result.

What each lead came to:

- The CPU model. `-cpu max` against a named model with `RDSEED`: no difference. Kept.
- The IOMMU. Dropping `intel-iommu` and `iommu_platform=on` moved 10 percent, inside the noise
  above, and would take away the confinement the kernel suite runs on this machine. Kept.
- The paint path. After the fix, the median line is 0.42 s and what remains is proportional to
  output: the `caps` lines and the two supervised jobs are the tail. The console still blocks on one
  paint per write because 624's batcher is off (`SCREEN_BATCHING_ENABLED`). That is 624's to finish,
  not this milestone's.
- The harness. It polls every 100 ms for the echo and the prompt, the same on every leg, and
  waits no fixed time. Not a lever.
- Acceleration. `swish_check` clears `NIFE_ACCEL` (since 2026-08-02, before the x86_64 leg
  existed) because HVF bought nothing on a leg that waits on QEMU's serial. The OVMF runner never
  read it. Under KVM on CI's x86_64 runner, before the fix, the leg was 358.4 s for 128 lines,
  2.8 s a line: KVM alone bought 2.7x because the time was ticks, not instructions.

## CI

| run | runner | leg | first boot | per line | second boot |
|---|---|---|---|---|---|
| 37083676625 (624, reference) | arm64, TCG | x86_64 | 985.3 s / 128 | 7.7 s | 24.3 s |
| 37085722237 (628, KVM only) | x86_64, KVM | x86_64 | 358.4 s / 128 | 2.8 s | 23.9 s |
| 37089120726 (628, fix) | arm64, TCG | x86_64 | 364.5 s / 128 | 2.85 s | 2.0 s |
| 37089120726 (628, fix) | x86_64, KVM | x86_64 | 33.4 s / 128 | 0.26 s | 0.8 s |
| 37089120726 (628, fix) | arm64, TCG | aarch64 | 25.1 s / 131 | 0.19 s | 1.5 s |
| 37089120726 (628, fix) | arm64, TCG | riscv64 | 28.2 s / 131 | 0.22 s | 1.5 s |

With the fix and KVM, the x86_64 leg's median line is 0.10 s, the same as aarch64's and riscv64's.
Its slowest is `interrupt_ignorer` at 7.0 s, which is among the slowest lines on every leg (3.2 s on
aarch64) and is not looked into here. Under TCG on arm64 the fix alone takes the leg from 7.7 s a
line to 2.85 s, so KVM is worth a further 11x.

Job wall time, setup included: the reference `swish-check` job (all three legs) took 21m07s. Run
37085722237 (the split, before the fix) took 4m47s for the arm64 job's two legs. Run 37089120726
took 3m17s for `swish-check-x86_64`, and 10m17s for an arm64 job that ran all three legs as a
control. The two jobs run in parallel, so the check now costs the longer of about five minutes and
about three. That run printed line times (`NIFE_SHOW_LINE_TIMES`) from a commit that was dropped
before the branch was curated; the shipped workflow does not set it.

## Why main took 38 minutes and 624 took 21, when 624 saved 18 percent locally

On 2026-10-01 the leg took exactly 1942.0 s in two separate merge-group runs (36808011108 and
36815082049), to the tenth of a second. Wall time that repeats that closely is time the guest spent
waiting on its own clock, not time spent emulating. On a tick-paced leg, a change cuts wall time by
the ticks it removes, not by its instructions. So 624's saving on CI (1942 s to 985 s) and on
patagonia (6.3 s to 5.2 s a line, the 624 lane's figures) need not agree: the two hosts did not
spend their time on the same thing. Not measured further, because the fix removes the ticks.

## A gate for the next one

Milestone 722 (swish-check fails a leg that costs five times the others per line), provisional,
adds one. Nothing gated the cost of a leg for the two weeks the defect lasted, because the per-line bound is
sized to catch a hang and the job's wall time moves whenever a line joins it. Each boot of 20 lines
or more now prints its median seconds a line beside its baseline, and fails above five times that
baseline (`LEG_COST_BASELINE` in `xtask/src/swish_check/verdict.rs`, with the CI runs each row came from). The
defect here was about 30 times its KVM row, so it fails; it was 2.7 times its TCG row, so under TCG
it would still pass. The rule compares a leg to its own history rather than to the other legs,
because x86_64 under TCG is legitimately fourteen times aarch64 under TCG.

## BUGS

- One run per row. The local table is single runs on a laptop that other lanes share.
- The input driver still polls. With the boot thread gone the core still never idles at a
  prompt, because the input driver yields forever. Interrupt-driven COM1 input is the named
  follow-up in `components/src/input.rs`.

# Counter frequency and x86 calibration

*An appendix to [`notes/benchmarks.md`](../benchmarks.md), which carries the current numbers and is written so a reader can act without opening this file. This one holds the riscv64 hardcoded counter rate and the x86 TSC calibration that was wrong by up to 12x, with the dates, tables and corrections behind them. Name: ratified 2026-09-24 (calef); [the naming record](README.md) holds it.*

## The page and its bounds

`crates/counter_frequency_contract` defines the page that tells a process how many counter ticks
make a second where no register says so. `x86_64`'s TSC rate comes from `CPUID` leaf `0x15` or is
measured, and `riscv64`'s is `/cpus/timebase-frequency` in a device tree no process can read.
Either way the kernel learns it once, at boot. A hardcoded rate is not an answer: radon's `time`
CSR runs at 4 MHz, not QEMU's 10 MHz.

Every process that calls `now` needs the rate, with no capability to ask for, so the page is mapped
unconditionally. `kernel::user::load` and `kernel::user::map_timebase_page` map it into the spaces
the kernel builds. `supervision_protocol::build_child_space` maps it too, and there the parent
writes a fresh page from its own `user_mode_runtime::cntfrq_checked` reading. A parent that does
not know the rate writes zeroes, and `TimebasePage::hz` reads that as `None`, never a fabricated
rate. A process spawned another way has no page, and `cntfrq` faults on the unmapped read. That is
deliberate: dying says the rate is unknown, and a plausible number would not. A new spawn path
must map the page or not link anything that calls `cntfrq`.

`is_plausible` bounds the rate at both ends. A rate checked against zero alone passes a firmware
that reports 1 Hz, or a calibration that returns garbage because the host descheduled the vCPU
mid-measurement, and that number then poisons everything derived from it. The bounds are
deliberately wide, because a false refusal on somebody's silicon is worse than the failure it
prevents. The slowest counter in this tree's world is radon's 4 MHz `time` CSR. The 32.768 kHz
watch crystal is the slowest thing anyone drives a timer from, and the 100 kHz floor sits above it
on purpose, because nothing here is a watch. The 100 GHz ceiling is generous by two orders of
magnitude, to catch a division that went the wrong way rather than to police hardware.

## 2026-09-21: the userspace counter frequency was a constant on riscv64, and what it did not reach

`user_mode_runtime::cntfrq` returned a hardcoded `10_000_000` on riscv64 from the port until this
date. The std PAL (`sys/pal/nife/rt.rs`) carried its own copy of the same constant. QEMU `virt` runs
its `time` CSR at exactly that rate, so the number was right on every machine the suite runs on. It
was wrong by 2.5x on radon, whose JH7110 states 4 MHz. calef's ruling that ended it: *"We should
ensure that the program only returns accurate numbers versus leveraging hard coded ones."*

### No committed number was wrong, and why

- The bench baselines are icount, not time, and every baseline is compared only against itself. A
  baseline file holds counter ticks, and a tick is `counter_frequency / 1e9` of a guest instruction
  under `shift=0`: one riscv64 tick is 100 instructions, one aarch64 tick is 16, one x86_64 tick is
  1 ([the icount tick scales appendix](icount-tick-scales.md)).
- The `ns/iter` column does not come from userspace. `os_primitives_benchmarker` reports raw ticks
  over IPC, and `xtask/src/bench.rs` divides them by the `bench: cntfrq <hz>` line the *kernel*
  prints from `arch::timer::frequency()`. That has read `/cpus/timebase-frequency` since milestone
  100 (read the machine's PSCI and its CPU list, not QEMU `virt`'s). So the published EL0 figures
  used the machine's own rate, on QEMU and on a board.
- The job mix is the same shape, from milestone 168 (a multi-tasking workload benchmark).
  `kernel/src/job_mix.rs` prints the kernel's `hz` and the tasks report ticks.
- CoreMark reports its own `cntfrq` (`fixtures/src/coremark.rs` sends `[crc, ticks, freq]`), so it
  is the one measurement that *did* carry the constant. Nothing published a CoreMark score from it.
  [The cross-OS appendix](cross-os-primitives.md) says the binary "reports correctness, not yet a
  score", and the kernel test asserted only `freq > 0`.

### What the constant did reach

It reached every program's own sense of how long something took. That is not a published figure,
and it is not harmless. `Instant`, `thread::sleep`, the shell's `time`, `uptime`, and the timeouts in
`net_stack` all ran 2.5x off on radon, in the direction that makes everything look slower. On
`x86_64` the equivalent defect was a 1 GHz fallback, read by any process the userspace ELF loader
built, `coremark` included.

### The fix and the test

The fix widened a `cfg`. `x86_64` had already built the mechanism (`counter_frequency_contract`, a
page the kernel fills and maps read-only into every process), and riscv64 now uses it. An unknown
rate refuses rather than falling back. The test is what would have caught this in July:
`system_tests/src/user/counter_frequency_tests.rs` asserts that what a userspace program reports equals
what the kernel measured, on all three architectures. A test that compares a rate against a
constant cannot catch a constant, which is why `freq > 0` held for two months.

## 2026-09-21: the x86 boot calibration was wrong by up to 12x, and what that does to published numbers

The `tscdrift` lane measured the TSC against the CMOS RTC and found the counter ticks at exactly
1000.000 MHz under plain TCG. It also found that the rate the kernel *stores* had nothing to do with
that: twenty-two boots of one binary wrote down 1001 MHz to 4330 MHz. This lane fixed it, and this
section is what the fix means for anything already published. [`notes/tsc-under-tcg.md`](../tsc-under-tcg.md)
owns the measurement. The fix is `kernel/src/arch/x86_64/timer.rs` and milestone 571 (the x86 boot
calibrates the TSC once, and can be wrong by 4x).

### Two instruments wear the same name

`script/bench --x86` and `script/bench --x86 --real` are different instruments, and neither stands
in for the other. Without `--real`, `bench --x86` adds `-icount shift=0,sleep=off`. Under `-icount`
a guest nanosecond is a function of the instruction stream, not of real time: the `tscdrift` lane
measured the implied rate moving 37% between two workloads inside one boot. That is `-icount`
working as designed and is what makes the tripwire deterministic. It also means an icount leg's
`ns` column is not wall time and never was. Every appendix that quotes x86 numbers says which
instrument it used.

The calibration defect below affects the `--real` instrument only, and within it the `ns` column
only. Ticks are raw `rdtsc` counts and are untouched.

### What was wrong, and by how much

`init_frequency` timed one 10 ms PIT window by polling. The poll can only notice the terminal count
late, never early, and the whole TSC delta was divided by exactly 10 ms. So a single descheduling of
the QEMU thread inside that window inflated the stored rate without bound. Two hundred boots against
a counter known to tick at 1000.000 MHz, on a host deliberately saturated to load 30 on eight cores:

| Windows taken | Median error | 99th percentile | Worst of 200 | Boots wrong by >1% |
|---|---|---|---|---|
| 1 (what shipped) | +0.36% | +884% | **+1153%** | 56 / 200 |
| 3 | +0.00% | +120% | +131% | 24 / 200 |
| 5 | +0.00% | +20.6% | +51.4% | 11 / 200 |
| 9 | +0.00% | +0.49% | +7.1% | 2 / 200 |
| 16 | +0.00% | +0.01% | **+0.47%** | **0 / 200** |

Every one of the 490 boots measured across three host loads was high, never low. That signature
named the mechanism.

### Which published numbers this reaches

- The 2026-08-24 x86 `ns/iter` table (`yield_switch`, `tss_iomap_switch`, debug and release), in
  [the TSS I/O-bitmap appendix](x86-tss-iomap.md), is the one body of published x86 wall-clock
  figures derived from the stored rate. It is marked in place with what is still safe to quote. An
  inflated rate makes `ns/iter` too small, so those figures may read faster than the runs were.
  Every conclusion there that rests on a ratio between two rows of one boot survives, because the
  calibration cancels exactly in a ratio.
- The 2026-09-15 lazy-TSS measurement (same appendix) and the icount baselines are not affected.
  They quote deterministic ticks under `-icount`, and `bench/baseline-*.txt` holds ticks.
- `coremark`'s self-reported rate on x86 read the stored number, so any x86 CoreMark score computed
  from it carried the same error. No such score is published (see the riscv64 section above).
- `Instant`, `uptime` and the shell's `time` on x86 all read it through `counter_frequency_contract`'s
  page. Every process's sense of elapsed time was off by that boot's error.
- Nothing went red, which is why it lasted. `kernel::user::wait_for` takes `now()` plus two seconds,
  so an inflated rate makes a timeout longer in real time, never shorter. The defect could only fail
  safe.

### Before and after, as a controlled pair

Two `script/bench --x86 --real` runs of the same tree, differing in one constant
(`CALIBRATION_WINDOW_CAP` at 1 and at 16), so the before case is the old estimator exactly:

| | Implied stored rate | Error against 1000.000 MHz |
|---|---|---|
| Before (one window) | 1003.7 MHz | +0.37% |
| After (min of N) | 1000.1 MHz | +0.01% |

That before figure is one draw, and a lucky one. The 200-boot table is the real evidence: at this
host's load roughly one boot in four was wrong by more than a per cent, and the worst was +1153%.
Quoting the pair alone would understate the defect by two orders of magnitude.

The `ticks` columns of those two runs differ by up to 4x, and that is not the calibration. It is
host load between the runs, which is what `--real` measures and why `script/bench --x86` pins the
virtual clock by default. A wall-clock bench on a shared Apple Silicon host running TCG is a
magnitude to read, never a number to gate.

# RISC-V `map_new` and the RFENCE probe

*An appendix to [`notes/benchmarks.md`](../benchmarks.md), which carries the current numbers and
lets a reader act without opening this file. This one holds the 2026-08-15 `map_new` +15.6% and the
2026-08-17 probe that refuted its first reading, with the dates, tables and corrections behind them.
Name: ratified 2026-09-24 (calef); [the naming record](README.md) holds it.*

## 2026-08-15: `map_new` moved 15.6% on RISC-V, and the movement was a bug rather than a cost

The earlier re-saves ([the service-path appendix](service-path.md) holds two) are about the
instrument measuring a cost: a correctness fix with a price to pay, or a win it could not see.
This is the first time it caught a defect. It did so on a benchmark nobody was looking at, in a
change whose entire test suite was green.

Four pull requests from the VisionFive 2 lane family failed `script/bench --riscv --check` on the
same benchmark by the same amount, and a fifth from the same stack did not:

| tree | riscv64 `map_new` | note |
|---|---|---|
| `main` | within `2362 ± 236` | its push run executed the riscv leg rather than skipping it |
| #172, #173, #175, #176 | **2731 to 2732** (+15.6%) | tolerance is `±236`, so this fails by a wide margin |
| #178 | **2362, exactly** | `kernel/src/smp.rs` byte-identical to #176's |

The stack's change routes every per-cpu loop through `smp::online_cpus()` and masks the RISC-V
RFENCE calls with `online_harts_mask()`. So a machine whose online set is `{1,2,3}` stops being
indexed as `0..count` (see [notes/visionfive2.md](../visionfive2.md) and `crates/cpu_set`). The bench
boots a single hart. On one hart the online mask names one cpu and the shootdown has nobody to send
to. The correct instruction count for the masked version is the count for the unmasked one: 2362 is
right and 2731 is not.

### The first reading, kept because it is the plausible one

+370 ticks over 64 iterations was read as 5.8 instructions per map, the shape of "consult a mask
instead of a count on every TLB flush". That reading makes the regression the price of the
correctness fix. It leads to the action `script/bench` itself recommends on failure: rerun with
`--save` and commit the new baseline with the change that moved it. (The 2026-08-17 section below
shows the arithmetic was wrong: a tick is not an instruction.)

The third row of the table refuted it. #178's `kernel/src/smp.rs` is byte-identical to #176's, and
#178 measures 2362. A cost carried by the code cannot be absent from a tree that contains the same
code. So the extra work is something the mask should have prevented and did not, on the trees that
lack the later registration fix. The best available reading was remote RFENCEs issued on a
single-hart machine, from a mask that over-reports which harts are online. (Refuted on 2026-08-17,
below.)

Rebaselining was one command away. It would have written 2731 into `bench/baseline-riscv64.txt`,
and every later run of the tripwire would have been silent on the defect it had just caught. A green
tripwire counts as evidence, and this one would have been evidence of nothing.

### What was proven, what was inferred, and what would settle it

Proven: the four trees measure 2731 to 2732 and #178 measures 2362, against a baseline `main`
still satisfies. The `smp.rs` files are identical. All of it is from CI's own runs, not computed or
scaled, per the discipline the 2026-08-04 riscv64 re-save established ([the service-path
appendix](service-path.md)).

Inferred: that the delta is remote RFENCEs fired against an over-reporting mask. Nothing counted a
fence. The evidence was that the number returns to the baseline exactly rather than approximately,
which is what a path that stops executing looks like.

What would settle it: count RFENCE issues on a single-hart boot, or print `online_harts_mask()` at
bench time on both trees and compare. Neither was built on 2026-08-15.

### The case for the job's wall-clock

Nothing else noticed. `build + test (host + QEMU)` passed on #176. So did `cpu matrix (riscv64
across QEMU CPU models)`, which exists to boot RISC-V across CPU models. The kernel worked; it did
more than it needed to, on the one configuration where the extra work is provably unnecessary. A
bug that leaves behavior correct is invisible to every test by construction, and an instruction
counter is the only instrument here that can see it.

The stated purpose of milestone 21 (performance measurement) for the tripwire is catching "the *introduction* of performance
problems proximate to the changes that introduce them". This case is worth citing the next time
the job's five minutes come up for debate.

## 2026-08-17: the RFENCE probe was built, and it refuted the reading above

Both instruments the 2026-08-15 section named exist now: `arch::riscv64::remote_fence_count`,
the `bench-probe:` lines beside `map_new`, and `rfence_self`. The answer is not the one the reading
predicted.

### What the probe measures

| tree | `map_new` | remote fences in the timed window | `online_harts_mask` | `rfence_self` |
|---|---|---|---|---|
| `main` (53ca491) | 2362 | **0** | `0x1` | 9.81 ticks/call |
| pre-fix (593c00e) | 2361 | **0** | `0x1` | 8.91 ticks/call |
| **PR #176's head** (f601c6b) | **2362** | **0** | `0x1` | 8.91 ticks/call |

The third row is the tree CI measured at 2731, fetched from `pull/176/head` and run with the probe
cherry-picked onto it. Here it measures the baseline exactly, issues zero remote RFENCEs across
`map_new`, and reports a mask with one bit set. The mask does not over-report and there are no
fences to be the cost.

### The arithmetic error that made the wrong reading plausible

The 2026-08-15 section computed "+370 ticks over 64 iterations is 5.8 instructions per map". A tick
is not an instruction. The bench counter is `rdtime` at the machine's timebase, ~10 MHz on QEMU's
`virt`, and `-icount shift=0` makes one instruction one nanosecond of virtual time. So one tick is
~100 instructions, and the delta was ~577 instructions per map, not 5.8.

That kills the "plausible" reading from the other direction. Consulting a mask instead of a count is
a handful of instructions, ~0.05 ticks, invisible at this resolution. The section rejected it for the
right reason (a cost cannot be absent from a tree containing the same code), while its arithmetic
was two orders of magnitude out.

### What one RFENCE costs, and why it does not fit either

`rfence_self` prices the firmware path by naming this hart in the SBI hart mask, a legal call the
firmware serves with a local fence. It costs 8.9 to 9.8 ticks. So one extra remote RFENCE per map
would have moved `map_new` by ~570 ticks over 64 iterations. The observed move was 369, or 0.65
fences per map, which is not a whole number of anything.

### What is proven, what is not, and what to run next

Proven: three trees, including the accused one, issue zero remote RFENCEs during `map_new` and
carry a correct single-bit mask. One RFENCE costs ~9 ticks. The 2026-08-15 inference fails for PR
#176's tree.

The confound, and why it turned out small: the three runs above are on QEMU 8.2.2 with
`-device riscv-iommu-pci` removed. The QEMU in that container is not the pinned 11.0.2 and does not
implement the device. CI measured 2731 on 11.0.2 with the IOMMU present, so the worry was that the
regression belonged to the machine rather than the branch.

CI has since run this probe on the pinned machine. On QEMU 11.0.2 with the IOMMU present, `main`
reports `map_new_remote_fences 0`, `online_harts_mask 0x1`, and `map_new 2362`. The pair of numbers
either side of that machine change:

| | container (8.2.2, no IOMMU) | CI (11.0.2, IOMMU) |
|---|---|---|
| `map_new` | 2362 | **2362** |
| `rfence_self` | 8.91 ticks/call | **11.70 ticks/call (+31%)** |

`map_new` is bit-identical across the machine change while the RFENCE benchmark moves 31%. A
benchmark made entirely of SBI calls is visibly sensitive to the firmware and emulator, and
`map_new` is insensitive to them, which is what a path making no SBI calls looks like. The fence
counter and the machine-sensitivity say zero independently. So the container's 2362 for
`pull/176/head` is very unlikely to be a machine artifact, and the refutation stands.

Still unexplained is the observation: CI measured 2731 to 2732 on four branches, reproducibly, in
August, and no live hypothesis remains. The experiment that would speak to it is the probe run
against `pull/176/head` in CI, on the pinned machine; a lane can do that deliberately and nothing
does it by accident. Until then the 2026-08-15 diagnosis is withdrawn rather than replaced.

### What survives of the 2026-08-15 section

Rebaselining would still have been the expensive mistake. Writing 2731 into the baseline would have
silenced a real difference nobody had explained. The tripwire's value here was never the
diagnosis, which was wrong. It was that the number moved and stayed unexplained until somebody
measured. This correction lands on the reading, not the instrument.

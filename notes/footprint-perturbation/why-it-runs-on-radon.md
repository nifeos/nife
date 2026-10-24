# Why the experiments moved to radon, and what the PMU adds

*Name: ratified 2026-10-18 (calef, "Approve as a group"). Refused `background`.*

An appendix to [notes/footprint-perturbation.md](../footprint-perturbation.md). It holds why this page
exists, the three obstacles that kept E1, E3 and E4 off the board, the static footprint table, and
what the riscv64 half of milestone 74 (cycle counters) adds and cannot see.

## Why this page exists: an instrument aimed at the wrong machine

The E3 of milestone 134 (the register of measures) pads the IPC fastpath with resident dead code, and
asks whether latency moves. Its own block states the asymmetry that makes it worth re-taking:

> A *positive* result on the dev Mac is conclusive: if padding hurts on a machine with large caches,
> it certainly hurts on a 32 KB L1i. A *negative* result on that machine proves little, because an
> M-series core may simply absorb the whole path.

E3 ran on patagonia on 2026-08-22 and read 2 to 3% between the padded and un-padded builds, inside
run-to-run noise. That is the weak direction. E1 (IPC latency against thread count) and E4
(application displacement) are in the same position. Both found small, direction-consistent effects
that their own notes attribute to the dev Mac's large L1d muting the knee.

radon is the machine all three were designed against: a StarFive VisionFive 2, four SiFive U74 cores,
32 KB L1i and 32 KB L1d each (notes/benchmarks/kernel-footprint-and-caches.md's machine table). E1's
prediction was computed against that number by name. Until 2026-09-04 none of the three could run
there.

## What was in the way, and what closed it

Three separate things, and only the first was obvious.

| the obstacle | why it blocked | what closed it |
|---|---|---|
| E1 and E4 were `#[cfg(target_arch = "aarch64")]` | this tree has no riscv64 *accelerator* with a real cache, so they were gated on the accelerator rather than on the cache | the cfgs are `any(aarch64, riscv64)`; the TCG self-skip is now a per-arch counter frequency (10 MHz on QEMU `virt`, 4 MHz on the JH7110) rather than an arch-specific one |
| both require one hart, and a card has no `-smp 1` | radon seats four U74s, so a `board,bench` card would boot four and both benchmarks would print a skip line on the machine they were built for | the `single_hart` kernel feature: `smp::bring_up_secondaries` marks the boot core online and starts nobody |
| the padding was reachable only from `ipc_send` | milestone 188 phase 1 split the footprint gate into two closures and found the CALL/reply one is larger and is the shape real services run; the pad landed on the other one | `ipc_call` calls `maybe_pad` too |

That third row is a finding, not a chore, and it is the reason this lane touched the kernel at all.
Measured on riscv64 before the fix, `--features fastpath_pad` moved `ipc_send_receive` to 2.10x and
`ipc_call_reply` to 1.00x. E3 as built was padding a shape nothing in this tree runs. It was correct
when it was written: the split did not exist on 2026-08-22, and `ipc_fastpath` was one number. Anyone
who had taken E3 to the board in the four days after milestone 188 (the IPC fastpath) landed would
have measured the padding of a path their own benchmark barely uses.

## The static half, which needs no board and is already taken

`script/fastpath-footprint` is `objdump` over the built kernel. Run on patagonia, 2026-09-04, with
`ipc_call` padded:

| ISA | shape | un-padded | padded | ratio |
|---|---|---|---|---|
| riscv64 | `ipc_send_receive` | 4,632 | 9,726 | 2.10x |
| riscv64 | `ipc_call_reply` | 5,936 | 11,070 | 1.86x |
| aarch64 | `ipc_send_receive` | 5,356 | 11,192 | 2.09x |
| aarch64 | `ipc_call_reply` | 7,028 | 12,860 | 1.83x |

Read the riscv64 `ipc_call_reply` row against 32 KB. 5,936 bytes is 18% of radon's L1i; 11,070 is
34%. The padded build still fits. That is exactly the condition that makes the experiment interesting
rather than trivial: this is a footprint change large enough to matter under contention, and small
enough that a naive "does it still fit" reading predicts no effect at all.

`ipc_fastpath` is the max of the two shapes. So it reads 1.86x on riscv64, rather than the 2.00x the
2026-08-22 run recorded against the old single-closure number. That is the same padding measured
against a larger denominator, not a weakening of the pad. "Roughly double", which is the block's own
wording, still holds.

x86_64 is not padded on either shape: `kernel/src/arch/x86_64/fastpath_pad.rs` does not exist. The
scope note in [notes/x86-port.md](../x86-port.md)'s `BUGS` says why, and what would make it a plan.

## What milestone 74's riscv64 half adds, and what it still cannot see

`kernel/src/arch/riscv64/pmu.rs` landed 2026-09-04, and reads real cycles through the SBI PMU
extension. It was not available when E3 was designed, and E3 was designed to work without it. The
whole point of padding is that it tests Liedtke's claim with no cache counter, by making a static
footprint tool agree or disagree with a wall clock.

What the PMU adds here is precision, not a new answer. Every row is a `rdtime` tick count at 4 MHz,
which is a 250 ns quantum; a 2% effect on a low-microsecond round trip is a handful of ticks. Cycles
at the core clock resolve that by three orders of magnitude. They remove the one methodological
complaint nobody could answer on patagonia, which is whether a small percentage was a real effect or a
timer artifact.

What it still cannot see is the mechanism. A cycle counter says a round trip got slower; it does not
say the instruction cache is why. The direct measurement is M6, instruction-cache misses per IPC, and
nothing in this tree reads a cache-miss counter on any architecture. Milestone 134's Tier B says so.
Its own BUGS warns that real PMUs do not implement every architected event, so whether the U74 counts
what M6 wants is unverified. Until then E3 remains what it was designed to be: an inference from a
perturbation, not an observation of a cache.

Wiring the PMU into these rows is `design/roadmap/0374-cycles-per-ipc-on-the-bench-card.md`.

Since 2026-09-16 the conversion exists without that wiring. radon measured `cycles_per_tick 250.00`
(milestone 74's block, transcript `bench/radon-2026-09-16/bench-134300.log`). So every tick row
converts at 250 cycles per tick: the unpadded `call_reply` is about 1,254 cycles, and the padded one
1,272. And the 2026-09-04 capture answers the question that proposal said to wait for, which is
whether the 250 ns quantum was the binding problem. It was not. The within-condition spread was 0 to
2 ticks against a gap of 74, so E3's difficulty is the layout confound, not the ruler.

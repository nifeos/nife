# The kernel footprint and the cache question

*An appendix to [`notes/benchmarks.md`](../benchmarks.md), which carries the current numbers and is written so a reader can act without opening this file. This one holds image size, the hot path by symbol, the L1i figures, and the 4 KiB target, with the dates, tables and corrections behind them. Name: ratified 2026-09-24 (calef); [the naming record](README.md) holds it.*

## The kernel's memory footprint, and the cache question Mach got wrong (2026-08-17)

calef asked how large the kernel is in memory. He then asked the better question: what size avoids
the cache thrashing that made Mach slow, which L4 and seL4 were built to fix. The two questions have
different answers.

Measured on `main` at 6e97bb1, release profile, both ISAs.

### The image, which is the number that does not matter

| | aarch64 | riscv64 |
|---|---|---|
| `.text` | 172,032 | 143,360 |
| `.rodata` | 32,768 | 36,864 |
| `.data` | 45,056 | 45,056 |
| `.bss` | 40,960 | 28,672 |
| **kernel proper** | **290,816 (284 KiB)** | **253,952 (248 KiB)** |
| `.secondary_stacks` | 557,056 | 557,056 |
| `.interrupt_stacks` | 163,840 | 163,840 |
| **total** | **1,011,712 (988 KiB)** | **974,848 (952 KiB)** |

The flat binary QEMU loads is 249,856 bytes on aarch64 and 225,280 on riscv64. That is exactly
`.text + .rodata + .data` to the byte, which confirms the rest is `NOBITS`: reserved at runtime,
zero file bytes.

The stacks are 70% of the total and are not a code-size fact. `.secondary_stacks` is `MAX_CPUS` (8)
times `SECONDARY_STACK_SLOT` (64 KiB plus a 4 KiB guard) = 557,056. `.interrupt_stacks` is 8 times
(`interrupt_stack::SIZE` 16 KiB plus a 4 KiB guard) = 163,840. Both are reserved for eight cores
whether eight exist or not, so a single-hart boot never touches seven eighths of 704 KiB. Runtime
allocation is separate again: page tables, and 28 KiB per thread for a kernel stack
(`thread::STACK_PAGES` is 6, plus a guard page).

### What Mach actually paid for, which is a different quantity

Liedtke's *On µ-Kernel Construction* (SOSP 1995) argued that Mach's IPC cost was dominated by the
cache working set of the hot path, not by anything inherent to microkernels. A kernel that touches a
lot of memory per IPC evicts the application's working set. The cost then appears as capacity misses
spread through the workload rather than as time spent in the kernel. L4 was written against that:
the original i386 kernel was around 12 KB of hand-written assembly, sized so the hot path lived in
L1. The seL4 kernel keeps the same idea as a deliberately maintained fastpath for the common IPC case, which
bypasses the general dispatch.

So the image size above is close to irrelevant to this question. `.text` that never runs during an
IPC costs nothing in cache.

### Our hot path, by symbol size (aarch64 release)

| on the path | bytes |
|---|---|
| `exception_dispatch` | 124 |
| `syscall::dispatch` | **2,024** |
| `sched::current_cap` | 376 |
| `sched::ipc_send` | 952 |
| `sched::schedule` | 1,240 |
| `sched::finish_switch` | 816 |
| **one-way send** | **5,532 (5.4 KiB)** |
| plus `sched::ipc_receive` (1,320) | **6,852 (6.7 KiB) round trip** |

`switch_to` and `dispatch_on_interrupt_stack` are assembly and report no symbol size. They are
missing from the sum, so the real figure is a little higher.

Data touched per round trip: two `TrapFrame`s (288 bytes, asserted in
`arch/riscv64/exceptions.rs`), two `Thread`s (744 bytes, from the census in milestone 106 (a wait that ends on either the interrupt or the deadline)), the endpoint,
and the run queue. Roughly 2 to 3 KiB, about 40 cache lines of 64 bytes.

`syscall::dispatch` is 37% of the instruction path on its own. It is a general decoder over every
object type and method, which is exactly what seL4's fastpath exists to skip. It is the first place
to look if this ever needs to shrink.

### Which cache we are optimizing for, and why the constraint has loosened but not vanished

calef's point, and it is right in one direction and wrong in the other.

Right: capacity grew. Liedtke was writing against i486 and Pentium L1 caches of about 8 KB, which is
why a 12 KB kernel was a tight fit. The machines this project actually runs on:

| machine | L1i | L1d | L2 | role here |
|---|---|---|---|---|
| SiFive U74 (VisionFive 2) | 32 KB | 32 KB | 2 MB | first silicon |
| Cortex-A57 (Jetson TX1) | 48 KB | 32 KB | 2 MB | milestone 127 (the seL4 machine) |
| Core i5-7500T (OptiPlex) | 32 KB* | 32 KB* | 256 KB* | milestone 87 (the x86_64 bare-metal machine), x86_64 first light |
| Apple M-series P-core | far larger | far larger | many MB | the bench and dev host |

\*xenon's row is from the Kaby Lake microarchitecture's published figures and has not been read off
the machine, the distinction this tree's fabricated-quote scar exists to keep. It can be settled the
moment xenon boots: `CPUID` leaf 4 reports cache size, ways and line size per level, and the boot
tour already decodes `CPUID`. Until then treat the asterisked cells as a strong prior rather than a
measurement.

xenon is the binding case. It pairs the
smallest L1i of the three targets with the largest fastpath: x86_64 measured 8,404 bytes against
riscv64's 7,174 and aarch64's 9,156 at the time. If any machine tests Liedtke's argument first, it
is this one. The omission has the same shape the [gate appendix](fastpath-footprint-gate.md) records
about the gate itself: x86_64 arrived after the argument was written and was fitted in afterwards.

That is four to six times Liedtke's budget on the small machines and far more on the host.

Wrong, or at least incomplete: the penalty grew faster than the capacity. A main-memory miss in 1995
cost single-digit cycles against a slow clock; on a modern core it is a few hundred. Capacity went up
perhaps six times on the machines we care about, while the cost of exceeding it went up by
considerably more. What changed in our favor is L2 more than headroom. A large on-die L2 means
overflowing L1 now costs tens of cycles rather than a trip to DRAM, a safety net Liedtke's low-end
targets did not have.

These board figures should still be confirmed against the silicon. The TX1's are worth taking from
the machine when it arrives rather than from a datasheet summary.

### Where the frontier actually is, which sharpens the paragraph above (2026-08-18)

calef asked the follow-up: where are frontier RISC-V, ARM and x86_64 caches now. Looked up rather
than recalled, because the paragraph above was written from memory and one of its claims does not
survive the numbers.

| core | ISA | L1i | L1d | L2 (per core) |
|---|---|---|---|---|
| AMD Zen 5 | x86_64 | **32 KB** (8-way) | 48 KB (12-way) | 1 MB (16-way) |
| Intel Lion Cove / Cougar Cove | x86_64 | **64 KB** | 192 KB, plus a 48 KB L0 | 2.5 to 3 MB |
| Arm Cortex-X925 | aarch64 | **64 KB** (4-way) | | 2 or 3 MB, private |
| SiFive P870 | riscv64 | **64 KB** | | configurable, 4 MB in their example |
| Apple M4 / M5 P-core | aarch64 | **192 KB** | 128 KB | shared, several MB |

The correction: L1i has not been growing. The paragraph above says capacity grew four to six times
and implies the trend continues. It does not. Frontier L1i clusters at 64 KB, Zen 5 is still at
32 KB and unchanged from Zen 4, and only Apple is an outlier at 192 KB. L1i is latency-critical and
area-expensive, so it has sat between 32 and 64 KB across every vendor for roughly a decade. What
actually ballooned is L2, from nothing or a small off-die cache in Liedtke's era to 1 to 3 MB
private per core today.

So the constraint loosened once, decades ago, and then stopped. The win is not L1 headroom; it is
that L2 turned an L1 overflow from a DRAM trip into tens of cycles. The fastpath discipline still
applies to L1i, and the number to respect there is 32 to 64 KB, not something that grows every
generation.

One figure was refused rather than repeated. A search result claimed the Ventana Veyron V2 has
512 KB of L1 instruction cache. No shipping core has an L1i anywhere near that, and it is
inconsistent with every other datapoint in the table, so it is almost certainly garbled or
mis-attributed. It is recorded here because the next person to look this up will hit the same
result.

This does not move the target. The machines this project runs on are not frontier parts: the U74's
32 KB L1i remains the binding constraint, and it sits at the bottom of the frontier range anyway. A
4 KiB fastpath is about an eighth of that, a sixteenth of a 64 KB frontier L1i, and a rounding error
against Apple's 192 KB.

### The target

Expressed as a fraction of the smallest L1i among machines we actually run on (32 KB, the U74), so
it tracks the board list rather than a number somebody liked:

- IPC fastpath instructions: under 4 KiB, about an eighth of that L1i.
- Data touched per IPC: under 1 KiB, about 16 cache lines.
- The whole-kernel image: no target at all. Optimizing it would be optimizing the wrong thing.

The reasoning behind the fraction is Liedtke's rather than a round number. The constraint is not that
the kernel fits; it is that the kernel leaves most of L1 intact for the application, because cache
pollution is what Mach actually charged its users.

On 2026-08-17 the one-way send measured 5.4 KiB and roughly 40 cache lines: the right order of
magnitude, and not comfortable. The shape the system runs is worse. Measured 2026-09-21 on
`nightly-2026-09-20`, `ipc_call_reply` is 6,038 bytes on riscv64, 7,104 on aarch64 and 8,234 on
x86_64: 1.47x to 2.01x this target, and 18% to 25% of the U74's L1i. Those are the current measured
figures. The stored baselines in `bench/fastpath-*.txt` (7,028 / 5,936 / 8,122 for aarch64, riscv64
and x86_64) are the milestone 188 (the IPC fastpath) readings and sit slightly below them.
`script/fastpath-footprint` prints both ratios on every run; the
[gate appendix](fastpath-footprint-gate.md) has the output and the reason.

Whether 4 KiB is the right number is a measurement nobody has taken. calef ruled on 2026-09-21 that
the target is reported and not gated, and that the question waits on milestone 370 (a layout
control). 370 is built, and it is not enough: its padding is never executed. The experiment that
would settle it on radon, with layout held fixed and a decision rule written first, is E5 in
[`notes/footprint-perturbation/executed-footprint.md`](../footprint-perturbation/executed-footprint.md).
It needs a build lane before it needs an evening.

### The hot section (2026-10-07)

Milestone 796 (pin the hot trap path's placement). On 2026-10-05 radon's one-task `null_syscall`
read 110, 118, 116 and 111 ticks across four builds that only shifted kernel text, and a merge that
added no instruction to the trap path moved it 16
([`notes/job-mix/radon-2026-10-05.md`](../job-mix/radon-2026-10-05.md)). So every function a
syscall or an IPC round trip runs now lives at one address in every build.

**How.** Each such function carries a `.text.hot.<name>` section: `#[link_section]` on the Rust
(gated on `target_os = "none"`, since the prover builds `kernel/src` for the host), a `.section`
directive in the assembly. Each ISA's linker script gathers them right after the boot stub,
`SORT_BY_NAME` so the order does not follow rustc's codegen units, from a 16 KiB boundary. That is
the U74's L1i way (32 KiB, two-way), so the block's sets do not depend on the boot stub's size, and
nothing else in `.text` precedes it. The set is `script/fastpath-footprint`'s: both IPC closures
plus the entry set, 15 to 18 symbols per ISA, plus aarch64's `memcpy` by input-section name.

**What it changed, statically**, on the same commit:

| | riscv64 | aarch64 | x86_64 |
|---|---|---|---|
| hot lines (64 B) | 151 to 139 | 195 to 184 | 183 to 170 |
| pages | 10 to 3 | 14 to 3 | 14 to 3 |
| L1i sets holding more than two hot lines | 2 to 0 | (no geometry read) | (no geometry read) |
| `.text.hot` | 8,874 B | 11,724 B | 11,125 B |

The cost is alignment padding: `.text` grew 16 KiB on aarch64 and riscv64 and 8 KiB on x86_64,
in an image of about 3 MiB. The instructions did not change (the `--layout` hash differs only where
objdump names `yield_now`, an alias of `schedule`, by its other name).

**What keeps it.** `script/fastpath-footprint` fails when a hot symbol is outside
`__text_hot_start`..`__text_hot_end` or the block is off its boundary, on every ISA, and names the
function to give a section to. On aarch64 and riscv64 it also builds two `fastpath_pad` kernels
20,520 bytes of text shift apart and fails if any hot symbol's address or size differs. Both were
broken on purpose before landing and both failed.

**What radon shows.** The milestone's sweep, repeated on 2026-10-20 with the pin, read the one-task
`null_syscall` at 121 ticks in all four shifted images, where the unpinned 2026-10-05 sweep read 110
to 118 ([`bench/radon-2026-10-20/`](../../bench/radon-2026-10-20/README.md)). The number no longer
moves with kernel text. What it has not shown is why it moved: the leading reading was hot code
evicting itself, which a contiguous block under one way cannot do, but nothing counted misses.

#### BUGS

- The userspace stub is not pinned. Its sets come from `crates/user_mode_runtime/link.ld`, and it
  shares index bits with the kernel's block. Option 2 in the milestone's block, held until a cross-build comparison shows the stub moving the number.
- Placement is fixed; alignment within the block is not. Five of riscv64's seventeen hot symbols
  start 8-byte aligned, which the U74's BTB wants for a zero-bubble taken branch (the U74-MC core
  manual, section 4.2.6). A hot function that grows still moves the ones sorted after it.
- RISC-V reach is a placement fact. A call is four bytes or eight depending on how far it goes,
  so past 1 MiB of text unrelated growth could change hot code size. A plain `j` into the block can
  also fall out of range: `user_return`'s `j trap_return` failed to link in the debug test image on
  2026-10-07, and is a `tail` now. Hand-written jumps into `.text.hot` from outside it must be `tail`
  or `call`.
- x86_64 has no `fastpath_pad`, so it gets the containment check and not the shift check.
- aarch64 pins the whole 2 KiB vector table for the one 128-byte slot a syscall runs, and x86_64
  pins `isr_common` because it falls through into `isr_restore`.

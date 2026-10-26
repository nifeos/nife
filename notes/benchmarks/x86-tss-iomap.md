# The x86 TSS I/O-bitmap switch cost

*An appendix to [`notes/benchmarks.md`](../benchmarks.md), which carries the current numbers and
lets a reader act without opening this file. This one holds the naive and lazy bitmap writes that
§121 (what a device capability is when the device has no page) asked to have priced, with the dates,
tables and corrections behind them. Name: ratified 2026-09-24 (calef); [the naming
record](README.md) holds it.*

## 2026-08-24: the TSS I/O-bitmap switch cost (DECISIONS §121's amendment)

§121 is choosing how x86 userspace drivers reach legacy port-I/O devices: the UART, the PIT, the
8259s, the CMOS clock. Its option 1 is a port-range capability enforced by the TSS's I/O permission
bitmap. The 2026-08-24 amendment names that option's dominant unmeasured cost: writing the bitmap
into the current CPU's TSS on every context switch. This section is that measurement, taken as soon
as milestone 161 (the x86_64 kernel port) item 4 made real two-thread switching exist on `x86_64`.

### A third instrument, and why the other two do not apply

`crate::arch::timer::now()` already dispatches to `rdtsc` on `x86_64`, calibrated against the 8254
PIT at boot (`kernel/src/arch/x86_64/timer.rs`). So `kernel/src/bench.rs`'s `timed()` helper needed no
change to run on this ISA. What was missing is everything around it:

- No icount leg, at the time this was written. `icount()` in `xtask` refused `--arch x86_64`, and
  this section inferred that nothing pinned QEMU's virtual clock to the instruction stream on this
  port either. *(Correction, 2026-08-25: that inference was wrong. `icount()`'s refusal stands, but a
  plain duration measurement under `-icount` works on x86_64; see
  [x86_64 instruments](x86-instruments.md).)*
- No HVF, no KVM. The dev machine is Apple Silicon, with no hardware acceleration for `x86_64`. Every
  number below is plain QEMU TCG, translating x86 instructions on an aarch64 host one at a time. That
  is slower than real silicon and slower than KVM, by an unknown ratio, so magnitudes here are not a
  stand-in for real x86 hardware. What is real is the comparison within one boot: two benches, same
  host, same QEMU process, same instant, differing by one write.
- The bench boot needed a home on this ISA. `kernel_main`'s `x86_64` arm was a fixed tour with no
  `#[cfg(feature = "bench")]` branch; the other two architectures have had one since milestone 21 (performance measurement). It
  now diverges into `bench::run()` right after `smp::bring_up_secondaries()`, where the aarch64 half
  of `kernel_main` does. `cargo xtask bench --x86` builds and runs it; see `bench_x86()` in
  `xtask/src/bench.rs`.

Every EL0-plane bench (`null_syscall_el0`, `ctx_switch_el0`, `ipc_rtt_el0`, `sink_throughput`,
`map_el0`, `spawn_el0`) self-skips on this leg through its existing mechanism. `crate::user::program`
finds nothing, because `crates/user_mode_runtime` has no `x86_64` arms yet, so it took no new
gating. `fs_read`, `fs_throughput` and `smp_throughput` self-skip as they do on a single-hart
`--real` run elsewhere. What runs cleanly is the kernel-thread plane: `yield_switch`, `ipc_rtt`,
`relay_rtt`, `call_reply`, `broker_rtt`, `spawn_reap`, `map_new`, `coremark`, plus one new x86-only
bench.

### `tss_iomap_switch`: `yield_switch` plus one write

The x86 port space is 16 bits (64 Ki ports) with one permission bit each, so the real bitmap is
`65536 / 8 == 8192` bytes exactly. The "8 KiB" in §121's text is architecture, not a round number.

`tss_iomap_switch` is byte-for-byte `yield_switch`: the same two threads, the same
`YIELD_ITERS = 2000`, the same warmup. It adds one call on every resume, in both threads:
`arch::segments::bench_write_io_bitmap`. That call `write_bytes`-fills a CPU-owned 8192-byte static
and reads its last byte back, so the write is not provably dead code.

It is a stand-in for the write, not for the enforcement. `iomap_base` in the live TSS is untouched,
`ltr` is never reissued, and no ring-3 program executes `in`/`out` in this boot. There is no ring-3
program on `x86_64` yet outside the hand-assembled ones in `user::x86_programs`. What it prices is the
cost the amendment named: an 8 KiB per-CPU memory write added to the switch path. It runs twice per
iteration (each thread resumes once), so the delta divided by two is the cost of one write.

### The numbers

> **CORRECTION, 2026-09-21: every `ns/iter` in this section is suspect, and the `ns` column alone.**
> These were computed from the guest's calibrated TSC. On 2026-09-21 the x86 boot calibration was
> found to be wrong by up to +1153%, always high, differently on every boot. An inflated rate makes
> `ns/iter` proportionally too small, so these figures may read faster than the runs were, by
> anything from a fraction of a per cent to a factor of several. The true values cannot be
> recovered: the rate each of these eleven boots stored was not written down. The ticks behind them
> are unaffected. So is every conclusion here, since each rests on a ratio between two rows of one
> run, where the calibration cancels exactly: the debug-versus-release argument and the 4.2x. What is
> not safe is quoting any single figure below as a nanosecond count. See
> [counter frequency and x86 calibration](counter-frequency-and-calibration.md) and
> `kernel/src/arch/x86_64/timer.rs`'s `BUGS`.

QEMU 11.0.2 (`.qemu-version`, pinned), `-machine q35 -cpu max`, one hart, plain TCG. Six boots debug,
five release. `ns/iter` is computed from the guest's own calibrated TSC, the arithmetic `xtask`'s
`run_bench` does for every other leg.

| build | bench | ns/iter, median | all runs |
|---|---|---|---|
| debug (6) | `yield_switch` | **12,320** | 12172, 12483, 12022, 12293, 12456, 12347 |
| debug (6) | `tss_iomap_switch` | **15,360** | 15286, 15342, 14944, 15378, 15494, 15751 |
| release (5) | `yield_switch` | **1,267** | 616, 1654, 678, 1267, 1406 |
| release (5) | `tss_iomap_switch` | **6,769** | 4603, 7741, 3935, 6830, 6769 |

| build | delta (`tss_iomap_switch` − `yield_switch`), median | per single 8 KiB write (delta / 2) | overhead over a bare switch |
|---|---|---|---|
| debug | 3,040 ns/iter | **~1,520 ns** | +25% |
| release | 5,363 ns/iter | **~2,682 ns** | +423% |

Read the two rows together. Debug's ~25% looks tolerable; release's ~4.2x is the honest number. A
debug build carries so much fixed overhead around the switch (unoptimized bookkeeping, unelided
checks) that the 8 KiB write is a modest fraction of a slow baseline. In release the baseline switch
gets ~10x faster (12,320 ns to 1,267 ns), while the write's own cost barely moves (~1,520 ns to
~2,682 ns, both plain memory bandwidth). On the switch path the write would actually run on, it
dominates the cost. §121's amendment called it "the dominant cost" from architecture, before any
number existed, and against a release-shaped kernel it undersold it.

The release row is noisy. Its five runs span 3,935 to 7,741 ns for `tss_iomap_switch`, roughly 2x
peak to trough, against debug's 14,944 to 15,751. Plain TCG with no `-icount` runs on the host's wall
clock. A release iteration takes microseconds, so host scheduling jitter on a shared dev machine is a
real fraction of the window; a debug iteration takes tens of microseconds, so the same jitter is
proportionally smaller. That is why `--real` never gates: a wall-clock magnitude is something to read
as a median over single runs. `--real` is still the only mode `--release` can use on this ISA; see
[x86_64 instruments](x86-instruments.md).

### What this does and does not settle

It gives §121 the number its amendment asked for, on both sides of the 1-versus-3 call. Option 3's
cost was already on record: ~337 ns per IPC round trip, which §121 cites from the cross-OS table in
[cross-OS primitives](cross-os-primitives.md). Option 1's is now on record too: ~1.5 to 2.7 us per
switch for the write alone. That is before the capability type, the revocation shootdown, or anything
else option 1 would also cost. Both are worse than a raw `in`/`out` instruction (single-digit cycles).
That makes option 2, keeping legacy devices in the kernel, the correct default absent a reason to want
a userspace console on `x86_64`. The conclusion is the amendment's own, now with a number under its
option-1 half.

It does not settle which option §121 picks. That is an architect's call per the decision's closing
question; this section changes only what he decides with.

It does not build option 1 either. There is no port-range capability, no `MemoryRegion::SPLIT`-derived
granting and no syscall surface change. `bench_write_io_bitmap` is deliberately not wired to the live
TSS's `iomap_base`, so nothing here could pass for the real mechanism.

*(Correction, 2026-09-15: §121 was since reversed to build option 1 as milestone 299 (the x86 port-range capability), with the lazy
write below.)*

## 2026-09-15: the LAZY TSS I/O-bitmap write, the number §121's refinement asked for (milestone 299)

DECISIONS §121 was reversed on 2026-09-15 to build the port-range capability (milestone 299). Its
2026-08-25 refinement is now binding rather than a footnote. The ~2,682 ns per write above prices the
naive always-write of the whole 8 KiB bitmap on every switch. The real implementation is the lazy
write: `arch::segments::set_port_grant` compares the incoming thread's grant against what the core
already holds, and writes only the bits that move. This section times the lazy write against a bare
switch, as the section above timed the naive one.

Two new benchmarks join `tss_iomap_switch` in the x86 bench boot (`kernel/src/bench.rs`). Both are the
same two-thread yield ping-pong, differing only in what each thread installs on resume:

- `tss_iomap_lazy_switch`: the peer installs `None` (no ports) and the main thread installs COM1's
  `(0x3F8, 8)` range. Every switch is then a holder<->non-holder transition, which takes the write.
  This is the worst case, not the common one.
- `tss_iomap_lazy_nop`: both threads install `None`, so `set_port_grant` finds the core already holds
  `None` and returns after one comparison, writing nothing. Nearly every switch on nearly every
  machine takes this case.

Measured on the pinned QEMU under TCG plus icount, which gives deterministic instruction-count ticks
(the x86 icount leg from [x86_64 instruments](x86-instruments.md)). Debug build, 2000 iterations, one
run each, since icount is reproducible:

| benchmark | ticks/iter | delta over `yield_switch` |
|---|---|---|
| `yield_switch` (bare two-switch round trip) | 10,580 | baseline |
| `tss_iomap_lazy_nop` (nothing holds a port) | 10,796 | **+216** |
| `tss_iomap_lazy_switch` (every switch crosses a holder) | 12,567 | **+1,987** |
| `tss_iomap_switch` (naive 8 KiB always-write) | 12,953 | **+2,373** |

The second row is the load-bearing one. Where nothing holds a port (every switch but the console
driver's), the lazy mechanism adds +216 ticks per iteration, ~108 per switch: a load of the per-core
"installed grant" and a branch. That, not the ~2,682 ns naive write, is what x86 pays for having the
enforcement present.

The lazy-switch and naive rows look close under icount, +1,987 against +2,373, only ~16% apart. The
byte counts (2 bytes moved against 8,192 written) would suggest an order of magnitude. The instrument
is the reason: the naive `write_bytes(_, 8192)` compiles to a `rep stos`, a handful of instructions
for 8 KiB of memory traffic, and icount counts instructions, not traffic. That is why §121 measured
the naive write in wall-clock. So the lazy form's advantage is mainly not fewer instructions on a
holder crossing, though it has slightly fewer. It has two parts. The crossing almost never happens, so
the common switch pays the +216-tick nop path. And a crossing moves ~2 bytes instead of issuing an
8 KiB `rep stos`, a memory cost icount cannot see and a cache can.

The magnitudes are fiction. TCG models no caches, so the ns the tool would print are meaningless and
only deterministic ticks are quoted. There is no HVF for x86_64 on this Apple Silicon host, so no
`--real` magnitude leg exists here as it does for aarch64. The naive row here (+2,373 ticks, icount)
and the naive row above (~2,682 ns, wall-clock release) are the same benchmark measured two ways, and
the numbers do not compare. Both say the always-write is the dominant cost of a switch. This section
adds that the lazy write moves that cost off every switch that does not cross a holder.

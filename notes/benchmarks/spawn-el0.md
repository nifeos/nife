# `spawn_el0`: a ceiling, and a page

*An appendix to [`notes/benchmarks.md`](../benchmarks.md), which carries the current numbers and is written so a reader can act without opening this file. This one holds the 2026-08-27 occupancy bound and the 2026-09-21 current-CPU page, with the dates, tables and corrections behind them. Name: ratified 2026-09-24 (calef); [the naming record](README.md) holds it.*

## 2026-08-27: raising a ceiling made `spawn_el0` 16.5% slower, and the fix made it 41% faster than it had ever been

The `--check` tripwire caught `sched::MAX_THREADS` going 128 to 256. `spawn_el0` measured 2,418,606
ticks against a 2,070,473 baseline, +348,133 (+16.8%), well outside the ±10% band. Every other row
was flat.

### Attribution, by measurement rather than arithmetic

The technique is `1259dc07`'s: hold everything constant and remove one suspect at a time. All four
numbers are `spawn_el0`, aarch64, TCG + icount, at `MAX_THREADS = 256` unless stated.

| configuration | ticks | attributed |
|---|---|---|
| baseline, `MAX_THREADS = 128` | 2,070,473 | (reference) |
| everything at 256 | 2,418,606 | +348,133 total |
| 256, capability sweep stubbed out | 2,277,247 | the sweep = **141,359** |
| 256, `revoke::MAX_SPACES` pinned at its old 160 | 2,280,022 | the registry = **138,584** |

There are two causes of nearly equal size, not one. The first was predicted.
`sched::delete_page_frame_caps_where` walks every thread's capability table on every
`MemoryRegion::DESTROY`. `generational_table::iter_mut` yields only live entries but visits every
slot to filter them, so its cost tracked `MAX_THREADS` rather than the number of live threads.

The second was not predicted. `revoke::MAX_SPACES` is derived from `MAX_THREADS`, and the registry is
a plain array walked linearly. `forget_root` scans all of it unconditionally on every
`AddressSpace::drop`. The two do not quite sum to the total (280k of 348k). The remainder was not
chased separately, because the fix below removed all of it.

### The fix, and why it is not a re-baseline

Both are the same disease: a scan whose cost tracks a ceiling rather than occupancy. Both get the
same cure, a `top` field holding one past the highest live slot, with every walk bounded by it. The
invariant that makes it sound is that every slot at or above `top` is empty. A Kani harness and two
host tests in `generational_table` carry it.

Re-baselining was available and was the wrong move, for the reason the maintainer who caught this
gave. Live threads peaked at 130 and did not move; only the ceiling did. Paying 16.5% for slots
nothing occupies attaches a cost to the wrong thing, and baking it in would make every future raise
pay again.

### What it actually measured

`spawn_el0` is now 1,212,888 ticks: 41% faster than the 128-slot baseline it was supposed to be
restored to. The bound removed cost that had been there all along. The bench boot holds far fewer
threads and address spaces than either ceiling allows, so both walks were mostly visiting slots that
had never been occupied. Every other row is within noise (`map_el0` +0.5%, `spawn_reap` +0.1%). The
baseline is re-recorded in the commit that moved it, per this instrument's own rule.

### The ratchet is gone

Doubling the ceiling again, to 512, and re-running gives 1,213,475 ticks: +587 on the 256 number,
0.05%. Before the fix the same doubling cost +348,133. Whatever the thread ceiling is raised to next,
this benchmark will not notice.

### BUGS

- This is a mitigation, and milestone 183 (a physical-range index for capability holders) is the fix. That milestone ("a physical-range index for
  capability holders, so revocation stops scanning every thread") removes the sweep rather than
  making it cheaper. The sweep is now O(live threads) instead of O(`MAX_THREADS`). That is a much
  better constant on a boot holding 130 threads, and no help to one holding 130 threads that all
  need checking. The index is still the answer for a machine with real tenancy.
- The 41% is `spawn_el0` on a bench boot, not a claim about spawn in general. A boot whose tables
  are genuinely full would see the old cost, because then the bound and the ceiling agree. Cost now
  tracks what the machine holds, and on this benchmark that is very little.

## 2026-09-21: spawn got 6.3% longer for the current-CPU page, and where the other 4% went

calef ruled that day that a thread reads its own CPU from a page rather than through a crossing. So
every address space now owns one more frame: allocated, zeroed, stamped and mapped read-only before
the thread runs. `spawn_el0` pays for it, because it builds and tears down a whole child per
iteration. The `--check` tripwire caught it at +10.25% against the aarch64 baseline, over the 10%
bound.

The attribution was A/B'd on one tree with one nightly. Same build, the page disabled and enabled:

| aarch64 `spawn_el0` | ticks | against baseline 1,290,216 |
|---|---|---|
| page disabled entirely | 1,297,943 | +0.60%, and this is the drift |
| frame allocated, zeroed and stamped, never mapped | 1,325,656 | +2.75% |
| mapped at `0xC000_0000_0000` (the first draft) | 1,422,462 | **+10.25%, the failure** |
| mapped at `0x3FFF_F000` (what shipped) | 1,372,031 | +6.34% |

So 124,519 of the 132,246 ticks were this change and 7,727 were drift. No baseline was blessed to
make the red go away.

### The cost was the address, not the page

Allocating and zeroing a 4 KiB frame is 277 ticks per spawn. The other 968 was the walk. An address
alone in a far corner of the space is alone in its page tables too. So `0xC000_0000_0000` needed a
fresh L1 entry, a fresh L2 table and a fresh L3 table: three frames retyped and zeroed per address
space.

Moving the page to the last page of the first gigabyte puts it under L1 and L2 tables the process's
own segments already paid for. The mapping then buys one L3 instead of three: 741 ticks per spawn
instead of 1,245. Sharing the L3 too would mean sitting in the same 2 MiB as a program's own
segments, a collision hazard rather than a saving, so that is where it stops.
[notes/current-cpu-page.md](../current-cpu-page.md) carries the reasoning for the address.

### What this does to the published figure

The cross-OS spawn row ([the cross-OS appendix](cross-os-primitives.md)) reads ~7.7 µs against Linux
`fork`+`exit`'s ~19.7 µs. That is an HVF wall-clock reading, not this instrument. A 6.3% longer path
implies roughly ~8.2 µs, ~2.4x rather than ~2.6x. The "lighter object than a Unix process" caveat is
untouched, because one more mapped page does not change that structural difference. That number is
implied, not re-measured. The HVF reading wants a quiet machine, and this one was 1.7x
oversubscribed.

*Correction, 2026-09-24: the ~8.2 µs was computed from a stale figure. The 2026-07-29 refresh
([per-core magnitudes](per-core-and-multi-hart.md)) had already called ~7.7 µs a single sample from a
busier machine, and put the settled per-core `spawn_el0` median at ~4.4 µs. HVF has not been
re-measured since, and the icount path has moved both ways in between (-41% on 2026-08-27 above,
+6.3% here), so no current wall-clock figure follows from arithmetic alone.*

The aarch64 and riscv64 `spawn_el0` baselines were re-saved with that attribution beside them
(riscv64 moved +6.26%, the same shape). Nothing else was touched. That mattered because the same lane
then found the x86_64 `map_new` failure, now in
[the preemption appendix](preemption-in-the-window.md), and left it unblessed.

# 3. What the timer interrupt path costs, and what a deadline scan adds to it

*Name: provisional.*

Section 3 of [`notes/timed-wait.md`](../timed-wait.md) in full: the timer tick's instruction counts
on both ISAs, and the exact cost of the deadline check a prototype added.

This is the number that matters most, because it is paid on every tick on every core whether or not
anyone is waiting.

## Today's path

These are static instruction counts of the Rust half, in the debug build the icount tripwire
measures. They come from `llvm-objdump -d`, whole functions, excluding the assembly vector's frame
save.

| aarch64 | | riscv64 | |
|---|---|---|---|
| `exception_dispatch` | 76 | `riscv_trap_dispatch` | 69 |
| `exception_body` | 127 | `riscv_trap_body` | 238 |
| `handle_irq` | 79 | | |
| `gic::acknowledge` | 31 | | |
| `gic::end_of_interrupt` | 29 | | |
| `timer::tick` | 31 | `timer::tick` | 30 |
| `timer::rearm` | 72 | | |
| `sched::on_tick` | 9 | `sched::on_tick` | 14 |
| `canary::check` | 17 | `canary::check` | 19 |
| `preempt_if_needed` | 12 | `preempt_if_needed` | 18 |
| `take_need_resched` | 8 | `take_need_resched` | 12 |
| total | 491 | total | 400 |

## What the deadline check adds

A prototype was wired into `on_tick`. It reads the counter, compares against a cached
`EARLIEST_DEADLINE`, and walks only if that fired. The two builds' disassembly of `on_tick` was then
compared instruction by instruction on the executed path, which is exact rather than static.

- aarch64: `on_tick` goes 9 -> 25 executed instructions. The debug build keeps two callees as real
  calls (`timer::now` 6, `Atomic<u64>::load` 8). That is +30 instructions per tick per core.
- riscv64: the same shape, `on_tick` 14 -> 30, callees 6 and 9. That is +31.

Parity is not a coincidence. The added work is one counter read, one relaxed load and one compare
on both, and the ISAs differ only in how the debug build spills.

So "nothing when the list is empty" is nearly true, and is now proved rather than asserted. It is
not literally nothing: it is 30 instructions, at 100 Hz, per core. At 400 ticks per second across
four cores that is 12,000 instructions per second. Against a 1 GHz core that is under three parts
per million, and less on a faster one.

In a release build the same code collapses to roughly four instructions: an `mrs`, a load, a
compare and a not-taken branch. The debug number is the one reported because the debug build is
what this tree gates.

## The error bar, stated plainly

The 491 and 400 totals are static whole-function counts, and therefore over-count. They include
arms of `exception_body` and `handle_irq` that a timer tick does not take. The +30 / +31 figures do
not have that problem, because they were read off the executed path of one function. So the ratio
"+30 on ~491" is a lower bound on the percentage. The percentage is somewhere above 6% of the Rust
half of the tick handler, and the absolute number is exact.

Nothing was measured under icount. The icount tripwire's own note records ±5% codegen drift between
binaries (notes/benchmarks.md), which swamps a 30-instruction change. The disassembly is the finer
instrument here, not the coarser one.

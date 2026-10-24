# `terminal_supervisor`'s quiesce bound, and the bays

An appendix to [`notes/hung-component.md`](../hung-component.md), which states the bound. This
holds the measurement behind `terminal_supervisor`'s `QUIESCE_BOUND_MS` and why its instances and
stand-ins are built in bays.

## The bound

`QUIESCE_BOUND_MS` is 250 ms, about thirty times the slowest healthy answer measured. The answer was
timed from the stand-in's start to its answer on 2026-10-19 (UTC). Each run did eleven swaps under
`script/test`, on a 16 GB Apple Silicon host with a load average of 5 to 6 from other lanes' builds.

| architecture | runs | healthy answers |
|---|---|---|
| aarch64 | three | 2.0 to 2.8 ms |
| riscv64 | three | 3.6 to 7.8 ms |
| `x86_64` | its two legs of one run | 1.9 to 2.6 ms |

The measured answer covers a parked reader answered `FLAG_RETRY` and the blob written. `swapper`'s
fixture has neither, so this is the component's own bound and not a copy of `swapper`'s.

It is wall time, because `script/test` runs TCG without `-icount`, and the margin is for a loaded
host. The deadline fires at the first 10 ms tick at or after the bound. The hung test prints the
real wait.

## Why instances are built in bays

A child's region comes home to the region it was split from. It only lowers that region's
watermark when it is on top. Split straight from the budget, the incumbent's region is always under
its replacement's when it dies. Every swap would then leave a hole the size of an instance, and the
second swap would fail for want of budget.

A stand-in has the same problem one size down. Split straight from the budget, it sits under
whatever the build retyped from the budget while it lived, such as a scratch page table the first
time the loader's cursor reaches a new 2 MiB. When it dies it leaves a hole the size of a stand-in.
On riscv64 that hole after a second display swap leaves `STOP` no budget for its own stand-in. So
every stand-in is built in one bay of its own.

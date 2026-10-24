# 5. What the current spinning costs, so the comparison is not against zero

*Name: provisional.*

Section 5 of [`notes/timed-wait.md`](../timed-wait.md) in full: what one second of today's
yield-spin costs, derived from the tree's own baselines, against one second waited on a deadline.

## The spin

Five consumers spin today. The cleanest to price is the `std` PAL's `thread::sleep`
(`patches/std-nife/overlay/std/src/sys/thread/nife.rs`), which is exactly:

```rust
let deadline = rt::now().saturating_add(ticks);
while rt::now() < deadline {
    rt::yield_now();
}
```

That is one `SYS_YIELD` per iteration, as fast as the core will go, for the whole sleep.
`net_stack`'s `wait_for_nic` has the same shape over a retransmit window, and its own note is
honest about it: "yielding across a retransmit window spins a hart until the timer is due".

## The primitives

The figures come from the tree's own gated icount baselines, `bench/baseline-aarch64.txt` and
`bench/baseline-riscv64.txt`. Under `-icount shift=0` one guest instruction is 1 ns of virtual
time. `CNTFRQ_EL0` is 62.5 MHz, so one counter tick is 16 instructions. riscv's
`timebase-frequency` is 10 MHz, so one `rdtime` tick is 100.

| primitive | aarch64 | riscv64 |
|---|---|---|
| `null_syscall`, one `svc`/`ecall` in and out | 324 instructions | 361 |
| `ctx_switch`, one EL0 yield round trip | 9,368 instructions | 9,745 |

A spinner alone on its hart pays near the `null_syscall` end: the run queue is empty, and
`schedule` takes its no-switch exit. One sharing a hart pays the `ctx_switch` end. So one second of
spinning is between 10^5 and a few times 10^6 syscalls, and 100% of a hart for the whole second.
The tree has the real-hardware figures too: ~27 ns per `null_syscall` and ~112 ns per EL0 yield
round trip, release build on HVF (notes/benchmarks.md).

## The same second, waited on a deadline

One blocking syscall, one switch out, 100 ticks at 30 instructions, one wake, one switch back.
That is about 12,400 instructions, and the hart is in `wfi` for the rest.

The ratio is at least 10^5 to 1, and it grows with core clock rather than shrinking. A core-second
is 10^9 instructions at 1 GHz and 3 x 10^9 at 3 GHz, while the 12,400 does not move. That is the
number the fork is about. It is five orders of magnitude larger than any difference between the
three shapes or the three data structures.

## Error bars

This is *derived* from measured primitives rather than measured end to end. The icount figures are
the tree's own gated baselines and the ns figures are its own HVF medians. But no run was made that
counted `SYS_YIELD` calls during a real retransmit backoff.

The direction is not in doubt. The spin is a hot loop at 100% of a hart, which
`notes/net/the-outbound-gates.md` and `testing.rs` both already record. The magnitude is uncertain
by about one order of magnitude, which is where the "10^5 to 10^7" range comes from rather than a
single number.

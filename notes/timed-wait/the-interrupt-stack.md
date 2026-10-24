# 4. The interrupt-stack proof holds, and the reason it holds was already in the tree

*Name: provisional.*

Section 4 of [`notes/timed-wait.md`](../timed-wait.md) in full: why a deadline expiry in the timer
handler keeps the interrupt-stack proof, the gate run that showed it, and what it does cost.

Milestone 124 (a thread is born where it lives) gave each core an interrupt stack, and gates one
rule in CI: nothing that runs on an interrupt stack may context-switch away from it.
`script/stack-depth-check` proves no context switch is *reachable* from the interrupt-stack entry
point on either ISA. A deadline expiring in the timer handler wants to wake a thread, and this was
the question most likely to be the real cost.

## The kernel already does this

It is not the real cost, and the reason is that the kernel does exactly this already. `handle_irq`
calls `sched::irq_notify` for every routed device interrupt. That happens in `exception_body`, on
the interrupt stack (`arch/aarch64/exceptions.rs`). `irq_notify` takes `SCHED`, signals the
endpoint, and calls `wake_load_aware`, which moves a `Blocked` thread to `Ready` and pushes it onto
a run queue.

A wake is an enqueue, not a switch. The switch is already deferred: `exception_body` returns
`true`, and `exception_dispatch` runs `sched::preempt_if_needed` one frame out, back on the
interrupted thread's own stack. A deadline expiry is the same operation on the same stack through
the same function.

## Measured rather than argued

A prototype `expire_deadlines` was wired into `on_tick`, walking the table under `SCHED` and
calling the existing `wake`, and the gate was run:

```
    aarch64: deepest thread-stack chain 13568 of 24576 bytes, 11008 spare; interrupt stack 3888 of
             16384, no context switch reachable from it; 1802 functions reachable, none over the
             guard page
    riscv64: deepest thread-stack chain 13232 of 24576 bytes, 11344 spare; interrupt stack 4208 of
             16384, no context switch reachable from it; 1796 functions reachable, none over the
             guard page
```

The same gate on the unmodified tree gave 4352 on aarch64 and 4160 on riscv64 for the interrupt
stack. So the budget moves by -464 bytes on aarch64 and +48 on riscv64. The aarch64 number going
*down* is the tell that this is codegen churn rather than the scan.

The deepest interrupt-stack chain on either ISA runs through `watchdog_tick` ->
`check_test_ceiling` -> `dump_threads` and a panic. That is a test-build failure path and has
nothing to do with deadlines. The suite's own runtime measurement agrees: the interrupt stacks'
high-water is 1088 of 16384 bytes, 6%, on all four cores.

## What it does cost

This is the finding worth carrying to the fork. A deadline expiry has to take `SCHED`, and `SCHED`
is the whole-machine lock.

`irq_notify` establishes that this is *allowed* from interrupt context. `IrqSafeMutex` masks
interrupts for as long as it is held, so the interrupted code on this core cannot have been
holding it. But a device interrupt is rare and a timer tick is not. Four cores ticking at 100 Hz,
all reaching for `SCHED`, is contention that nothing pays today.

The cached `earliest` is what keeps that off the common path. It is therefore load-bearing rather
than an optimization: without it, every tick on every core takes the whole-machine lock. That is
the one place where "add a deadline" genuinely does touch scheduler structure, and it is a word,
not a wheel.

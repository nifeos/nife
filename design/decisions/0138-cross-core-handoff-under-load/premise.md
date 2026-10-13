# §138 appendix: the premise, and three precedents

An appendix of [§138 (how a saturated workload is made to hand threads across
cores)](../0138-cross-core-handoff-under-load.md). The main page is complete without it. This file
holds the bench run, the code behind it, and the precedents the tree already holds.

## The premise, verified by this lane rather than taken from the milestone

Confirmed on patagonia, 2026-09-01, `script/soak --for 40s` on aarch64, four cores, twenty workers:

```
soak: t=5s  beat=1 rounds=320605  rate=64121/s ... crossings=15 remote=23 steals=2
soak: t=35s beat=7 rounds=2182566 rate=63512/s ... crossings=15 remote=23 steals=2
```

`crossings` is frozen at 15 across seven beats and 2.18 million IPC round trips. It is not slow, it
is stopped. Milestone 219's riscv64 row (21 crossings) and its explanation both survive
re-measurement, and its three structural causes are each visible in the code:

1. A rendezvous wake is local by design. `sched::wake` queues the woken peer on the *waker's* own
   core (§28, part 2: the message is in registers and the cache is warm). A communicating set
   converges onto one core within a few exchanges and stays.
2. `wake_load_aware` has exactly one caller, `sched::irq_notify`, which is called from the three
   architectures' interrupt dispatchers and from nowhere else. `git grep wake_load_aware` is the
   whole proof.
3. A work steal needs an idle core and a queued thread elsewhere. A rendezvous keeps at most two
   threads runnable per group, so queues are empty; add grinders to fill them and no core is idle.
   A steady state holds neither end.

Nothing dissolves the question. This lane looked for an existing way to make a saturated workload
cross cores and found none that a user program can reach. That is the finding that makes this
decision necessary rather than optional.

## What this tree already does in the analogous case

Four greps, and they move the argument more than any of the options do.

§28 already deferred this exact thing, with a trigger. Its deferred list reads: *"an explicit
placement grant in the spawn manifest (milestone 23's contract; overrides the default, recovering
seL4's userspace-owns-placement story for pinned components)"*. So affinity is not a new idea here.
It was considered at ratification, deferred deliberately, and its shape was already chosen: a
grant in the spawn manifest, not a syscall on a running thread. Any proposal that reaches for a
`Thread::SET_AFFINITY` method is reopening a decision rather than making one, and owes a reason.

This tree already has affinity, and it is kernel-internal. Device-line IRQ affinity picks which
hart's PLIC context a source lands on (`kernel/src/drivers/plic.rs`, `kernel/src/arch/riscv64/irq.rs`).
It is policy the kernel decides and remembers; no program asks for it and no capability names it.
That is the precedent for "affinity exists here" and it is on the opposite side of the syscall
boundary from option A.

This tree has already refused to pin threads once, on instrument grounds. `kernel/src/cpu.rs`,
in the per-CPU test: *"The alternative was giving kernel tests boot-core affinity, and it is the
wrong trade. Pinning tests to core 0 would buy this one assertion back at the cost of running the
whole suite on one core, which is precisely where the placement bugs §28 introduced would hide. A
harness that avoids the scheduler it is meant to test is not a harness."* The reasoning transfers
with the sign flipped: a *pinned* soak would be running a workload the scheduler is not free to
place, which is one interleaving explored very thoroughly.

The fourth, the feature-gated affordances such as `fastpath_pad`, stays on [the main
page](../0138-cross-core-handoff-under-load.md#what-this-tree-already-does-in-the-analogous-case).

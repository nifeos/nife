# §138 appendix: the working behind the options

An appendix of [§138 (how a saturated workload is made to hand threads across
cores)](../0138-cross-core-handoff-under-load.md). The main page is complete without it. This file
holds the detail of options A and B, all of option C, and how option D was read, run and costed.

## A. Thread affinity: a program says where a thread runs

What the surface grows by, exactly. In the manifest form: one field on the spawn contract, plus
its validation, plus whatever names the authority. In the invocation form it would be one method on
the thread control block capability (`crates/abi`'s `thread_control_block` module) and one arm in
`kernel/src/syscall.rs`. Either way the thing that is irreversible is not the code. It is that every
future program is written against it and every future scheduler must honor it.

The capability question it opens, and this is the part with no default answer. Is pinning an
authority? Three readings, and the tree does not currently prefer one:

- **It is not an authority at all**, because a thread choosing its own core is choosing where to
  spend time it already has. This is roughly Linux's position for the self case.
- It is an authority over the machine, because a program that pins ten threads to core 0 has
  taken a scheduling decision away from everyone else. There is no CPU budget mechanism here to
  bound that; §28 defers budgets too, and with no priorities the scheduler is round-robin, so a
  pinning program's effect on its neighbors is unbounded by construction.
- It is an authority over a thread, in which case the thread control block capability already
  names it and the answer falls out with no new object. This is seL4's non-MCS answer, and seL4
  replaced it: see the prior art above, where the same kernel gates priority with a bounding
  capability and gated placement with nothing.

The prior art narrows this further than §28 did. Both microkernel families that solved it put the
core in a capability rather than in a message, and that form is available to nife with or without
a manifest field. It is also the form §28's own wording points at: a *grant* is a capability, and a
grant that names a core is exactly seL4's `sched_control_cap`.

What it buys the experiment. A pinned caller and a pinned responder must cross on every round
trip. But the crossing is the caller's `CALL` blocking and its reply arriving on the responder's
core, which is the rendezvous path. The defect was on the `wake_load_aware` path. So A produces
sustained cross-core traffic adjacent to the defect rather than on it.

## B. A periodic rebalancer in the scheduler

The point in its favor, which reading Linux produced and which nothing in this tree had said.
B is not "add balancing to a kernel that has none". nife already balances at three of the four
moments Linux does; all three are event-driven, and B is the fourth, the one that fires on a clock
whether or not anything happened. Stated that way it is a smaller change than it looked, and it is
the *only* one of the four that a saturated workload cannot starve of its trigger. Anyone arguing
for B later should start here rather than from the numbers.

What it costs. A timer-driven path in the hottest subsystem, and tuning constants that are a
permanent maintenance surface with nothing to measure them against. Linux is the measurement of that
surface rather than an assertion about it: `include/linux/sched/topology.h` carries `min_interval`,
`max_interval`, `balance_interval` and `imbalance_pct` per domain level, and
`sched_balance_domains` walks the whole chain deciding at each level whether its interval has
elapsed. That is the shape the constants take once the mechanism exists, and Linux is where they
have a topology and thirty years of workloads to be tuned against. nife has neither. This tree also
has no workload whose fairness visibly fails, which is the trigger §28 named for reopening
scheduling policy, so B would be adding policy to make a test possible and then living with it in
every boot forever.

What it buys the experiment. Real migration of the workload's own threads, which is more than A
or D give. But it moves threads for reasons the defect had nothing to do with, and it changes the
system under test in the act of testing it, which is the `cpu.rs` objection above pointed the other
way.

## C. A userspace-drivable interrupt source

What it is, as the milestone framed it. A syscall a program calls that causes an interrupt, so
the workload itself drives `irq_notify` at rate.

Verified fact that reshapes this option: the userspace half already exists. `abi::irq::WAIT` is a
method on an `Irq` capability, `crates/user_rt`'s `irq_wait` calls it, and `kernel/src/soak.rs`
already hands workers capabilities at spawn. A program can already block on an interrupt route
with no new surface at all. What is missing is only the raise, and the raise does not have to be
a syscall, because the kernel is already the thing that raises interrupts.

So the option as stated ("userspace can cause interrupts") is asking for the more expensive half of
a thing that is already half built, and it is dominated by:

## D. A soak-only, kernel-side interrupt raiser (this lane's finding, and it was measured)

It reaches `wake_load_aware` on its real path, and the path was read rather than assumed.
`sched::on_tick` is called by all three timer interrupt dispatchers
(`arch/aarch64/exceptions.rs`, `arch/riscv64/exceptions.rs`, `arch/x86_64/exceptions.rs`), in real
interrupt context, on every core independently. A `#[cfg(feature = "soak")]` call from there into
`sched::irq_notify` runs the identical sequence a device interrupt runs:
`rendezvous.signal()` -> `handshake.serve()` -> `wake_load_aware` -> `pick_wake_target` ->
`place_on` -> the reschedule IPI. `irq_notify`'s own doc comment already states it is safe from
interrupt context, and the reason (`IPC_TABLES` is an `IrqSafeMutex`, §9) holds for the timer arm
exactly as it does for the device arm.

The spike, and its numbers. Written, run, and thrown away on 2026-09-01. 53 lines added across
three files, no syscall change, no architecture-specific code, no change to any production path:
a routed rendezvous plus a tick hook in `kernel/src/soak.rs`, three lines in `kernel/src/sched.rs`,
and one role in `fixtures/src/soaker.rs` that loops on `irq_wait`. Same command, same host, same day as
the baseline above:

| Architecture | `crossings` at 25s, today | with the spike | round trips/s |
|---|---|---|---|
| aarch64, 4 cores | 15, frozen from beat 1 | 3,785 and rising linearly | 64,000 -> 44,500 |
| riscv64, 4 cores | 21, frozen (milestone 219) | 1,948 and rising linearly | 24,000 -> 10,600 |

What the numbers cost, stated as honestly as the milestone states its own. The round-trip rate
falls by about 30% on aarch64 and about 55% on riscv64, because the waiters are extra threads and
every migration is real work. A soak with this on is therefore a *different* soak, and its `rounds`
figure is not comparable with a soak without it, which is one more row in `notes/soak.md`'s
comparability table rather than a new kind of problem.

And the honest limitation, which nobody should discover later. The threads that cross are the
waiters, not the rendezvous pairs. The `crossings` counter cannot say which thread moved, and the
mechanism says it must be the waiters: rendezvous wakes stay local whatever else is happening, so
the callers and responders stay exactly as pinned as they are today. **D sustains the wake protocol
across cores under load. It does not make the IPC workload itself migrate.** Only B does that.

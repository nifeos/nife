# The tick route: how the soak was made to cross cores

An appendix to [notes/soak.md](../soak.md). It is milestone 221 (the soak never crosses cores, so
build the hook that makes it): the mechanism, the two ordering bugs it had, and what it establishes
about risk 5.

## The decision

`design/decisions/0138-cross-core-handoff-under-load.md` (*how a saturated workload is made to hand
threads across cores*) put four options in front of calef. He approved option D on 2026-09-02.

## The mechanism, and it is short

Under `--features soak_test` and nowhere else, `sched::on_tick` signals a rendezvous. One worker per
group blocks on that rendezvous through the `Irq::WAIT` a device driver uses. All three
architectures' timer dispatchers call `on_tick` in real interrupt context on every core. So a tick
runs the identical sequence a device interrupt runs:

```
soak::signal_waiters -> sched::irq_notify -> Rendezvous::signal -> handshake.serve
                     -> sched::wake_load_aware -> pick_wake_target -> place_on -> the reschedule IPI
```

Each group has its own route, and each tick signals one of them, round-robin across the machine.
Both halves of that are fixes rather than flourishes, and the section below says what they fix.

That last chain is why this was worth building rather than the alternatives. `wake_load_aware` is
where risk 5's one observed defect lived, on radon. It had exactly one caller (`sched::irq_notify`),
which no user workload could reach.

Four properties, each of which was a requirement rather than a bonus:

- No syscall is added. The userspace half already existed: `abi::interrupt::WAIT` is a method on an
  `Irq` capability, and `user_mode_runtime::interrupt_wait` calls it. Only the *raise* was
  missing, and the kernel is already the thing that raises interrupts.
- Nothing exists in a production build. That is proved in [qemu-baselines.md](qemu-baselines.md),
  not asserted.
- It is architecture-neutral, and that is load-bearing. riscv64 has no software-raisable line that
  reaches `irq_route` at all. So an aarch64 `send_sgi` or an x86 self-IPI would have left radon out.
  (Radon was believed to be the machine that produced fatal risk 5's defect. That reading is
  retracted by `notes/visionfive2.md`'s fifth bench stop, 2026-08-15, and it never happened. Staying
  architecture-neutral is still the right call on its own merits.) The timer is the one source all
  three share, through a function that is already portable.
- The timer is the one event a saturated workload cannot starve. That is the whole reason this works
  where three existing balancing moments do not.

## What crosses is the waiters, not the pairs

This must not be misquoted. Rendezvous wakes are local by design whatever else is happening, so the
callers and responders are as pinned as they ever were. This sustains the wake protocol across cores
under load. It does not make the IPC workload migrate. Only a periodic rebalancer would, and
DECISIONS 138 declines one on the reopening trigger of DECISIONS §28 (*a real workload where
fairness visibly fails*), which has not fired. The kernel says this in words at the start of every
run, and `script/soak-test` says it again in its summary. The flattering reading is available, and
a summary gets quoted.

## The soak-only interrupt numbers

Group `g`'s route is bound to intid `255 - g`. None of those names hardware or can be delivered on
any of the three architectures. On aarch64 and riscv64 a routed interrupt arrives only if something
enabled it at the controller, and nothing enables these. On x86_64 the top of the band is the local
APIC's spurious vector, answered in its own arm before `irq_route` is asked. The rest sit at the far
end of an MSI band allocated upward from 0xc0.

None of that is what makes it safe. `soak::bind_tick_routes` asks `sched::irq_route` about every
number before it takes any of them. It refuses to start a soak whose routes would steal somebody
else's interrupt. A soak boot runs the whole tour first, so every device has already claimed what it
is going to claim by the time that check runs.

## Two bugs this mechanism had, both found by running it, both about ordering

Neither is visible in review, and getting either wrong produces the same symptom: a soak reporting
workers as wedged when the defect is in the instrument.

One rendezvous for every waiter starves all but one, on a loaded host, so each group has its own.
`crates/inter_process_communication`'s
`Rendezvous::receive` takes a pending signal before it looks at the receiver queue. That is right
for a driver, since an interrupt that already happened must not be missed. It is wrong for four peers
sharing a source. When ticks arrive in a burst, whichever waiter is already running drains the whole
backlog through the pending path and never queues, and the others sit at the head of a queue
nothing pops. A rendezvous per group means a backlog can only ever belong to the waiter it
accumulated for. A shared rendezvous passes on an idle machine: the starvation needs a busy host to
appear at all.

Routes are bound before the first waiter is spawned, and the signaling is switched on last. Arming
last is correct for the *signaling*. A route signaled before anyone waits on it hands the first
waiter a backlog, and makes the first beat measure setup. It is wrong for the *routing*. A waiter
that reaches `Irq::WAIT` before its route exists gets `WrongObject`, and with no channel to report
a refusal on, it stops counting. The run fails a beat later with workers apparently wedged. aarch64
tolerates the race and riscv64 does not, which is the ordinary shape of this class.

## What it establishes about risk 5, and what it does not

- It makes the second experiment runnable. It does not run it. The run needs an evening at a bench
  on radon, argon or xenon. QEMU cannot show the defects this risk is about; that is the risk's
  premise, not a limitation of the tooling.
- It says nothing about what a crossing rate should be. The numbers in
  [qemu-baselines.md](qemu-baselines.md) are a shape. There is no baseline to compare a board
  against until a board has produced one, and the first board run is what creates it.
- The hook fires on a timer, which is why it works and why it proves nothing about the machine
  without it. A soak with the tick route live is evidence about the wake path under sustained
  cross-core traffic. It is not evidence that a workload would ever generate that traffic on its
  own; measurement says it would not.
- The interrupt controller is not on this path. The timer is not a controller-routed source, so the
  claim, mask and complete sequence (the GIC, the PLIC, the local APIC) is untouched. The experiment
  is about the wake protocol, and that is what it runs.

---
status: BUILT
raised: 2026-09-02
built: 2026-09-02
---
# 221. The soak never crosses cores, so build the hook that makes it

Built. It is option D of DECISIONS §138 (how a saturated workload is made to hand threads across
cores). calef approved it on 2026-09-02.

In brief. Milestone 219 (the boot tour ends and the kernel halts, so there is nothing to soak)
built a workload that lasts and then found something nobody expected: **a saturated workload never
migrates between cores.** Measured on both multicore architectures, the crossing count freezes
within the first second while the machine does tens of thousands of IPC round trips a second.

DECISIONS 138 established why, and it is three structural facts rather than a bug. Rendezvous wakes
are local by design (DECISIONS §28.2), `wake_load_aware` is reachable only from a device interrupt,
and a work steal needs an idle core and a queued thread elsewhere, which a saturated machine
does not have. It also found that nife has counterparts for three of Linux v6.12's four balancing
moments, all event-driven, and that a saturated rendezvous workload starves all three of their
triggers. That is the whole explanation.

**So the second half of fatal risk 5's decisive experiment has never been runnable**, and it was
believed to be the half that mattered. At the time this milestone was minted, the one defect this
risk had produced, on radon, was read as a receiver made Ready with nothing delivered, on the
`irq_notify` to `wake_load_aware` path. That reading is retracted (`notes/visionfive2.md`'s fifth
bench stop, 2026-08-15, before this milestone was minted): the dumps were a completed tour's terminal
state, not a stranded receiver, so radon has not actually produced a confirmed instance of risk 5's
defining claim. Found still repeating the retracted reading here 2026-09-23. What still stands: the
existing soak exercises contention on shared kernel state and does not exercise the
`irq_notify`-to-`wake_load_aware` path at all, which is reason enough on its own to build the hook
below.

## What it needs

A `--features soak` hook that signals a routed rendezvous from `sched::on_tick`, so soak workers
block on it through the `Irq::WAIT` that already exists. `sched::on_tick` is called by all three
timer dispatchers in real interrupt context on every core, so the hook runs the identical sequence a
device interrupt runs, down through `wake_load_aware`, `pick_wake_target`, `place_on` and the
reschedule IPI.

The research lane spiked this and threw the spike away, so the shape is known rather than
guessed: about 53 lines across three files, no syscall, no architecture-specific code. Measured:

| | today | with the spike |
|---|---|---|
| aarch64, 4 cores | 15 crossings, frozen | 3,785 and rising |
| riscv64, 4 cores | 21 crossings, frozen | 1,948 and rising |

Round-trip rate falls (aarch64 64,000 to 44,500 a second), which is expected and must be recorded
rather than hidden: the machine is doing more work per round trip.

**Being architecture-neutral is load-bearing, not incidental.** riscv64 has no software-raisable
line reaching `irq_route`, so an aarch64 `send_sgi` or an x86 self-IPI would have left radon out.
(Radon was believed to be the machine that produced the risk-5 defect this milestone exists to hunt;
that reading is retracted, see below. Staying architecture-neutral is still the right call regardless,
since radon is the only board with no other software-raisable interrupt path to lean on.)

## What it must not claim

**The threads that cross are the waiters, not the rendezvous pairs.** This sustains the wake protocol
across cores under load. It does not make the IPC workload itself migrate, and only a rebalancer
would, which DECISIONS 138 declines. Anything this milestone prints or records must not let a reader
conclude otherwise, in the same way milestone 219's own output refuses to let a green soak be quoted
as proof the concurrency is correct.

**A soak build is already not a production build** (milestone 219 measured the fastpath at about
1.05x with instrumentation compiled in), and this widens that gap. A soak number compares only with
another soak number.

## What was built

`sched::on_tick` gained one `#[cfg(feature = "soak")]` call. `kernel/src/soak.rs` gained the tick
route (a rendezvous, a soak-only intid, and the signal the tick sends it). `fixtures/src/soaker.rs`
gained a waiter role that loops on `user_rt::irq_wait`. `crates/soak_page` gained a third counter
array so a wake is never mistaken for a round trip. `crates/serial_console` and the `script/soak`
summary carry the new field and the sentence that says what the crossings are.

No syscall, no architecture-specific code, and no new feature: it is all behind the `soak` feature
milestone 219 already had.

Measured on patagonia under QEMU, 2026-09-02, `script/soak --for 30s`, each pair back to back on
an otherwise idle machine, read at the 25-second beat (20 on x86, whose runner is single-core):

| Architecture | Cores | Round trips before | after | Crossings before | after |
|---|---|---|---|---|---|
| aarch64 | 4 | 1,623,764 and 1,630,605 | 1,632,746 and 1,632,803 | 15, frozen from beat 1 | 1,452 and 3,779, rising |
| riscv64 | 4 | 871,047 and 886,428 | 662,787 and 823,783 | 10 and 14, frozen | 2,573 and 4,358, rising |
| x86_64 | 1 | 77,372 | 51,749 | 0 | 0, and one core is the whole reason |

Two runs of each leg, because one would have been misleading and the first pass was: it was taken
while another lane's suite ran on the same laptop and measured the host's load rather than this
change. aarch64 pays nothing measurable (0.6% *more* round trips after, which is noise, and the
workers that complete them are the same set in both legs). riscv64 pays about 7% on the
closest-matched pair. x86_64 pays about a third, which is arithmetic: two more threads on one core
in a round-robin scheduler. All three are far below DECISIONS 138's spike, which saw 30% and 55%,
and the difference is recorded rather than explained away: the spike was thrown away and cannot be
re-measured.

A production build is unchanged, checked rather than asserted. Without the feature, on all three
architectures, every symbol and every loaded section has the same size and `ipc_fastpath` and
`syscall_entry` are unchanged at 6,687 and 1,637 bytes. The loadable image differs by 45 bytes on
aarch64, all of them panic-location line numbers below the insertion point, and rebuilding the base
commit with ten comment lines at the same place gives a byte-identical image on all three.

Two bugs, both found by running it, both about ordering, both recorded in `notes/soak.md`. A
single shared tick rendezvous starves all but one waiter on a loaded host, because `Rendezvous::recv`
takes a pending signal before it looks at the receiver queue (right for a driver, wrong for four
peers). Each group has its own route now. And binding the routes *after* spawning the waiters races:
a waiter reaching `Irq::WAIT` before its route exists is refused and has nowhere to report it. Routes
are bound before the first waiter; only the signaling is switched on last.

## BUGS

- **This does not run the experiment**, it makes it runnable. The run needs a bench evening on radon,
  argon or xenon, and that is milestone 219's other proposed follow-on rather than this one.
- It says nothing about what a crossing rate should be. The spike's numbers are a shape, not a
  target, and there is no baseline to compare a board against until a board has run one.
- The hook fires on a timer, which is the one thing the workload cannot starve. That is why it
  works, and it is also why it is not evidence about what happens without it.

## Follow-on

- **Milestone 225.** Running the experiment. This milestone makes it runnable; the run itself needs
  a bench evening on radon, argon or xenon, which is where fatal risk 5's answer means anything.
  The radon defect once recorded here was retracted by
  [`notes/visionfive2.md`](../../notes/visionfive2.md)'s fifth bench stop (2026-08-15), so no board
  has produced one yet ([`notes/multicore-defect-curve.md`](../../notes/multicore-defect-curve.md),
  row D6).
- **Decision.** A rebalancer, the only thing that would make the IPC workload itself migrate rather
  than its waiters, is declined in `design/decisions/0138-cross-core-handoff-under-load.md`, which is
  also where option D (the tick-signaled rendezvous this milestone built) was approved.
- **Recorded.** Nothing here says what a crossing rate should be. The QEMU numbers are a shape, not
  a target, and there is no baseline to compare a board against until a board has run one.
  `notes/soak.md`.
- **Recorded.** A soak build is not a production build, and this widens the gap milestone 219
  measured. Round-trip rate falls because the machine does more work per round trip, so a soak
  number compares only with another soak number. `notes/soak.md`.
- **Recorded.** The hook fires on a timer, which is the one thing a saturated workload cannot
  starve. That is why it works and also why it is not evidence about what happens without it.
  `design/roadmap/0221-a-soak-that-crosses-cores.md`.

## Index row

`sched::on_tick` signals a routed rendezvous under `--features soak` and one worker per group
blocks on it through the `Irq::WAIT` a device driver already uses. A tick runs the whole real
wake path down through `wake_load_aware`, `pick_wake_target`, `place_on` and the reschedule IPI.
That is DECISIONS 138's option D, which calef approved: no syscall, no architecture-specific code,
and nothing in a production build. Checked, not asserted: every symbol and every loaded section
the same size on all three architectures, and a base commit padded with ten comment lines builds a
byte-identical image. aarch64 went from 15 crossings, frozen from the first beat, to 1,452 and
3,779 across two runs; riscv64 from 10 and 14 to 2,573 and 4,358. The round-trip cost is nothing
measurable on aarch64 and about 7% on riscv64, far below the spike's 30% and 55%, and that
difference is recorded rather than explained away. Running it found two ordering bugs in the
mechanism itself, both written up. What crosses is the waiters, not the rendezvous pairs, and
the kernel and the tool both say so in words, because only a rebalancer would move the pairs and
138 declines one. It makes risk 5's second experiment runnable; it does not run it, which needs a
bench evening on radon.

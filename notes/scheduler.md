# The scheduler: per-core run queues, two-choice placement, message-shaped stealing

This is the working note for DECISIONS §28 (SMP placement) as built. The decision record is the
authority on *why*; this is *how it fits together* and the caveats worth rereading. The evidence
behind it, including the full record of two closed lost wakeups, is in
[scheduler/](scheduler/README.md).

## The shape

Each core owns one run queue, single-owner, touched only by that core with interrupts masked
(`cpu::PerCpu::with_runq`). No core takes another core's run-queue lock, ever. The only cross-core
structure is a per-core migration inbox (a real lock) plus the reschedule SGI that pokes a core
to drain it. That is the whole concurrency surface: run queues are private, the inbox is the one
shared thing, and it is small.

Two relaxed atomics mirror a core's load so another core can read it without touching the queue:
`runq_len` (updated in `with_runq`) and `inbox_len` (updated under the inbox lock). `runnable()` is
their sum. Stale reads are fine and expected; every placement and steal decision tolerates being a
beat out of date (the gossip lesson).

## Placement: the power of two choices

`spawn` -> `pick_spawn_target`: a per-core xorshift PRNG picks two online cores, the lighter by
`runnable()` wins, the thread is carried there by `spawn_on` (local push, or remote via the inbox +
SGI). Near-optimal balancing that reads at most two remote counters no matter the core count. The
PRNG is seeded per core from a fixed constant, so a given boot makes the same choices and the
icount benches stay reproducible. One online core: a no-op.

## Stealing: pull, by message

An idle core's `run_idle` calls `try_initiate_steal`: pick the most-loaded *other* core by run-queue
depth alone (never its inbox, which is work already in flight to it), CAS a one-slot steal request,
and poke it with the reschedule SGI. The victim's `serve_steal_request`, at its next scheduler
entry, hands back one queued thread through the requester's inbox. Pull beats push under
uncertainty, and no shared run-queue lock ever appears. Cost: a steal lands a tick late, bounded.

## Wakes: local for a rendezvous, load-aware for a device interrupt (§28.2, as amended)

An IPC rendezvous wakes its partner on the waker's core: the message is in registers, the cache
is warm, and a serial pipeline (net_stack<->std) stays co-located and fast. `wake` does this, and every
IPC path, supervision, and revocation uses it.

A device interrupt is different: it carries no locality, and pinning the woken driver to the
IRQ-handling core re-concentrates a pipeline or drops it on a busy core. So `irq_notify` wakes
load-aware through `wake_load_aware` / `pick_wake_target`: the least-loaded core, ties won by
the current core so a driver taking a completion interrupt every request (the block server at a
RedoxFS mount) is not migrated each time. The device-line affinity that spreads which core takes
each IRQ in the first place is the companion mechanism, in notes/interrupts.md.

## The undelivered-wake gate (boot 8, VisionFive 2)

A wake is not a favor, it is the second half of a delivery, and since 2026-08-14 the scheduler
enforces that. Every genuine wake of a thread parked in IPC happens in the same `SCHED` critical
section that gave the parked operation something to return. A rendezvous stages the mailbox, an
interrupt counts a signal, a reply fills the reply words, revocation flags the abort. Each of
those sites sets the handshake's `ipc_served` (or `ipc_aborted`; both on
`thread_wake_handshake::Handshake`, embedded in `Thread`) before calling `wake`. `wake` /
`wake_load_aware` refuse to make a waiting thread Ready when neither flag is set, recording
`refuse:tid` on the per-core event ring. The parked thread stays parked and still linked on its
endpoint's wait queue, and the real counterparty completes the rendezvous normally later.

The gate turns that transition into a refused no-op plus a ring event, whoever the caller is. The
receive tails carry a `debug_assert` tripwire ("resumed with nothing delivered") for the state the
gate makes unreachable. Every `ipc_served` setter records a `serve:tid/site` ring event so a
bench dump names who completed a rendezvous instead of leaving it to inference.

The failure it was built against was misread, and that correction is the important part. It was
built from the VisionFive 2's boot 8, read at the time as a wake with no sender. The fifth bench stop
(2026-08-15) overturned that reading: the wake was a real send. So the gate has never fired on a
field failure, and `refuse:` has never appeared on a board ring. It stays because the transition it
forbids is reachable, which a loom harness proves. It is hardening against a reachable state, not a
repair of a field failure. Two companions from the same boot guard `ipc_reply` and a switch into
the current thread. The protocol is a crate, `crates/thread_wake_handshake`, shipped and
model-checked as the same code. The misreading, the companions and the loom harnesses are in
[scheduler/undelivered-wake-gate.md](scheduler/undelivered-wake-gate.md); the model's limits are in
notes/interleaving.md.

BUGS. The gate protects threads whose `wait_on` is set, which is every IPC block site today; a
future block path that forgets to set `wait_on` opts itself out silently. A kernel-thread caller
of `ipc_receive` still cannot tell an abort from a message unless it checks `take_ipc_aborted`
itself; the gate guarantees *something* was delivered, not which thing. And the blocking `ipc_send`
resume path carries no tripwire assert, deliberately. It takes no lock after `schedule()` today, and
adding one on the IPC hot path to double-check an invariant the gate
already holds was judged not worth the cycles the bench tripwire watches.

## The costs migration made real

Turning on any migration at all strips the accidental cover off same-core assumptions. Each of these
bit us, and each is fixed and tested:

- RISC-V `tp` (the per-hart pointer) is thread-frame state. A kernel thread preempted on one
  hart and resumed on another used to come back reading the wrong hart's per-CPU block. Fixed in
  `arch/riscv64/trap.s`; the full story and the regression test are in notes/riscv-port.md. aarch64
  is immune (its pointer is a system register the frame never carries). This is the concrete face of
  rule 4 (assume weak ordering) and rule 1 (arch state lives in arch).

- A per-CPU field read from preemptible code is two steps on every ISA: the block's address into a
  general register, then the load. A kernel thread migrated between them reads the core it left.
  For the running thread's own name that is identity, not a hint. A split read in
  `sched::adopt_address_space` binds a new space to whichever thread the stale name resolves to,
  which then runs in a stranger's space, and its caller resumes U-mode on the reserved root
  ([mmu.md](mmu.md), BUGS). So `sched::current_thread_id` reads through
  `arch::read_current_thread`: one load relative to the per-CPU register on riscv64 and `x86_64`,
  and a read with IRQs masked on aarch64, which has no such addressing mode. Any other field a
  preemptible caller reads through `cpu::current()` describes a core it may have left.

- The hang watchdog counts progress, not test starts. A slow-but-live workload (std_net once spent
  about 300 s in net_stack's userspace smoltcp poll, CPU-bound, no wakes or output for stretches over a
  minute) must not read as a deadlock. The watchdog credits a completed wake, a line of output, OR
  any core running a non-idle thread; only a real lost wakeup, every thread blocked and every core on
  its idle thread, stalls it. See `kernel/src/testing.rs`.

- And because that alone traded a flake for a silent hang, there is a per-test wall-clock
  ceiling. See the section below: the progress heartbeat is blind to a livelock that keeps doing
  IPC, which is a real failure we hit, not a theoretical one.

## The two hang watchdogs, and what each one cannot see

The harness asks two independent questions, and either failing fails the run. They exist because there
are two ways a test never finishes, and no single instrument sees both.

| | **No-progress heartbeat** | **Per-test wall-clock ceiling** |
|---|---|---|
| Question | Is anything happening at all? | Has this test taken longer than allowed? |
| Catches | Deadlock, lost wakeup | Any non-terminating test, livelock included |
| Window | ~60 s of total silence | The test's budget (90 s default) |
| Blind to | Any loop that keeps doing IPC | Nothing that fails to terminate, but slow to react |
| Scope | Anywhere, including before tests start | Only while a test is running |

Why the ceiling had to be added. The heartbeat credits a completed rendezvous as progress. The
RedoxFS repeat-write livelock spins in an allocator commit *while still serving blk IPC*, so every
rendezvous reset the heartbeat. A failure that had been a loud 60 s trip became an infinite silent
hang at about 400% CPU with no watchdog fire. A livelock that makes progress is indistinguishable from
healthy work to a progress-only instrument. Turning a loud failure into a silent one is worse than the
flake the heartbeat fixed, so both mechanisms are live now.

Why budgets are per test. std_net once honestly ran 300 to 344 s, so one global ceiling would have
sat near 700 s and let a two-second unit test spin for eleven minutes before failing. By 2026-10-11
it took 1.3 to 2.5 s and its entry was removed, leaving the table empty. The default is a tight
90 s; a test that is honestly slower declares its cost in `SLOW_TESTS` in `crates/test_times`, with
the reason. Keep entries near 2x measured, so host load does not make them flaky.

The honest limit. Neither mechanism can tell a livelock from slow-but-correct work while it is
running. Only the budget, a human declaration of expected cost, separates them. That is why a new
`SLOW_TESTS` entry deserves a sentence about *why* the test is slow, not a number.

Proving it. The `watchdog_probe` feature adds a test that loops forever doing a full rendezvous
each pass, so the heartbeat sees a healthy kernel and only the ceiling stops it. It is expected to
fail, so it is not in the normal suite:

```text
helpers/qemu-bounded.sh 200 cargo test -p kernel \
    --features watchdog_probe --target aarch64-unknown-none-softfloat
```

The outermost backstop is on the host. Both guest watchdogs run off the boot core's timer IRQ, so a
kernel that loses it loses both. No kernel leg of `script/test` runs through
`helpers/qemu-bounded.sh`, so xtask watches the transcript for the running test's name and kills QEMU at that test's budget plus `HOST_GRACE_SECS` (60 s), 150
s by default. The failure line names the leg, the image, the test and how long it ran:

```text
HOST WATCHDOG: x86_64 system_tests: test system_tests::user::e1000e_tests::a_client_... ran 150 s,
past its 150 s bound (its guest budget plus 60 s). ...
```

It reads the same budget table as the ceiling (`crates/test_times`), so a `SLOW_TESTS` entry
lifts both. The grace keeps it out of the ceiling's way, since the guest's report carries a thread
dump and the host's cannot. `xtask/src/time_record.rs` holds the reasoning and what it does not
cover (the build before the guest's first line, and `--hvf`).

## CLOSED: the lost wakeup on `reclaim_frees_a_started_then_exited_childs_regions`

A refused region reclaim killed the child the test was waiting for. One line of test code, no
kernel defect, and not a RISC-V defect either: reproduced on aarch64 the moment the window was
widened. Milestone 72, 2026-08-03.

The test probed `reclaim_region(tcb_region).is_err()` before waiting for the child's SEND. Since
DECISIONS §16 (object revocation) was amended, a refused reclaim marks every live thread in the region `killed`, and
`schedule()` turns a killed thread into a corpse at its next preemption. The child then raced its
own next tick and sometimes lost. Widening the window with a call-free delay loop made it hang on
every run on both ISAs; deleting the probe fixed it on any machine at any speed. The first four
sightings were riscv64 because CI boots riscv64 six times per pull request to aarch64's once, not
because of the ISA. The accumulation is not this bug: the 101 leaked threads everyone suspected were
not the cause, and the A/B
showed it. They were real, though: the table later hit 128, and `MAX_THREADS` is 256 now with the
peak reported at every boot.

The general lesson: a call that returns `Err` may still have done something. The mechanism, the
proof, the occurrences table and the thread-table follow-up are in
[scheduler/reclaim-lost-wakeup.md](scheduler/reclaim-lost-wakeup.md).

## CLOSED: a second lost wakeup, and the fingerprint is what told the two apart

`spawn_on` asked "is this target my own core?" twice, on either side of a window in which the
asking thread can be stolen onto a different core. When the two answers disagreed the placement
went into a remote core's inbox and the reschedule SGI was skipped. The thread sat `Ready` forever
beside four idle cores. Found 2026-08-28 on the `rv64` CPU-matrix leg of PR #574.

It shares the watchdog's message with milestone 72 (a lost wakeup)'s bug and is a different one: the dump showed
one `Ready` thread and one undrained inbox. Read the dump, not the banner. Only the reschedule-SGI
handler drains an inbox, by decision (DECISIONS §133 (whether an idle core should drain its own inbox), calef, 2026-08-28), so a missed poke is
permanent. The fix makes `place_on` return the core that owes an SGI as a `#[must_use]`
`Option<usize>`, so there is one comparison and no second answer. A widened window reproduced it,
and the lost work matched the stranded placements exactly.

The general lesson: a decision made under a lock and re-derived outside it is two decisions. The
trace-ring diagnosis, the fix, why a strand usually goes unnoticed and the proof are in
[scheduler/missed-reschedule-poke.md](scheduler/missed-reschedule-poke.md).

## The switch and the rendezvous paths

`schedule`, its locked pick and the reaper's far side are in
[scheduler/switch.md](scheduler/switch.md). Send, receive, call and reply, and how a caller its
server can no longer answer is freed, are in [scheduler/rendezvous.md](scheduler/rendezvous.md).

## Tests that guard this

- `smp::a_batch_of_cpu_bound_work_reaches_every_core`, `smp::work_can_be_placed_on_every_core`:
  placement and stealing fill the machine.
- `smp::a_migrated_kernel_thread_keeps_its_hart_pointer`: the tp invariant under forced migration
  (a no-op on aarch64, a real check on RISC-V via `sscratch` ground truth).
- `smp::a_migrated_kernel_thread_reads_its_own_name`: a preemptible kernel thread's own name under
  the same motion, on all three ISAs; a split read fails it on every one.
- `sched::a_finished_thread_is_reaped_and_its_memory_returned`,
  `user::a_dead_user_thread_frees_its_whole_address_space`: reaping and exact frame accounting under
  cross-core reap lag (the latter waits the lag out rather than reading the instant the count drops).

**No test guards the missed-SGI bug above, and that is deliberate.** Its trigger is a work steal
landing inside a handful of instructions, which no test can schedule. A test that tried would be a
flake pretending to be a guard, and the suite already has the honest instrument for it in the
no-progress heartbeat. The guard is `place_on`'s return type and its `#[must_use]`, which is rung
one of CLAUDE.md's ladder for the part it can reach. A caller now has to be *handed* the answer
rather than derive it. Dropping the answer on the floor is a warning, which CI's `-D warnings` makes
a failure. What it cannot stop is someone writing a fresh `target != cpu::id()` beside it,
which is why `place_on`'s doc comment says out loud what that costs.

### Two flakes that were the tests' fault

`cpu::tests::percpu_is_self_consistent_on_whatever_core_we_run` replaced a test that asserted it
ran on the boot core, an affinity §28's stealing never promised. And several `sched::tests` cases
that wait with a fixed number of `yield_now()` calls fail under host CPU contention, about 15% under
four burners and 0% quiet. They fail in about 23 s with a named assertion, where a lost wakeup fails
at 60 s with a thread dump. A run that fails one is not evidence about a branch until a quiet
machine or a control run says so. These bounds should be progress-based, not a yield count. Both
are recorded in [scheduler/test-flakes.md](scheduler/test-flakes.md).

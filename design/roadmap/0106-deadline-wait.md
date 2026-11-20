---
status: BUILT
raised: 2026-08-04
built: 2026-09-26
---
# 106. A wait that ends on either the interrupt or the deadline

Built 2026-09-26 (PR #1378). A `Timer` kernel object, `Timer::ARM(deadline,
notification)` and `CANCEL`, on aarch64, riscv64 and x86_64, as §147 (a timer a userspace service
cannot hold) ruled. The kernel owns the comparator and signals a notification at the deadline; a
thread waiting on that notification, or blocked receiving with it bound, wakes on either.

## Why this exists

There was no timed wait anywhere in the kernel. Every consumer that wanted to act at a time
yielded in a loop instead, at a derived cost of about `10^5` to one against a timed wait
(`notes/timed-wait.md`, section 5). The one that forced the question was `net_stack`:
smoltcp's retransmits need a `poll` at a deadline, and the only way to get one was to keep a hart
spinning through every backoff.

It waited, after §147 ruled on 2026-09-05 (calef, option 1, the new object), on the TCB binding of
§101 (notification objects), which milestone 151 (notification objects) built (#1351). Until
2026-09-26 this block still gated on milestone 263 (can a userspace process hold a timer, on all three architectures), which was stale
from the day §147 ruled: 263's answer was §147's input, not a separate gate.

The history of the fork is in §147 and in `notes/timed-wait.md`, which priced it. Milestone 51
(wall-clock time, the `date` command, and an NTP service) offered three shapes. calef preferred a
userspace timer service on 2026-09-05, milestone 263 found riscv64 has no comparator U-mode can
hold, and §147 ruled the same day.

## What was built

| piece | where |
|---|---|
| `objtype::TIMER = 5`, `abi::timer::{ARM = 0, CANCEL = 1}`, and `counter_ticks_for` | `crates/abi/src/lib.rs` |
| the decision core: one deadline per timer, the cached earliest, the expiry walk | `crates/inter_process_communication/src/timer.rs` |
| three Kani harnesses, each with a replayable falsification | the same file, `falsifications/timer.verification.*` |
| the registry, `ARM`, `CANCEL`, the tick's one comparison, `expire_timers`, the region sweep | `kernel/src/sched.rs` |
| `Object::Timer`, the system call arm, `RETYPE_OBJ(TIMER)` | `kernel/src/cap.rs`, `kernel/src/system_call.rs` |
| `timer_arm`, `timer_cancel`, `sleep_until`, `retype_sleeper` | `crates/user_mode_runtime/src/lib.rs` |
| five kernel tests, all three ISAs | `kernel/src/user/timer_tests.rs` |

The kernel tests prove what the harnesses cannot:

- the real tick reaches the walk;
- a wait ends on a signal before the deadline, with the timer still armed;
- a wait ends at the deadline and never before it, in `WAIT` and in a bound `RECV`;
- a cancelled or re-armed timer never fires its old deadline;
- `ARM` checks `WRITE` on both the timer and the notification.

A fifth test measures the before and after.

`CANCEL` is §147's by implication rather than by name. §147 names only `ARM`, but the common case in
`notes/timed-wait.md` section 2 is a retransmit timer thrown away when its ACK arrives. Without a
cancel, a consumer would re-arm to a far deadline, which is a cancel spelled worse.

### The consumers converted

| consumer | before | after |
|---|---|---|
| `std::thread::sleep` | `yield` until the counter passes | `ARM`, then `WAIT`; the loop survives as a fallback |
| `net_stack`'s `wait_for_nic` | `yield` and re-poll across a retransmit window | `Irq::WAIT` ends on a frame or smoltcp's deadline |
| the network time client's retry gap | `yield` for 2 ms | one tick asleep |
| `swish`'s `^C` watch | `yield` between looks | 10 ms asleep between looks |

`net_stack`'s notification is made and bound by its spawner (`kernel/src/user/virtual_io_service.rs`),
because a running thread cannot bind itself. See the proposal below.

### Confirmed, and still provisional

calef confirmed the surface on 2026-09-27 (UTC): `objtype::TIMER = 5`, `Timer::ARM(deadline,
notification, bits) = 0` and `Timer::CANCEL = 1`. He declined a slack argument after reading the
prior art (Zircon's `zx_timer_set`, Mach's `mk_timer`, seL4's badge bits, Linux's `timerfd`,
kqueue): *"Keep it confirmed. We will build a slack if we need it."*

Still provisional: the `Object::Timer` variant name, the `TimerId` type, the note name `notes/timer.md`, the module `inter_process_communication::timer`,
and the runtime names `timer_arm`, `timer_cancel`, `sleep_until` and `retype_sleeper`.

## What it cost, measured

### The before and after: 200 ms asleep

From `a_timer_sleep_costs_a_wake_where_a_yield_loop_costs_a_core`, run on each ISA under QEMU on
2026-09-26 (patagonia, `script/test --test timer_tests`). The yield loop is the one
`thread::sleep` ran until this milestone; the timer sleep is its new body.

| ISA | yield loop: yields | yield loop: ticks charged | timer: wakes | timer: ticks charged |
|---|---|---|---|---|
| aarch64 | 69,951 | 20 of 20 | 1 | 0 |
| riscv64 | 34,102 | 20 of 20 | 1 | 0 |
| `x86_64` | 53,087 (15,526 on the one-core leg) | 20 (13) | 1 | 0 |

So a sleeping thread went from a whole core for the duration, and tens of thousands of scheduler
passes, to one wake and nothing charged. That is the `10^5` to one `notes/timed-wait.md` derived,
measured end to end at the low end of its range on emulated cores; a faster real core spins more.

### The IPC fastpath: unchanged within a few bytes

`script/fastpath-footprint`, release build, against the baselines milestone 151 recorded from CI.
riscv64 and `x86_64` are local runs and match CI's:

| ISA | `ipc_call_reply` | `ipc_send_recv` | `system_call_entry` (flat) |
|---|---|---|---|
| riscv64 | 6186 -> 6200 B (+14) | 4854 -> 4862 B (+8) | 1894 -> 1912 B (+18) |
| `x86_64` | 8542 -> 8598 B (+56) | 6512 -> 6560 B (+48) | 1701 -> 1733 B (+32) |
| aarch64 | 7280 -> 7280 B (0) | 5572 -> 5572 B (0) | 1512 -> 1528 B (+16) |

The closure grows by the `Timer` arm of `invoke`'s object match, which the IPC roots pass through;
`timer_invoke` itself is out of line. §147's scaffold measured `system_call_entry` at +12, +158 and +96
B. The aarch64 row is CI's (run 36276567183), because a local aarch64 build disagrees with CI's,
as milestone 151 also found.

### Instructions: icount, against milestone 151's CI figures

`script/bench --check`, TCG with icount, all three passing the 10% tripwire. The left figures are
milestone 151's from CI run 36258812324 and the right ones this branch's on patagonia, so a small
move can be build difference rather than this change.

| ISA | `ipc_rtt` | `call_reply` | `ipc_rtt_el0` |
|---|---|---|---|
| aarch64 | 1045327 -> 1045334 (+7) | 1061673 -> 1061676 (+3) | 11034763 -> 11050503 (+0.14%) |
| riscv64 | 171076 -> 171449 (+0.22%) | 177534 -> 177534 (0) | 1865150 -> 1866611 (+0.08%) |
| `x86_64` | 17313622 -> 17313736 (+114) | 17830619 -> 17827655 (-0.02%) | not measured |

The idle tick pays one relaxed load, one counter read and one compare, which `notes/timed-wait.md`
section 3 priced at about 30 debug-build instructions per tick per core. It lands in benches long
enough to span ticks, which is why only the `_el0` row moves.

## Proposed: binding a notification to yourself

A running thread holds no `ThreadControlBlock` capability to itself, so it cannot `BIND` a
notification to itself. Milestone 151's handoff named this as 106's to propose. The options, with
the seven questions answered, are in `notes/timer.md` ("Binding a notification to yourself:
PROPOSED"). The short form:

- The premise is half true. `thread::sleep` does not need a binding: it waits on the notification
  directly. A binding is needed only to wait on a notification and an endpoint at once.
- A spawner can bind for its child today, with no wire change. `net_stack` is built that way.
- A self sentinel in `BIND`'s slot, a capability to your own TCB, or a new `BIND_SELF` method are
  each a wire decision, so they are an architect's, offered without a recommendation.

Nothing in this milestone waits on the answer. The first consumer that might is `timetable` under
milestone 129 (scheduled execution: a cron whose every entry is a grant).

## BUGS

- Resolution is the scheduler tick. A deadline fires at the first 10 ms tick at or after it: never
  early, up to one tick late. A tickless comparator would fix it at the rearm seam §178 (where the
  timer re-arm seam goes) placed, on all three architectures. Nobody has asked for finer.
- `x86_64` compares deadlines against each core's own TSC. That is right on an invariant,
  synchronized TSC, which QEMU provides. Nothing checks it at boot, and xenon has not been checked.
- A deadline already passed signals from `ARM` with a thread's local placement, and one that expires
  signals load-aware from the tick. The two paths wake the same way; only placement differs, as
  `notes/notification-objects.md` describes for the two signal entries.
- The shell's watch still wakes 100 times a second while a job runs. That is milestone 103 (`^C`
  stops spinning: the shell's interrupt watch, blocking)'s to finish. And
  `grant_plan::COOP_GRACE_TICKS` counts watch iterations, so its 200 are now two seconds of wall
  time where they were 200 yields.
- A sleeper blocks only when it can be made safely, and otherwise keeps the yield loop, silently.
  `RETYPE_OBJ` puts a capability in the first free slot, and an empty fixed slot is how a std
  program, the shell and the NTP client learn a service was not granted. So `retype_sleeper` and
  the std PAL refuse while any such slot is empty; a std program granted no network or no directory
  still spins in `thread::sleep`. Found by the std tests, where the first version faulted: its
  notification landed in slot 4 and the PAL took it for a directory. The fix that ends the fallback
  is the loader granting the pair, as `net_stack`'s spawner does, which is a change to the std
  runtime contract and is recorded in
  `design/roadmap/0674-the-last-yield-spinners-sleep-on-a-timer.md`.
- `std::thread::sleep` costs a process two pages of its heap untyped on first use.
- `counter_ticks_for` is tested, not proved. CBMC did not finish it with a symbolic rate in ten
  minutes, nor with five concrete rates in five.
- "There is no timed wait" is still written in about a dozen places this milestone made false:
  `components/src/timetable.rs`, `crates/top`, `notes/clock.md`, `notes/ntp.md`,
  `notes/scheduled-execution.md`, `notes/hung-component.md`, `notes/pipes.md` and its
  `one-wait-point.md`, and the blocks of milestones 51, 126 and 129. They were left because most sit
  in files other lanes are editing today. `grep -rn "no timed wait"` finds them; each owner's next
  edit should correct its own.
- The expiry walk is O(timers) per timer fired, under the whole-machine lock, in interrupt context.
  At the registry bound of 256 that is a short walk; it is the number to watch if timers multiply.

## What it unblocks

- Milestone 103 (`^C` stops spinning: the shell's interrupt watch, blocking) has its primitive.
- Milestone 129 (scheduled execution)'s timetable, and the per-user session's cost the schedule
  decision names, can end their yield loops. What each needs is in
  `design/roadmap/0674-the-last-yield-spinners-sleep-on-a-timer.md`.
- Milestone 23 (a capability-routed component OS with live replacement)'s liveness watch: a `RECV`
  on the reports endpoint with a deadline, through a binding its spawner makes.

## Follow-on

- **Recorded.** Binding a notification to yourself is a wire decision for an architect, written up with
  its options in `notes/timer.md` ("Binding a notification to yourself: PROPOSED").
- **Milestone 674.** Milestone 674 (the last yield-spinners sleep on a timer). The soak supervisor, a long-running NTP client and the timetable's adoption:
  `design/roadmap/0674-the-last-yield-spinners-sleep-on-a-timer.md`.
- **Milestone 103.** The shell's watch the rest of the way, waking only on the job's end or `^C`.
- **Milestone 129.** The timetable sleeps until its next deadline, and the per-user session's yield
  cost ends with it.
- **Recorded.** Tick resolution, the unchecked TSC synchronization, the silent sleep fallback and the
  stale "no timed wait" claims, in this block's `BUGS` beside `kernel/src/sched.rs`'s timer code.

## Index row

Built 2026-09-26 on §147 option 1: a `Timer` object whose `ARM(deadline, notification)` signals a
notification at the deadline, on all three ISAs, proved and falsified. `thread::sleep`, `net_stack`'s
retransmit window, the NTP retry gap and the shell's `^C` watch no longer spin. Self-binding is
proposed, not built.

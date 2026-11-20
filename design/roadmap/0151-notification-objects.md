---
status: BUILT
raised: 2026-08-22
built: 2026-09-26
---
# 151. Notification objects: async multiplexing without wait-any

Built 2026-09-26 (PR #1351). A notification object with the four methods of §101 (notification objects) and its
TCB binding, on aarch64, riscv64 and `x86_64`, proved (four Kani harnesses, each falsified) and
tested through the system call boundary by a real program on all three. One part of §101 could not be
built as written, because its premise was false: how a woken receiver tells a notification from a
message. calef ruled on it the same day (option B, below). Minted 2026-08-22, from DECISIONS §101's
own sequencing (step 2), which specified this milestone's shape and scope without a number.

## Why this exists

A concrete, waiting bug forces it: milestone 40's `terminal_sink_caretaker` narrowing (§101 step 1,
DECISIONS §106 (take the `terminal_sink_caretaker` narrowing)) can outrun its own completion signal. A child's exit is delivered cleanly today
via §26's fault/exit endpoint, but the caretaker's own trailing delivery to `line_editor` is a
second, independent `CALL` with no ordering primitive against the shell printing its next prompt.
`notes/tail-output-narrowing.md` names this precisely: a page's last line and the next `$ ` can
interleave under contention. A display glitch, not a confinement or correctness failure, and it is
being carried as a documented `BUGS` entry on milestone 40 until this milestone lands.

Three other consumers are already named and waiting on the same primitive (§101's own table).
The shell multiplexing child-output-or-exit into one wait, the compositor's per-client wakeup, the
FS server's async-event notification, and the network stack retiring its yield-and-re-poll spin
(milestone 106's current interim).

## What to build

Exactly §101's spec, no re-derivation needed:

- A new `objtype::NOTIFICATION` (= 4), holding a data word and a wait queue, plus at most one
  bound TCB (`Option<ThreadId>`, set by `BIND`).
- Four methods under the existing `SYS_INVOKE` surface (no new system call number): `SIGNAL`
  (async, non-rendezvous, never lost), `WAIT` (blocks until a signal arrives, returns the word),
  `POLL` (non-blocking `WAIT`), `BIND` (attach a notification to a TCB, at most one).
- The binding mechanism: when a TCB with a bound notification is blocked in `RECV` on an
  endpoint, `SIGNAL` on that notification wakes it the same way a message would, and the woken
  thread can tell which happened. This is the multiplexing primitive: `RECV`-on-endpoint-or-signal
  in one wait point, without a wait-any/port-set container (§101's "Why not the alternatives"
  section already rejects that shape and explains why the simpler design suffices).
- Kani proof of the object's state machine, matching the coverage `Endpoint` already has.

Explicitly out of scope, per §101: badged capabilities (a separate, later decision already
named in notes/supervision.md, notes/compositor.md, notes/dir-capability.md) and timed wait
(milestone 106, orthogonal: `WAIT` blocks indefinitely here exactly as `RECV` does today).

## What was built

| piece | where |
|---|---|
| the state machine: a word, a wait queue, `signal`/`wait`/`poll`, and the binding as one boolean | `crates/inter_process_communication/src/notification.rs` |
| four Kani harnesses: every operation preserves "a non-zero word and a queued waiter never coexist", a signal loses no bit, a waiter is woken before the bound receiver, a wait never returns zero; each carries a replayable falsification patch, and `script/falsifications --sweep inter_process_communication` turned all ten of the crate's harnesses red | same file, `falsifications/notification.verification.*.patch` |
| `objtype::NOTIFICATION = 4`, `abi::notification::{SIGNAL, WAIT, POLL, BIND, BOUND}` | `crates/abi/src/lib.rs` |
| `Object::Notification`, the registry (`MAX_NOTIFICATIONS = 256`), `RETYPE_OBJ`'s arm | `kernel/src/cap.rs`, `kernel/src/sched.rs`, `kernel/src/system_call.rs` |
| the binding: a signal reaches the bound thread parked in `RECV`, `RECV_CAP` or `Irq::WAIT`, unlinks it from that endpoint's queue and wakes it; a receive takes a word counted while the thread was elsewhere, on entry | `sched::signal_locked`, `bound_receiver`, `deliver_bound`, `take_bound_signal` |
| one signal path for a thread and for the kernel: `signal_notification_from_interrupt` is the entry milestone 106's timer will call from the tick, placed load-aware as `irq_notify` is | `kernel/src/sched.rs` |
| teardown: a region's notifications are swept (waiters aborted with `Gone`) before its threads are finished; a thread blocked in `WAIT` is unlinked by `finish_blocked_resident` | `reap_region_notifications` |
| `Wait` is an enum (`Rendezvous(id, role)` or `Notification(id)`), so a notification's name cannot be read as a rendezvous's | `kernel/src/thread.rs` |
| user wrappers: `notification_{signal,wait,poll,bind}`, and `recv_bound` returning `Received::{Message, Notification}` | `crates/user_mode_runtime/src/lib.rs` |
| seven kernel tests, all three ISAs, 7/7 on each locally before CI; one drives `fixtures/src/notification_binder.rs` through the real boundary | `kernel/src/user/notification_tests.rs` |

Method semantics are in `abi::notification`'s doc comments, and the reasoning is in
`notes/notification-objects.md`. Method and opcode numbers are §101's (`SIGNAL` 0, `WAIT` 1,
`POLL` 2, `BIND` 3, `NOTIFICATION` 4), recorded here for the maintainer to confirm in
`design/decisions/`.

## Decided in the build: how a woken receiver tells a notification from a message

calef, 2026-09-26: option B. §101's encoding (`w0 = 2` on a bound delivery, `w0 = 0` for a
message) rested on a false premise: `RECV`'s `w0` is the sender's own first data word, so any
sender could forge a notification. The kernel now writes `abi::notification::BOUND` (2) into `w4` on
every receive that the bound notification ends, on all three receive methods, and keeps §101's
`w0 = 2` with the word in `w1`. `w4` is written only by the kernel, and is `0` on every other
receive. The options, costs and prior art are in `notes/notification-objects.md`; a maintainer is
recording the ruling as an amendment to §101.

## What it cost, measured

Code size on the IPC fastpath (`script/fastpath-footprint`, release build, against the base
commit 256815e5 on the same nightly, not against the recorded baseline, which main had already
drifted from by +2.0% to +3.9%):

| ISA | `ipc_send_recv` | `ipc_call_reply` | `system_call_entry` |
|---|---|---|---|
| riscv64 | 4778 -> 4854 (+76 B, +1.6%) | 6082 -> 6186 (+104 B, +1.7%) | 1892 -> 1894 (+2 B) |
| `x86_64` | 6464 -> 6512 (+48 B, +0.7%) | 8398 -> 8542 (+144 B, +1.7%) | 1701 -> 1701 |
| aarch64 | 5500 -> 5572 (+72 B, +1.3%) | 7172 -> 7280 (+108 B, +1.5%) | 1508 -> 1512 (+4 B) |

The aarch64 row is CI's measurement, and the local one disagreed. A local macOS run measured this
commit at 4664, 6052 and 1988: LLVM appeared to stop inlining `IrqSafeMutex<IpcTables>::lock`, so
every IPC function shrank while `system_call::dispatch` grew. The same commit built by CI's
`fastpath-footprint` job showed no such flip, and the baseline records CI's numbers. The cause of
the local difference was not found; see BUGS. About 1.5% to 1.7% on the call/reply shape, on every
ISA.

Where the riscv64 and `x86_64` bytes are. The entry check (a load of `bound_notification` and a
branch, with the body in a `#[cold]` function) costs `ipc_recv` about 80 to 90 bytes after one
round of shrinking. The first version returned early from inside the lock hold, which duplicated the
unlock path and cost 138 and 174 bytes. Joining the decision's other arms instead halved it. The
`RECV_CAP` store to `x4` and the five-word return cost `ipc_recv_cap` 34 bytes (riscv64) and 54
(`x86_64`) on their own, measured by compiling the entry check out; that is the cost of option B on
the call/reply path. After rebasing onto a main that had re-recorded its own baselines, this
change sits inside the gate's 5% band on every ISA, so `bench/fastpath-*.txt` keep main's figures.

Instructions (the `bench` job's TCG + icount counts, deterministic, CI run 36258812324), against
the recorded baselines in `bench/baseline-*.txt`:

| ISA | `ipc_rtt` | `call_reply` | `ipc_rtt_el0` |
|---|---|---|---|
| aarch64 | 1034642 -> 1045327 (+1.03%) | 1057883 -> 1061673 (+0.36%) | 10994714 -> 11034763 (+0.36%) |
| riscv64 | 170436 -> 171076 (+0.38%) | 177088 -> 177534 (+0.25%) | 1861156 -> 1865150 (+0.21%) |
| `x86_64` | 17254582 -> 17313622 (+0.34%) | 17788619 -> 17830619 (+0.24%) | not measured on this ISA |

All inside the tripwire's 10%, and all under about one percent. These are against the recorded
baselines rather than the base commit, so they include whatever main had drifted since the
baselines were saved. The footprint figures above say that drift is real, so the notification
share is at most these numbers.

Size against §101's estimate ("the same scale as 19a"): 19a (66553029e) was 377 lines added
across 8 files. This is about 1,800 across 13, of which the kernel's own logic is about 470 lines of
`sched.rs` (whitespace-insensitive), the proved crate 458 with its harnesses, and the tests 544.
About five times 19a, and the binding (reaching into another object's queue, and the teardown it
implies) is most of the difference.

## What it unblocks

- Milestone 106 (a timed wait), whose §147 (a timer a userspace service cannot hold) shape is `Timer::ARM(deadline, notification)`. The
  signal path it needs exists: `sched::signal_notification_from_interrupt(id, bits)` from the tick,
  and a thread blocked in `RECV` or `Irq::WAIT` with the notification bound wakes on either. What
  106 still owns is below, under BUGS: a running thread cannot bind to itself.
- Closes milestone 40's caretaker-hop race (its `BUGS` entry converts to: the shell binds a
  notification to its own TCB and `WAIT`s on "the caretaker's queue for this client has drained"
  instead of guessing, per §101's retrofit step.
- The shell's exit-wait hack retires, replaced by a proper multiplexed wait (§101 step 3).
- The compositor, FS server, and network stack can each take the retrofit named in §101's table,
  as their own follow-on work; this milestone builds the primitive, not their consumers.

## BUGS

- A running thread cannot bind a notification to itself. `BIND` names its thread through a
  `ThreadControlBlock` capability, and a running thread holds none to itself: a spawner holds the
  child's until `START`, and the kernel's own services never had one. So a binding is made by
  whoever builds the thread, before or after it starts, or (as the tests do) by a spawner that mints
  the capability. That is enough for a spawner-built service; it is not enough for `std`'s
  `thread::sleep`, which milestone 106 wants and which must bind the calling thread. The fix might be a
  "this thread" sentinel in `BIND`'s slot argument (as `exit` and `yield` are authority over
  yourself) or a spawn-time grant. Either is outside §101 and is a wire decision, so it is 106's to
  propose, not this milestone's to invent.
- There is no unbind. §101 has none. A binding lasts until either side is destroyed; a thread
  whose notification was destroyed may be bound again, because the stale name resolves to nothing.
- `WAIT` and `POLL` return the word in `x0`, where a negative value is an error. A word with its
  top bit set whose value lands on an error code (-1 to -11) reads as that error. It needs a signaler
  to set almost every bit, so it is unlikely rather than impossible. The fix is a register convention
  (`x0` status, `x1` word), which is a wire change and was not taken without a consumer asking.
- `SIGNAL` with zero bits does nothing and wakes nobody. A choice (`WAIT` never returns zero, so
  zero can mean "nothing" from `POLL`), and a program that signals zero expecting a wake will wait.
- A bound delivery unlinks the thread from its endpoint by drain-and-repush, O(receivers queued
  on that endpoint), because the intrusive queue is singly linked. A server's endpoint usually holds
  one receiver, itself.
- The kernel-originated path has no interrupt-context caller yet. `signal_notification_from_interrupt`
  is exercised from a kernel thread; that it is safe on the interrupt stack is argued from
  `irq_notify`, which takes the same lock the same way, not tested. Milestone 106's tick is the first
  real caller and the first proof.
- A sender can make a receiver read an error. Not new, found while checking the premise above:
  `RECV` returns the sender's `w0` in `x0`, and a sender that sends a negative `w0` equal to an error
  code is decoded as that error by every wrapper that checks the sign. Recorded at
  `abi::rendezvous::RECV`, where a reader meets it.
- A local footprint measurement can disagree with CI's on aarch64. This lane's local macOS run of
  `script/fastpath-footprint` showed an inlining flip (closures down 15%, `system_call_entry` up 32%)
  that CI's job on the same commit did not. The local target directory had also been shared with a
  second worktree for a base comparison, which is a plausible contaminant and was not ruled out.
  Take CI's number when the two disagree.
- Test residue: `a_kernel_signal_ends_an_irq_wait_...` binds INTID 251, which no device on any of
  the three machines raises, and `bind_irq` has no unbind, so the route outlives the test.

## Follow-on

- **Milestone 106.** The timer that calls `sched::signal_notification_from_interrupt` from the tick,
  which is also the first test of that path in interrupt context. And a way for a running thread to
  bind a notification to itself, which `thread::sleep` needs and §101 does not specify (a wire
  decision 106 proposes).
- **Milestone 40.** Milestone 40 (documentation as a system service) owns the caretaker-hop race's retrofit: the shell binds a notification and waits on
  the caretaker's drain instead of guessing. The primitive is here; the consumer is 40's.
- **Milestone 33.** Milestone 33 (a compositor) owns the compositor's per-client endpoints woken by a bound notification, §101's
  table's second row. Its own follow-on, not built here.
- **Recorded.** No unbind, `WAIT`'s word sharing `x0` with the error encoding, `SIGNAL(0)` as a
  no-op and the O(queue) unlink, all in this block's `BUGS` and in `crates/abi/src/lib.rs` beside
  `abi::notification`.
- **Done.** The latent `w0 = 1` IRQ forgery this lane found: calef ruled B on 2026-09-26 (§101's
  second amendment), and milestone 603 (an interrupt's endpoint refuses every send) built it.
- **Recorded.** A sender can make a receiver read an error by sending a negative `w0`: beside
  `abi::rendezvous::RECV` in `crates/abi/src/lib.rs`.
- **Decision.** The receive tag (option B), ruled by calef on 2026-09-26, is being recorded as an
  amendment to `design/decisions/0101-notification-objects.md` by the maintainer; the method and
  object-type numbers are §101's own.

## Index row

BUILT 2026-09-26. A notification object (DECISIONS §101): a data word and a wait queue, with
`SIGNAL`, `WAIT`, `POLL` and `BIND` under the existing `SYS_INVOKE`. Its TCB binding wakes a thread
blocked in `RECV`, `RECV_CAP` or `Irq::WAIT` on either a message or a signal. Proved in
`inter_process_communication`, tested through the system call boundary on all three ISAs. §101's
receive encoding was forgeable; calef ruled the kernel-written `w4` tag on 2026-09-26. About 1.5% to 1.7%
on the call/reply fastpath, on every ISA. Unblocks milestone 106's timer and the retrofits §101's
table names (the shell, the compositor, the FS server, `net_stack`).

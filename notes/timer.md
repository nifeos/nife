# The timer: a wait that ends on a deadline

*(Written 2026-09-26 by the lane for milestone 106 (a wait that ends on either the interrupt or the
deadline). The note name is provisional. The pricing this was built from, and the three shapes
DECISIONS §147 (a timer a userspace service cannot hold) chose between, are in
[timed-wait.md](timed-wait.md).)*

## What was built

Built 2026-09-26 on §147's option 1 and on the binding from milestone 151 (notification objects),
on all three architectures. The prototype `timed-wait.md` describes was thrown away in August. This
is the same shape, built again on purpose, and the numbers in the milestone block are its own.

### The object

`objtype::TIMER` and `Timer::{ARM, CANCEL}` live in `crates/abi`, and their numbers are provisional.
`ARM(deadline, notification_slot, bits)` replaces any pending deadline. A deadline already reached
signals before `ARM` returns. `CANCEL` returns `1` if it stopped a pending deadline and `0`
otherwise, so a `0` tells the caller any signal has already happened.

§147 names only `ARM`, so `CANCEL` is there by implication. The common case in `timed-wait.md`
section 2 is churn: an ACK arrives and the retransmit timer is thrown away. Without a cancel, a
consumer would re-arm to a far deadline instead, which is a cancel spelled worse.

Deadlines are absolute counter ticks. That is the counter a program already reads without a syscall,
so nothing converts between clocks. `abi::timer::counter_ticks_for` turns a duration into ticks,
rounded up so a sleep is never short.

### The tick

`on_tick` does the one comparison `timed-wait.md` section 3 priced. It loads `EARLIEST_DEADLINE` and
compares it with the counter. Only a due tick calls the out-of-line `expire_timers`. That takes
`IPC_TABLES`, fires every due timer through milestone 151's `signal_locked` with the load-aware
placement, and recomputes the cache exactly. An arm lowers the cache. A cancel leaves it stale-low,
which costs one walk that fires nothing.

### What is proved, and what is only tested

The arithmetic is in `crates/inter_process_communication/src/timer.rs`, under three Kani harnesses,
each with a replayable falsification:

- the cache is never later than an armed deadline, across arm, cancel and walk;
- a walk fires exactly the due timers, once each;
- a cancelled or re-armed timer never fires its old deadline.

`counter_ticks_for` is tested rather than proved. CBMC did not finish it in ten minutes with a
symbolic rate, nor in five with five concrete ones; the 64-bit divide is what costs. Its tests pin
every boundary at every counter rate this tree runs on.

### The consumers wired

- `std::thread::sleep` blocks. The yield loop survives only as a fallback, for a process whose
  untyped cannot pay the two pages.
- `net_stack`'s retransmit window. Its `Interrupt::WAIT` ends on a frame or on smoltcp's next
  deadline,
  through a notification its spawner binds.
- `swish`'s `^C` watch sleeps 10 ms per look instead of yielding. That is an interim; the
  destination belongs to milestone 103 (`^C` stops spinning: the shell's interrupt watch, blocking).

### Resolution is the tick

A deadline fires at the first tick at or after it: never early, up to 10 ms late. A tickless
comparator would program each core's timer for the earlier of its next tick and the earliest
deadline. That removes the lateness, and it is a change to all three `arch/*/timer.rs` rearm paths,
at the seam §178 (where the timer re-arm seam goes) placed. Nobody has asked for a wake finer than
10 ms: smoltcp's timers are in milliseconds and its retransmits in hundreds of them.

## Binding a notification to yourself: PROPOSED

The question comes from milestone 151's handoff. `Notification::BIND` names its thread through a
`ThreadControlBlock` capability, and a running thread holds none to itself. So a thread cannot bind
a notification to itself. Is that a gap to close, and how?

### The premise is half true

`thread::sleep` was named as the consumer that needs it, and it does not. A thread that only sleeps
waits on the notification directly with `WAIT`. A binding is for waiting on a notification and an
endpoint at once.

The threads that need one are blocked on an endpoint and also want a deadline:

- `net_stack`, in `Interrupt::WAIT`;
- `timetable`, in `receive_fault` on its supervision endpoint;
- the liveness watch of milestone 23 (a capability-routed component OS with live replacement);
- milestone 103's real `^C` watch.

Nothing in milestone 106 waits on the answer. Each of those has a spawner that holds the child's
`ThreadControlBlock` capability, and that is option A below, which works today.

### What the tree already does

`SYS_EXIT` and `SYS_YIELD` are authority over yourself with no capability. `abi::rendezvous::NO_CAP`
is a sentinel in a slot argument. Every capability a process holds arrived from its spawner, and
`kernel/src/user/virtio_service.rs` now binds `net_stack`'s notification that way.

### Prior art, from memory and marked as such

seL4's `seL4_TCB_BindNotification` takes a TCB capability. Its root task is handed
`seL4_CapInitThreadTCB`, a capability to itself, and CAmkES gives the threads it builds their own.
Zircon has no binding: a thread waits on a port that many objects post to.

### The options

| option | what it is | wire change |
|---|---|---|
| A. The spawner binds | make the notification, `BIND` it to the child's TCB, grant it in a slot | none |
| B. A self sentinel | `BIND(SELF)` with a reserved slot value such as `u64::MAX` | one argument value |
| C. A capability to your own TCB | every spawner grants each thread one, as seL4's root task gets | none in the ABI |
| D. A new method | `Notification::BIND_SELF`, a fifth method number | a method number |

What each costs:

- A is what `net_stack` does now: a region page per object, in a spawner willing to do it. A thread
  cannot get a binding later.
- B is under twenty lines in `syscall.rs` and one wrapper. Consent is not in question, since binding
  changes only what your own receives return.
- C is a line in each spawner and one of 24 slots per process. It also hands out `CAP_INSERT` on
  yourself, which nothing uses and nobody has reviewed.
- D costs what B does, plus a number on the surface forever.

A is code in spawners, and reversible. B, C and D are each a wire decision programs will be written
against. They are calef's, so this note gives them without a recommendation. Nobody has acted on any
of them yet.

### What waits on the answer

Nothing in milestone 106. The first real case is `timetable` under milestone 129 (scheduled
execution). Its spawner is the per-user session, so even that is option A if the session binds.

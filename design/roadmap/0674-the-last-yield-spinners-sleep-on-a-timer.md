---
status: NOT-STARTED
raised: 2026-09-26
promoted_from: the-last-yield-spinners-sleep-on-a-timer
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 674. The last yield-spinners sleep on a timer

*Title and slug are drafts.*
Raised by the lane for milestone 106 (a wait that ends on either
the interrupt or the deadline), which built the `Timer` object and converted four consumers. This
records the ones it did not convert, and what each needs.

The first two items below are code in one file each, with no syscall, wire format or
dependency. The timetable item is gated as its own section says.

## What milestone 106 converted

`std::thread::sleep`, `net_stack`'s retransmit window, the network time client's retry gap and the
shell's `^C` watch (to a 10 ms sleep per look). See `notes/timer.md`.

## What is left

### The soak supervisor, a kernel thread

`kernel/src/soak.rs` yields in a loop, and its `BUGS` says why: "this kernel has no sleep-until
primitive a kernel thread can use." It has one now. A kernel thread can call `sched::timer_arm` and
`sched::notification_wait` directly, as `kernel/src/user/timer_tests.rs` does.

The catch is that its spin is partly wanted. Its `BUGS` calls the load "not entirely a cost (it is
one more thread contending)". Converting it changes what a soak measures, so the lane doing it
should record the round-trip rate before and after, and say which one the soak's numbers now mean.

### A long-running network time client

`components/src/network_time_client.rs` is one-shot, and the kernel does not force that: a
64-second poll sleeps on a timer rather than yield-spinning. A continuously polling client still needs a supervisor and a
policy for what it does between polls, which is the territory of milestone 51 (wall-clock time, the `date` command, and an NTP service). This
item is only the record that the kernel is not the blocker.

### The timetable, and the per-user session's cost

`components/src/timetable.rs` yields between fires, and it blocks in `receive_fault` on its supervision
endpoint when its budget is spent. The decision on who holds a user's schedule, still on an open
branch as this is written, names the yield loop as its recommended option's cost, "paid once per scheduling user", and says it ends when milestone 106 lands.

What adopting takes:

1. A timer and a notification, retyped from its own budget with `user_mode_runtime::retype_sleeper`.
2. Between fires, `sleep_until(Registry::next_deadline)`. That alone ends the spin.
3. To also collect corpses while asleep, the notification must be bound to its own thread, so
   `receive_fault` ends on a death message or the next deadline. A running thread cannot bind itself
   (`notes/timer.md`, "Binding a notification to yourself: PROPOSED"). Its spawner, the per-user
   session of milestone 152 (durable delegation: authority that outlives the session that requested it), can: make the pair, `BIND` it to the timetable's TCB, grant both.
4. A bound-aware `receive_fault`, testing `w4` for `BOUND` as `receive_bound` does.

Steps 1 and 2 need nothing new. Steps 3 and 4 wait on the session's spawner being willing to bind,
or on calef's answer to the self-binding proposal. This belongs to milestone 129 (scheduled
execution: a cron whose every entry is a grant), not to a lane of its own.

### A std program with an empty contract slot

`std::thread::sleep` makes its timer and notification from slot 0 on first use. `RETYPE_OBJ` puts
each in the first free slot, and the std runtime contract says "not granted" by leaving a slot
empty. So a program with no network or no directory would find its sleeper read as that service.
The PAL refuses to make one while any contract slot is empty, and such a program still spins.

The fix is the loader granting the pair in two new contract slots, as `net_stack`'s spawner does.
That changes `crates/std_runtime_protocol`, which the progenitor, the kernel's `std_service` and the
PAL all read, so it wants the contract's owner, milestone 595 (provisional), in the room. The
alternative, a way to retype into a named slot, is a syscall-surface change and calef's.

### The shell's `^C` watch, the rest of the way

Milestone 103 (`^C` stops spinning: the shell's interrupt watch, blocking) owns it. The shell now
sleeps a tick per look. A watch that wakes only when the job ends or `^C` arrives needs the job's
exit and the terminal's `^C` to signal a notification the shell waits on.

## Index row

Milestone 106 (a wait that ends on either the interrupt or the deadline) converted four consumers to the `Timer` object and left the rest. Proposed: convert the soak supervisor and the other remaining yield loops.

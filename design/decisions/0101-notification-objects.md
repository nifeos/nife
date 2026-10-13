---
status: AMENDED
raised: 2026-08-20
decided: 2026-08-20
ratified_by: calef
---

# 101. Notification objects: async multiplexing without wait-any

The single-wait-point limitation is the structural constraint every component in this tree works
around. Decided 2026-08-20: the seL4 notification object model is the right long-term solution for
this kernel, and this file is the specification a build lane works from.

What is blocked: nothing directly. The `terminal_sink_caretaker` narrowing (the remaining fork of
milestone 40 (documentation as a system service)) unblocks the documentation viewer without this.
What this unblocks is the class of problem: every component that needs to wait on more than one
thing at once gets the primitive it is currently working around the absence of.

## Amended 2026-09-26: a bound delivery is marked in `w4`, which only the kernel writes

calef, 2026-09-26 (recorded 16:54 UTC): *"151: confirm B."*

The encoding in "The return-value distinction" is forgeable. `RECV` returns the sender's own first
data word in `w0` (the `RECV` arm of `kernel/src/syscall.rs` returns `msg[0]`), so any sender can
`SEND(2, word, 0)` and pass for a bound notification. That section stands as ruled; this replaces
it.

The ruling, option B:

- `w0 = 2` with the notification word in `w1` stays.
- On a notification delivery, the kernel also writes `2` to `w4`, in each of `RECV`, `RECV_CAP`
  and `Irq::WAIT`.
- A receiver tests `w4`. A `SEND` carries three words and cannot reach it; the kernel writes `0`
  there for every ordinary message and every §26 (the fault endpoint) death message.

The constraint this leaves: §26.4 (dead-until-reaped, which reserves that word for a fault-reply
protocol) must never put `2` in `w4`.

Refused, per `notes/notification-objects.md` on pull request #1351: A (as written) and E (a new
error meaning "notified") are forgeable. C (each method's own kernel-written register) is free on
every path but makes three rules of one. D spends a method number on a register convention.

The IRQ signal, `w0 = 1`, was forgeable too; the next amendment closes it.

## Amended 2026-09-26: an endpoint bound to an interrupt refuses every send

calef, 2026-09-26 (recorded 21:33 UTC): *"B"*, on the IRQ migration.

The lane for milestone 151 found that `w0 = 1` held only by wiring. All fifteen `bind_irq`
endpoints are made for their interrupt and never granted as a `Rendezvous` capability, and a soak
build binds endpoints its caller supplies. Nothing refused a `WRITE` grant.

The ruling, option B:

- An endpoint bound with `bind_irq` refuses every deposit from userspace: `SEND`, `SEND_CAP`,
  `CALL`, and any later method that deposits a message, with an error.
- So `w0 = 1` is unforgeable by kernel rule, not by wiring.
- Moving interrupts onto notification objects (option A) is decided later, driver by driver, when a
  driver needs to wait on its interrupt and its requests together.

Refused: C, a `BUGS` entry that relies on wiring.

Built by milestone 603 (an interrupt's endpoint refuses every send), a provisional number.
`bind_irq` marks the rendezvous, `Rendezvous::send` answers `Send::Refused`, and the three methods
return `NotPermitted` without blocking. A §26 death message sent there is dropped, because `EVENT_FAULT`
is also `1`.

## What is being decided

Whether to add a notification object - a new kernel object type, created by `RETYPE_OBJ`, that
holds a data word and a wait queue, supports `SIGNAL` (async, non-rendezvous, never lost) and `WAIT`
(blocking receive of the accumulated word), and can be bound to a TCB so that a signal arriving
on the notification wakes a thread that is currently blocked in `RECV` on an *endpoint*.

This is seL4's notification object (seL4 manual, chapter "Notifications"), adapted to this kernel's
idioms: untyped-funded, generational, capability-governed, and riding on the existing `INVOKE`
dispatch rather than adding a new syscall.

## Why this is the right long-term solution

### The single wait point is the structural limit

A process in this kernel has exactly one blocking wait point. `SEND` blocks until a receiver takes
the message; `RECV` blocks until one arrives; there is no select, no poll, no receive-on-a-set, no
timed wait (DECISIONS §51's fork is still open, milestone 106 is NOT-STARTED). Every component that
must distinguish more than one class of sender hits this wall:

- Shell (notes/pipes.md): cannot feed a chain and read from it, so `doc page.md` is refused
  without a barrier downstream.
- Compositor (notes/compositor.md; §33 (the compositor's authority is memory, not messages)):
  cannot hold per-client endpoints, so it routes everything through one endpoint and carries
  authority in memory. "The constraint is structural, not stylistic."
- Supervision (§26.5): chose a shared endpoint with kernel-stamped identity *specifically to
  avoid* needing wait-any or a thread per child.
- FS server (notes/fs-server.md): wants a set of endpoints, doesn't have it.
- Credentialer (notes/credentials.md): "this kernel has one wait point."
- Network stack (milestone 106): yields and re-polls across every retransmit window, burning a
  hart, because there is no timed wait.

Every one of these is the same missing primitive wearing a different hat. The `terminal_sink_caretaker`
narrowing solves one instance for one program; a notification object solves the class.

### The kernel already has half the mechanism

`ipc::Endpoint` (`inter_process_communication::Rendezvous` since §113 and 2026-09-19) already
carries a pending-signal count: `signal()` wakes a waiting receiver or counts the signal so it
is not lost; `recv` drains a pending signal first. IRQ capabilities are bound to endpoints via
`bind_irq`, and `irq_notify` calls `endpoint.signal()`. The proved invariant ("at most one wait
queue is ever non-empty") holds for signals because a signal never queues the signaller - it is
deliberately not a rendezvous.

What is missing is a user-callable signal, a separate object type, binding to a TCB, and a badge.
The design adds the first three. The list was cut on 2026-09-26 to fit the second amendment
under §212's cap, and is
[at `256815e56`](https://github.com/nifeos/nife/blob/256815e5609cfb961ccb6526f8a0d3316e589402/design/decisions/0101-notification-objects.md#the-kernel-already-has-half-the-mechanism).

## What other operating systems do

Moved to [`prior-art.md`](0101-notification-objects/prior-art.md): seL4, then Unix, Fuchsia, Mach
and Redox.

## The design

### New object type

```
objtype::NOTIFICATION: u64 = 4
```

A notification is a page-resident kernel object, created by `RETYPE_OBJ`, owned by the caller's
untyped budget. It holds:

- A notification word (`u64`), acting as a bitfield of binary semaphores.
- A wait queue (intrusive `Fifo<Thread>`, same as an endpoint's receiver queue).
- A bound TCB (`Option<Tid>`, set by `BIND`, at most one).

One page per object, same as every other object type (§14's one-object-per-page rule).

### New methods

Four: `SIGNAL`, `WAIT`, `POLL` and `BIND`, in
[`methods.md`](0101-notification-objects/methods.md) verbatim as ruled.

### Rights

The existing rights model, with no new bits: `READ` for `WAIT` and `POLL`, `WRITE` for `SIGNAL`
and `BIND`, `GRANT` to delegate. The table is in [`methods.md`](0101-notification-objects/methods.md#rights).

### The binding mechanism

This is the part that makes it more than a counting semaphore. When a notification is bound to a
TCB:

1. If the TCB is runnable or blocked on the notification itself, `SIGNAL` behaves as on an
   unbound notification: wake the waiter or count the signal.

2. If the TCB is blocked in `RECV` on an endpoint, `SIGNAL` wakes the TCB from that endpoint's
   wait queue instead. The `RECV` returns with a distinguished return value indicating "this was a
   notification, not an IPC message," and the notification word is available in the return
   registers (the badge).

3. If the TCB is blocked anywhere else (in `SEND`, in `CALL`, in `REAP`), the signal is counted
   in the notification word and delivered when the TCB next enters `RECV` on any endpoint or calls
   `WAIT` on the notification. This is the same "remember, don't lose" semantics the existing signal
   count has.

This means a thread that does `RECV` on its IPC endpoint can be woken by either:
- A synchronous IPC message (a sender rendezvoused), or
- An async notification signal (the bound notification was signaled)

and it can tell which by the return value.

### The return-value distinction

*Amended 2026-09-26: forgeable; a receiver tests `w4` (the amendment at the top).*

`RECV` today returns `w0` (the message's first word, or `1` for a signal). We extend the convention:

- `w0 = 0`: this was an IPC `SEND` rendezvous. `w1`, `w2` carry the sender's data words.
- `w0 = 1`: this was an IRQ signal (existing, unchanged).
- `w0 = 2`: this was a bound notification. `w1` carries the notification word (the accumulated
  badge). No `w2`.

This is additive - the existing `0` and `1` cases don't change - and it costs one new constant in
the ABI.

### What this does NOT add

- No new syscall. `SIGNAL`, `WAIT`, `POLL`, and `BIND` are methods under the existing
  `SYS_INVOKE` dispatch, the same way `SEND`, `RECV`, `REAP`, and `SURVEY` are. §4's "the syscall
  surface stays narrow" holds: `SYS_INVOKE` already exists, and this adds one new `objtype` and four
  new method constants under it.

- No badged capabilities (yet). seL4 badges the *capability* to the notification, so each
  signaler's copy ORs a different bit. This kernel has no badge field on capabilities (notes/abi.md,
  notes/supervision.md, notes/compositor.md all record this as a design fork). The notification word
  is passed as an argument to `SIGNAL`, so a signaler *can* identify itself by choosing a bit, but
  the kernel does not stamp it. This is the honest interim: a signaler can lie about who it is, same
  as a `SEND` can. Badged capabilities are the fork that retires the lie, and this decision does not
  take it; it records that the notification object is ready for the day it arrives.

- No timed wait. `WAIT` blocks indefinitely, same as `RECV`. A timed wait is milestone 106's
  fork, and a notification bound to a TCB does not give the TCB a deadline. What it does give is the
  ability to be woken by a *timer notification* - a user process that holds a clock capability and a
  thread can signal a notification when the timer fires, which is how seL4 userspace implements
  sleeps without a kernel timed-wait.

## What this replaces and what it doesn't

### The `terminal_sink_caretaker` narrowing (milestone 40)

The narrowing is still the right short-term move. It's a wiring change, not a kernel change, and it
unblocks the documentation viewer immediately. With notification objects, the shell could instead
bind a notification to its TCB, `RECV` on the child's output endpoint, and be woken by either the
child's `OP_EOF` (IPC) or a bound notification (exit, interrupt). That is the cleaner long-term
shape, but it requires the child's exit to signal the notification, which requires the kernel's
death-delivery path (§26) to signal notifications in addition to endpoints. That is a follow-on, not
part of this decision.

The narrowing remains valid as a permanent shape for "a tail stage whose output goes to the screen"
- seL4 systems use direct device access too. Notifications are the general primitive; the narrowing
is a specific wiring that doesn't need them.

### The existing signal mechanism on endpoints

The `signal()` method on `ipc::Endpoint` and the `bind_irq` / `irq_notify` path remain unchanged.
IRQ delivery to endpoints is built, proved, and working. The notification object is a *new* object
type for *user-initiated* async signals, not a replacement for the IRQ path.

Over time, IRQs may be rerouted to notifications (a driver would bind a notification to its TCB and
receive both IPC and IRQ signals on one wait point), but that is a migration, not a requirement, and
this decision does not mandate it.

### The shared supervision endpoint (§26.5)

§26.5 chose a shared endpoint with kernel-stamped identity to avoid needing wait-any. That decision
holds: the kernel is the only sender on the supervision path, so the tid is trustworthy without
badges, and a notification object does not change that. What it does change is the *alternative* a
supervisor could take: per-child fault endpoints, with a notification bound to the supervisor's TCB,
waking it from whichever child's endpoint delivers first. That is now possible but not required;
§26.5's choice remains valid.

## Surface cost

| Area | Cost |
|------|------|
| New `objtype` | `NOTIFICATION = 4` in `abi::objtype` |
| New methods | `SIGNAL`, `WAIT`, `POLL`, `BIND` in `abi::notification` |
| New ABI constant | One return-value tag (`w0 = 2` for notification delivery on a bound RECV) |
| New kernel object | A `Notification` struct (word + queue + bound TCB), page-resident, in the object registry |
| `RECV` dispatch | One new case: check whether the blocked TCB has a bound notification with a non-zero word, and if so, return it instead of blocking |
| Kani proofs | The notification's invariant: "the word is zero iff no signal is pending and no waiter is queued" (same shape as the endpoint's "at most one queue non-empty") |
| Syscall surface | **No new syscall.** All methods dispatch under the existing `SYS_INVOKE`. |

## What this unlocks (by consumer)

| Component | What it gets |
|-----------|-------------|
| **Shell** | Wait on child output (IPC) and child exit (notification) at the same time; no barrier required for `doc page.md` |
| **Compositor** | Per-client endpoints, woken by a bound notification for input/screenshot; unforgeable sender identity (when badges arrive) |
| **FS server** | One `RECV` woken by any client's message, with a notification for async events (disk full, queue drain) |
| **Network stack** | A timer process signals a notification; the stack's `RECV` on the socket endpoint is woken by either a packet or the timer, retiring the yield-and-re-poll |
| **Supervisor** | Per-child fault endpoints with a bound notification, if desired (§26.5's shared endpoint remains valid) |
| **Pager** | `doc <page>` rendering with pager: a keypress signals a notification, the viewer's `RECV` on the terminal endpoint is woken by either the next input byte or the interrupt |
| **Milestone 106** | A timed wait becomes "a timer process signals my bound notification," which is how seL4 userspace does it without a kernel `sleep` |

## Sequencing

1. Now: take the `terminal_sink_caretaker` narrowing (milestone 40, no kernel change).
2. Notification object: new `objtype`, four methods, the binding mechanism, Kani proof. This is
   a kernel milestone - one lane, estimated from the existing `Endpoint` and `signal()` work at the
   same scale as 19a (which added `RETYPE_OBJ` and the endpoint object). Minted as milestone 151
   (2026-08-22), forced by a concrete bug rather than scheduled speculatively: §106's
   `terminal_sink_caretaker` narrowing can race its own completion signal, and this is that fix's
   tracked home.
3. Retrofit: the shell binds a notification to its TCB and replaces the "wait for exit" hack
   with a proper multiplexed wait. The compositor can take per-client endpoints. The network stack
   can stop yield-spinning.
4. Badge fork (separate decision): badged capabilities, so notification identity is
   kernel-stamped rather than signaler-supplied. This is the fork notes/supervision.md,
   notes/compositor.md, and notes/dir-capability.md already record.

## Why not the alternatives

Moved to [`alternatives.md`](0101-notification-objects/alternatives.md): wait-any on endpoints,
non-blocking `RECV`, a pull-based source, and a buffering stage.

## The honest caveats

- Binding is the hard part. The `RECV` dispatch change - "check whether the blocked TCB has a
  bound notification with a non-zero word" - touches the IPC fastpath, which is the most
  performance-sensitive code in the kernel (milestone 132, notes/l4-lessons.md rows 11/13/14). The
  check is one load and one compare on a field the TCB already carries, but it is on the path that
  `ipc_rtt_el0` measures, and the cost must be measured rather than assumed. If the check is
  non-zero (the common case is no notification bound), it is one branch not taken.

- The badge is not kernel-stamped. A signaler chooses the word it ORs in, and nothing prevents
  a signaler from lying. seL4 stamps the badge at the capability level, so a signaler cannot
  impersonate another. This kernel has no badge field on capabilities, and adding one is a separate
  fork (badged endpoints/notifications) that touches every capability in the tree. This decision
  takes the honest interim: the notification word is signaler-supplied, same as a `SEND`'s data
  words are signaler-supplied. Badges are the fork that retires the lie, and this design is ready
  for them.

- One notification per TCB. seL4 allows one bound notification per TCB, and so does this design.
  A thread that needs to wait on multiple async sources binds one notification and has each source
  signal a different bit. This is sufficient for every consumer identified above, and it keeps the
  TCB's binding field to one `Option<Tid>`.

- The notification object is not a substitute for milestone 106 (timed wait). A timed wait is a
  kernel primitive that blocks for a deadline. A notification object lets a *userspace timer
  process* wake a thread at a deadline, which is how seL4 does it, but it requires that timer
  process to exist and to hold a clock capability. If the kernel itself needs a timed wait (for a
  watchdog, a scheduling deadline, or a retransmit timer in-kernel), milestone 106 is still owed.
  This decision does not retire it; it gives userspace a way to build the same thing without waiting
  for the kernel to.

## What this does not decide

- Badged capabilities. Recorded as a separate fork in notes/supervision.md,
  notes/compositor.md, notes/dir-capability.md, and notes/grant-expression.md. The notification
  object is compatible with badges and ready for them; this decision does not add them.
- The `terminal_sink_caretaker` narrowing. That is milestone 40's fork, and this decision does
  not take it. The two are independent: the narrowing is a wiring change that works today; the
  notification object is a kernel change that replaces the class of workaround.
- Direct process switch, §96 (process kernel or event kernel). The L4 lessons audit found this
  kernel has no direct process switch (rows 11/13/14). That is a separate decision about the
  scheduling model, and the notification object does not depend on it or change it.
- IRQ migration. Deferred driver by driver; see the second amendment.

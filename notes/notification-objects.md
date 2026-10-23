# Notification objects

*(Written 2026-09-26 by the lane for milestone 151 (notification objects), which builds DECISIONS §101 (notification objects). The note's name is
provisional, like everything a lane mints. It records what was built, the semantics of each method,
and the one place §101's specification could not be built as written.)*

§101 decided the design on 2026-08-20: a page-resident object holding a data word and a wait queue,
with four methods under the existing `SYS_INVOKE` (`SIGNAL`, `WAIT`, `POLL`, `BIND`). A TCB binding
lets a thread blocked receiving on an endpoint wake on either a message or a signal. This note
is the implementation record and the one proposal the build produced.

## Decided: how a woken receiver tells a notification from a message

calef ruled option B on 2026-09-26, the day this was raised: the kernel writes
`abi::notification::BOUND` (2) into `w4` on every receive a bound notification ends (`RECEIVE`,
`RECEIVE_CAP`, `Irq::WAIT`), keeping §101's `w0 = 2` and the word in `w1`. A maintainer records it as
an amendment to §101. What follows is the proposal as it reached him, kept because how the decision
was reached is part of the record. Its measured cost on the call/reply path is in
`design/roadmap/0151-notification-objects.md`: about 34 bytes of `ipc_receive_cap` on riscv64 and 54 on
`x86_64`.

### The premise §101 rests on is false

§101's "return-value distinction" says:

> `RECEIVE` today returns `w0` (the message's first word, or `1` for a signal). We extend the
> convention: `w0 = 0`: this was an IPC `SEND` rendezvous. `w0 = 1`: this was an IRQ signal.
> `w0 = 2`: this was a bound notification.

`RECEIVE` does not return a tag in `w0`. It returns the sender's own first data word
(`kernel/src/syscall.rs`, the `RECEIVE` arm: `Ok(msg[0] as i64)`, where `msg[0]` is whatever the sender
passed as `a0` to `SEND`). So `w0 = 0` does not mean "a message arrived"; it means a sender chose to
send 0. Any holder of a `WRITE` capability to the endpoint can `SEND(2, word, 0)` and be
indistinguishable from the bound notification under §101's encoding. The existing `w0 = 1` for an
IRQ signal has the same weakness today, and is harmless only because no ordinary sender shares an
interrupt's endpoint. A notification's whole purpose is to share a wait point with ordinary
senders, so here the forgery is the common case rather than an edge.

That is a correctness defect in the decided encoding, not a taste question, so it cannot be built as
written.

### What in a receive's result a sender cannot write

One grep answers it. A `SEND` carries three words (`w0..w2`). `RECEIVE` returns five registers
(`x0..x4` on aarch64, `a0..a4` on riscv64, `rdi, rsi, rdx, r10, r8` on `x86_64`). The top two
are written only by the kernel: `0` for every ordinary message, and the fault address and a reserved
`0` for a §26 (the fault endpoint) death message. `RECEIVE_CAP` returns three, and its middle one (the delivered slot, or
`NO_CAP`) is also kernel-written. So an unforgeable discriminator is available without widening
anything; the question is which register and which value.

### The options

| | shape | forgeable? | fastpath cost | cost to receivers |
|---|---|---|---|---|
| A | §101 as written: `w0 = 2`, word in `w1` | yes, by any sender | none | none; and wrong |
| B | `w0 = 2` kept, word in `w1`, and `w4 = 2` on every receive (`RECEIVE`, `RECEIVE_CAP`, `Irq::WAIT`). A receiver tests `w4` | no | `RECEIVE`: none (it already writes `w4` from the mailbox). `RECEIVE_CAP`: one store to `x4`. `Irq::WAIT`: two stores, off the fastpath | one rule for all three receives |
| C | per method, whichever register is already kernel-written: `RECEIVE` tests `w4`; `RECEIVE_CAP` returns a sentinel slot (`u64::MAX - 1`) in `x1` with the word in `x2`; `Irq::WAIT` needs nothing because it has no senders | no | none on any path | three rules, one per receive |
| D | a new receive method, used only by bound threads | no | none | a new method number, outside §101 |
| E | a new `Error` variant meaning "you were notified" | yes: a sender can already send a negative `w0`, which a receiver decodes as an error (a separate, pre-existing wart; see BUGS below) | none | none; and wrong |

Built, and then ruled: B. It is the one rule a reader has to remember, and its only cost on
either measured IPC shape is one store on the `CALL`/`RECEIVE_CAP` path. That number is in the block's
measurement section, not asserted here. C is the zero-cost alternative and would be the right call if
calef weighs one store on the call/reply shape over one rule; it is a small change from B (the
`RECEIVE_CAP` arm and its wrapper). A and E are wrong. D spends a method number to avoid a
register convention, which is the larger irreversible change for the same result.

What was not decided here, and could not be. The question §92 (a caretaker is supervised by the client it serves) asks, put to B against C: *would I
still choose this if both cost the same?* Yes; B is chosen for having one rule, not for being less
work (it is slightly more). The remaining disagreement is performance against uniformity, which is
an architect's trade.

### Prior art

seL4 (read from the seL4 reference manual, "Notifications" and "Binding Notifications", recalled
rather than re-read today): a bound notification delivered to a thread in `seL4_Recv` arrives as the
badge register holding the notification word. The receiver tells the two apart because badges
are kernel-stamped on the capability, and a system allocates endpoint badges and notification bits
disjointly. This kernel has no badges (§101's own "no badged capabilities (yet)"), so the seL4
answer is not available, and the kernel-written register is the nearest equivalent: something the
sender did not choose.

### What would have happened if calef had said no

If he had picked C, the kernel's `RECEIVE_CAP` arm and `ipc_receive_cap` would have changed, and so would
`user_mode_runtime`'s wrapper; nothing else would have, since no program outside this milestone's
tests was written against either. D would have moved the binding delivery behind a new method and
left `RECEIVE` exactly as it was.

## The semantics, and the choices §101 left open

§101 specified the four methods; `abi::notification`'s doc comments are the contract. Four choices
were not in §101 and were made in the build, each for a stated reason:

- The binding wakes every receive, not only `RECEIVE`. A thread parked as a receiver on an endpoint
  is in `RECEIVE`, `RECEIVE_CAP` or `Irq::WAIT`, and the kernel parks all three the same way. One rule covers
  "blocked receiving on an endpoint", which is §101's own phrase. It is also the rule milestone 106
  (a wait that ends on either the interrupt or the deadline) needs. `net_stack` blocks in
  `Irq::WAIT`, and the case in §147 (a timer a userspace service cannot hold) is a timer ending that
  wait.
- A zero-bit `SIGNAL` does nothing. So `WAIT` never returns zero, and zero from `POLL` means
  nothing was pending.
- `BIND` delivers a pending word at once if the thread is already blocked receiving. Otherwise a
  word counted before the bind would wait for the thread's next receive, which for a server blocked
  forever is never.
- A stale binding does not count as a binding. Either side destroyed frees the other to bind
  again, because every reader resolves the generational name and treats a miss as unbound.

What is proved and what is argued: the state machine (no bit lost, the order of rule 1 before rule 2,
the invariant) is proved by Kani in `crates/inter_process_communication/src/notification.rs`. The one
input the proof takes on trust, "the bound thread is parked in a receive right now", is argued at
`sched::bound_receiver`, and `sched::deliver_bound` asserts it in every debug build.

### Two checks the maintainer asked for (2026-09-26)

Can `w4` already be 2 on a §26 death message? No. There is exactly one place the kernel builds
a fault or exit message, `depart` in `kernel/src/sched.rs`: `let msg = [event, current, pc, addr,
0];`. `w4` is the literal `0` on every death, fault or exit alike, and `abi::fault`'s own table calls
it "reserved 0 today". Ordinary messages get `0` there too (`wide()` pads a three-word send, and the
`CALL` rendezvous writes `[w0, slot, w1, 0, 0]`). So B's tag collides with nothing that exists. It
does constrain the future: §26.4's fault-reply protocol, which `abi::fault` says "arrives here
additively", must not put `2` in `w4` of a death message, or it must move the tag. That constraint
is written beside the constant (`abi::notification::BOUND`) so the author of §26.4 meets it.

Is `SEND(1, ...)` a forged IRQ signal today? Not reachable today, and nothing stops it
becoming reachable. An IRQ signal is `[1, 0, 0, 0, 0]` in the receiver's mailbox, the same `w0`
a sender controls, so the encoding is forgeable in principle. It is not forgeable in practice,
because every endpoint the kernel routes an interrupt to (`bind_irq`, thirteen wiring call sites
today, the soak's loop beside them) is created for that purpose and never handed out as a
`Rendezvous` capability. Fifteen was written here at birth; the 2026-09-29 documentation audit
re-derived the count. The driver reaches it
only through `Irq::WAIT`, and an `Irq` capability has no send method. A grep for any
`rendezvous_cap(` naming an interrupt endpoint finds none. Two caveats keep this from being a
guarantee. First, it holds by wiring discipline, which is rung zero: nothing in the type system or
in `script/lint` refuses a future service that binds an interrupt to an endpoint it also grants
with `WRITE`. Second, `kernel/src/soak.rs`'s `bind_tick_routes` binds the timer tick to endpoints
its caller supplies, in a `--features soak_test` build only; those are the soak's own endpoints and
carry no senders today. Recorded as a `BUGS` entry in the block rather than fixed here, because the
fix (make an interrupt endpoint unsendable, or move IRQ delivery onto notifications) is §101's
"IRQ migration", which §101 explicitly left undecided.

Closed on 2026-09-26. calef ruled the first fix (§101's second amendment), and milestone 603 (an
interrupt's endpoint refuses every send) built it: `bind_irq` marks the endpoint, and a `SEND`,
`SEND_CAP` or `CALL` to it answers `NotPermitted`. Moving interrupts onto notifications is
decided driver by driver.

## The kernel half (`kernel/src/sched/notification.rs`)

### Who counts as a bound receiver

The crate's `signal` takes one boolean on trust: whether the bound thread is parked in a receive.
`bound_receiver` decides it, as `Blocked` on a rendezvous as a `Receiver` with nothing delivered yet.
The last clause matters. A receiver a sender has already served stays `Blocked` with its `wait_on`
intact until the wake completes, which can be a whole context switch later (a wake deferred behind
`on_cpu` finishes in `finish_switch`). Its mailbox is full, and a notification delivered there would
overwrite the message.

Every way off a receiver queue marks the thread delivered or aborted in the same critical section: a
sender's `serve`, an interrupt signal's `serve`, a drain's `abort`, a bound delivery's `serve`. The
one exception is `finish_blocked_resident`, which ends the thread. So the test is exactly "still
linked on that receiver queue", and `deliver_bound` asserts it. `RECEIVE`, `RECEIVE_CAP` and
`Irq::WAIT` all park this way, so the binding wakes all three.

`deliver_bound` takes the thread off its rendezvous's receiver queue with `remove_receiver`, a drain
and repush, O(receivers queued). That is the cost of reaching into another object's singly linked
FIFO. A server's endpoint usually holds one receiver, itself.

### One signal path

`signal_locked` serves a thread's `SIGNAL` and a kernel-originated signal alike. They differ only in
where the woken thread is placed. A thread's signal wakes locally, because the pair is often a
conversation and the signaling core is warm. A signal from interrupt context (the timer expiry walk)
wakes load-aware (§28 step 2), because an interrupt carries no locality. Everything in it is legal in
interrupt context for `irq_notify`'s reason: `IPC_TABLES` masks interrupts while held, a wake is an
enqueue rather than a switch, and nothing allocates. The timer expiry walk calls it directly, under
one hold, for several timers. `signal_notification_from_interrupt` is the one-at-a-time entry,
exercised by a kernel test and otherwise uncalled.

### Binding, and a word already waiting

A word waiting at bind time is delivered at once if the thread is already receiving. Otherwise a
signal counted before the bind would wait for the thread's next receive, which for a server blocked
forever in `RECEIVE` never comes. On entry to a later receive, `take_bound_signal` hands a bound
thread a word counted while it was elsewhere. It is `#[cold]` and out of line. The receive fastpath
tests `bound_notification.is_some()` on the thread it holds, §101's "one load and one compare", so
only a bound thread pays for the body.

`MAX_NOTIFICATIONS` is 256, half of `MAX_RENDEZVOUS`, because a notification is a per-thread or
per-service doorbell where a rendezvous is per conversation. The pages come from the creators' own
regions, so it bounds only the 16-byte-a-slot name table. Raise it when a real workload refuses a
`RETYPE_OBJ` here.

## BUGS

- The notification and timer registries are machine-wide (`MAX_NOTIFICATIONS` and `MAX_TIMERS`,
  256 each, `kernel/src/sched.rs`), and a full registry answers every creator with one flat
  `OutOfMemory`. A victim cannot tell a full registry from its own empty region. One domain
  holding 256 pages of untyped can deny notification and timer creation to every other domain
  for the rest of the boot. This copies the rendezvous registry's shape (`MAX_RENDEZVOUS`, 512,
  which one test suite actually filled), and the recorded stance is to raise the bound when a
  real workload refuses, not before. Recorded by the 2026-09-29 security audit, which weighed a
  per-domain quota and left it unproposed: nothing multi-domain has asked for the objects yet.

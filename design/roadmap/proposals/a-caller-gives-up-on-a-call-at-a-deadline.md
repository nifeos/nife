---
status: PROPOSED
raised: 2026-11-11
milestone_dependencies: 371
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# A caller gives up on a call at a deadline

Raised by lane/timed-call-proposal. On 2026-11-11 (UTC) the architect accepted, on #3499, that the board's PMIC route bounds its own wait because `reboot` cannot bound its call. The same ruling named a timed call as its own milestone. This file is that milestone, written as a question, because the syscall surface is the architect's call.

Reuse: the `Timer` object and the bound notification, both already built; `remove_sender` in `crates/inter_process_communication`; milestone 371 (a reply capability that names a call). No new object, syscall number or dependency.

Name: provisional. Every new name below is the lane's, not a ruling.

## Background: how a call works today

A client makes a call with `Rendezvous::CALL` (`crates/abi/src/rendezvous.rs`, `CALL`). It sends two words and blocks until a reply arrives. The kernel mints a one-shot `Reply` capability that names the caller. The server receives it with `RECEIVE_CAPABILITY` and answers once with `reply::REPLY`. §12 (Call/Reply IPC: a one-shot reply capability) is the design.

The caller waits in one of two phases. First it may sit on the rendezvous's sender queue, because no server has taken the request yet. Then it is reply-parked: the server holds the `Reply`, and the caller sits on no queue at all. `ipc_call_badged` in `kernel/src/sched/rendezvous.rs` parks it with `WaitRole::Reply` and says that only the reply, or an abort, may wake it. Its `BUGS` say it plainly: a live server that never replies blocks its caller forever.

Time enters through two other objects. A `Timer` (`crates/abi/src/timer.rs`, `ARM`) signals a notification when the counter reaches an absolute deadline. §147 (a timer a userspace service cannot hold) chose that object on 2026-09-05 (UTC), and the architect refused a deadline argument on `RECEIVE` or `CALL` as a way to save effort. A notification bound to a thread ends that thread's `RECEIVE` (§101 (notification objects)). `bound_receiver` in `kernel/src/sched/notification.rs` wakes a bound thread only when it waits as a `WaitRole::Receiver`.

So a client cannot bound a call. §101's rule 3 says a signal that arriving while the thread waits in `CALL` stays in the word until its next receive. A timer the client armed fires, the bit lands in the word, and the client stays parked.

## Who would use it

- `reboot` calls the PMIC route, and the route carries the timeout instead (the motivating case).
- The swapper's operator bounds a quiesce with a stand-in process that makes the call for it (`quiesce_within` in `components/src/swapper.rs`, `ROLE_QUIESCER` in `crates/hot_swap_contract`).
- `terminal_supervisor` does the same with its own stand-in (`stand_in` in `components/src/terminal_supervisor.rs`). notes/hung-component.md explains why both exist.
- Tests hang rather than fail when a server never answers. Milestone 402 (a service report nobody is obliged to drain) measured one such hang, an NTP test blocked in a call to the entropy server.
- Every client of a userspace server. notes/scheduler/rendezvous.md lists the hang in its `BUGS`.

## The recommendation

Option A: a new `Rendezvous` method, `CALL_BOUND`, which is `CALL` that the caller's bound notification can end in either phase. The caller arms its `Timer` onto that notification and calls. The call returns the reply, or the notification's word with a tag in `x4`, as a bound receive already does. No syscall number changes, and plain `CALL` keeps §101's rule 3 unchanged.

It is the composition §147 chose. The timer supplies the deadline and the notification is the wake source, so a call also ends on anything else that signals that word, such as a `^C` or a supervisor's cancel. The question at the end asks which option to build.

### What `CALL_BOUND` returns

- A reply: `x0` and `x1` are the reply's words and `x2` the carried slot or `NO_CAPABILITY`, as `CALL` returns them. `x4` is 0.
- The notification ended it: `x0` is `notification::BOUND`, `x1` the word, `x2` `NO_CAPABILITY`, and `x4` `BOUND` again, the register to test. `x3` says how far the request got. `CALL_WITHDRAWN` means no server took it, so a retry is safe. `CALL_ABANDONED` means a server took it and may have acted on it, and the kernel drops its reply.
- A word already pending at entry ends the call at once, withdrawn, and the kernel sends nothing.
- The kernel refuses a caller with no bound notification before it sends anything, so the method never blocks without a bound.

### What happens on each side

In the sender phase the kernel unlinks the caller with `remove_sender` and drops the `Reply` it carried. The server never sees the request.

In the reply phase the server keeps its `Reply`. The danger is that a stale `Reply` answers the caller's next call. Today one sweep prevents that: `strand_reply_caller` deletes every `Reply` naming the thread before it wakes it. That is safe on the teardown paths it serves now, but not as a routine event. The sweep empties the server's slot, and the server's next `RECEIVE_CAPABILITY` can file a new caller's `Reply` in that free slot. A server that parks replies, as `line_editor` does with its `pending` field, would then answer the wrong caller from the slot number it kept. Milestone 371 (a reply capability that names a call) removes the hazard: the stale `Reply` stays in its slot, names a call that is over, and the reply guard refuses it. So this milestone depends on 371.

Nothing tells the server that its caller left. The kernel drops its late `REPLY`, which returns 0, as a reply to a caller that is no longer waiting already does. `REPLY_CAPABILITY` already returns 1 in that case. A later opt-in can tell the server, as QNX's unblock pulse does, without changing this method.

### A caller's code

`reboot` once the spawn service grants it a `Timer` and a notification bound to its thread. The names are provisional.

```rust
/// How long `reboot` waits for the route to answer: the route's own two seconds, and one more.
const ROUTE_CALL_BOUND_SECONDS: u64 = 3;

let deadline = now() + abi::timer::counter_ticks_for(ROUTE_CALL_BOUND_SECONDS, 0, cntfrq());
if timer_arm(TIMER_SLOT, deadline, NOTIFICATION_SLOT, DEADLINE_BIT) < 0 {
    complain(b"reboot: the timer refused the deadline; asking the kernel\n");
} else {
    match call_bound(ROUTE_SLOT, route::request(route::RESET), 0) {
        CallOutcome::Replied(status, word) => {
            let _ = timer_cancel(TIMER_SLOT);
            report_route_answer(status, word); // as today
        }
        CallOutcome::Abandoned(_) => {
            complain(b"reboot: the power controller took the request and did not answer in three seconds; asking the kernel\n");
        }
        CallOutcome::Withdrawn(_) => {
            complain(b"reboot: the power controller never took the request; asking the kernel\n");
        }
    }
}
reboot_object_invoke(REBOOT_SLOT); // the kernel's REBOOT, as today
```

## The options

Each option keeps plain `CALL` and its round trip unchanged. Every new path is out of line, as `take_bound_signal` is, so `script/fastpath-footprint` and `script/icount` must show no change. Nobody has measured the costs below yet, because nothing exists to measure; the building lane measures them.

### A. `CALL_BOUND`: the bound notification ends the call (recommended)

- Cost: a per-thread flag that marks the call as one the notification may end, and two new cases in `bound_receiver`. The sender phase reuses `remove_sender`. The reply phase needs milestone 371. A caller needs a `Timer`, a notification and a binding, which `retype_sleeper` in `user_mode_runtime` already makes from a memory region.
- ABI: method 8 on `Rendezvous` and two `x3` values, with no new syscall number. No existing encoding changes, so by the precedent of the 2026-11-11 rename that kept method 2, `ABI_VERSION` stays 1. The tree has no ruled rule for an added method, and the version check is by equality, so a bump would refuse every program built before it.
- A late reply: dropped by 371's guard. The caller learns from `x3` whether the server ever took the request.

### B. A deadline argument: `CALL_BEFORE(w0, w1, deadline)`

`CALL` leaves its third argument unused, so a new method could carry an absolute deadline there. The kernel would keep the deadline on the thread and wake it at the tick.

- For: a caller needs no `Timer` and no binding, so `reboot` would need no new capability.
- Against: §147 refused this shape on 2026-09-05. It has one wake source, the deadline, so a `^C` cannot end the call. Each blocked caller holds a kernel timer it did not pay a page for, where a `Timer` comes from the caller's own memory region.
- ABI: a new method, as in A. Putting the deadline in `CALL` itself would change `CALL`'s encoding, a bump to `ABI_VERSION` 2 and a rebuild of every program.
- A late reply and the server side: as in A, and it needs 371 for the same reason.

### C. A timeout for the next blocking call, as QNX's `TimerTimeout` sets

A new `Timer` method would arm a deadline that ends whatever the thread blocks in next.

- Against: it is ambient per-thread state, which §10 (the capability-based microkernel process model) refuses, and it ends the wrong wait if the program blocks somewhere else first. QNX itself calls the result undefined when the next call cannot block.
- ABI: a new `Timer` method, and a rule that every blocking method must honor.

### D. No kernel change: the runtime packages the stand-in

`user_mode_runtime` would gain the swapper's pattern: start a small process to make the call, and wait with a deadline on its forwarded answer.

- Cost: a process, a region and an image per bounded call. The stand-in stays blocked until the operator destroys its region, which is a teardown, not a give-up. `reboot` holds no construction budget, and a library cannot assume one.
- ABI: none. That is its whole merit, and the two copies in the tree show its price.

### E. Bounds in servers only

This keeps the status quo of #3499: each server bounds its own work. It covers a slow server that still answers. It does not cover a hung server, the case the bound exists for.

## Prior art, read 2026-11-11 (UTC) unless marked

- seL4 has no timed call. A bound notification wakes a thread only in `BlockedOnReceive`, so a thread inside `seL4_Call` stays parked (`sendSignal` in `src/object/notification.c`). The MCS kernel adds timeout faults. A thread's scheduling-context budget runs out, and a fault goes to that thread's timeout endpoint. That bounds execution time, not a client's wait. nife differs under A: the bound notification ends the reply wait too, and only when the caller asked for it.
- Fuchsia's `zx_channel_call_etc` takes an absolute monotonic deadline and fails with `ZX_ERR_TIMED_OUT`. Its notes warn that a late reply could match another outbound request after about 2^31 calls, and that a timeout should end use of the channel. That is option B with a transaction id; nife's 371 makes the late reply impossible to match, not unlikely.
- QNX Neutrino's `TimerTimeout` arms a one-shot timeout for the next kernel call, with separate bits for the send and reply states, and the call fails with `ETIMEDOUT`. A server whose channel asks for the unblock flag at creation gets a pulse instead, and the client stays reply-blocked until that server replies. That is option C, plus the server opt-in that A defers.
- Original L4 gave each IPC four timeouts, encoded as floating point, for the send phase, the receive phase and two page-fault phases. Elphinstone and Heiser ("From L3 to seL4", SOSP 2013, section 3.2.4) record why they went. No theory chose the values, practice used only zero and infinity, watchdogs detected hangs better, and userland got a timer instead. Their tag is "Abandoned: IPC timeouts in seL4, OKL4." Option A follows that lesson: the timer is the user's object, and the caller chooses its bound from what it knows.
- L4Re (Fiasco.OC) keeps finite receive timeouts and documents the stale-reply hazard: a partner may still hold the reply capability and answer later. notes/blocked-thread-teardown.md quotes it.
- Mach's `mach_msg` takes a timeout in milliseconds for the send and the receive, returning `MACH_SEND_TIMED_OUT` or `MACH_RCV_TIMED_OUT`. An RPC is a send then a receive in one call, so each phase has its own bound, and a timed-out send returns the message to the caller. nife's `x3` gives the same two answers, withdrawn or taken.

## The seven questions

1. Considered and refused: B, for §147's reason and its single wake source; C, as ambient state; D, for its cost per call and its teardown in place of a give-up; E, because it misses the hung server.
2. The tree already ends a bound receive with a notification and tags it in `x4`. A copies that shape onto a call.
3. Prior art is above. The lane read each claim from its source today, and quotes the L4 paper's verdict.
4. The premise holds. `bound_receiver` matches only `WaitRole::Receiver`, and §101's rule 3 counts a signal during `CALL`.
5. Nobody has measured the costs, because nothing exists yet. 371's sweep, which A avoids, is 128 by 16 comparisons per caller freed.
6. A new method is permanent once programs call it. Nife has no installs and no outside programs, so before the first release it is a two-way door.
7. Yes. B is no harder to build than A, and A wins on composition and authority, not effort.

## What I need from you

A client cannot give up on a call today, because a signal during a call waits for the next receive. Option A adds a call that the caller's bound notification ends, so a `Timer` bounds it. B puts a deadline in the call itself, which §147 refused. C sets a timeout for the next blocking call, and D keeps the stand-in process. Every option except D leaves the syscall numbers unchanged.

Should nife add `Rendezvous::CALL_BOUND`, a call that the caller's bound notification ends in either phase, built after milestone 371? Recommended: yes (A). The alternatives are B, a deadline argument; C, a timeout on the next blocking call; or D, no kernel change and a runtime stand-in. If the answer is no, `reboot` keeps the route's own bound and the two stand-ins stay.

## What drives it

`reboot`'s bound on its call to the PMIC route waits on this, and the two stand-ins and the hanging tests would use it on the day it lands.

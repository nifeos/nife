# Ending a permanently blocked thread

*This note starts where notes/hung-component.md stopped: its case (c), a thread `Blocked` on an
endpoint its supervisor cannot reach, which nothing in this kernel could end. Read that note first
for the taxonomy and for why the supervisor's whole vocabulary is refused. This one surveys how
seven other systems solve the same problem, maps each onto this kernel, and lays four proposals side
by side. The survey and the proposals in full are in
[`blocked-thread-teardown/`](blocked-thread-teardown/README.md).*

The fork is answered. calef chose proposal A on 2026-09-03, and milestone 133 (ending) built it
(design/roadmap/0133-blocked-thread-teardown.md). The argument is kept as it was written, because
it is what the decision was made against.

What changed against the text. The mechanism landed in `sched::finish_blocked_resident` and
`RegionReap::FinishInPlace`. `Endpoint` is spelled `Rendezvous` in this tree, and `remove_receiver`
is now written. And the claim below that `WaitRole` says which queue holds a thread is wrong, which
the implementation found and this note did not. `ipc_call`'s `Send::Blocked` arm records a caller as
`WaitRole::Reply` while queueing it as a sender. So role and queue disagree on every call that meets
no server, and the shipped code asks both queues rather than believing the role.

Proposal C's first half shipped as milestone 254 (caller) on 2026-09-04, on calef's ruling that it
is a defect rather than a fork, because it asks for no authority anybody does not hold. A caller
reply-parked on a server that stops being able to answer now returns `abi::Error::Gone`. The stale
reply capability this note identified as the blocking hazard is deleted before the wake. B, C's
second piece and D are not built. *(Updated 2026-10-11 (UTC): this paragraph said the fork was
untouched and milestone 133's, which stopped being true when A was chosen and built.)*

*The code this note reads: `kernel/src/sched.rs` (`schedule`, `ipc_call`, `ipc_reply`,
`set_ipc_aborted`, `reap_region_objects`), `crates/inter_process_communication/src/lib.rs` (`drain_waiters`,
`remove_sender`), `crates/thread_wake_handshake/src/lib.rs` (`park`, `abort`, `try_wake`), and
`kernel/src/thread.rs`'s `WaitRole`.*

## The problem in one paragraph

`Untyped::DESTROY` on a region holding a live thread does not fail passively: it marks the thread
`killed` and refuses, so the owner's retry reclaims a runaway (§16's amendment, §24's forcible `^C`).
The kill is spent at the top of `schedule()`, and only for a thread whose state is `Running`:

```rust
if let Some(t) = sched.threads.get_mut(current)
    && t.killed
    && t.handshake.state == State::Running
{
    t.handshake.state = State::Finished;
}
```

A permanently `Blocked` thread never becomes `current` again, so the kill is armed and never lands
and the refusal is permanent. The region is unreclaimable for the life of the machine, and so is the
region of every caller that thread stranded. No privilege fixes this. It is a scheduler property,
not an authorization one, which is why §32 (supervisor)'s "stronger right" is not merely large for the purpose
but insufficient.

For a backup target meant to run unattended for months, the shape of the risk is that the number of
hangs the system survives is a function of spare budget.

## What the machine already has, which reframes the question

The most useful finding in this lane is not from the prior art. It is that the mechanism is nearly
free and the authority is the whole problem, and that is the opposite of how the question reads
from outside. [`what-the-machine-already-has.md`](blocked-thread-teardown/what-the-machine-already-has.md)
has the reading behind each point:

- Every blocked thread is a sender on an endpoint queue, a receiver on one, or reply-parked on no
  queue at all, and `handshake.wait_on` says which. (The role-versus-queue error the opening
  paragraphs record is in this claim.)
- The abort-and-resume pair, `set_ipc_aborted` then `wake`, already exists and already hands
  userspace `abi::Error::Gone`. It is only ever entered from an endpoint's wait queue.
- The one missing primitive was `remove_receiver`, the twin of `Endpoint::remove_sender`, about
  twelve lines. A reply-parked thread needs no unlink at all.
- Reaching outside the region is settled precedent: `reap_region_objects` already unlinks a corpse
  from its supervisor's endpoint, which lives in another region.

So: the kernel could end any blocked thread today, in about thirty lines, with no new syscall.
Every proposal below differs in who may ask and in what the victim and its peers observe, not in
whether it can be done.

## A hazard any proposal must solve: the stale reply capability

This is a live correctness bug that no current code path can reach, and the first proposal that
wakes a reply-parked caller reaches it.

`cap::reply_cap` mints `Object::Reply(tid)`. The payload is a generational `ThreadId` and nothing else:
there is no call sequence number, no endpoint, no nonce. `generational_table` bumps a slot's generation when the
*thread* is removed, not per call.

`ipc_reply`'s guard, the one boot 8 added, checks the role and not the call:

```rust
if !matches!(t.handshake.wait_on, Some((_, WaitRole::Reply))) {
    return;
}
```

The `_` is the endpoint, and it is discarded. Today this is sound, and the reason is worth writing out
because it is what an abort would invalidate. There are three ways out of a reply park, and no
server holds an unconsumed `Reply(tid)` at the end of any of them:

- The reply arrives. `abi::reply::REPLY` calls `ipc_reply` and then `delete_current_cap(slot)`,
  "so a second reply is `NoSuchSlot`". The cap is gone whether or not the delivery landed.
- The caller is drained off a sender queue. A `CALL` that met no receiver leaves the caller
  *both* Reply-parked and queued as a sender, with the reply cap riding in `outgoing_cap` rather than
  in anyone's capability table, so `drain_waiters` reaches it and `Gone` does resolve. No server ever held
  that cap, because no server ever collected the message.
- The caller dies. The `generational_table` generation bumps and every `Reply(tid)` naming it goes stale on its
  next use.

The dangerous shape is the one no path produces: a caller that resumes from a `CALL` whose request
*was* collected, so a server holds the cap, and then makes a second call.

An abort creates that path. Free a caller stranded by a hung server, hand it `Gone`, let it call
a healthy server instead, and the hung server still holds a live `Reply(tid)`. Its next invocation
passes the role check, clobbers the caller's mailbox, and wakes it with a forged reply belonging to a
different conversation. A merely hung server never does this; a compromised or confused one does, and
the confinement claims in this tree are written against the second.

L4Re documents this exact hazard as a consequence of its own finite receive timeouts, which is the
strongest corroboration available that it is real rather than theoretical:

> Special care is required if a finite timeout for the receive phase of an IPC call is specified: The
> IPC receive operation could abort before the partner was able to send the reply message. Under
> certain circumstances the partner may still have the temporary reply capability to the calling
> thread and may use this capability to reply to the caller at a later, unexpected time specifying an
> arbitrary IPC label. This case is relevant for servers which call another, possibly untrusted,
> server while serving a client request.
>
> Source: [L4Re, *L4 Inter-Process Communication (IPC)*](https://l4re.org/doc/l4re_concepts_ipc.html), IPC
> Timeouts (fetched 2026-08-17)

Two fixes exist, and both have prior art below:

- Delete the outstanding reply capability at abort, sweeping every capability table for `Object::Reply(tid)`
  the way `sched::delete_frame_caps` sweeps for `Object::Frame(phys)`. This is seL4 non-MCS's
  `cteDeleteOne(callerCap)`, reached from `cancelIPC`. Cost is O(threads x 16 slots) on a teardown
  path, which is 2048 comparisons at `MAX_THREADS = 128` and `CAPABILITY_TABLE_SLOTS = 16`.
- Do not wake the caller at all, so no second call can exist. This is seL4's `ThreadState_Inactive`
  and is discussed under proposal A.


Zircon documents the same hazard as a consequence of `zx_channel_call` timing out, in its own
vocabulary; the quote is in [`prior-art.md`](blocked-thread-teardown/prior-art.md).

## Prior art

Seven systems were surveyed, every quote fetched and read on 2026-08-17, and
[`prior-art.md`](blocked-thread-teardown/prior-art.md) has each with its sources. seL4 has no abort
operation: suspension cancels the IPC and leaves the thread inactive, so no error code is ever
defined. Mach needed a safe abort because its threads block inside arbitrary kernel work; here every
block is at one of three well-defined points. Zircon shipped thread killing and removed it. Linux's
`TASK_KILLABLE` gives the transferable rule: an abort that only ever precedes teardown costs
userspace nothing, while one a program must survive costs every `CALL` site.

### The survey in one table

| System | Verb | Authority | What the victim sees | Does it reach a reply-parked caller? |
|---|---|---|---|---|
| seL4 | `seL4_TCB_Suspend` (cancel is a side effect) | a TCB capability | nothing: `ThreadState_Inactive` | yes, via the reply object's `replyTCB` back-link |
| Mach | `thread_abort` / `thread_abort_safely` | task/thread port | an interrupted-syscall return code | yes (message primitives) |
| L4Re | `ex_regs` with `..._CANCEL` (cancel is a flag) | a thread capability | `L4_IPC_SECANCELED` / `RECANCELED` | yes; also avoided by IPC timeouts |
| Zircon | `zx_task_kill`, refused for threads | `ZX_RIGHT_DESTROY` on a process/job handle | `ZX_ERR_PEER_CLOSED` / `ZX_ERR_CANCELED` from the channel | yes, by closing the channel, not by touching the thread |
| QNX | server death, implicitly | none: it is automatic | `ESRCH` from `MsgSend` | yes, and this is its headline behavior |
| Linux | fatal signal + `TASK_KILLABLE` | ambient, by pid | nothing (the process dies) | n/a |
| NT | user APC to an alertable wait | thread handle | `STATUS_USER_APC` | only if the wait opted in |

Nobody in this survey blocks forever with no way out. nife was alone in that when this table was
written, and alone in it by accident rather than by decision, which is what made it worth a fork
rather than a `BUGS` entry. Milestone 254 took the row's last column: a reply-parked caller here is
now reached, by the server's death and by the rendezvous's, which is QNX's route and Zircon's
together. The first column is still open, because nothing here can end a blocked thread.

## Mapping onto this kernel, and who may end a thread

[`mapping-and-authority.md`](blocked-thread-teardown/mapping-and-authority.md) has the full
analysis. Every proposal keeps the static proof from milestone 124 (a thread is born where it
lives), since each enters from a syscall on the destroyer's own stack. Every one that wakes a reply-parked caller must sweep its outstanding
`Reply(tid)` capabilities. And `killed` should end up with one meaning, not one that depends on the
victim's state.

The mechanism is small; the authorization question is not. The region capability adds no authority
at all, since `DESTROY` already commits to ending every resident and only cannot finish. The
`ThreadControlBlock` capability composes and is seL4's answer, but it is inert once its thread runs,
so a method on a running thread would turn a builder's tool into a lifetime handle. The supervision
endpoint is the most tempting and least available: §32 declined it, and milestone 126 (who else is running) ruled
that a domain names its members and does not act on them.

## Four proposals

Ordered from smallest surface to largest, and written unranked.
[`the-four-proposals.md`](blocked-thread-teardown/the-four-proposals.md) has each one's mechanism,
authority, cost, what it breaks and how it fails.

- A, *the completed reclaim*: `DESTROY` unlinks a blocked resident, sweeps its reply capabilities
  and writes it `Finished` without waking it. Authority is the region capability, unchanged. It
  does not free a stranded caller in another region. Chosen by calef on 2026-09-03 and built as
  milestone 133.
- B, a terminate verb on the `ThreadControlBlock` capability: seL4's `suspend()` transliterated. It
  separates stopping a thread from reclaiming its memory. That may be exactly right (it separates
  policy from memory) or exactly wrong (it produces a stopped thread nobody can free), and which one it
  is depends on a spawn-time endowment convention that does not exist yet.
- C, *the abortable call*: free the stranded caller, not the hung server. Piece 1 wakes a
  reply-parked caller with `Gone` when its server can no longer answer, and it shipped as milestone
  254. Piece 2, a deadline on `CALL`, is the fork of milestone 106 (a wait that ends on the interrupt
  or the deadline). C complements the others.
- D, accept the leak, bound it with the existing quotas and say so. Argued properly, because
  Zircon removed thread killing and a capacity failure is survivable. It leaves this kernel alone
  in the survey with no way out at all.

### Side by side

| | A: completed reclaim | B: `ThreadControlBlock` terminate | C: caller's side | D: accept and bound |
|---|---|---|---|---|
| **New syscall / method** | none | one method on `ThreadControlBlock` | none (piece 1) | none |
| **New right** | none | none | none | none |
| **New error visible to userspace** | none | none | `Gone` at a new place | none |
| **Authority** | the region cap, unchanged | a `ThreadControlBlock` cap, widened | unchanged | unchanged |
| **Reclaims the hung component's region** | yes | only with a region cap too | no | no |
| **Frees its stranded callers** | each by its own owner | no | **yes** | no |
| **Needs the reply-cap sweep** | yes | yes | yes | n/a |
| **Victim runs again** | no | no | yes, with `Gone` | n/a |
| **Overturns a recorded decision** | no | widens `ThreadControlBlock` semantics | no | no |
| **Rough size** | ~30 lines + `remove_receiver` | that plus ABI and a §-section | ~40 lines + sweep | docs and a program |

## EXAMPLES

See the kill that cannot land. The condition is one line, and the state it requires is the one a
blocked thread cannot reach:

```sh
grep -n -A6 'Forcible teardown, before anything else' kernel/src/sched.rs
```

See the abort machinery that already exists, and that it is only ever entered from a wait queue:

```sh
grep -n 'set_ipc_aborted' kernel/src/sched.rs
```

Every hit is either a stale-endpoint check at the top of an IPC primitive or the `drain_waiters`
sweep in `reap_region_objects`. Nothing addresses a thread by name.

See that a reply-parked caller is on no queue, which is why the sweep misses it:

```sh
grep -n -B4 -A10 'pub enum WaitRole' kernel/src/thread.rs
```

See the guard that checks the role and not the call, which is the hazard:

```sh
grep -n -A12 'pub fn ipc_reply' kernel/src/sched.rs
```

See the precedent for unlinking a thread from an endpoint outside the region being destroyed:

```sh
grep -n -B10 'remove_sender' kernel/src/sched.rs
```

Read the taxonomy this note starts from, which is `notes/hung-component.md` on `main`.

## BUGS

**Proposal A is built (milestone 133), proposal C's first piece is built (milestone 254), and B,
C's piece 2 and D are not.** The remaining proposals are sketches at the level of "which lines
change", not designs; B would still owe a `DECISIONS` section for a new method under §10's rule.
C's piece 1 was exempted by calef on 2026-09-04 for the reason its own block gives: it adds no
authority, no method and no error, so there is no semantics to record that §12's reply capability
did not already imply.

**The stale-reply-capability hazard was unreachable when this note was written. Milestone 254 made
it reachable and tested; milestone 133 is the one design here that does not reach it at all.** It
was established here by reading `cap::reply_cap`, the generational naming, and `ipc_reply`'s guard,
and by enumerating the exits from a reply park: an argument, not a proof. Two milestones then took
seL4's two fixes, one each and for different reasons. 254 wakes a stranded caller and so must delete
the outstanding capability before the wake (`cteDeleteOne(callerCap)` from `cancelIPC`), which two
tests in `kernel/src/sched.rs` assert. 133 never wakes its victim at all
(`ThreadState_Inactive`, so no second `CALL` can exist to be forged against) **and** sweeps anyway,
which `user::force_kill_tests::tearing_down_a_reply_parked_caller_sweeps_the_reply_capability`
pins.

What neither can test is the *negative* the argument rested on: nothing proves the sweep runs at
every future way a server can stop answering, and nothing states the guard's rule anywhere a checker
can read it. That is `design/roadmap/0371-a-reply-capability-that-names-a-call.md`, and it is
rung one where both sweeps are rung two.

**The claim that `WaitRole` enumerates every blocked thread depends on every block site calling
`park`, and milestone 133 made that claim load-bearing rather than diagnostic**
(`design/roadmap/0366-a-block-site-that-writes-blocked-by-hand.md`). `thread_wake_handshake`'s own `BUGS` says nothing enforces that: the fields are public because the
kernel has legitimate out-of-protocol writers, so a future block site writing `state = Blocked`
directly opts out silently and would leave `wait_on` stale. Proposals A, B and C all unlink using
`wait_on`, so all three inherit that as their sharpest failure mode. I did not audit every write of
`State::Blocked` in the tree to confirm the enumeration is currently complete; I read the three IPC
primitives and the `WaitRole` doc comment, which assert it.

**The prior-art survey is thin on two systems.** Mach is read from the XNU `osfmk` man pages, which
document the interface and not the implementation, so what `thread_abort_safely` does to a Mach port's
in-flight message is not established here. QNX is read from the architecture guide and the library
reference, both vendor documentation with no source available. So the claim that the kernel actively
unblocks a REPLY-blocked client (rather than the client discovering the death later) is the
documentation's word and not verified.

**No number in this note was measured.** The line counts ("about thirty lines") are estimates from
reading, the sweep cost is arithmetic from `MAX_THREADS` and `CAPABILITY_TABLE_SLOTS`, and no benchmark was run.

**No number in the survey table was re-checked at build time.** The prior art was fetched and read on
2026-08-17 and none of those systems was consulted again while milestone 133 was written.


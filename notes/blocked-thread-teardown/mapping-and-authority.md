# Mapping onto this kernel, and the authorization question

*Name: provisional.*

What each shape in the survey would break here, and which held capability could express the right
to end a blocked thread.
[`notes/blocked-thread-teardown.md`](../blocked-thread-teardown.md) keeps the conclusions.

## Milestone 124's static proof stays intact, for all four proposals

Milestone 124 (a thread is born where it lives) built the proof this section relies on.

`script/stack-depth-check` proves in CI, on both ISAs, that no context switch is reachable from the
interrupt-stack entry point, and `schedule()` carries the runtime half as a `debug_assert!`. No
proposal here delivers an abort from an interrupt context. Every one of them is entered from a
syscall on the *destroyer's* own thread stack, takes `SCHED`, mutates the victim's `Handshake` and
possibly one endpoint queue, and returns. The victim is touched as data, never switched to, and the
destroyer never calls `schedule()` from anywhere new.

The property to preserve if a proposal ever grows a timer (a deadline on `CALL`, milestone 106's
fork) is that firing a deadline from the timer IRQ must not switch there either. The existing
answer is already written: the interrupt path defers its switch to `preempt_if_needed`, one frame
outside the trampoline, back on the interrupted thread's own stack. A deadline would have to use the
same deferral, and that is a constraint to write into 106 rather than a reason to refuse it.

## The reply capability's semantics are the sharp edge

One-shot, `WRITE` without `GRANT`, minted at the rendezvous, consumed on use, living in the callee's
capability table. Three consequences:

- The operator cannot answer on the component's behalf, and this is by construction rather than
  by omission: the cap is minted without `GRANT` "so it could not have been delegated here in the
  first place". Nothing about freeing a stranded caller can route through delegating the reply.
- The cap carries a thread name, not a call name. See the parent's hazard section. Any proposal that
  wakes a reply-parked caller and lets it call again must either delete the outstanding `Reply(tid)`
  caps or refuse to let the caller run.
- Deleting them is a sweep of the whole capability table, and `sched::delete_frame_caps` is the existing pattern
  for exactly that. It is not free but it is a teardown path.

## §16's revocation model and the `killed` flag

§16 (object revocation) is the model `killed` belongs to.

`killed` today means "convert to a corpse at the next preemption", and its one enforcement point
requires `state == Running`. Two of the proposals ([`the-four-proposals.md`](the-four-proposals.md)) change what `killed` means for a `Blocked`
thread, and that is a semantic change to a flag that `Untyped::DESTROY`, §24's `^C` escalation and
`sched::kill_thread` all set. Whatever is decided, `killed` should end up with one meaning, not a
meaning that depends on the victim's state. The current pair (armed for a runaway, inert for a
sleeper) is exactly the kind of state-dependent rule this tree's ladder says to design out.

## Endpoint-only naming survives, but only if the authority is an existing capability

The rule is that nothing may acquire the ability to name a thread it was not handed. A `ThreadControlBlock`
capability *is* a thread name that was handed over, so it does not breach the rule. But note what it
would change: `Object::ThreadControlBlock` is a construction-time authority today. Every method on it
(`CONFIGURE`, `CAP_INSERT`, `START`) refuses a thread that is not an `Embryo`, so a `ThreadControlBlock` cap is inert
the moment the thread runs. Giving it a method that works on a *running* thread converts it from a
builder's tool into a lifetime handle, which is a real widening and has to be decided as one.

## Weak memory ordering

Everything in this area is written under `SCHED`, which is the reason `thread_wake_handshake`'s module doc
opens by saying there are no hand-rolled atomics in the protocol. The concurrency lives in the gaps
between critical sections. Three rules guard those gaps: the deferred wake for an `on_cpu` thread,
the undelivered-wake gate, and the single-owner run queue. All three apply unchanged to an abort,
because an abort *is* a wake with `ipc_aborted` set. The one new question a proposal raises is
whether a victim can be `Blocked` with `on_cpu` still set: it can, briefly, and `try_wake` already
defers that case into `wake_pending`. A proposal that instead writes `Finished` directly must check
`on_cpu` itself, which is precisely the `RegionReap::RefuseStanding` rule that cost four CI panics to
learn.

## The authorization question

The mechanism is small. This is the part that is not, and it is the part this system exists to get
right. In Unix you kill by pid with ambient authority; here the question is which held capability
expresses the right to end a blocked thread, and there are four candidate answers with different
consequences.

Milestone 126 (procps) set the frame. A domain names its members and does not act on them, with
`Rights::ENUMERATE` separating looking from acting, and `SURVEY` taking `ENUMERATE` while `RECEIVE` and
`REAP` take `READ`. Any answer here has to say which side of that line it falls on.

| Candidate | Who holds it today | What it already authorizes | What ending a blocked thread would add |
|---|---|---|---|
| The region capability (`Untyped`) | the builder | `DESTROY`: reclaim the region and everything retyped from it | nothing new. `DESTROY` already commits to ending every resident; it just cannot finish |
| The `ThreadControlBlock` capability | the builder, during construction | configure, endow, start | a lifetime handle where there is now a construction tool. seL4's answer |
| The supervision endpoint (`READ`) | the supervisor | `RECEIVE` a death message, `REAP` a corpse | the "stronger right" §32 declined, and the hung-component note showed it is insufficient for case (c) anyway |
| A new right (`Rights::TERMINATE`) | nobody | nothing | a fifth bit; `Rights::ALL` is `0b1111` today, so this is an ABI change to `abi::rights` |

Three observations, offered as analysis rather than as a verdict.

The region capability is the answer that adds no authority at all, and that is a strong argument
in a system whose first principle about rights is that they should not widen. A holder of an
`Untyped` cap can already end every `Ready` and `Running` thread in the region, and can already leave
every `Blocked` one permanently killed-and-refused. It cannot only *finish*. Framed that way, ending
a blocked resident is not a new power; it is the completion of one that is already granted and
already destructive, and `reclaim_region`'s own `BUGS` already says a refused reclaim is destructive.

The `ThreadControlBlock` capability is the answer that composes, and it is what seL4 chose. It names one thread
rather than a region, so it can end a hung component without touching its neighbors, and it is
already delegable and already narrowable. What it costs is the widening above: a builder that hands
out a `ThreadControlBlock` cap today is handing out "you may assemble this thread", and afterwards would be handing
out "you may end this thread whenever you like, forever". Those are different offers and existing
call sites made the first one.

The supervision endpoint is the answer that is most tempting and least available. It is where a
reader expects the authority to live, because the supervisor is the party that notices. But §32
declined it, milestone 126 ruled that a domain names its members and does not act on them, and the
hung-component note proved the supervisor's vocabulary is insufficient for case (c) regardless. Adding
a terminate verb to `Endpoint` would overturn all three at once. It should be refused unless calef
means to overturn them.

And the watchdog does not want any of these (hung-component's answer 1, which stands). A watchdog
holds `ENUMERATE` and produces a verdict; the party that acts is the one that already holds the
authority to act. Nothing in this note changes that separation, and every proposal keeps it.

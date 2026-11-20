---
status: BUILT
raised: 2026-08-17
built: 2026-09-04
---
# 133. Ending a permanently blocked thread, and deciding who may

Built 2026-09-04. Proposed 2026-08-17 by the research lane
`roadmap/blocked-thread-teardown`, from the residual milestone 23's hung-component lane recorded and
declined to open.

The fork was answered before the work: calef chose proposal A on 2026-09-03, *`DESTROY`
finishes what it starts*. Until then this block carried `Gate: DECISION`, because no lane could pick
one of four proposals whose difference is which held capability expresses the right to end a
thread. The authority is the region capability, unchanged; nothing is added to the system call
surface, no new right, and no new error is visible to userspace. The paragraphs below are kept as
they were written, because the argument they record is what the decision was made against; what has
changed is only that it is no longer open.

**Why A and not B, so it is not relitigated.** A's one live risk was that it would settle *what a
spawner retains over a child after `START`* by accident. `DECISIONS` §142 (what a spawner retains
over a child after `START`) settled that on its own terms the same day. Retention becomes a declared
field on `ChildEndowment`, and what it declares today is what the tree already does, which is retain
nothing. Settling it removed the accident and freed A to ship, which that section says in as many
words. **B stays open behind a customer that does not exist**: a terminate
verb on a `ThreadControlBlock` capability widens a construction-time authority into a lifetime
handle, and nobody has asked to end a thread without owning its region. C is milestone 254, minted
separately. D is what the tree did until today.

In brief. `Untyped::DESTROY` on a region holding a live thread marks the thread `killed` and
refuses, so the owner's retry reclaims a runaway (§16's amendment, §24's forcible `^C`). The kill is
spent at the top of `schedule()` and only for a thread whose state is `Running`. A permanently
`Blocked` thread never becomes `current` again, so the kill is armed and never lands, the refusal is
permanent, and the region is unreclaimable for the life of the machine. No privilege fixes it; it
is a scheduler property, which is why §32's "stronger right" is not merely large for this purpose
but insufficient. `reap_region_objects` records the fact in its own comment, and notes/hung-component.md
is where it was first stated as a taxonomy: case (c), a thread blocked on an endpoint its supervisor
cannot reach.

The cost is a capacity cost rather than a tidiness one, and it compounds. A hung server strands its
callers, and a caller stranded mid-`CALL` is itself permanently `Blocked` in its own region, so one
hang can cost two unreclaimable regions and a service with many callers in flight costs one per
caller. The number of hangs the system survives is therefore a function of spare budget, which is
the wrong shape for milestone 55's unattended backup target.

A second, separable defect the research turned up. `abi::Error::Gone` does not reach a
reply-parked caller. The abort machinery walks an endpoint's wait queues, and a caller whose request
was taken was popped off at the rendezvous; it is woken by `sched::ipc_reply` and by nothing else. So
a caller stranded by a server that merely *died* is stranded too, which QNX Neutrino has not permitted
since the 1990s ("If the server thread fails, exits, or disappears, the client thread becomes READY,
with MsgSend() indicating an error"). Nothing in the tree records this as intended. It looks less like
a fork than the rest of this milestone and may want to be split off.

And a hazard whichever way the fork goes. `cap::reply_cap` mints `Object::Reply(tid)`, whose
payload is a generational thread name with no call identity, and `ipc_reply`'s guard checks the
`WaitRole` and discards the endpoint. That is sound only because nothing can currently leave a reply
park and enter a second `CALL` while an unconsumed `Reply` names it. Any proposal that wakes a
reply-parked caller creates exactly that path, and a hung server's stale reply capability would then
forge an answer to a later, unrelated call. L4Re documents the identical hazard as a consequence of
its own finite receive timeouts. The two known fixes are seL4's: delete the outstanding reply
capability at abort (a cspace sweep, the pattern `sched::delete_frame_caps` already establishes), or
do not wake the victim at all.

What the research established, and it reframes the work. The mechanism is close to free and the
authority is the whole problem. `thread::WaitRole` is a closed enumeration of three places a blocked
thread can be (an endpoint's sender queue, its receiver queue, or no queue at all for a `CALL`
caller). `handshake.wait_on` names the endpoint. `set_ipc_aborted` plus `wake` is the existing
abort-and-resume pair. `Endpoint::remove_sender` already exists for surgical unlinking and its
receiver twin is the same twelve lines. And `reap_region_objects` already reaches into an endpoint
*outside* the region being destroyed, with its reasoning written down. So the kernel could end any
blocked thread today in about thirty lines with no new system_call. **What has never been decided is who
may ask, and that is the milestone.**

The prior art says nobody else blocks forever. seL4 folds cancellation into `seL4_TCB_Suspend`
(`suspend()` is `cancelIPC` + dequeue + `ThreadState_Inactive`, authorized by a TCB capability, and
the victim is never handed an error at all). L4 makes it a flag on `ex_regs`. Mach splits
`thread_abort` from `thread_abort_safely` on exactly the hazard of interrupting a non-restartable
operation. Zircon **removed** thread killing in RFC-0007 and answers a blocked `zx_channel_call` by
closing the channel, with `ZX_RIGHT_DESTROY` on a process or job handle as the authority. Linux had to
invent `TASK_KILLABLE` because two states were a false choice. nife is currently alone in having no
way out, and it is alone by accident rather than by decision.

Deliberately in scope: refusing. Proposal D in the note is "accept the leak, bound it with the
`QuotaToken` machinery that already holds a spawner's slot for precisely this case, make it visible,
and recover the service rather than the memory". It is argued rather than listed. §40 is "no reaper of
last resort", Fuchsia deleted this feature after shipping it, and a capacity failure is a visible
refusal where a forcible teardown's failure is a broken invariant inside a component that was still
holding something. If this milestone ends as `RECORDED`, that is a result.

**What is not in scope.** Detection. Deciding that a component *is* hung needs milestone 106's timed
wait and is milestone 23's residual, not this one's; this milestone is about what can be done to a
thread already known to be stuck. It may be stuck because a peer hung, because a peer died, or
because it was written wrong.

## What was built

`sched::region_reap_verdict` grows a fourth answer, `RegionReap::FinishInPlace`, and
`reap_region_objects` grows a phase that acts on it. A resident that is `Blocked` and off its
kernel stack is unlinked from whatever queue holds it, every outstanding `Object::Reply` naming it
is deleted from every capability table, and its state is written straight to `Finished`. It is
never woken and never runs another instruction. `crates/inter_process_communication` gains `Rendezvous::remove_receiver`,
`remove_sender`'s twin.

The authority is unchanged, which is why proposal A was the one that could ship without
deciding anything else: the untyped holder could already end every `Ready` and `Running` resident,
and could already leave every `Blocked` one killed-and-refused. It could not only *finish*. Nothing
was added to the system call surface, no right was added, and no new error is visible to userspace.

Both queues are asked, rather than the recorded `WaitRole`. The research described the role as
deciding which queue holds the thread, and it does not: `ipc_call`'s `Send::Blocked` arm records a
caller as `WaitRole::Reply` while queueing it as a *sender*. So role and queue disagree on every
call that meets no server. Each remove compares pointers and reports whether it found anything, so
asking the wrong queue costs one drain-and-repush of a short queue and cannot be wrong.

**A `Blocked` thread with `on_cpu` still set is refused, not finished**, and lands on
`RegionReap::RefuseStanding`. It is mid-switch-out, so a core is standing on the stack that freeing
its `Thread` would unmap; that is the bug four CI panics taught this path in 2026-08. The condition
clears itself one context switch later and the owner's existing retry loop finds it.

The proof it works is two tests, both in `kernel/src/user/force_kill_tests.rs`, and each was
checked against the mutation it is meant to catch:

- `destroy_reclaims_a_region_whose_resident_blocks_on_a_rendezvous_it_does_not_own` builds a child
  parked in `RECV` on a rendezvous created outside its region, so no sweep can reach it, and
  reclaims. **It hung before this milestone**, and reverting the `Blocked` arm of
  `region_reap_verdict` makes it spend its two-second deadline and fail on its own assertion.
- `tearing_down_a_reply_parked_caller_sweeps_the_reply_capability` stages the hardest shape: a
  caller whose `CALL` was collected, so it sits on no queue at all, with the test itself holding the
  one-shot `Reply`. Deleting the sweep from `sched::finish_blocked_resident` leaves the capability
  in its slot and fails that test **and only that test**, which is the point of having it. A change
  that traded a permanent block for a forgeable reply would pass every capacity assertion above it.

`crates/inter_process_communication` gains a Kani harness, `removing_a_waiter_preserves_the_invariant`, whose interesting
case is the miss rather than the hit, because the kernel asks both queues on every victim.

## Follow-on

- **Milestone 254.** The caller stranded by a server that merely *died*, which `abi::Error::Gone`
  did not reach because the abort machinery walks a rendezvous's wait queues and a reply-parked
  caller left them at the rendezvous. It landed the same day as this one, and the two are
  complementary rather than alternatives: this milestone reclaims the hung component's own region,
  254 frees its stranded callers. They also took one of seL4's two answers to the stale-reply hazard
  each, and the merge folded their two copies of the sweep into one function,
  `sched::delete_reply_caps_naming`. So the invariant they share (no unconsumed reply capability
  names a thread that has left its park) has one implementation. 254's removal-phase
  `strand_callers_of` also runs on the residents this milestone finishes, so a hung *server* ended
  here frees its own clients for free.
- **Milestone 366.** `design/roadmap/0366-a-block-site-that-writes-blocked-by-hand.md`. Nothing
  forces a block site to call `Handshake::park`, so `wait_on` can in principle go stale, and
  `finish_blocked_resident` now *acts* on it: a stale rendezvous name means a freed page still
  linked into a live wait queue. This lane bought what a caller can buy alone (ask both queues, and
  a `debug_assert!` pairing `Blocked` with a recorded wait) and left the name undefended.
- **Milestone 371.** `design/roadmap/0371-a-reply-capability-that-names-a-call.md`, milestone 254's,
  which this lane had written a near-duplicate of and dropped at the merge in favor of the better
  one. The soundness of `ipc_reply`'s role-and-not-call guard is an argument by exhaustion in a note
  rather than a stated property, and both milestones' sweeps protect a rule nothing checks. A call
  identity in the payload is rung one where a sweep is rung two.
- **Done.** By `maintainer/msix-completion-flake` on 2026-09-04, which is why the proposal file it
  was recorded in (`an-msi-x-completion-that-arrives-only-sometimes.md`) is gone. Not this
  milestone's subject at all: one `x86_64` leg of this lane's gate
  failed on `a_userspace_driver_reads_a_file_over_the_pcie_transport`'s interrupt assertion and
  passed alone twice and in a full re-run, so it is recorded rather than left in a lane report. The
  test was sampling its baseline after the driver had already been spawned, and `ROUTED_IRQS` meant
  something different on `x86_64` than on the other two architectures; both are fixed, and
  notes/load-sensitive-assertions.md carries the account.
- **Milestone 368.** `design/roadmap/0368-a-flat-entry-set-counts-bytes-no-system-call-fetches.md`.
  `script/fastpath-footprint`'s `system_call_entry` set is flat and so cannot exclude anything, and this
  lane's change to region teardown made LLVM fold `timer::tick` into `riscv_trap_body`, putting 226
  bytes (12.1%, against a 5% bound) onto a number that measures the system call path. Closed here with
  `#[inline(never)]` on `tick`, which restores the baseline exactly and is the pattern this tree
  already uses twice; the gate's own limitation is what wants a lane.
- **Refused.** Proposal B, a terminate verb on a `ThreadControlBlock` capability. It would widen a
  construction-time authority (every method on that capability refuses a thread that is not an
  `Embryo`) into a lifetime handle, and it answers a question nobody has asked: ending a thread
  without owning its region. `DECISIONS` §142 removed the reason A had to wait for it, and is
  explicit that nothing in it decides this milestone. It stays available behind a customer that does
  not exist.
- **Refused.** Proposal D, accept the leak and bound it with the `QuotaToken` machinery. Its case
  rested on a measurement of what the customer path needs, and the customer path is vacant
  (AGENTS.md, 2026-08-30), so the measurement cannot be taken. Its stronger half survives regardless:
  a per-supervisor bound does not compose across supervisors, and this milestone removes the leak it
  was proposed to bound rather than competing with it.
- **Recorded.** In `sched::reclaim_region`'s BUGS: an `Ok` is now destructive in one more way. A
  resident that was `Blocked` is ended by the call that succeeded, without returning from its
  system call and without running a destructor, and a waiter it sat beside on somebody else's
  rendezvous simply stops being there. The rendezvous's owner observes a peer that never arrived,
  which it cannot tell from a client that never called. That is Zircon's RFC-0007 objection to
  thread killing, and it lands; what makes it bearable here is that a `Blocked` thread holds no
  kernel state in flight and its region is going away regardless.

## Index row

Milestone 23's case (c): `DESTROY` arms a kill that `schedule()` spends only for a `Running`
thread, so a permanently `Blocked` one is refused forever and its region never returns. One hang
can cost two unreclaimable regions (its own and its stranded caller's), which makes the hangs an
unattended backup target survives a function of spare budget. **calef chose proposal A on
2026-09-03**, *`DESTROY` finishes what it starts*. Such a resident is unlinked from whatever queue
holds it, every outstanding `Reply` capability naming it is swept out of every capability table,
and it is written to `Finished` without ever being woken. The authority is the untyped capability,
unchanged, and nothing is added to the system call surface

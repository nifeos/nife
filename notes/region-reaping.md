# Region reaping: what destroying a region does to the objects in it

`sched::reclaim_region` is how a region's owner gets its memory back (§16 (object revocation)). It
tears down every kernel object retyped from the region, unpins it and returns the pages. The code is
`kernel/src/sched/region_reaping.rs`. The supervisor's way in, `rendezvous::REAP` (§32 (reap without
build)), and the survey beside it are built on the same pass. The stale-reply hazard the pass is
shaped around is argued in [notes/blocked-thread-teardown.md](blocked-thread-teardown.md), and the
`on_cpu` refusal's evidence is in
[notes/stack/kernel-stack-freed-under-its-owner.md](stack/kernel-stack-freed-under-its-owner.md).

## The order of a reclaim

1. A region carved into children refuses before any teardown, so that refusal leaves the region
   exactly as it was. The children own part of its run and free those pages themselves.
2. `reap_region_objects` runs under `IPC_TABLES`. It is the caller-driven half of revocation and
   must run outside any teardown `Drop`.
3. `user::reap_address_spaces_in_region` takes the registry's lock, which takes `IPC_TABLES` beneath
   it to ask about bound threads. The registry owns bound spaces (§249 (a running address space stays
   nameable)), so this step also collects the space of every thread step 2 removed, and a corpse's
   space whose root is in the region. A space whose thread can still run was refused in step 2.
4. `memory_region::unpin` and `memory_region::destroy`, neither of which takes `IPC_TABLES`. That is
   why the reaper's `Drop` to `destroy` path cannot deadlock against step 2.

Generational names make every capability to a dead object stale on its next use, so there is no
capability tree to walk and no copies to hunt (seL4 walks its CDT instead).

## The phases of `reap_region_objects`

An object lies in the region when its backing page does: a TCB sits at the start of its page, so the
thread pointer translated back to a physical address is the whole test.

1. **Rendezvous.** Every rendezvous in the region is removed, refusal or not. Removing one drains
   its wait queues, marks each waiter's IPC aborted and wakes it. A resident parked in `RECEIVE` on
   the region's own rendezvous becomes schedulable, so the kill the refuse phase arms can land on it.
   `strand_callers_awaiting` then frees the callers the drain cannot reach: a `CALL` whose request
   was taken has left the sender queue, and only its `wait_on` records that it awaits a reply here.
2. **Notifications, timers, processes.** Notifications get the same sweep, for the same reason. An
   armed timer has no waiter to abort and simply never fires. A process in the region is ended, its
   embryo members detached, and forgotten once empty.
3. **Finish.** Every resident whose verdict is `FinishInPlace` is ended where it is blocked (below).
4. **Refuse.** Any resident still able to run is armed and the pass refuses. Any resident still on
   its own kernel stack refuses the pass without arming.
5. **Removal.** Every thread in the region is `Embryo`, `Finished` or `Dead`, and each is removed.
   A supervised corpse whose death message nobody received is still parked on its supervisor's
   rendezvous, which is outside the region, so it is unlinked first; freeing it while linked would
   leave a pointer the supervisor's next `RECEIVE` follows into a recycled page.
   `strand_callers_of` frees the callers named by reply capabilities the thread collected. `depart`
   and the kill conversion in `schedule` do the same for threads that ran.

### One at a time, by rescan

Each phase that removes finds one victim, removes it and rescans, rather than collecting the victims
into an array first. `remove` mutates the table, so a collect-then-remove shape needs a scratch array
of `MAX_RENDEZVOUS` or `MAX_THREADS` names, and this is one of the deepest frames in the kernel. A
`[u64; 512]` is 4 KiB of a 24 KiB kernel thread stack ([notes/stack-high-water.md](stack-high-water.md)
has the frame's measured size). Rescanning costs O(live objects) per removal, on a teardown path that
removes a handful. Each pass changes the victim so it no longer matches, so each loop terminates.

The rendezvous drain wakes inside its callback for the same reason. That is safe because
`Rendezvous::drain_waiters` pops an entry before calling back, so the thread's one intrusive link is
free for `wake` to push it onto a run queue, and `rendezvous_of` returns a `'static` reference that
does not hold the `sched` borrow the callback needs.

## The verdict on one resident

`region_reap_verdict(state, standing)` decides, and it is a function so it can be tested without
staging a race. `state` says whether a thread can run again. `standing` says whether a core is
standing on its kernel stack, and freeing a `Thread` unmaps that stack. They are different
questions.

| state | standing | verdict | what the pass does |
|---|---|---|---|
| `Ready`, `Running` | either | `RefuseAndArm` | marks it `killed` and refuses; the scheduler converts it to a corpse at its next preemption |
| `Blocked` | no | `FinishInPlace` | ends it in the finish phase |
| `Blocked`, `Dead`, `Finished`, `Embryo` | yes | `RefuseStanding` | refuses and arms nothing |
| `Embryo`, `Finished`, `Dead` | no | `Reap` | removes it |

`standing` is `handshake.on_cpu` or'd with `Thread::being_reaped`. `on_cpu` is cleared by the core's
successor in `finish_switch`. `depart` marks a supervised thread `Dead`, delivers its death message,
releases `IPC_TABLES` and only then switches away, so a supervisor on another core can ask for a reap
inside that window. `being_reaped` marks a thread whose stack and space the reaper is freeing outside
`IPC_TABLES`; reaping it now would let its owner reuse memory the dying thread has not given back.
Both conditions clear by themselves, so the refusal arms nothing and the owner's retry succeeds.

A `Blocked` thread is not armed because the arm cannot reach it. `schedule()` spends a kill only for a
thread whose state is `Running`, and a thread blocked on a rendezvous nobody will serve never becomes
`Running` again. The finish phase is calef's proposal A of milestone 133 (2026-09-03): the holder of
the untyped capability could already end every runnable thread in the region, and now it can also
finish a blocked one. Nothing is added to the syscall surface.

A `Dead` corpse never runs again, so it is reapable exactly like an `Embryo` or a `Finished` thread;
that is what §26 (the fault endpoint) means by reaped with §16 revocation.

The refuse phase meets a `FinishInPlace` verdict only if the finish phase missed one. It refuses
conservatively there rather than panicking, because a teardown-path panic costs more than one more
retry by an owner that already has a retry loop.

## Ending a blocked resident

`finish_blocked_resident` unlinks the thread from whatever queue holds it, deletes every outstanding
`Reply` capability naming it, and writes it to `Finished` without waking it.

- **It never wakes the victim, and that is the security half.** `Object::Reply(tid)` carries a
  generational thread name and no call identity. Waking a reply-parked caller with `Gone` would let
  it call a second server while a hung server's `Reply` still names it, and that stale reply would
  pass `ipc_reply`'s role check and answer the wrong conversation. A `Finished` thread makes no
  second `CALL`.
- **The reply capabilities are swept anyway**, through `delete_reply_caps_naming`, the function
  `strand_reply_caller` runs before its wake. Both keep one invariant: no unconsumed reply capability
  names a thread that has left its park. A `Reply` outliving its thread would otherwise go stale only
  on its next use, through a slot generation nobody reading `ipc_reply` sees.
- **The role does not say which queue holds the thread.** `ipc_call`'s `Send::Blocked` arm records an
  unserved caller as `WaitRole::Reply` while it sits on the sender queue. So both queues are asked;
  each remove compares pointers, and asking the wrong one costs a drain and repush of a short queue.
  The rendezvous name in `wait_on` is still taken on trust, and a stale one would leave a dangling
  pointer in a queue, which is why a `debug_assert` pairs `Blocked` with a recorded wait.
- A thread waiting on a notification, whose page may outlive the region, is removed from its waiter
  queue. A thread in `AddressSpace::WAIT` is removed from the futex table under its key, where a
  later `WAKE` would otherwise walk a freed TCB. A suspended thread (§300 (the paused state)) is on
  no queue and holds its own token.
- `killed` is set as well as the state, so the flag keeps one meaning: a thread this teardown doomed.
  `wait_on` is cleared, because a hang dump reads `Some` as still waiting.

## Why dropping a resident's space is safe

The pin is not what makes it safe. A drop inside `reap_region_objects`, under `IPC_TABLES`, is
refused by `memory_region::destroy` while the region is pinned, but the reaper (`finish_switch`)
hoists a dead thread's space out, releases `IPC_TABLES` and drops it later, when `reclaim_region`
may already have unpinned. What makes it safe is that a space built from a region it does not own
never frees that region (`user::Backing`). That is a property of the space, not of the timing.

## The supervised reap

`reap_supervised(ep, tid)` collects a corpse for the supervisor that holds its supervision
rendezvous. Authorization is the relationship the kernel already tracks: the thread's recorded
`fault_ep` (§26 implementation note 1) must be the invoked rendezvous, so the tid needs no badge or
handle. `capability::reap_decision` is the rule, proved for every input in that crate. A stale or
recycled tid resolves to nothing and reads as unsupervised.

The reclaim is then `reclaim_region` on the region the TCB was retyped from, the name the owner
would pass to `MemoryRegion::DESTROY`, so the two paths cannot drift. The pages go back to the
region's owner under §13, the builder, so a supervisor frees a child's memory without being able
to spend it.

A corpse whose core has not switched away yet is waited for, not refused. `reclaim_region` refuses
it, and that refusal reaches userspace as `NotPermitted`, which the ABI gives another meaning and
which two supervisors in this tree treat as fatal. The condition ends at that core's next
`finish_switch` and needs only the lock, so the reap releases `IPC_TABLES`, yields and looks again.
`NotPermitted` still comes back for `DESTROY`'s own reasons: the child split its budget, another
thread in the region can still run, or a racing reap made the name stale. A restart policy reads it
as "not yet".

`ReclaimRefusal` and `last_reclaim_refusal` (system tests only) record why the last refusal
happened, because every refusal reaches userspace as one `NotPermitted`. The failure they were built
for (milestone 321) happened about once in a thousand runs.

## The survey

`survey_supervised` is `rendezvous::SURVEY`, and its design is in
[notes/process-view.md](process-view.md). Three kernel-side rules:

- An unknown `record` is `BadMethod` before the lock and before the walk, so the refusal does not
  depend on whether the domain has a member. The `match` keeps a refusing default arm so a record
  added to `is_known` without an arm fails loudly instead of reporting zero.
- No scheduler yet, or a cursor past the table, is `DONE`, not a refusal: the caller's authority was
  never in question.
- One entry per call, with `IPC_TABLES` released between calls, so a program surveying a large
  domain cannot hold the lock at its own discretion.

The words it reports: CPU time is milliseconds, converted in the kernel so no reader depends on
`TICK_HZ` (100 Hz outside the bench build, so it moves in steps of ten). Placement maps the kernel's
`u8::MAX` "not placed" to `record::NO_CPU` (`u64::MAX`), because 255 is a plausible CPU id for a
reader to tally. State maps the two states a supervised thread cannot reach, `Embryo` and
`Finished`, to `READY` and `DEAD`, its nearest neighbors; the `match` is exhaustive so a new state
meets it.

## BUGS

- **`Err` is destructive, and it does not read that way at a call site.** A refusal caused by a live
  thread arms the kill on every live thread in the region, so `reclaim_region(r).is_err()` cannot be
  asked as a question: asking it dooms the answer. No call asks whether a region is busy without
  ending what is in it. `user::tests::reclaim_frees_a_started_then_exited_childs_regions` lost a
  wakeup to a test that probed this way (milestone 72).
- **`Ok` is destructive too.** A `Blocked` resident is ended by the call that succeeded, without
  returning from its syscall, and the peer it was queued beside on somebody else's rendezvous sees
  a caller that never arrived, which it cannot tell from one that never called. §24's `^C`
  escalation depends on this.
- **It reaches outside the region.** Tearing down a rendezvous aborts every waiter wherever it
  lives, and frees every caller reply-parked on it or named by a reply capability in a thread this
  reclaim reaps. Those callers see `abi::Error::Gone` from a `CALL` to somebody else's rendezvous.

*Name: provisional. The note is named for the file it documents.*

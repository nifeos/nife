//! **Region reaping**: what a region's destruction does to every thread, rendezvous, notification
//! and timer it pinned, `reclaim_region`, and the supervised reap and survey built on them (§271
//! part 3).
//!
//! Cut out of `sched.rs` unchanged by milestone 840 (the scheduler file is split along its seams).
//!
//! Name: provisional (milestone 840's lane, 2026-10-11 UTC; §271 (the scheduler file splits into
//! submodules of `sched`)).

use super::*;

/// Tear down every kernel object whose backing page lies in `[base, end)`, so `memory_region::destroy`
/// can reclaim the region (object revocation). `Err` if a **live** thread (`Ready`/`Running`/
/// `Blocked`) sits in the region, or if a dead one is still standing on its kernel stack
/// (`handshake.on_cpu`); the region stays pinned. But the refusal is no longer passive:
/// it **arms the kill** (DECISIONS §16 amendment), marking each live resident thread so the
/// scheduler tears it down at its next preemption, so an owner that retries (the shell's `^C`
/// escalation, §24) reclaims a runaway rather than being told forever to wait for it. `Embryo` and
/// `Finished` threads are removed here (dropped, and their generational names killed, so every
/// outstanding `ThreadControlBlock` capability to them goes stale on its next use).
///
/// **A `Blocked` resident is not refused-and-armed; it is ended** (milestone 133, proposal A,
/// calef 2026-09-03). The arm is spent by `schedule()` only for a thread whose state is `Running`,
/// so a resident blocked on a rendezvous nobody will ever serve was armed and never reaped and the
/// region never came back. The finish phase unlinks each such thread from whatever queue holds it,
/// deletes every outstanding `Reply` capability naming it, and writes it to `Finished` **without
/// waking it**; see [`finish_blocked_resident`] for why not waking it is the security half. The
/// authority is unchanged: it is still the untyped capability, and nothing was added to the
/// syscall surface.
///
/// **The region's endpoints go first, on every pass, refusal or not**, and that ordering is
/// load-bearing rather than tidy: it is what wakes a resident blocked in `RECEIVE` so the armed kill
/// can actually land on it. The long comment at the sweep says why, and notes/frames.md carries the
/// boot it fixed.
///
/// Takes `IPC_TABLES`, so it must run **outside** any teardown `Drop`: this is the caller-driven half of
/// revocation, and `memory_region::destroy` is the `IPC_TABLES`-free half (which is why the reaper's
/// `Drop` -> `destroy` path cannot deadlock against it). See `memory_region::unpin`.
/// What the refuse phase of [`reap_region_objects`] decides about one resident thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RegionReap {
    /// Free its pages: it can never run again and no core is standing on its stack.
    Reap,
    /// It can still be scheduled. Refuse, and arm DECISIONS §16's kill so the owner's retry
    /// reclaims a runaway rather than being told to wait forever.
    RefuseAndArm,
    /// It is dead but has not left its own kernel stack yet. Refuse, and arm **nothing**: there is
    /// nothing left to doom, and the condition clears on its own one context switch from now.
    RefuseStanding,
    /// **It is `Blocked`, and arming a kill would not reach it** (milestone 133). Unlink it from
    /// whatever queue holds it, sweep the outstanding `Reply` capabilities that name it, and write
    /// it straight to `Finished` without ever letting it run again. That is proposal A's whole
    /// mechanism, and the verdict exists as its own word because the fact that
    /// [`RegionReap::RefuseAndArm`] is *insufficient* for a `Blocked` thread is the defect this
    /// milestone fixes and is not visible from the arm itself.
    ///
    /// Named for what the pass does rather than for the state it found, because the reader who
    /// needs it is the one asking why a `Blocked` resident is not simply armed like the others.
    FinishInPlace,
}

/// **The refuse phase's rule, lifted out so it can be stated and tested without staging a race.**
///
/// The `on_cpu` arm is the one that had to be learned the expensive way, and it is why this is a
/// named function rather than a condition inside the loop. The rule a reader must carry away:
/// **`state` says whether a thread can run again, and `on_cpu` says whether a core is standing on
/// its stack, and freeing a `Thread` unmaps that stack.** Those are different questions; this path
/// asked only the first for months, and the answer to the second is what four CI panics were.
/// See notes/stack/kernel-stack-freed-under-its-owner.md.
///
/// `standing` is `on_cpu`, or'd since 2026-10-04 with
/// [`being_reaped`](crate::thread::Thread::being_reaped): a thread whose stack and address space the
/// reaper is freeing outside `IPC_TABLES` is refused the same passive way, for a different reason
/// with the same shape (the condition clears by itself, and reaping the thread now would let its
/// owner reuse memory the dying thread has not given back yet).
pub(super) fn region_reap_verdict(state: State, standing: bool) -> RegionReap {
    if matches!(state, State::Ready | State::Running) {
        RegionReap::RefuseAndArm
    } else if state == State::Blocked && !standing {
        // Milestone 133, proposal A. `Blocked` used to sit in the arm above, and being there is
        // what made a permanently blocked resident unreclaimable for the life of the machine: the
        // arm is spent at the top of `schedule()` and only for a thread whose state is `Running`,
        // and a thread blocked on a rendezvous nobody will ever serve does not become `Running`
        // again. The arm was not too weak, it was aimed at a thread that never arrives.
        RegionReap::FinishInPlace
    } else if standing {
        // A `Blocked` thread with `on_cpu` still set reaches here, and that is deliberate. It is
        // mid-switch-out, so its saved context is stale and a core is standing on the stack that
        // freeing its `Thread` would unmap; ending it now is the four-CI-panic bug wearing a new
        // hat. Refusing without arming is exactly right, because the condition clears itself one
        // context switch from now and the owner's next retry finds it `Blocked` and off its stack.
        RegionReap::RefuseStanding
    } else {
        RegionReap::Reap
    }
}

/// **End one permanently blocked resident of a region being destroyed** (milestone 133, proposal
/// A). Unlink it from whatever queue holds it, delete every outstanding `Reply` capability that
/// names it, and write it straight to `Finished`. Caller holds `IPC_TABLES`.
///
/// **It never wakes the victim, and that is a security property rather than an economy.** The
/// research (notes/blocked-thread-teardown.md) found a live hazard that any waking design has to
/// buy its way out of: `cap::reply_cap` mints `Object::Reply(tid)` whose payload is a generational
/// thread name with **no call identity**, and `ipc_reply`'s guard checks the `WaitRole` and
/// discards the rendezvous. That is sound today only because nothing can leave a reply park and
/// enter a second `CALL` while an unconsumed `Reply` still names it. Wake a reply-parked caller,
/// hand it `Gone`, let it call a healthy server, and a hung server's stale `Reply` passes the role
/// check and forges an answer to a different conversation. `L4Re` documents the identical hazard as
/// a consequence of its own finite receive timeouts, and Zircon documents it for
/// `zx_channel_call`'s timeout. **A change that traded a permanent block for a forgeable reply
/// would be a worse defect than the one it fixes.**
///
/// Both of seL4's fixes are taken here, deliberately, and the second is why the first is cheap.
/// The victim is set to `Finished` and never runs another instruction, which is
/// `ThreadState_Inactive` and means no second `CALL` can exist to be forged against. **And the
/// reply capabilities are swept anyway**, through [`delete_reply_caps_naming`], which is
/// `cteDeleteOne(callerCap)` from `cancelIPC` and is the same function milestone 254's
/// [`strand_reply_caller`] runs before *its* wake. The two milestones took one of seL4's answers
/// each, a day apart, and share the invariant: **no unconsumed reply capability names a thread
/// that has left its park.**
///
/// **The role is not trusted to say which queue holds the thread, and must not be.** A `CALL`
/// caller that met no server is recorded as `WaitRole::Reply` and *is* on the sender queue
/// (`ipc_call`'s `Send::Blocked` arm), so the role and the queue genuinely disagree in a case that
/// happens on every unserved call. Both queues are asked; each remove compares pointers and
/// reports whether it found anything, so asking the wrong one costs one drain-and-repush of a
/// short queue and cannot be wrong. This is the sharpest failure mode the research named
/// (`thread_wake_handshake`'s own BUGS says nothing forces a block site to call `park`, so
/// `wait_on` can in principle go stale) narrowed to the one part of it a caller can defend
/// against: the *rendezvous name* is still taken on trust, and a stale one leaves a dangling
/// pointer in a queue, which is why the `debug_assert` below pairs `Blocked` with a recorded wait.
pub(super) fn finish_blocked_resident(sched: &mut IpcTables, tid: ThreadId) {
    debug_assert!(
        sched
            .threads
            .get(tid)
            .is_none_or(|t| t.handshake.wait_on.is_some()),
        "a Blocked thread with no recorded wait: some block site wrote the state by hand",
    );

    // Unlink. `wait_on` names the object; the pointer identifies the thread on its queue.
    match sched.threads.get(tid).and_then(|t| t.handshake.wait_on) {
        Some(Wait::Rendezvous(ep, _role)) => {
            let ptr = thread_control_block_ptr(sched, tid);
            if let Some(rendezvous) = rendezvous_of(sched, ep) {
                // `ptr` is only compared. Whichever queue held the thread hands its token back,
                // and it goes home to the thread, to be freed with it.
                for token in [
                    rendezvous.remove_sender(ptr),
                    rendezvous.remove_receiver(ptr),
                ]
                .into_iter()
                .flatten()
                {
                    hold_token(token);
                }
            }
        }
        // A thread blocked in `Notification::WAIT` (milestone 151) is linked on that
        // notification's queue, and its page may outlive this region.
        Some(Wait::Notification(id)) => {
            let ptr = thread_control_block_ptr(sched, tid);
            if let Some(page) = notification_of(sched, id)
                && let Some(token) = page.state.remove_waiter(ptr)
            {
                hold_token(token);
            }
        }
        // A thread blocked in `AddressSpace::WAIT` (milestone 812) is on the futex table under its
        // key. Leaving it there would leave a freed TCB on a queue a later `WAKE` walks.
        Some(Wait::Futex(key)) => {
            let ptr = thread_control_block_ptr(sched, tid);
            if let Some(token) = sched.futexes.remove(key, ptr) {
                hold_token(token);
            }
        }
        // A suspended thread (DECISIONS §300 (the paused state, a TCB verb, silent)) is on no
        // queue and already holds its token.
        Some(Wait::Resume) | None => {}
    }

    // Sweep the outstanding reply capabilities, sharing milestone 254's function rather than
    // carrying a second copy of the same invariant. This victim is never woken, so the sweep is
    // belt as well as braces; it is here anyway because the thread name is generational and a
    // `Reply` outliving its thread would go stale only on its *next use*, which leaves the tree
    // depending on a slot generation nobody reading `ipc_reply` can see.
    delete_reply_caps_naming(sched, tid);

    // End it. `killed` is set as well as the state, so the flag keeps one meaning: it marks a
    // thread this teardown doomed, whoever ends up observing it. `wait_on` is cleared because the
    // thread is on no queue any more and a `Some` here is what a hang dump reads as "still
    // waiting".
    if let Some(t) = sched.threads.get_mut(tid) {
        t.killed = true;
        t.handshake.state = State::Finished;
        t.handshake.wait_on = None;
    }
}

pub(super) fn reap_region_objects(base: u64, end: u64) -> Result<(), ()> {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        record_refusal(ReclaimRefusal::NoScheduler);
        return Err(());
    };
    // A TCB sits at the start of its page, so the page's physical address is the thread pointer
    // translated back. That is the whole test for "this object lives in the region".
    let page_of = |t: &Thread| crate::arch::mmu::virt_to_phys(t as *const Thread as u64);

    // --- Rendezvous phase: the region's endpoints go FIRST, refusal or not. ---
    //
    // **This ordering is what lets `DESTROY` reclaim a region full of blocked servers**, and until
    // 2026-08-16 it could not. The sweep used to sit after the refusal below, so a region holding a
    // process parked in `RECEIVE` was refused forever: the refusal armed §16 (object revocation)'s kill, the kill is spent
    // by `schedule()`, and a `Blocked` thread never reaches `schedule()`. The owner retried until it
    // gave up, and the memory stayed spoken for until the machine stopped. That is the whole reason
    // the aarch64 test boot ran out of frames: `the_console_server_builder_brings_up_a_console_that_prints_for_it` builds a
    // console server out of the progenitor's budget and that server blocks in its serve loop, so the progenitor's
    // 2048-frame region was unreclaimable by construction. See notes/frames.md.
    //
    // Sweeping first fixes it because **the wake is already here**: removing an rendezvous drains its
    // wait queues, marks each waiter's IPC aborted and wakes it, which is precisely the transition a
    // blocked resident needs to become schedulable and so to spend the kill the refusal arms one
    // paragraph below. A server whose endpoints came out of the region being destroyed dies; one
    // blocked on somebody else's rendezvous still does not, and `reclaim_region`'s caller is told so by
    // the refusal rather than by a hang (see `user::holding::Holding`'s BUGS).
    //
    // **A refused reclaim was already destructive** and says so in `reclaim_region`'s BUGS: it arms
    // kills on every live resident. This makes the same pass also end the region's endpoints, which
    // is the same commitment one object over: the caller has said this region is going away. What it
    // must not do is *surprise* anyone, which is why it is written here rather than assumed.
    //
    // **Rescan for one at a time rather than listing them all first.** The obvious shape is to walk
    // the table into a `[u64; MAX_RENDEZVOUS]` and then walk that, because `remove` mutates the table
    // and you cannot remove while iterating it. That array is **4096 bytes of a 24 KiB kernel thread
    // stack** (`thread::STACK_PAGES`), and this function is already the deepest frame in the kernel;
    // measured with `-Z emit-stack-sizes` it was 6816 bytes, of which 6144 was three such scratch
    // arrays, against a measured thread-stack high-water of 11672 bytes: this one frame wanted 2104
    // bytes MORE than all the headroom there was. See notes/stack-high-water.md.
    //
    // Rescanning costs O(live endpoints) per removal instead of O(1). That is the right trade here
    // and nowhere near a hot path: this runs when a region is torn down, the table is 512 slots, and
    // a real teardown removes a handful. Stack is the scarce resource, not these comparisons.
    loop {
        let doomed = sched
            .rendezvous_table
            .iter()
            .find(|&(_, &phys)| base <= phys && phys < end)
            .map(|(name, _)| name);
        let Some(name) = doomed else { break };

        // Drain the rendezvous's waiters, **waking each one inside the drain rather than listing
        // them into a `[u64; MAX_THREADS]` first**, which is the same rule the paragraph above
        // states and this line did not follow: that array was 1 KiB at 128 threads and grows with
        // the ceiling, on the frame this function's own comment calls the deepest in the kernel.
        //
        // The wake is safe here for the reason the collected version relied on one line later:
        // `Rendezvous::drain_waiters` pops an entry off its queue *before* calling back, so the
        // thread's one intrusive link is already free and `wake` may push it onto a run queue.
        // `rendezvous_of` returns a `'static` reference, so the rendezvous does not hold the
        // `sched` borrow the callback needs.
        if let Some(rendezvous) = rendezvous_of(sched, name) {
            rendezvous.drain_waiters(|w| {
                let tid = hold_token(w);
                set_ipc_aborted(sched, tid);
                wake(sched, tid);
            });
        }
        sched.rendezvous_table.remove(name);
        // **And the callers the drain structurally cannot reach** (milestone 254). A `CALL` whose
        // request was taken left the sender queue at the rendezvous, so `drain_waiters` above walks
        // straight past it; only `wait_on` still records that it awaits a reply through this
        // rendezvous, which no longer exists.
        strand_callers_awaiting(sched, name);
    }

    // --- Notification phase (milestone 151): the same sweep for the region's notifications. ---
    //
    // After the rendezvous phase and before the finish phase, for the rendezvous phase's reason:
    // a thread blocked in `WAIT` on a notification in this region is aborted and woken here, and so
    // becomes schedulable enough to spend its kill.
    reap_region_notifications(sched, base, end);
    // And its timers (milestone 106), which have no waiters to abort: an armed one just never fires.
    reap_region_timers(sched, base, end);
    // And its processes (milestone 812): ended, embryo members detached, forgotten once empty.
    process::reap_region(sched, base, end);

    // --- Finish phase: end every resident the arm below could never reach (milestone 133). ---
    //
    // **This is proposal A, "`DESTROY` finishes what it starts"** (calef, 2026-09-03;
    // design/roadmap/0133-blocked-thread-teardown.md, and notes/blocked-thread-teardown.md for the
    // four proposals and the survey they came from). The refuse phase below arms DECISIONS §16's
    // kill, `schedule()` spends that kill only for a thread whose state is `Running`, and a thread
    // blocked on a rendezvous nobody will ever serve never becomes `Running` again. So the arm was
    // armed and never landed, the refusal was permanent, and **the region was unreclaimable for
    // the life of the machine**. No privilege fixed that, because it was a scheduler property and
    // not an authorization one, which is why §32's "stronger right" was insufficient rather than
    // merely large.
    //
    // The rendezvous sweep above already rescues the case where the rendezvous the resident waits
    // on came out of *this* region. What it cannot rescue is a resident blocked on somebody
    // else's, which is the ordinary shape of a hung component: a server parked in `RECEIVE` on a
    // client's rendezvous, or a client parked in `CALL` on a server that will never reply.
    //
    // **The authority is unchanged, and that is the whole reason this shape was chosen.** The
    // holder of the untyped capability could already end every `Ready` and `Running` thread in the
    // region and could already leave every `Blocked` one killed-and-refused. It could not only
    // *finish*. Nothing is added to the syscall surface, no new right, and no new error reaches
    // userspace.
    //
    // **Rescan for one at a time**, the same rule the rendezvous sweep states and for the same
    // reason: a `[u64; MAX_THREADS]` worklist is a kilobyte of scratch on the deepest frame in the
    // kernel. Each pass writes one resident to `Finished`, which no longer matches, so this
    // terminates.
    loop {
        let doomed = sched
            .threads
            .iter_mut()
            .find(|t| {
                let phys = crate::arch::mmu::virt_to_phys(&raw const **t as u64);
                base <= phys
                    && phys < end
                    && region_reap_verdict(t.handshake.state, t.handshake.on_cpu || t.being_reaped)
                        == RegionReap::FinishInPlace
            })
            .map(|t| t.id);
        let Some(tid) = doomed else { break };
        finish_blocked_resident(sched, tid);
    }

    // --- Refuse phase: no thread in the region may still be able to run. ---

    // A live thread (Ready/Running/Blocked) in the region: freeing its page would pull the stack,
    // or the running address space, out from under a thread that can still be scheduled. We may not
    // reclaim under it this pass, but the forcible tier of `^C` (DECISIONS §24) needs `DESTROY` to
    // *tear a runaway down*, not merely refuse it. So arm the kill (§16 amendment): mark every live
    // resident thread `killed` and refuse. A killed thread never runs again; the scheduler converts
    // it to a corpse at its next preemption, with no queue surgery here and no core stopping another
    // (each core reaps its own on the timer). The region's owner retries `DESTROY` (the shell's
    // escalation loop already does), and once the runaway has torn down this pass finds it gone and
    // reclaims. A thread that only ever blocks, never scheduled to hit that preemption, is the
    // cooperative tier's job (send it its interrupt rendezvous), not this one.
    let mut live = false;
    // **And a second refusal, which is not about being alive at all**: a thread whose
    // `handshake.on_cpu` is still set. That flag means "a core is standing on this thread's kernel
    // stack", it is cleared by that core's successor in `finish_switch`, and freeing the `Thread`
    // is what unmaps the stack. `finish_switch` is built around exactly this and says so; this
    // path was not, because it reasoned from `state`, and `Dead` genuinely does mean "never runs
    // again". **Never runs again is not the same as off its stack**, and the gap between them is a
    // real window: `depart` marks a supervised thread `Dead`, delivers its death message (waking
    // the supervisor, possibly on another core), releases `IPC_TABLES`, and only *then* calls
    // `schedule()`. A supervisor that reaps inside those few hundred instructions unmapped the
    // stack under the corpse, whose next store then walked the exception vector down to this
    // slot's base. Four CI runs over five days, always the same test, always the same slot, and
    // read as a stack overflow for three of them. See
    // notes/stack/kernel-stack-freed-under-its-owner.md.
    //
    // No kill is armed for this one, deliberately: the thread is already dead, so there is nothing
    // to doom, and the refusal clears on its own one context switch from now. The caller retries,
    // which is `reclaim_region`'s existing contract.
    let mut standing = false;
    //
    // A live thread (Ready/Running/Blocked) in the region: freeing its page would pull the stack, or
    // the running address space, out from under a thread that can still be scheduled. A `Dead`
    // corpse (milestone 22) is *not* live: it never runs again, so it is reapable here exactly like
    // an `Embryo` or a `Finished` thread, which is precisely what "reaped with §16 revocation"
    // (DECISIONS §26) means.
    //
    // **The pin is not what makes dropping a resident's address space safe, and saying it was cost
    // us a double free.** This comment used to argue that the region stays pinned through the reap,
    // so a bound space dropping here is refused by `memory_region::destroy`. That is true for a drop that
    // happens *inside* this function, under `IPC_TABLES`. It is false for the one that matters: the
    // reaper (`finish_switch`) hoists a dead thread's space out, releases `IPC_TABLES`, and drops it
    // afterwards, by which time `reclaim_region` may already have unpinned. What makes it safe is
    // that a space built from a region it does not own never frees that region at all
    // (`user::Backing`), which is a property of the space rather than of the timing.
    for t in sched.threads.iter_mut() {
        let phys = page_of(t);
        if !(base <= phys && phys < end) {
            continue;
        }
        let verdict = region_reap_verdict(t.handshake.state, t.handshake.on_cpu || t.being_reaped);
        if verdict != RegionReap::Reap {
            record_refusal(ReclaimRefusal::resident(t));
        }
        match verdict {
            RegionReap::RefuseAndArm => {
                t.killed = true;
                live = true;
            }
            RegionReap::RefuseStanding => standing = true,
            // Unreachable: the finish phase above ran this same verdict to exhaustion, so nothing
            // in the region is still `Blocked` and off its stack. Written as the conservative
            // refusal rather than as an `unreachable!`, because the cost of being wrong here is a
            // panic on a teardown path against the cost of one more retry by an owner that already
            // has a retry loop.
            RegionReap::FinishInPlace => {
                t.killed = true;
                live = true;
            }
            RegionReap::Reap => {}
        }
    }
    if live || standing {
        return Err(());
    }
    // --- Removal phase: every object in the region is reapable. ---

    // Threads: `remove` mutates the table, so this cannot remove while iterating. **Rescan for one
    // at a time**, exactly as the rendezvous sweep above does and for the same reason it gives: the
    // obvious `[u64; MAX_THREADS]` list is a scratch array on the kernel's deepest frame that grows
    // every time the thread ceiling does. Each pass removes one resident, so the set shrinks and
    // this terminates; the cost is O(live threads) per removal on a teardown path. Both Embryo and
    // Finished go.
    loop {
        let doomed = sched
            .threads
            .iter_mut()
            .find(|t| {
                let phys = page_of(t);
                base <= phys && phys < end
            })
            .map(|t| t.id);
        let Some(tid) = doomed else { break };
        // **Unlink a corpse from its supervision rendezvous first.** A supervised thread that died
        // with nobody in `RECEIVE` is parked on that rendezvous's *sender* queue holding its death
        // message (DECISIONS §26 implementation note 2), and that rendezvous is the supervisor's, so
        // it is not in this region and the rendezvous sweep above did not touch it. Freeing the TCB
        // while it is still linked there would leave a dangling pointer that the supervisor's next
        // `RECEIVE` would follow into a recycled page. §16's `DESTROY` could already reach this (reap
        // before receiving); §32's rendezvous reap makes it easy to reach, because a supervisor can be
        // told a tid by its builder and never collect the message at all.
        let parked = sched
            .threads
            .get(tid)
            .filter(|t| t.handshake.state == State::Dead)
            .and_then(|t| t.fault_ep);
        if let Some(ep) = parked {
            let ptr = thread_control_block_ptr(sched, tid);
            // `ptr` is only compared; the corpse's token comes home, to be freed with it.
            if let Some(rendezvous) = rendezvous_of(sched, ep)
                && let Some(token) = rendezvous.remove_sender(ptr)
            {
                hold_token(token);
            }
        }
        // **And the callers this thread will never answer** (milestone 254): a resident reaped
        // here is an `Embryo` or a corpse whose table still holds the reply capabilities it
        // collected, and freeing the TCB is what makes them unreachable. `depart` and the kill
        // conversion in `schedule` cover the threads that ran; this covers the rest.
        strand_callers_of(sched, tid);
        let _ = process::leave(sched, tid); // its process's space falls to the registry sweep
        sched.threads.remove(tid);
    }

    process::forget_empty(sched, base, end)
}

/// **Why [`reclaim_region`] last refused** (milestone 321). Test support: a refusal reaches
/// userspace as one `NotPermitted` whatever its cause, and the failure this was built for happened
/// once in 255 boots, so the next one has to name its cause in the log it leaves. Each refusing
/// check overwrites it; nothing clears it, so read it right after the refusal it explains.
///
/// For a resident it records the last one the refuse phase turned away, with the two flags that
/// together make up `standing`.
///
/// Name: provisional, minted 2026-10-22 (UTC) by lane/321-reclaim-refused.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
#[derive(Debug, Clone, Copy)]
pub enum ReclaimRefusal {
    /// A child region still owns part of the run.
    HasChildren,
    /// The region name is stale: it was destroyed already, or never existed.
    NoRegion,
    /// No scheduler yet.
    NoScheduler,
    /// A thread in the region can still run, or is still standing on its own kernel stack.
    Resident {
        tid: ThreadId,
        state: State,
        on_cpu: bool,
        being_reaped: bool,
    },
}

impl ReclaimRefusal {
    fn resident(t: &Thread) -> Self {
        Self::Resident {
            tid: t.id,
            state: t.handshake.state,
            on_cpu: t.handshake.on_cpu,
            being_reaped: t.being_reaped,
        }
    }
}

#[cfg(feature = "system_tests")]
static LAST_REFUSAL: crate::sync::IrqSafeMutex<Option<ReclaimRefusal>> =
    crate::sync::IrqSafeMutex::new(crate::sync::rank::RECLAIM_REFUSAL, None);

#[cfg_attr(not(feature = "system_tests"), allow(unused_variables))]
fn record_refusal(why: ReclaimRefusal) {
    #[cfg(feature = "system_tests")]
    {
        *LAST_REFUSAL.lock() = Some(why);
    }
}

/// The cause [`reclaim_region`] recorded at its most recent refusal. See [`ReclaimRefusal`].
///
/// Name: provisional, minted 2026-10-22 (UTC) by lane/321-reclaim-refused.
#[cfg(feature = "system_tests")]
pub fn last_reclaim_refusal() -> Option<ReclaimRefusal> {
    *LAST_REFUSAL.lock()
}

/// **Reclaim an untyped region and every object retyped from it** (object revocation, the region-
/// ownership half). The owner, holding the untyped capability, reclaims: tear the region's objects
/// down (refusing if any is still live), unpin, and return the memory. Generational names make
/// every capability to the now-dead objects stale on next use, so there is no capability tree to
/// walk and no copies to hunt (contrast seL4's CDT; DECISIONS records the choice).
///
/// Must run outside any `Drop`, because the reap takes `IPC_TABLES` (see `reap_region_objects`); the
/// `unpin` + `destroy` that follow are `IPC_TABLES`-free.
///
/// # BUGS
///
/// **`Err` is destructive, and it does not read that way at a call site.** A refusal caused by a
/// live thread arms DECISIONS §16's kill on *every* live thread in the region, so the first call
/// dooms them and the owner's retry reclaims. **Since milestone 133 an `Ok` is destructive in one
/// more way**: a resident that was `Blocked` is ended outright by the call that succeeded, without
/// running another instruction and without returning from its syscall, and a waiter it was queued
/// beside on somebody else's rendezvous simply stops being there. The rendezvous's owner observes
/// a peer that never arrived, which it cannot tell from a client that never called. That is the point (§24's `^C` escalation is built on
/// it), but it makes `reclaim_region(r).is_err()` unusable as a question: asking it kills the
/// answer. A caller that wants to know whether a region is busy without ending what is in it has no
/// such call today. Milestone 72 traced an intermittent lost-wakeup hang to one line of test code
/// that used the refusal as a probe; see `user::tests::reclaim_frees_a_started_then_exited_childs_regions`.
///
/// **It reaches outside the region, and since milestone 254 it reaches one step further.** Tearing
/// down a rendezvous already drained its wait queues and aborted every waiter, wherever those
/// waiters lived; it now also frees every caller reply-parked on that rendezvous, and every caller
/// still named by a reply capability in a thread this reclaim reaps. Those callers observe
/// [`abi::Error::Gone`] from a `CALL` they made to somebody else's rendezvous. That is the same
/// commitment one object over (`reap_region_objects`'s own comment argues it for the wait queues),
/// and it is written here because the surprise, if there is one, lands on a reader of this function.
pub fn reclaim_region(region: u64) -> Result<(), ()> {
    // A region carved into children cannot be reclaimed: its child regions own part of its run and
    // free those pages themselves. The owner must destroy the children first. Refuse before any
    // teardown, so a refused reclaim leaves the region exactly as it was.
    if crate::memory_region::has_children(region) {
        record_refusal(ReclaimRefusal::HasChildren);
        return Err(());
    }
    let Some((base, size)) = crate::memory_region::region_bounds(region) else {
        record_refusal(ReclaimRefusal::NoRegion);
        return Err(());
    };
    // Threads first (IPC_TABLES), then the address spaces the region's destruction ends (the
    // registry's lock, which takes IPC_TABLES beneath it to ask about bound threads). Since §249 the
    // registry owns bound spaces too, so this second step also collects the space of every thread
    // the first step removed, and takes a corpse's space whose root is in the region; it leaves a
    // space whose thread can still run, which is milestone 765's to refuse in the first step.
    reap_region_objects(base, base + size)?;
    crate::user::reap_address_spaces_in_region(base, base + size);
    crate::memory_region::unpin(region);
    crate::memory_region::destroy(region);
    Ok(())
}

/// **Collect a corpse a supervision rendezvous supervises** (DECISIONS §32, `rendezvous::REAP`).
///
/// The one thing a supervisor could not previously do without holding the authority to *build* a
/// process. `ep` is the rendezvous the supervisor invoked; `tid` is the id the kernel stamped on the
/// death message. Authorization is the relationship the kernel already tracks (`Thread::fault_ep`,
/// §26 implementation note 1) rather than a new registry: the named thread's recorded supervision
/// rendezvous must *be* the invoked one, which is why the tid needs no badge and no handle. The
/// decision itself is `capability::reap_decision`, proved for every input in that crate.
///
/// Then the reclaim is §16's, unchanged: `reclaim_region` on the region the TCB was retyped from,
/// which is exactly the region name the owner would have passed to `MemoryRegion::DESTROY`. One teardown
/// path, so the two cannot drift. **The pages go back to the region's owner under §13**, which is
/// the builder, not the reaper: a supervisor frees a child's memory without ever being able to spend
/// it, because it never holds a capability to it.
///
/// Takes and releases `IPC_TABLES` before the reclaim, which takes it again: `reap_region_objects` must
/// run with the lock and cannot be called under it.
pub fn reap_supervised(ep: RendezvousId, tid: ThreadId) -> Result<(), abi::Error> {
    let region = {
        let guard = IPC_TABLES.lock();
        let sched = guard.as_ref().ok_or(abi::Error::NotSupervised)?;
        // A stale or recycled tid resolves to `None` here (generational names, `crates/slots`), so
        // it presents exactly as an unsupervised thread and cannot alias a fresh one.
        let t = sched.threads.get(tid);
        let fault_ep = t.and_then(|t| t.fault_ep);
        let dead = t.is_some_and(|t| t.handshake.state == State::Dead);
        match capability::reap_decision(fault_ep, ep, dead) {
            capability::Reap::NotSupervised => return Err(abi::Error::NotSupervised),
            capability::Reap::StillAlive => return Err(abi::Error::StillAlive),
            capability::Reap::Permitted => {}
        }
        // A supervised thread is always one `create_thread_control_block` built out of a region, so this is `Some`;
        // be honest rather than unwrap, and report "nothing here to collect" if it ever is not.
        t.and_then(|t| t.thread_control_block_region)
            .ok_or(abi::Error::NotSupervised)?
    };
    // `NotPermitted` for the same reasons `DESTROY` gives it: the child `SPLIT` its own budget and a
    // child region still owns part of the run, or a racing reap got there first and the name is now
    // stale. A restart policy reads it as "not yet", which is what it means.
    reclaim_region(region).map_err(|_| abi::Error::NotPermitted)
}

/// **Read one entry of the domain a supervision rendezvous supervises** (milestone 126,
/// `rendezvous::SURVEY`). Returns `(next_cursor, tid, word)`, where `word` is the fact `record`
/// selected; a `next_cursor` of `abi::survey::DONE` means the walk is finished and the other two
/// words are 0.
///
/// **`record` is a selector over per-thread facts**, calef's 2026-09-21 ruling: a new fact is a new
/// `abi::survey::record` value rather than a fourth return register, because a register row that
/// must be redesigned at the sixth field is the wrong mechanism at the fourth. The cursor and the
/// tid are the same for every record, so only the third word moves, and a caller wanting two facts
/// walks the domain twice and joins on the tid.
///
/// **An unknown record is refused before the walk**, so a bad selector against an empty domain is a
/// refusal rather than a `DONE` that a reader would print as "nothing here". A plausible wrong
/// answer is worse than an error.
///
/// **The domain is the one level of supervision the kernel already maintains**, so there is no registry
/// to keep in step with reality and no way for the view to disagree with it. Membership is
/// `capability::survey_includes`, which is the same relationship `reap_supervised` authorizes with
/// and is proved in that crate; a thread appears here exactly when its `Thread::fault_ep` *is* the
/// invoked rendezvous. Nothing about the caller's own identity is consulted, because holding the
/// rendezvous with `READ` is the whole of the claim (the rights check is the syscall layer's).
///
/// **One entry per call, and the lock is given back between them.** A survey of a domain with a
/// hundred children would otherwise hold `IPC_TABLES` for a hundred children's worth of work at a
/// userspace program's discretion, which is a scheduler-latency hole a program could open on
/// purpose. The cost is that the survey is a sequence of snapshots rather than one; see the `BUGS`
/// section of notes/process-view.md, which states exactly what that does and does not promise.
pub fn survey_supervised(
    ep: RendezvousId,
    cursor: u64,
    record: u64,
) -> Result<(u64, u64, u64), abi::Error> {
    // Before anything else, including before the lock: a record this kernel does not answer is
    // `BadMethod`, because the selector is part of the method's name. Doing it here rather than at
    // the point of extraction is what makes the refusal independent of whether the domain happens
    // to have a member to extract from.
    if !abi::survey::record::is_known(record) {
        return Err(abi::Error::BadMethod);
    }
    let guard = IPC_TABLES.lock();
    // Before IPC_TABLES exists there is no domain to report, which is "nothing here", not a
    // refusal: the caller's authority was never in question.
    let Some(sched) = guard.as_ref() else {
        return Ok((abi::survey::DONE, 0, 0));
    };
    // A cursor past the table is empty rather than an error (`iter_from`'s contract), so a caller
    // that keeps feeding back what it was given cannot walk off the end into a refusal that would
    // read as "you may not look".
    let from = usize::try_from(cursor).unwrap_or(usize::MAX);
    for (slot, t) in sched.threads.iter_from(from) {
        if capability::survey_includes(t.fault_ep, ep) {
            let word = match record {
                abi::survey::record::STATE => survey_state(t.handshake.state),
                abi::survey::record::PLACEMENT => survey_placement(t.placement),
                abi::survey::record::CPU_TIME => survey_cpu_time(slot),
                // `is_known` refused every other value above, before the walk began. This arm is
                // not dead defensiveness: it is what makes adding a record a loud two-line edit
                // (the constant, and an arm here) instead of a record that silently reports zero
                // because somebody widened `is_known` and stopped.
                _ => return Err(abi::Error::BadMethod),
            };
            return Ok((slot as u64 + 1, t.id, word));
        }
    }
    Ok((abi::survey::DONE, 0, 0))
}

/// The CPU time a survey reports, as an `abi::survey::record::CPU_TIME` word: **milliseconds**.
///
/// Keyed by the walk's own slot rather than by the tid, because the walk already holds it and the
/// two are the same number (`generational_table` packs the slot in a name's low word). Under
/// `IPC_TABLES` with the thread in hand, so the slot is live and its counter is this thread's.
///
/// **Milliseconds are converted here so the unit never leaves the kernel as a tick.** A tick count
/// would make every reader depend on `TICK_HZ`, and would change meaning underneath them the day
/// that constant moved; the conversion costs one multiply and one divide, on a path that already
/// crossed a syscall boundary and took a lock. All three architectures tick at 100 Hz today, so
/// the answer advances in steps of ten.
pub(super) fn survey_cpu_time(slot: usize) -> u64 {
    let ticks = CPU_TICKS.get(slot).map_or(0, |c| c.load(Ordering::Relaxed));
    // `TICK_HZ` is 100 everywhere, so this is `ticks * 10` and cannot overflow a `u64` short of
    // 58 million years of continuous CPU time.
    ticks * 1000 / crate::arch::timer::TICK_HZ
}

/// The placement a survey reports, as an `abi::survey::record::PLACEMENT` word.
///
/// Widens a cpu id, and turns the kernel's `u8::MAX` "not placed" into `record::NO_CPU`, which is
/// `u64::MAX`. The two sentinels are deliberately not the same number: widening `u8::MAX` would
/// hand userspace **255**, which is a perfectly plausible cpu id for a reader to tally, and this
/// tree's ruling on a counter it could not trust was that a wrong number is worse than no number.
///
/// Unreachable in practice, and mapped anyway rather than guessed at: a supervision endpoint is
/// recorded at `START` (DECISIONS §26 (the fault endpoint: thread death becomes a message a
/// supervisor holds)), so an embryo is not yet in any domain, and a corpse keeps
/// the placement it started with.
pub(super) const fn survey_placement(placement: u8) -> u64 {
    if placement == u8::MAX {
        abi::survey::record::NO_CPU
    } else {
        placement as u64
    }
}

/// The run state a survey reports, as an `abi::survey` code.
///
/// `Embryo` and `Finished` are unreachable for a supervised thread and are mapped anyway rather
/// than left to a panic or a wildcard: supervision is recorded at `START`, so an embryo has no
/// `fault_ep` to match, and a supervised death goes to `Dead` rather than `Finished`. Reporting
/// them as `READY` and `DEAD` is the honest nearest neighbour if either ever becomes reachable,
/// and the `match` is exhaustive so a seventh state cannot be added without meeting this.
pub(super) const fn survey_state(state: State) -> u64 {
    match state {
        State::Embryo | State::Ready => abi::survey::READY,
        State::Running => abi::survey::RUNNING,
        State::Blocked => abi::survey::BLOCKED,
        State::Finished | State::Dead => abi::survey::DEAD,
    }
}

#[cfg(test)]
mod tests {
    //! Tests for reclaiming a region: an embryo's thread control block, an unbound address space,
    //! split pages, slot reuse and an idle rendezvous. Moved from `sched`'s own tests by milestone
    //! 840 (the scheduler file is split along its seams), bodies unchanged.

    /// **Object revocation reclaims a region holding an unstarted TCB** (the smallest proof of the
    /// mechanism). Retype a bare embryo into a fresh region, then `reclaim_region`: the TCB is torn
    /// down (its table slot freed, its generational name dead), the region's memory returns, and the
    /// free-frame count lands exactly where it began. No scheduler run, no address space, no reaper
    /// timing: find the object, kill it, unpin, free. The larger cases (a started-then-exited
    /// thread, its address space, the spawn-to-reap loop) build on this one.
    #[test_case]
    fn reclaim_frees_an_embryo_thread_control_blocks_region() {
        let region = crate::memory_region::create(2).expect("a fresh 2-page region");
        // The region's own frames, by name. A machine-wide free count would have this test
        // asserting that nothing else in the kernel allocated or freed while it ran; these two
        // frames are the property itself. See `testing::RegionRun`.
        let run = crate::testing::RegionRun::of(region);
        let tid = crate::sched::create_thread_control_block(region)
            .expect("retype a TCB from the region");

        // The embryo is named, not counted. This pair used to bracket `thread_count()` against a
        // baseline taken above (`threads_before + 1`, then `threads_before`), which is the reaper
        // count's defect in a different test: the headcount is the size of the whole table, so a
        // neighbouring thread exiting between the two reads lands the count BELOW what the
        // assertion demands and blames this embryo for it. `is_thread_present` on the ThreadId this
        // test created is immune by construction, and it is strictly the stronger claim: the old
        // "the TCB's table slot must be freed" could pass with the embryo still in the table, as
        // long as somebody else's thread left in the same window. Fourth appearance of this fix;
        // see notes/load-sensitive-assertions.md and `is_thread_present`'s own doc comment.
        assert!(
            crate::sched::is_thread_present(tid),
            "the embryo should be in the table before reclaim"
        );
        run.assert_held("the live region's pages should still be spent");

        crate::sched::reclaim_region(region)
            .expect("reclaim a region whose only object is an unstarted TCB");

        assert!(
            !crate::sched::is_thread_present(tid),
            "the TCB's table slot must be freed by reclaim"
        );
        run.assert_returned("reclaim must return every one of the region's own frames");
    }

    /// **Object revocation reclaims a region holding an unbound address space** (the address-space
    /// case of piece 1's mechanism). Create a space in its own region, not bound to any TCB, then
    /// reclaim: the space is torn down (its name goes stale, its ASID is freed by `Drop`) and the
    /// region's memory returns exactly to baseline. This is what retires the "an unbound space
    /// leaks" note the registry carried since 19b.
    #[test_case]
    fn reclaim_frees_an_unbound_address_spaces_region() {
        let region = crate::memory_region::create(8).expect("a fresh region");
        let run = crate::testing::RegionRun::of(region); // this region's frames, not the machine's
        let name = crate::user::user_address_space_create(region)
            .expect("an address space from the region");

        assert!(
            crate::user::user_address_space_root(name).is_some(),
            "the space should resolve before reclaim"
        );
        run.assert_held("the live region's pages should still be spent");

        crate::sched::reclaim_region(region).expect("reclaim the space's own region");

        assert!(
            crate::user::user_address_space_root(name).is_none(),
            "the space's name must be stale after reclaim"
        );
        run.assert_returned("reclaim must return every one of the region's own frames");
    }

    /// **`MemoryRegion` SPLIT returns a child's pages to the parent on reclaim (LIFO), so a split parent is
    /// not committed for its lifetime.** Carve a region into two children; the parent refuses reclaim
    /// while they live. Reclaiming a child out of order (not the top of the watermark) leaves a hole;
    /// reclaiming the top child un-bumps the parent, so its budget is re-splittable. Either way a
    /// child's pages go back to the *parent*, not the allocator, so the free-frame count does not move
    /// until the parent itself, now childless, is destroyed.
    #[test_case]
    fn split_returns_child_pages_to_the_parent() {
        let parent = crate::memory_region::create(8).expect("parent region");
        // The parent is a root, so its eight frames are the allocator's own bits: `of` asserting
        // they are used is "create spent the parent's pages", and it names which pages rather than
        // counting the machine's. A child's pages never reach the allocator at all, which is what
        // the `assert_held` calls below say.
        let run = crate::testing::RegionRun::of(parent);

        let child_a = crate::memory_region::split(parent, 4).expect("split child a"); // [0,4)
        let child_b = crate::memory_region::split(parent, 4).expect("split child b"); // [4,8), the top
        assert_ne!(child_a, child_b);
        assert!(crate::memory_region::has_children(parent));
        assert!(
            crate::sched::reclaim_region(parent).is_err(),
            "a parent with live children refuses reclaim",
        );
        assert!(
            crate::memory_region::split(parent, 1).is_none(),
            "a fully-carved parent cannot split further",
        );

        // Reclaim out of order (child_a is not the top): a hole, its pages returned to the parent,
        // nothing to the allocator.
        crate::sched::reclaim_region(child_a).expect("reclaim child a (leaves a hole)");
        run.assert_held("a reclaimed child returns pages to the parent, not the allocator");
        assert!(
            crate::memory_region::has_children(parent),
            "one child still lives"
        );

        // Reclaim the top child: the parent un-bumps and is childless, and its budget re-splits.
        crate::sched::reclaim_region(child_b).expect("reclaim child b (the LIFO top)");
        assert!(
            !crate::memory_region::has_children(parent),
            "no children remain"
        );
        let child_c =
            crate::memory_region::split(parent, 4).expect("the LIFO-returned pages re-split");
        crate::sched::reclaim_region(child_c).expect("reclaim child c");

        // Nothing reached the allocator until now: destroying the childless root parent frees the
        // whole run, the hole included, exactly once.
        crate::sched::reclaim_region(parent).expect("destroy the now-childless root parent");
        run.assert_returned("the root parent's pages return to the allocator");
    }

    /// **A destroyed region's table slot is reused** (generational regions). Create and destroy a
    /// region far more times than the table has slots: without reuse the 257th `create` would fail
    /// with the table full, the lifetime cap that made a long-running system untenable. With reuse
    /// each `destroy` frees the slot, so one free slot serves the whole loop, and the free-frame
    /// count nets to zero every iteration. This is the property that lets the kernel run workloads
    /// that come and go without end.
    #[test_case]
    fn destroyed_region_slots_are_reused() {
        // Comfortably more than MAX_REGIONS (256): without reuse this exhausts the table well before
        // the end. With reuse, one freed slot serves every iteration.
        //
        // The frame half of the claim is asked per iteration and about that iteration's own page,
        // which is both narrower than the old machine-wide delta and stricter: a leak of one page
        // in one round fails on the round that leaked it, rather than being netted out by another
        // test freeing a page somewhere in the 320.
        for _ in 0..320 {
            let r = crate::memory_region::create(1)
                .expect("a region slot must be reused, not exhausted");
            let run = crate::testing::RegionRun::of(r);
            crate::memory_region::destroy(r);
            // No round number in the message: the frame's own address is in it, and that says
            // which iteration far more usefully than a counter would.
            run.assert_returned("destroying a region did not return its page");
        }
    }

    /// **Object revocation reclaims a region holding an idle rendezvous.** An rendezvous nobody is
    /// blocked on is torn down with its region: removed from the registry (its name goes stale, so
    /// every Rendezvous capability to it fails), and its page returned. Frames back to baseline.
    #[test_case]
    fn reclaim_frees_a_regions_idle_rendezvous() {
        let region = crate::memory_region::create(2).expect("region");
        let run = crate::testing::RegionRun::of(region); // this region's frames, not the machine's
        let _ep = crate::sched::create_rendezvous_from(region).expect("rendezvous from region");
        run.assert_held("the live region's pages should still be spent");
        crate::sched::reclaim_region(region)
            .expect("reclaim a region with only an idle rendezvous");
        run.assert_returned("the idle rendezvous's region must give back every frame it held");
    }
}

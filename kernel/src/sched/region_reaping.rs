//! **Region reaping**: what a region's destruction does to every thread, rendezvous, notification
//! and timer it pinned, `reclaim_region`, and the supervised reap and survey built on them (§271
//! part 3).
//!
//! Cut out of `sched.rs` unchanged by milestone 840 (the scheduler file is split along its seams).
//!
//! Name: provisional (milestone 840's lane, 2026-10-11 UTC; §271 (the scheduler file splits into
//! submodules of `sched`)).

use super::*;

/// What the refuse phase of [`reap_region_objects`] decides about one resident thread. See
/// notes/region-reaping.md ("The verdict on one resident").
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
    /// It is `Blocked`, and an armed kill would never reach it: unlink it from whatever queue holds
    /// it, sweep the `Reply` capabilities that name it, and write it to `Finished` without letting
    /// it run again ([`finish_blocked_resident`]). Named for what the pass does, because
    /// [`RegionReap::RefuseAndArm`] is not enough for a `Blocked` thread and nothing at the arm
    /// shows that.
    FinishInPlace,
}

/// The refuse phase's rule for one resident.
///
/// `state` is the thread's run state. `standing` is `handshake.on_cpu` or'd with
/// [`being_reaped`](crate::thread::Thread::being_reaped): a core is standing on the thread's kernel
/// stack, or the reaper is freeing it outside `IPC_TABLES`. `state` says whether the thread can run
/// again and `standing` whether freeing it would unmap a stack in use; they are different
/// questions. Returns the verdict; the table and the reasons are in notes/region-reaping.md ("The
/// verdict on one resident"), and the stack fault behind `standing` in
/// notes/stack/kernel-stack-freed-under-its-owner.md.
pub(super) fn region_reap_verdict(state: State, standing: bool) -> RegionReap {
    if matches!(state, State::Ready | State::Running) {
        RegionReap::RefuseAndArm
    } else if state == State::Blocked && !standing {
        // An armed kill is spent only for a `Running` thread, which this one never becomes.
        RegionReap::FinishInPlace
    } else if standing {
        // Includes a `Blocked` thread still on its core: it is mid-switch-out, its saved context
        // is stale, and ending it would free the stack that core stands on.
        RegionReap::RefuseStanding
    } else {
        RegionReap::Reap
    }
}

/// End one permanently blocked resident of a region being destroyed: unlink it from whatever queue
/// holds it, delete every outstanding `Reply` capability that names it, and write it to `Finished`
/// without waking it.
///
/// `tid` must be `Blocked` with its wait recorded in `handshake.wait_on`. Waking it would let a
/// stale `Reply` forge an answer to its next `CALL`. See notes/region-reaping.md ("Ending a blocked
/// resident") and notes/blocked-thread-teardown.md.
///
/// Context: caller holds `IPC_TABLES`.
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
                // `ptr` is only compared. The token goes home to the thread, to be freed with it.
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
        // Linked on the notification's queue, whose page may outlive this region.
        Some(Wait::Notification(id)) => {
            let ptr = thread_control_block_ptr(sched, tid);
            if let Some(page) = notification_of(sched, id)
                && let Some(token) = page.state.remove_waiter(ptr)
            {
                hold_token(token);
            }
        }
        // On the futex table under its key, where a later `WAKE` would walk a freed TCB.
        Some(Wait::Futex(key)) => {
            let ptr = thread_control_block_ptr(sched, tid);
            if let Some(token) = sched.futexes.remove(key, ptr) {
                hold_token(token);
            }
        }
        // A suspended thread (§300 (the paused state, a TCB verb, silent)) is on no queue and
        // already holds its token.
        Some(Wait::Resume) | None => {}
    }

    // The victim is never woken, but a `Reply` outliving it would go stale only on its next
    // use, through a slot generation `ipc_reply` cannot see.
    delete_reply_caps_naming(sched, tid);

    // `killed` marks a thread this teardown doomed. `wait_on` is cleared because a hang dump reads
    // `Some` as still waiting.
    if let Some(t) = sched.threads.get_mut(tid) {
        t.killed = true;
        t.handshake.state = State::Finished;
        t.handshake.wait_on = None;
    }
}

/// Tear down every kernel object whose backing page lies in `[base, end)`, so
/// `memory_region::destroy` can reclaim the region (§16 (object revocation)).
///
/// Removes the region's rendezvous, notifications, timers and processes on every pass and ends each
/// `Blocked` resident in place. `Err`, with the region still pinned, if a resident can still run
/// (its kill is armed) or stands on its kernel stack; otherwise every resident is removed. See
/// notes/region-reaping.md ("The phases of `reap_region_objects`").
///
/// Context: takes `IPC_TABLES`, so it must run outside any teardown `Drop`.
pub(super) fn reap_region_objects(base: u64, end: u64) -> Result<(), ()> {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        record_refusal(ReclaimRefusal::NoScheduler);
        return Err(());
    };
    // A TCB sits at the start of its page, so the thread pointer translated back is its page.
    let page_of = |t: &Thread| crate::arch::mmu::virt_to_phys(t as *const Thread as u64);

    // Rendezvous phase, first and refusal or not: draining a rendezvous wakes its waiters, which
    // lets a resident parked in `RECEIVE` here become schedulable and spend its kill. Every phase
    // that removes rescans for one victim at a time, because a list of victims would be a scratch
    // array on one of the kernel's deepest frames (notes/stack-high-water.md).
    loop {
        let doomed = sched
            .rendezvous_table
            .iter()
            .find(|&(_, &phys)| base <= phys && phys < end)
            .map(|(name, _)| name);
        let Some(name) = doomed else { break };

        // Wake inside the drain, not from a list. `drain_waiters` pops each entry before calling
        // back, so its intrusive link is free for `wake`, and `rendezvous_of` returns a `'static`
        // reference that does not hold the `sched` borrow.
        if let Some(rendezvous) = rendezvous_of(sched, name) {
            rendezvous.drain_waiters(|w| {
                let tid = hold_token(w);
                set_ipc_aborted(sched, tid);
                wake(sched, tid);
            });
        }
        sched.rendezvous_table.remove(name);
        // A `CALL` whose request was taken has left the sender queue, so the drain misses it;
        // only its `wait_on` names this rendezvous.
        strand_callers_awaiting(sched, name);
    }

    // Notification phase, before the finish phase for the rendezvous phase's reason: a waiter
    // here is aborted and woken.
    reap_region_notifications(sched, base, end);
    // An armed timer has no waiter to abort; it just never fires.
    reap_region_timers(sched, base, end);
    // Processes: ended, embryo members detached, forgotten once empty.
    process::reap_region(sched, base, end);

    // Finish phase: end each resident blocked on a rendezvous outside the region, which an armed
    // kill would never reach. Each pass writes one to `Finished`, so the rescan terminates.
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

    // Refuse phase: arm the kill on every resident that can still run, and refuse without arming
    // while one stands on its own kernel stack. `Dead`, `Embryo` and `Finished` are reapable.
    let mut live = false;
    let mut standing = false;
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
            // Unreachable after the finish phase. A conservative refusal costs the owner one
            // retry, where an `unreachable!` would panic on a teardown path.
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
    // Removal phase: every resident is `Embryo`, `Finished` or `Dead`. Rescan, one at a time.
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
        // A supervised corpse nobody received from is parked on its supervisor's rendezvous,
        // outside this region (§26 (the fault endpoint) implementation note 2). Unlink it before
        // freeing the TCB, or the supervisor's next `RECEIVE` follows a pointer into a recycled
        // page.
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
        // Free the callers named by reply capabilities this thread collected; freeing the TCB
        // would make them unreachable.
        strand_callers_of(sched, tid);
        let _ = process::leave(sched, tid); // its process's space falls to the registry sweep
        sched.threads.remove(tid);
    }

    process::forget_empty(sched, base, end)
}

/// Why [`reclaim_region`] last refused, for test logs: a refusal reaches userspace as one
/// `NotPermitted` whatever its cause.
///
/// Each refusing check overwrites it and nothing clears it, so read it right after the refusal it
/// explains. For a resident it records the last one the refuse phase turned away, with the two
/// flags that make up `standing`. See notes/region-reaping.md ("The supervised reap").
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

/// Reclaim an untyped region and every object retyped from it (§16 (object revocation)).
///
/// Tears down the objects in `region`, reaps the address spaces it ends, unpins it and returns its
/// memory. `Err` if it has children, its name is stale or a resident refuses
/// (`last_reclaim_refusal` says which). See notes/region-reaping.md ("The order of a reclaim").
///
/// Context: takes `IPC_TABLES`, so it must run outside any `Drop`.
///
/// # BUGS
/// `Err` and `Ok` both end what they find, and it reaches outside the region (the note's BUGS).
pub fn reclaim_region(region: u64) -> Result<(), ()> {
    // Refuse before any teardown, so a refused reclaim leaves the region as it was.
    if crate::memory_region::has_children(region) {
        record_refusal(ReclaimRefusal::HasChildren);
        return Err(());
    }
    let Some((base, size)) = crate::memory_region::region_bounds(region) else {
        record_refusal(ReclaimRefusal::NoRegion);
        return Err(());
    };
    // Threads first (IPC_TABLES), then the address spaces (the registry's lock, which takes
    // IPC_TABLES beneath it).
    reap_region_objects(base, base + size)?;
    crate::user::reap_address_spaces_in_region(base, base + size);
    crate::memory_region::unpin(region);
    crate::memory_region::destroy(region);
    Ok(())
}

/// Collect a corpse a supervision rendezvous supervises (§32 (a supervisor may collect a
/// corpse without being able to build one), `rendezvous::REAP`).
///
/// `ep` is the invoked rendezvous and `tid` the id on the death message. Authorized when the
/// thread's `fault_ep` is `ep` (`capability::reap_decision`), it reclaims the region the TCB came
/// from. Returns `NotSupervised`, `StillAlive`, or `NotPermitted` when the reclaim refuses. See
/// notes/region-reaping.md ("The supervised reap").
///
/// Context: takes and releases `IPC_TABLES`, yielding while the corpse is still on its core.
pub fn reap_supervised(ep: RendezvousId, tid: ThreadId) -> Result<(), abi::Error> {
    let region = loop {
        let guard = IPC_TABLES.lock();
        let sched = guard.as_ref().ok_or(abi::Error::NotSupervised)?;
        // A stale or recycled tid resolves to `None`, so it reads as unsupervised.
        let t = sched.threads.get(tid);
        let fault_ep = t.and_then(|t| t.fault_ep);
        let dead = t.is_some_and(|t| t.handshake.state == State::Dead);
        match capability::reap_decision(fault_ep, ep, dead) {
            capability::Reap::NotSupervised => return Err(abi::Error::NotSupervised),
            capability::Reap::StillAlive => return Err(abi::Error::StillAlive),
            capability::Reap::Permitted => {}
        }
        // A corpse whose core still stands on its stack clears at that core's next
        // `finish_switch`; refusing would reach userspace as `NotPermitted`. Yield the lock and
        // look again.
        if t.is_some_and(|t| t.handshake.on_cpu) {
            drop(guard);
            yield_now();
            continue;
        }
        // A supervised thread is always retyped from a region, so this is `Some`.
        break t
            .and_then(|t| t.thread_control_block_region)
            .ok_or(abi::Error::NotSupervised)?;
    };
    // A restart policy reads `NotPermitted` as "not yet".
    reclaim_region(region).map_err(|_| abi::Error::NotPermitted)
}

/// Read one entry of the domain a supervision rendezvous supervises (`rendezvous::SURVEY`).
///
/// `ep` is the invoked rendezvous, `cursor` the value the last call returned (0 to start) and
/// `record` an `abi::survey::record` selector. Returns `(next_cursor, tid, word)`, where `word` is
/// the selected fact; a `next_cursor` of `abi::survey::DONE` ends the walk and the other words are
/// 0. A thread is in the domain when its `fault_ep` is `ep` (`capability::survey_includes`). An
/// unknown `record` is `BadMethod`, before the walk. See notes/process-view.md and
/// notes/region-reaping.md ("The survey").
///
/// Context: takes `IPC_TABLES` for one entry and releases it before returning.
pub fn survey_supervised(
    ep: RendezvousId,
    cursor: u64,
    record: u64,
) -> Result<(u64, u64, u64), abi::Error> {
    // Before the lock, so the refusal does not depend on whether the domain has a member.
    if !abi::survey::record::is_known(record) {
        return Err(abi::Error::BadMethod);
    }
    let guard = IPC_TABLES.lock();
    // No scheduler means no domain: "nothing here", not a refusal.
    let Some(sched) = guard.as_ref() else {
        return Ok((abi::survey::DONE, 0, 0));
    };
    // A cursor past the table is empty, not an error (`iter_from`'s contract).
    let from = usize::try_from(cursor).unwrap_or(usize::MAX);
    for (slot, t) in sched.threads.iter_from(from) {
        if capability::survey_includes(t.fault_ep, ep) {
            let word = match record {
                abi::survey::record::STATE => survey_state(t.handshake.state),
                abi::survey::record::PLACEMENT => survey_placement(t.placement),
                abi::survey::record::CPU_TIME => survey_cpu_time(slot),
                // Refused by `is_known` above. Kept so a record added there without an arm
                // here fails loudly instead of reporting zero.
                _ => return Err(abi::Error::BadMethod),
            };
            return Ok((slot as u64 + 1, t.id, word));
        }
    }
    Ok((abi::survey::DONE, 0, 0))
}

/// The CPU time a survey reports for the thread in `slot`, as an
/// `abi::survey::record::CPU_TIME` word in milliseconds.
///
/// The slot is the walk's own and equals the tid's low word. Milliseconds keep `TICK_HZ` out of
/// the ABI; at 100 Hz the value moves in steps of ten.
///
/// Context: called under `IPC_TABLES` with the thread in hand, so the slot is live.
pub(super) fn survey_cpu_time(slot: usize) -> u64 {
    let ticks = CPU_TICKS.get(slot).map_or(0, |c| c.load(Ordering::Relaxed));
    // Multiplied first; overflows only past 1.8e16 ticks, 5.8 million years of CPU at 100 Hz.
    ticks * 1000 / crate::arch::timer::TICK_HZ
}

/// The placement a survey reports, as an `abi::survey::record::PLACEMENT` word.
///
/// Widens a CPU id, and maps the kernel's `u8::MAX` "not placed" to `record::NO_CPU`
/// (`u64::MAX`), because 255 is a plausible CPU id for a reader to tally.
pub(super) const fn survey_placement(placement: u8) -> u64 {
    if placement == u8::MAX {
        abi::survey::record::NO_CPU
    } else {
        placement as u64
    }
}

/// The run state a survey reports, as an `abi::survey` code.
///
/// `Embryo` and `Finished` cannot be supervised (supervision is recorded at `START`, and a
/// supervised death is `Dead`); they map to their nearest neighbors, `READY` and `DEAD`.
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
    //! split pages, slot reuse and an idle rendezvous.

    /// Object revocation reclaims a region holding an unstarted TCB, the smallest proof of the
    /// mechanism: the TCB's slot is freed, its name dies, and the region's frames return.
    #[test_case]
    fn reclaim_frees_an_embryo_thread_control_blocks_region() {
        let region = crate::memory_region::create(2).expect("a fresh 2-page region");
        // The region's own frames, by name, not a machine-wide free count (`testing::RegionRun`).
        let run = crate::testing::RegionRun::of(region);
        let tid = crate::sched::create_thread_control_block(region)
            .expect("retype a TCB from the region");

        // Named, not counted: a headcount races neighboring threads. See
        // notes/load-sensitive-assertions.md.
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

    /// Object revocation reclaims a region holding an unbound address space: the space's name
    /// goes stale, its ASID is freed by `Drop`, and the region's frames return.
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

    /// **`MemoryRegion` SPLIT returns a child's pages to the parent on reclaim (LIFO), so a split
    /// parent is not committed for its lifetime.** Carve a region into two children; the parent
    /// refuses reclaim while they live. Reclaiming a child out of order (not the top of the
    /// watermark) leaves a hole; reclaiming the top child un-bumps the parent, so its budget is
    /// re-splittable. Either way a child's pages go back to the *parent*, not the allocator, so the
    /// free-frame count does not move until the parent itself, now childless, is destroyed.
    #[test_case]
    fn split_returns_child_pages_to_the_parent() {
        let parent = crate::memory_region::create(8).expect("parent region");
        // A child's pages return to the parent, never the allocator; only the root parent's
        // destroy frees the run.
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

    /// A destroyed region's table slot is reused: create and destroy more regions than the table
    /// has slots, and each round returns its own page.
    #[test_case]
    fn destroyed_region_slots_are_reused() {
        // More than MAX_REGIONS (256), and the frame is checked per round, so a leak fails on the
        // round that leaked.
        for _ in 0..320 {
            let r = crate::memory_region::create(1)
                .expect("a region slot must be reused, not exhausted");
            let run = crate::testing::RegionRun::of(r);
            crate::memory_region::destroy(r);
            // No round number: the frame's address is in the message.
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

//! **Reading the scheduler's state from outside it**: the soak counters, the accessors other kernel
//! modules call, the hooks `system_tests` drives, and `dump_threads`.
//!
//! Cut out of `sched.rs` unchanged by milestone 840 (the scheduler file is split along its seams).
//!
//! Name: provisional (milestone 840's lane, 2026-10-11 UTC; §271 (the scheduler file splits into
//! submodules of `sched`)).

use super::*;
#[cfg(feature = "system_tests")]
use crate::cap::Object;

/// **Scheduler anomaly and activity totals, for a run long enough that a sixteen-event ring is no
/// use** (milestone 219). Every one is a sum over the per-core counters `trace::record` keeps; see
/// [`trace::counted`] for why the sum is racy and why that is fine.
///
/// These are the kernel's numbers, read by the kernel's soak supervisor. They are deliberately not
/// reachable from userspace: a workload that could read its own tripwire is one step from a
/// workload that could clear it.
///
/// Names provisional (this lane's, 2026-09-01; public function names are an architect's call,
/// DECISIONS naming tenet as extended on 2026-08-23).
///
/// **The one that is a finding rather than a statistic.** A refused wake means a waker made a
/// parked receiver `Ready` without delivering anything, and the gate stopped it. It has never
/// fired in the field (see `thread_wake_handshake`'s crate doc, which is honest that the boot-8
/// reading it was built against was later overturned), so a nonzero here on a board is the single
/// most interesting number this kernel can produce.
// Soak builds only (milestone 219). These were `allow(dead_code)` and compiled everywhere, on the
// argument that a function invisible to clippy rots; the measurement overruled it. The counters
// they read are `cfg`-gated because their increments are on the IPC fastpath, so an accessor
// compiled without them would not build either. `script/lint` clippies `--features soak_test` on both
// ISAs, which is what keeps them seen.
#[cfg(feature = "soak_test")]
pub(crate) fn wake_refusals() -> u64 {
    trace::counted(trace::Event::WakeRefused)
}

/// How many wakes were parked because the target was still standing on a CPU. Ordinary, and
/// expected to be nonzero under load; a soak reports it so that "the machine was genuinely
/// contended" is a number rather than an assurance.
// Soak builds only (milestone 219). These were `allow(dead_code)` and compiled everywhere, on the
// argument that a function invisible to clippy rots; the measurement overruled it. The counters
// they read are `cfg`-gated because their increments are on the IPC fastpath, so an accessor
// compiled without them would not build either. `script/lint` clippies `--features soak_test` on both
// ISAs, which is what keeps them seen.
#[cfg(feature = "soak_test")]
pub(crate) fn wakes_deferred() -> u64 {
    trace::counted(trace::Event::WakeDeferred)
}

/// How many wakes and placements landed a thread on a core other than the waker's. **This is the
/// number that says the workload actually crossed cores**, which is the whole premise of a
/// multicore soak; a run reporting zero here soaked one core very thoroughly and proved nothing
/// about the others.
// Soak builds only (milestone 219). These were `allow(dead_code)` and compiled everywhere, on the
// argument that a function invisible to clippy rots; the measurement overruled it. The counters
// they read are `cfg`-gated because their increments are on the IPC fastpath, so an accessor
// compiled without them would not build either. `script/lint` clippies `--features soak_test` on both
// ISAs, which is what keeps them seen.
#[cfg(feature = "soak_test")]
pub(crate) fn remote_placements() -> u64 {
    trace::counted(trace::Event::PlaceRemote)
}

/// **How many times a thread ran on a different core than the one it last ran on.**
///
/// The cross-core handoff number, and the one a soak reports. See [`crate::thread::Thread::last_cpu`] for
/// why [`remote_placements`] could not be it: a rendezvous wake queues its peer on the waker's own
/// core (DECISIONS §28.2), so the placement is local even though the thread has moved.
// Soak builds only (milestone 219). These were `allow(dead_code)` and compiled everywhere, on the
// argument that a function invisible to clippy rots; the measurement overruled it. The counters
// they read are `cfg`-gated because their increments are on the IPC fastpath, so an accessor
// compiled without them would not build either. `script/lint` clippies `--features soak_test` on both
// ISAs, which is what keeps them seen.
#[cfg(feature = "soak_test")]
pub(crate) fn migrations() -> u64 {
    trace::counted(trace::Event::Migrated)
}

/// How many threads this machine handed from one core's run queue to another's on request. The
/// work-steal protocol's activity level, and the second of the two cross-core paths a soak is for.
// Soak builds only (milestone 219). These were `allow(dead_code)` and compiled everywhere, on the
// argument that a function invisible to clippy rots; the measurement overruled it. The counters
// they read are `cfg`-gated because their increments are on the IPC fastpath, so an accessor
// compiled without them would not build either. `script/lint` clippies `--features soak_test` on both
// ISAs, which is what keeps them seen.
#[cfg(feature = "soak_test")]
pub(crate) fn steals_served() -> u64 {
    trace::counted(trace::Event::StealServe)
}

/// **Spawn a thread exactly as [`spawn`] would, and say which core it landed on** (milestone 240).
///
/// The placement is [`pick_spawn_target`]'s, made here in the same call `spawn` makes it in, so
/// this is `spawn` with the answer kept rather than a second placement policy. **It hands the
/// caller no lever**: there is no argument to steer the choice with, which is deliberate, because
/// the thing milestone 240 is for is reporting the boot-time placement lottery and not overriding
/// it. Thread affinity is an architect's open question and DECISIONS 138 (how a saturated workload
/// is made to hand threads across cores) declined a rebalancer; neither is reopened by a caller
/// that can only read.
///
/// Soak and job-mix builds only, for the same reason the counters above are: nothing else needs
/// it, and a function nothing calls is a function that rots. Milestone 168's sweep is the second
/// caller and it is here for the first one's exact reason, that a workload number taken on real
/// silicon is uninterpretable without the arrangement that produced it.
///
/// Name: provisional (milestone 240 (the soak reports what happened and not where)): calef names
/// public items.
#[cfg(any(feature = "soak_test", feature = "job_mix"))]
pub fn spawn_reporting_placement<F: FnOnce() + Send + 'static>(f: F) -> Option<(ThreadId, usize)> {
    let target = pick_spawn_target();
    spawn_on(target, f).map(|id| (id, target))
}

/// **Where each of `ids` last ran**, written into `out`, or `u8::MAX` for a thread that has not run
/// yet or is gone (milestone 240).
///
/// One lock acquisition for the whole set rather than one per thread, because the caller is a soak
/// heartbeat asking about every worker at once and `IPC_TABLES` is the lock the IPC fastpath takes:
/// twenty-four separate acquisitions once a beat would be twenty-four chances to sit in front of
/// the workload being measured.
///
/// `u8::MAX` means "no answer", and it deliberately means both of the two ways there can be none.
/// A thread that has never been switched to still carries `Thread::last_cpu`'s initial `u8::MAX`,
/// and a thread that has died is not in the table at all; a reader who needs to tell those apart
/// has [`dump_threads`], and a census that guessed between them would be inventing a placement.
///
/// Name: provisional (milestone 240): calef names public items.
#[cfg(feature = "soak_test")]
pub fn last_cpus(ids: &[ThreadId], out: &mut [u8]) {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        out.iter_mut().for_each(|slot| *slot = u8::MAX);
        return;
    };
    for (slot, &id) in out.iter_mut().zip(ids) {
        *slot = sched.threads.get(id).map_or(u8::MAX, |t| t.last_cpu);
    }
}

/// **The ticks charged to the running thread so far**, for a test that measures what a wait costs
/// (milestone 106's before-and-after). Read by the thread itself, because a finished thread's slot
/// may be reused before anyone else looks. Test-only: userspace reads the same counter in
/// milliseconds through `SURVEY`.
#[cfg(any(test, feature = "system_tests"))]
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the system tests call it; a unit-test boot on some ISAs does not
pub fn current_cpu_ticks() -> u64 {
    CPU_TICKS
        .get(slot_of(current_thread_id()))
        .map_or(0, |c| c.load(Ordering::Relaxed))
}

/// The top of the current thread's kernel stack: **where its `TrapFrame` belongs.**
///
/// `None` for the boot thread, which runs on the stack `boot.s` set up and does not own it.
///
/// A user thread's `TrapFrame` is not an ordinary local. It must sit at exactly the address the
/// vector table's `SAVE_CONTEXT` will rebuild it at when the user traps in, because `eret`
/// leaves `SP_EL1` pointing just past it and the hardware does not consult our intentions.
pub(crate) fn current_kernel_stack_top() -> Option<u64> {
    let guard = IPC_TABLES.lock();
    let sched = guard.as_ref()?;
    sched
        .threads
        .get(current_thread_id())?
        .stack
        .as_ref()
        .map(|s| s.top())
}

pub fn current() -> ThreadId {
    current_thread_id()
}

/// **The user PC recorded in `tid`'s trap frame** (milestone 71, test support), read from the top of
/// its kernel stack, which is the one address the trap path and the user-entry path must agree on.
/// `None` if the name does not resolve or the thread has no kernel stack of its own.
///
/// This is [`dump_threads`]'s per-thread PC lookup, exposed so a test can assert the agreement
/// rather than only a human reading a hang dump. A thread that has reached user mode reads back a
/// user address here; a zero means nothing wrote a frame where the trap path will look for one.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn user_pc_of(tid: ThreadId) -> Option<u64> {
    let guard = IPC_TABLES.lock();
    let sched = guard.as_ref()?;
    let t = sched.threads.get(tid)?;
    t.stack
        .as_ref()
        .map(|s| crate::arch::exceptions::user_pc(s.top()))
}

/// **Postmortem: read a corpse's retained fault/exit message** (milestone 22, test support). A
/// `Dead` thread keeps its five-word §26 message until the supervisor reaps it, so this proves the
/// corpse's TCB still holds its fault-time state after the notification was delivered. `None` if
/// the name does not resolve or the thread is not a corpse.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn corpse_fault_msg(tid: ThreadId) -> Option<[u64; 5]> {
    let guard = IPC_TABLES.lock();
    let sched = guard.as_ref()?;
    let t = sched.threads.get(tid)?;
    (t.handshake.state == State::Dead)
        .then_some(t.fault_msg)
        .flatten()
}

/// **Is `tid` still in the thread table?** (test support.)
///
/// The narrow question "was *this* thread reaped", which is what a test that spawned one actually
/// wants to know. [`thread_count`] answers a wider one, and the width is a defect when it is used
/// this way: the count is the size of the whole table, so an unrelated process finishing its
/// teardown moves it, and a test waiting for the count to come back to a baseline is really waiting
/// for the rest of the system to hold still. It need not.
///
/// The name is generational, so a reaped thread's `ThreadId` never resolves again even if its slot is
/// reused: `false` here means gone, not "gone or replaced".
/// **`tid`'s capability table, read in place** (test support, milestone 757 (a test
/// kernel fails a process on its Nth retype), provisional name). `None` for a name that does not
/// resolve. A sweep that fails a service's Nth retype reads this before and after each run, because
/// a cleanup path that forgets a `cap_delete` crashes nothing and shows up only here. It lends the
/// table to `read` rather than returning a copy: a copy is a 2 KiB array in every frame that holds
/// it, which at 64 slots put the caller over the guard page (milestone 754 (the capability table
/// grows to 64 slots)). `read` runs under `IPC_TABLES`, so it must not call back into `sched`.
#[cfg(feature = "system_tests")]
pub fn with_capability_table<R>(
    tid: ThreadId,
    read: impl FnOnce(&crate::cap::CapabilityTable) -> R,
) -> Option<R> {
    let guard = IPC_TABLES.lock();
    let t = guard.as_ref()?.threads.capabilities(tid)?.lock();
    Some(read(&t))
}

/// **The root of `tid`'s address space**, `None` for a kernel thread or a name that does not
/// resolve (test support, milestone 779 (fuzz the surface a confined process can reach),
/// provisional name). The confined fuzzer's page oracle walks a fuzzer's mappings from here with
/// `revoke::list_mapping` and `arch::mmu::translate_at`; no syscall hands a process's own root to
/// anybody, which is why the test reads it here.
#[cfg(feature = "system_tests")]
pub fn thread_space_root(tid: ThreadId) -> Option<u64> {
    let guard = IPC_TABLES.lock();
    guard
        .as_ref()?
        .threads
        .get(tid)?
        .space
        .as_ref()
        .map(|s| s.root())
}

/// **The physical page a page-resident kernel object lives in** (test support, milestone 779,
/// provisional name): a rendezvous, a notification or a TCB. `None` for any other object, and for a
/// name that no longer resolves. The confined fuzzer's capability oracle admits an object it was not
/// granted only when this page lies inside a memory region the fuzzers were given, which is how it
/// tells an object a fuzzer made from one it reached. The same test `reap_region_objects` applies.
#[cfg(feature = "system_tests")]
pub fn object_page(object: &Object) -> Option<u64> {
    // An address-space object's page is its root, out of the user-space registry and *before*
    // `IPC_TABLES`, so the two locks never nest (the confined fuzzer retypes these from its own
    // region, milestone 779).
    if let Object::AddressSpace(name) = *object {
        return crate::user::user_address_space_root(name);
    }
    let guard = IPC_TABLES.lock();
    let sched = guard.as_ref()?;
    match *object {
        Object::Rendezvous(id, _) => sched.rendezvous_table.get(id).copied(),
        Object::Notification(id) => sched.notification_table.get(id).copied(),
        Object::ThreadControlBlock(tid) => sched
            .threads
            .get(tid)
            .map(|t| crate::arch::mmu::virt_to_phys(t as *const Thread as u64)),
        _ => None,
    }
}

/// **One service move on a rendezvous that can never park the caller** (test support, milestone
/// 779 (fuzz the surface a confined process can reach), provisional name). The confined fuzzer's
/// conductor must never block: it is the one thread judging every fuzzer, and three separate
/// hangs taught that a check-then-act around `ipc_send`/`ipc_receive_cap` is a race on a
/// multicore machine, because the party the check saw can be gone by the act, and the blocking
/// primitive then parks the conductor with no one left to wake it. This is the act and the check
/// as one: under a single hold of `IPC_TABLES`, deliver `word` to a parked receiver; otherwise
/// collect one queued sender exactly as `ipc_receive_cap` would (any capability it carried is
/// filed in the caller's table, `x1` names the slot); otherwise nothing, with nothing queued and
/// nothing parked. The speculative queue positions the crate's own `send`/`receive` take are
/// taken straight back, under the same hold, with `remove_sender`/`remove_receiver`.
#[cfg(feature = "system_tests")]
pub enum ConductorMove {
    /// Nothing parked either way: nothing happened, nothing was left behind.
    None,
    /// A parked receiver took `[word, 0, 0]`.
    Sent,
    /// A queued sender was collected: its five words, shaped as `RECEIVE_CAP` returns them.
    Took([u64; 5]),
}

/// [`ConductorMove`]'s one operation. See the enum for the contract and the reason.
#[cfg(feature = "system_tests")]
pub fn conductor_move(ep: RendezvousId, word: u64) -> ConductorMove {
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().expect("no scheduler");
    let current = current_thread_id();
    let me = thread_control_block_ptr(sched, current);
    let Some(rendezvous) = rendezvous_of(sched, ep) else {
        return ConductorMove::None; // a stale name: nobody is parked anywhere
    };
    // The running thread's token; `me` is its pointer, for the removes below to compare.
    match rendezvous.send(take_token(sched, current)) {
        inter_process_communication::Send::Rendezvous(receiver, token) => {
            hold_token(token);
            let receiver = hold_token(receiver);
            let r = sched.threads.get_mut(receiver).unwrap();
            r.mailbox = [word, 0, 0, 0, 0];
            r.handshake.serve(); // delivered: this wake passes the boot-8 gate
            trace::record(trace::Event::Served, receiver, 1);
            wake(sched, receiver);
            ConductorMove::Sent
        }
        // An endpoint bound to an interrupt takes no message from anybody (§101 ruling B).
        inter_process_communication::Send::Refused(token) => {
            hold_token(token);
            ConductorMove::None
        }
        inter_process_communication::Send::Blocked => {
            // `send` queued `current` as a sender; take that straight back, under the same hold,
            // so no receiver can match the speculative position. The removal hands back the token
            // the send took, which is what the receive below needs.
            let token = rendezvous
                .remove_sender(me)
                .expect("the send above queued this thread under this same hold");
            match rendezvous.receive(token) {
                inter_process_communication::Receive::FromSender(sender, token) => {
                    hold_token(token);
                    let sender = hold_token(sender);
                    let msg = sched.threads.get(sender).unwrap().mailbox;
                    let capability = sched.threads.get_mut(sender).unwrap().outgoing_cap.take();
                    let is_reply = matches!(capability, Some(c) if matches!(c.object, crate::cap::Object::Reply(_)));
                    let slot = match capability {
                        Some(c) => {
                            deliver_capability(sched.threads.capabilities(current).unwrap(), c)
                        }
                        None => NO_CAP,
                    };
                    if !is_reply {
                        // Collected: the sender's rendezvous is complete (the boot-8 gate).
                        let s = sched.threads.get_mut(sender).unwrap();
                        s.handshake.serve();
                        trace::record(trace::Event::Served, sender, 4);
                        wake(sched, sender);
                    }
                    let tag = if is_reply { reply_tag(slot) } else { 0 };
                    ConductorMove::Took([msg[0], slot, msg[1], msg[3], tag])
                }
                // A pending interrupt signal, as `ipc_receive_cap` would report it.
                inter_process_communication::Receive::Signal(token) => {
                    hold_token(token);
                    ConductorMove::Took([1, NO_CAP, 0, 0, 0])
                }
                inter_process_communication::Receive::Blocked => {
                    // This hold queued `current`; this hold takes it back, token and all.
                    let token = rendezvous
                        .remove_receiver(me)
                        .expect("the receive above queued this thread under this same hold");
                    hold_token(token);
                    ConductorMove::None
                }
            }
        }
    }
}

#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn is_thread_present(tid: ThreadId) -> bool {
    IPC_TABLES
        .lock()
        .as_ref()
        .is_some_and(|s| s.threads.get(tid).is_some())
}

/// **Arm a kill on one thread by name** (test support), the single-thread form of what
/// [`reap_region_objects`] does to a whole region.
///
/// This exists because a test that spawns a **bare** user thread had no way to take it back. A
/// thread spawned into a reclaimable region is torn down by reclaiming the region; a thread spawned
/// with plain [`spawn`] around `user::run` belongs to no region, so there was no handle to end it
/// with. Two tests need a subject that never exits on its own (a spinner whose point is that it
/// never yields, and a child that must hold the free-frame count still while it is read), and both
/// therefore leaked a runnable thread for the rest of the suite. Two spinning threads on a four-hart
/// machine is a scheduling load the rest of the suite then runs under, which is how it presented:
/// `reclaim_frees_a_started_then_exited_childs_regions` starved and tripped its watchdog, on CI,
/// intermittently, far from the tests that caused it.
///
/// **No new syscall and no change to the user-visible surface.** DECISIONS §16's armed kill is the
/// whole mechanism: setting `killed` makes the scheduler convert the thread to a corpse at its next
/// preemption, which is exactly how `DESTROY` and §24's `^C` escalation already work. Rule 3 governs
/// the syscall boundary; this is an in-kernel function for in-kernel tests.
///
/// Returns whether a live thread was found and marked. A `false` means the `ThreadId` did not resolve,
/// which for a generational name means the thread is already gone rather than that the kill failed.
///
/// The kill is **armed, not immediate**: the thread dies at its next preemption, so a caller that
/// needs it actually gone waits for [`is_thread_present`] to go false rather than assuming.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn kill_thread(tid: ThreadId) -> bool {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return false;
    };
    match sched.threads.get_mut(tid) {
        Some(t) => {
            t.killed = true;
            true
        }
        None => false,
    }
}

// The ordinary boot and the system tests call this; the kernel's unit-test boot does neither
// (milestone 609 (the system tests leave the kernel crate)).
#[cfg_attr(test, allow(dead_code))]
pub fn thread_count() -> usize {
    IPC_TABLES.lock().as_ref().map_or(0, |s| s.threads.len())
}

/// **Count the runnable threads that are not the caller and not an idle thread** (test support).
///
/// A leaked one-shot driver that spins forever instead of exiting is `Ready`/`Running` for the rest
/// of the boot; a thread doing legitimate work is `Blocked` on an rendezvous when the system is
/// quiescent. So, from a quiesced probe (yield until pending exits are reaped), this count is the
/// number of leaked spinners: the idle threads (one per core) and the probe itself are the only
/// runnable threads a clean system has. The regression proxy for the test-thread starvation that
/// made the RedoxFS mount overrun the hang watchdog.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn runnable_non_idle_count(&exclude: &ThreadId) -> usize {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return 0;
    };
    let mut idles = [u64::MAX; crate::cpu::MAX_CPUS];
    // Harvest each ONLINE core's idle tid by set membership, not `0..count` (first-silicon sweep,
    // 2026-08-14): with the VisionFive 2's {1,2,3} online, count-as-index misses cpu 3's idle tid,
    // and its idle thread would then be counted as a leaked spinner. The array slot order does not
    // matter; only membership in `idles` does.
    for (slot, c) in idles.iter_mut().zip(crate::smp::online_cpus()) {
        *slot = crate::cpu::of(c).idle.load(Ordering::Relaxed);
    }
    sched
        .threads
        .iter_mut()
        .filter(|t| {
            matches!(t.handshake.state, State::Ready | State::Running)
                && t.id != exclude
                && !idles.contains(&t.id)
        })
        .count()
}

/// Print every thread's scheduler state, for diagnosing a hang. A lost IPC wakeup leaves a thread
/// `Blocked` forever with nothing to wake it; this shows which thread, and the `on_cpu`/`wake_pending`
/// flags that would reveal a botched wake-before-switch-out handoff. Takes `IPC_TABLES`, which is free when
/// the hang is a blocked thread (not a lock deadlock). Used by the test watchdog.
/// Feed every live thread's stack into the high-water accounting (milestone 84). Long-lived
/// service threads (the FS server, the shape of the incident that motivated the measurement) are
/// never reaped, so their stacks are only visible here, not in `KernelStack`'s `Drop`. A thread may
/// be running on another core while its stack is scanned; the scan reads a snapshot, and a racing
/// deepening is at worst under-reported by this run (see `stack::high_water`).
#[cfg(any(test, feature = "system_tests"))]
pub(crate) fn scan_live_thread_stacks() {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return;
    };
    for t in sched.threads.iter_mut() {
        if let Some(s) = t.stack.as_ref() {
            // SAFETY: a `KernelStack` this thread still owns, so its pages are mapped until its
            // `Drop` runs, and `KernelStack::new` painted the whole span. `IPC_TABLES` is held, so the
            // thread cannot be reaped out from under the scan.
            let used = unsafe { crate::stack::high_water(s.bottom(), s.top()) };
            crate::stack::note_thread_stack_use(used);
        }
    }
}

#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn dump_threads() {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        crate::println!("  dump_threads: no scheduler");
        return;
    };
    // What this dump can and cannot honestly claim (first-silicon audit, 2026-08-14):
    //
    //   - `state`/`on_cpu`/`wake_pending`/`wait`, and the rendezvous counts, are a CONSISTENT
    //     snapshot: every writer holds IPC_TABLES, which this dump holds.
    //   - `pc` is the trap frame at the thread's stack top, which trap entry writes WITHOUT
    //     IPC_TABLES. For an off-cpu thread it is trustworthy (the frame write happened-before the
    //     state write on that core, and our lock acquire synchronises with its release). For a
    //     thread on a cpu it is a racing read of live state, printed with a `*`.
    //   - the per-core lines read other cores' atomics relaxed. `current` is written under
    //     IPC_TABLES, so it is quiescent while we hold the lock, except for a core mid-switch
    //     (between its IPC_TABLES release and its finish_switch): such a core's `current` names the
    //     incoming thread while the outgoing one still runs for a few more instructions.
    //   - `ticks` is that core's timer-interrupt count. A core whose ticks FREEZE across dumps
    //     is not taking traps at all: wedged with interrupts masked, or stuck inside an SBI call
    //     in M-mode (where delegated S-interrupts cannot preempt), which no other row can show.
    // The stage breadcrumb repeats here on purpose: a serial line printed once can go missing
    // from a bench log (boots 7 through 9 were convicted on exactly that absence), but a dump
    // that fires every couple of seconds re-states how far the tour got each time.
    crate::println!(
        "--- thread dump (hang diagnostic; pc* = on-cpu, racy; tour stage {}) ---",
        BOOT_STAGE.load(Ordering::Relaxed),
    );
    for t in sched.threads.iter_mut() {
        let pc = t
            .stack
            .as_ref()
            .map(|s| crate::arch::exceptions::user_pc(s.top()))
            .unwrap_or(0);
        // The address-space root, which is what tells threads of DIFFERENT PROCESSES apart. Without
        // it a `pc` in this dump is close to useless for a userspace hang: every user program is
        // linked at the same base (`address_space_map::IMAGE_BASE`), so a bare PC resolves plausibly against several
        // binaries at once and invites exactly the wrong conclusion. That is not hypothetical, it
        // cost an hour on 2026-07-30: three spinning threads read equally well as three FS servers
        // in RedoxFS directory code or as one std client looping in `read_to_end`, and the PCs alone
        // could not say which. Threads sharing a root are one process; distinct roots are distinct
        // processes, and 0 is a kernel thread with no user address space.
        let root = t.space.as_ref().map(|s| s.root()).unwrap_or(0);
        crate::print!(
            "  tid={:#06x} state={:?} on_cpu={} wake_pending={} has_outgoing_cap={} pc={:#010x}{} address_space={:#010x}",
            t.id,
            t.handshake.state,
            t.handshake.on_cpu,
            t.handshake.wake_pending,
            t.outgoing_cap.is_some(),
            pc,
            if t.handshake.on_cpu { "*" } else { "" },
            root,
        );
        // The wait reason, written by the same IPC_TABLES-held statement that wrote `Blocked`. A
        // `Blocked` thread with `wait=-` here is the smoking gun for a state byte written outside
        // the block paths (corruption, or a block applied to the wrong TCB): every legal block
        // records what it waits on. See notes/visionfive2.md, fourth bench stop.
        match t.handshake.wait_on {
            Some(Wait::Rendezvous(ep, role)) => crate::println!(" wait={ep:#x}/{role:?}"),
            Some(Wait::Notification(n)) => crate::println!(" wait={n:#x}/Notification"),
            Some(Wait::Futex(k)) => {
                crate::println!(" wait={:#x}@{:#x}/Futex", k.address, k.space);
            }
            Some(Wait::Resume) => crate::println!(" wait=Resume"),
            None => crate::println!(" wait=-"),
        }
    }
    // Rendezvous topology: which rendezvous each blocked thread is queued on, so a deadlock shows as a
    // sender with no receiver. Diagnostic only.
    for (name, &phys) in sched.rendezvous_table.iter() {
        // SAFETY: a live rendezvous page, direct-mapped, under IPC_TABLES.
        let ep = unsafe { &*(crate::arch::mmu::phys_to_virt(phys) as *const Rendezvous) };
        let (ns, nr, np) = ep.debug_counts();
        if ns != 0 || nr != 0 || np != 0 {
            crate::println!("  ep={name:#06x} senders={ns} receivers={nr} pending={np}");
        }
    }
    // And each notification with a waiter or a word pending (milestone 151): a thread asleep beside
    // a non-zero word is the notification twin of a sender with no receiver.
    for (name, &phys) in sched.notification_table.iter() {
        // SAFETY: a live notification page, direct-mapped, under IPC_TABLES.
        let n = unsafe { &*(crate::arch::mmu::phys_to_virt(phys) as *const NotificationPage) };
        let (waiters, word) = n.state.debug_counts();
        if waiters != 0 || word != 0 {
            crate::println!(
                "  notification={name:#06x} waiters={waiters} word={word:#x} bound={:?}",
                n.bound
            );
        }
    }
    // The online set, not `0..count` (first-silicon sweep, 2026-08-14): on the VisionFive 2 the
    // count-as-index loop printed parked slot 0 as if it were a live core and hid online core 3.
    for c in crate::smp::online_cpus() {
        let pc = cpu::of(c);
        let inbox_len = pc.inbox.lock().len();
        crate::println!(
            "  core {c}: current={:#06x} idle={:#06x} switched_from={:#06x} need_resched={} inbox_len={} ticks={} steal_req={:?}",
            pc.current.load(Ordering::Relaxed),
            pc.idle.load(Ordering::Relaxed),
            pc.switched_from.load(Ordering::Relaxed),
            pc.need_resched.load(Ordering::Relaxed),
            inbox_len,
            // A core whose tick count holds still between dumps is taking no timer traps: it is
            // spinning with interrupts masked, or parked inside an SBI call in M-mode (a remote
            // fence that never completes looks exactly like this). The one field that tells a
            // wedged core from a scheduler that merely chose not to run somebody.
            crate::arch::timer::ticks_on(c),
            // A steal request that stays claimed dump after dump means the victim never reached
            // a scheduler entry to serve it: the same wedge, seen from a thief's side.
            pc.steal_request.peek(),
        );
        trace::dump(c);
    }
    // A parked slot with a non-empty inbox is a thread nothing will ever run: the exact shape of
    // the VisionFive 2 placement hang (init modulo-counted into slot 0's inbox; that dead inbox in
    // this dump was the clue). Since the online-set sweep no path should produce it, so if this
    // prints, something is picking cpus by count again.
    let online = crate::smp::online_harts_mask();
    for c in (0..crate::cpu::MAX_CPUS).filter(|c| online & (1 << c) == 0) {
        let inbox_len = cpu::of(c).inbox.lock().len();
        if inbox_len != 0 {
            crate::println!(
                "  core {c}: PARKED with inbox_len={inbox_len} (placed on a dead core)"
            );
        }
    }
    crate::println!("--- end thread dump ---");
}

/// **How many senders are parked on an rendezvous.** Test support (milestone 22 phase B.2).
///
/// A negative assertion ("the supervisor sent nothing more") cannot be made with `RECEIVE`, which would
/// block forever on a quiet rendezvous. This is the non-blocking look that lets a test say "and then
/// nothing happened" instead of hanging when the code is right.
#[cfg(feature = "system_tests")]
pub fn rendezvous_waiting_senders(ep: RendezvousId) -> usize {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return 0;
    };
    match rendezvous_of(sched, ep) {
        Some(e) => e.debug_counts().0,
        None => 0,
    }
}

/// **What a death actually did to a thread**: its run state, the supervision rendezvous it was
/// spawned with, and the wait it is recorded as being in. `None` if the thread is no longer in the
/// table at all. Test support (milestone 321). **Provisional name.**
///
/// It exists because a corpse that does not reach its supervision rendezvous fails
/// `rendezvous_waiting_senders(ep) == 1` in a way a transcript cannot explain, and the failure
/// observed on xenon (2026-09-17) was on a bench nobody can re-run with a print added. The three
/// possible histories leave three different marks here, so one line in an assertion message
/// separates them:
///
///   - `Finished` with no `fault_ep`: [`depart`] took the *unsupervised* path, so the supervision
///     endpoint was lost before the fault rather than the delivery failing.
///   - `Dead` with `wait_on == Some((ep, Sender))`: the corpse did park, and something took it off
///     the queue again.
///   - `Dead` with `wait_on == None`: [`deliver_death`]'s `send` met a waiting receiver, so the
///     message was handed over and the corpse never joined the sender queue.
///   - `Ready`, `Running` or `Blocked`: it never departed, whatever was printed about it.
///   - `None`: the thread was reaped and freed.
///
/// This is milestone 318's lesson about assertions against constants, carried one step past the
/// count: knowing the count is 0 still leaves three readings, and this is what decides between them
/// on a machine the person reading the log cannot touch.
/// The three fields [`thread_death_disposition`] reports. **Provisional name.**
///
/// A struct rather than the tuple this started as, and clippy's `type_complexity` asking for the
/// change is the smaller half of the reason. The larger one is that this type exists to be read
/// **in a panic message**, through `{:?}`, by somebody holding a serial log from a machine they
/// cannot touch. A three-field `Debug` names its fields and a three-element tuple does not, so the
/// difference is between `Some((Dead, Some(4), Some((4, Sender))))` and a line that says which of
/// those numbers is the endpoint.
/// **Every field here is read through `Debug` and nowhere else, which the dead-code lint cannot
/// see**: a derived `impl` does not count as a use, so without this the three fields that are the
/// whole point of the type report as never read. The allow is the exception and this is it saying
/// so. It is also the tell that the type is doing one job: if a field ever gets read by code, the
/// allow should shrink rather than stay.
#[cfg(any(test, feature = "system_tests"))]
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct ThreadDeathDisposition {
    /// Where the scheduler thinks this thread is.
    pub state: State,
    /// The supervision rendezvous it was started with. `None` on a corpse means [`depart`] took the
    /// unsupervised path, so supervision was lost before the fault rather than in delivery.
    pub fault_ep: Option<RendezvousId>,
    /// The queue it is recorded as parked on, and in which role.
    pub wait_on: Option<Wait>,
}

#[cfg(feature = "system_tests")]
pub fn thread_death_disposition(tid: ThreadId) -> Option<ThreadDeathDisposition> {
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut()?;
    let t = sched.threads.get(tid)?;
    Some(ThreadDeathDisposition {
        state: t.handshake.state,
        fault_ep: t.fault_ep,
        wait_on: t.handshake.wait_on,
    })
}

/// **How many receivers are parked on an rendezvous.** The twin of [`rendezvous_waiting_senders`], and
/// test support for the same reason (milestone 81).
///
/// A test that wants to act *on* a blocked waiter needs to know the waiter is blocked, and "I
/// yielded, so it must have run" is not that knowledge: since DECISIONS §28 the waiter is placed on
/// another core, and on the physical core under HVF a yield on this one returns in nanoseconds. So
/// the wait has to be on the queue itself, which is what this reads.
#[cfg(any(test, feature = "system_tests"))]
pub fn rendezvous_waiting_receivers(ep: RendezvousId) -> usize {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return 0;
    };
    match rendezvous_of(sched, ep) {
        Some(e) => e.debug_counts().1,
        None => 0,
    }
}

/// **Test support: a wake with nothing delivered** (the boot-8 injector, 2026-08-14).
///
/// Issues a bare `wake()` against `tid` under `IPC_TABLES`, through the same function every scheduler
/// wake site funnels into, with no message written, no signal counted, and no abort flagged: the
/// transition the VisionFive 2's boot-8 event ring recorded against the boot thread (`wake:0x0`
/// on a boot where no sender to its rendezvous existed). This is deliberately not a hand-rolled
/// state poke: it exercises the real wake path, so whatever `wake()` does about an undelivered
/// wake is what this injects.
#[cfg(any(test, feature = "system_tests"))]
pub fn wake_without_delivery(tid: ThreadId) {
    let mut guard = IPC_TABLES.lock();
    if let Some(sched) = guard.as_mut() {
        wake(sched, tid);
    }
}

/// **The boot tour's last-reached stage**, printed in every [`dump_threads`] header (first-silicon
/// diagnostics, 2026-08-15; name provisional).
///
/// Boots 7 through 9 on the VisionFive 2 were called a hang inside the initial ramdisk demo because
/// the tour's serial lines after "init : measured, built, started" (the prefix that line carried
/// then; the demo's program is `builder` and the line says so now) never showed at the bench, while
/// the thread dumps kept printing. The dumps' own rows later proved the tour had in fact advanced
/// through the UART-driver step (notes/visionfive2.md, fifth stop), so "which step did the boot
/// thread reach" must not be inferable only from serial lines that can go missing: a breadcrumb
/// the periodic dump repeats survives a lossy or misread log. The riscv tour bumps this at each
/// step; the number-to-step table lives beside the tour in main.rs.
pub(super) static BOOT_STAGE: AtomicU32 = AtomicU32::new(0);

/// Record that the boot reached `stage`: 3 to 10 as `self_test::run`'s checks start, 11 once its
/// verdict and `boot_ladder::TOUR` have printed. Monotonic by convention, not enforced.
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)] // self_test is the caller
pub(crate) fn note_boot_stage(stage: u32) {
    BOOT_STAGE.store(stage, Ordering::Relaxed);
}

/// Read the tour stage back. The hang watcher uses it to fall silent once the tour has
/// finished (stage 11, and it was 10 until milestone 159 added the hardware-entropy step after
/// what used to be the last one): boot 13 completed healthily and still printed five dumps of a
/// quiescent machine, which reads as a hang to anyone who has not memorized the watcher.
// Dead since milestone 295 (retire `components/src/builder.rs`): the hang watcher in
// `riscv_initial_ramdisk_demo` was the only reader, and that function went with the program it
// loaded. `note_boot_stage` above still has ten callers, so the breadcrumb is still WRITTEN and
// `dump_threads` still prints it from `BOOT_STAGE` directly; what has no caller is this accessor.
// Kept with the canary below, for the reason written there.
#[allow(dead_code)]
pub(crate) fn boot_stage() -> u32 {
    BOOT_STAGE.load(Ordering::Relaxed)
}

/// Arm the [`canary`] over the thread table and the rendezvous registry, snapshotting under `IPC_TABLES`
/// so the baseline is a consistent cut. The riscv initial ramdisk demo arms before parking in its
/// receive and disarms when the receive returns; see notes/visionfive2.md (fifth stop) for what
/// boot 11 does with the output. **No caller since milestone 295**; see the note on `mod canary`.
#[allow(dead_code)]
pub(crate) fn canary_arm_registries() {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return;
    };
    let threads = (
        core::ptr::from_ref(&sched.threads.table) as usize,
        size_of_val(&sched.threads.table),
    );
    let endpoints = (
        core::ptr::from_ref(&sched.rendezvous_table) as usize,
        size_of_val(&sched.rendezvous_table),
    );
    canary::arm(&[threads, endpoints]);
}

/// Disarm the [`canary`]. The demo window's other bracket. Quiesces: when it returns, no check
/// pass is in flight on any core. **No caller since milestone 295**; see the note on `mod canary`.
#[allow(dead_code)]
pub(crate) fn canary_disarm() {
    canary::disarm();
}

/// **The number that says preemption is real**, read by the preemption tests and printed by the
/// milestone tour.
///
/// A `bench` boot diverges before the tour and runs no tests, so in that configuration this
/// genuinely has no caller. That is a property of the boot mode, not evidence the counter is dead,
/// which is why the allow is conditioned on exactly that feature rather than written
/// unconditionally. `x86_64`'s tour prints no preemption line, so there only
/// the tests read it.
#[cfg_attr(
    any(
        feature = "bench",
        all(target_arch = "x86_64", not(any(test, feature = "system_tests")))
    ),
    allow(dead_code)
)]
pub fn preemptions() -> u64 {
    PREEMPTIONS.load(Ordering::Relaxed)
}

/// Preemptions taken **on this core**, which is the question the global counter cannot answer.
///
/// A window with interrupts masked takes none of these however long it lasts, and that property is
/// what `kernel/src/bench/spawn_map.rs`'s `map_new` is built on.
/// See milestone 541 (a timed window that excludes preemption), and
/// `kernel/src/preemption_window_tests.rs`.
///
/// **Its two callers are both conditional**, which is why the allow is the inverse shape of
/// `preemptions()`'s above: that one has a caller in the ordinary build (the milestone tour) and
/// loses it under `shell` and `bench`, while this one has callers only under `test` and `bench`
/// and is genuinely dead in the build that ships. The counter it reads is written unconditionally,
/// so no configuration can make the number stale.
#[cfg_attr(
    not(any(test, feature = "system_tests", feature = "bench")),
    allow(dead_code)
)]
pub fn preemptions_here() -> u64 {
    preemptions_on(cpu::id())
}

/// Preemptions taken on **a named** core, for a reader that must not follow a migrating thread.
///
/// [`preemptions_here`] reads whichever core is running *now*, which is the wrong counter for any
/// observation that straddles a preemption: the preemption is exactly what may move the reader.
/// `kernel/src/preemption_window_tests.rs` samples one core across an unmask for that reason.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub(crate) fn preemptions_on(id: usize) -> u64 {
    PREEMPTIONS_PER_CPU[id].load(Ordering::Relaxed)
}

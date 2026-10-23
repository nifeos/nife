//! Reading the scheduler's state from outside it: the soak counters, the accessors other kernel
//! modules call, the hooks `system_tests` drives, and `dump_threads`
//! (notes/scheduler/thread-dump.md).
//!
//! Name: provisional (milestone 840's lane, 2026-10-11 UTC; §271 (the scheduler file splits into
//! submodules of `sched`)).

use super::*;
#[cfg(feature = "system_tests")]
use crate::cap::Object;

/// How many wakes the undelivered-wake gate refused: a waker made a parked receiver `Ready` without
/// delivering anything. Sums the per-core counters `trace::record` keeps ([`trace::counted`] says
/// why the racy sum is fine). Never fired in the field, so a nonzero here on a board is a finding.
///
/// The soak counters are the kernel's numbers for its own soak supervisor and are not reachable
/// from userspace: a workload that could read its own tripwire is one step from clearing it.
///
/// Names provisional (this lane's, 2026-09-01; public function names are an architect's call,
/// DECISIONS naming tenet as extended on 2026-08-23).
// Soak builds only: the counters' increments are `cfg`-gated off the IPC fastpath elsewhere, and
// `script/lint` clippies `--features soak_test` on both ISAs.
#[cfg(feature = "soak_test")]
pub(crate) fn wake_refusals() -> u64 {
    trace::counted(trace::Event::WakeRefused)
}

/// How many wakes were parked because the target was still standing on a CPU. Ordinary, and
/// expected to be nonzero under load; a soak reports it so that "the machine was genuinely
/// contended" is a number rather than an assurance.
// Soak builds only, as `wake_refusals`.
#[cfg(feature = "soak_test")]
pub(crate) fn wakes_deferred() -> u64 {
    trace::counted(trace::Event::WakeDeferred)
}

/// How many wakes and placements landed a thread on a core other than the waker's: zero means the
/// soak never crossed cores.
// Soak builds only, as `wake_refusals`.
#[cfg(feature = "soak_test")]
pub(crate) fn remote_placements() -> u64 {
    trace::counted(trace::Event::PlaceRemote)
}

/// How many times a thread ran on a different core than the one it last ran on: the cross-core
/// handoff number a soak reports. [`remote_placements`] is not it, because a rendezvous wake queues
/// its peer on the waker's own core (§28 (SMP placement), part 2);
/// see [`crate::thread::Thread::last_cpu`].
// Soak builds only, as `wake_refusals`.
#[cfg(feature = "soak_test")]
pub(crate) fn migrations() -> u64 {
    trace::counted(trace::Event::Migrated)
}

/// How many threads this machine handed from one core's run queue to another's on request: the
/// work-steal protocol's activity level.
// Soak builds only, as `wake_refusals`.
#[cfg(feature = "soak_test")]
pub(crate) fn steals_served() -> u64 {
    trace::counted(trace::Event::StealServe)
}

/// Spawn a thread exactly as [`spawn`] would, and return its id with the core it landed on.
///
/// The placement is [`pick_spawn_target`]'s, so this reports the policy and offers no lever to
/// steer it (affinity is an open question; §138 (how a saturated workload is made to hand threads
/// across cores) declined a rebalancer). Soak and job-mix builds only: a workload number from real
/// silicon needs the arrangement that produced it.
///
/// Name: provisional (milestone 240 (the soak reports what happened and not where)): calef names
/// public items.
#[cfg(any(feature = "soak_test", feature = "job_mix"))]
pub fn spawn_reporting_placement<F: FnOnce() + Send + 'static>(f: F) -> Option<(ThreadId, usize)> {
    let target = pick_spawn_target();
    spawn_on(target, f).map(|id| (id, target))
}

/// Write where each of `ids` last ran into `out`, or `u8::MAX` for a thread that has not run yet or
/// is gone; [`dump_threads`] tells those apart.
///
/// Context: takes `IPC_TABLES` once for the whole set, so a soak heartbeat asking about every
/// worker does not sit in front of the IPC fastpath once per thread.
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

/// The ticks charged to the running thread so far, for a test that measures what a wait costs.
/// Read by the thread itself, because a finished thread's slot may be reused before anyone looks.
/// Userspace reads the same counter in milliseconds through `SURVEY`.
#[cfg(any(test, feature = "system_tests"))]
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the system tests call it; a unit-test boot on some ISAs does not
pub fn current_cpu_ticks() -> u64 {
    CPU_TICKS
        .get(slot_of(current_thread_id()))
        .map_or(0, |c| c.load(Ordering::Relaxed))
}

/// The top of the current thread's kernel stack, where its `TrapFrame` belongs; `None` for the boot
/// thread, which runs on the stack `boot.s` set up.
///
/// A user thread's `TrapFrame` must sit exactly where the vector table's `SAVE_CONTEXT` rebuilds
/// it, because `eret` leaves `SP_EL1` just past it.
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

/// The user PC recorded in `tid`'s trap frame, at the top of its kernel stack: [`dump_threads`]'s
/// lookup, for a test (milestone 71 (the thread-start fault)). `None` if the name does not resolve
/// or the thread has no kernel stack. A zero means nothing wrote a frame where the trap path will
/// look.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn user_pc_of(tid: ThreadId) -> Option<u64> {
    let guard = IPC_TABLES.lock();
    let sched = guard.as_ref()?;
    let t = sched.threads.get(tid)?;
    t.stack
        .as_ref()
        .map(|s| crate::arch::exceptions::user_pc(s.top()))
}

/// A corpse's retained five-word §26 (the fault endpoint) fault or exit message, which a `Dead`
/// thread keeps until its supervisor reaps it (test support). `None` if the name does not resolve
/// or it is not a corpse.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn corpse_fault_msg(tid: ThreadId) -> Option<[u64; 5]> {
    let guard = IPC_TABLES.lock();
    let sched = guard.as_ref()?;
    let t = sched.threads.get(tid)?;
    (t.handshake.state == State::Dead)
        .then_some(t.fault_msg)
        .flatten()
}

/// `tid`'s capability table, lent to `read` in place (test support, milestone 757 (a test kernel
/// fails a process on its Nth retype), provisional name). `None` for a name that does not resolve.
///
/// Lent rather than copied: a copy is a 2 KiB array in every frame that holds it, which put the
/// caller over the guard page. A retype sweep reads it before and after each run, since a missed
/// `cap_delete` shows up only here.
///
/// Context: `read` runs under `IPC_TABLES`, so it must not call back into `sched`.
#[cfg(feature = "system_tests")]
pub fn with_capability_table<R>(
    tid: ThreadId,
    read: impl FnOnce(&crate::cap::CapabilityTable) -> R,
) -> Option<R> {
    let guard = IPC_TABLES.lock();
    let t = guard.as_ref()?.threads.capabilities(tid)?.lock();
    Some(read(&t))
}

/// The root of `tid`'s address space, `None` for a kernel thread or a name that does not resolve
/// (test support, milestone 779 (fuzz the surface a confined process can reach), provisional
/// name). No syscall hands out a root; the confined fuzzer's page oracle walks mappings from it.
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

/// The physical page a page-resident kernel object (a rendezvous, a notification, a TCB or an
/// address space's root) lives in, `None` for any other object or a stale name (test support,
/// milestone 779, provisional name). The confined fuzzer admits an object it was not granted only
/// when this page lies in a region the fuzzers were given, the test `reap_region_objects` applies.
#[cfg(feature = "system_tests")]
pub fn object_page(object: &Object) -> Option<u64> {
    // The user-space registry's lock before `IPC_TABLES`, never nested.
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

/// One service move on a rendezvous that never parks the caller (test support, milestone 779,
/// provisional name). The confined fuzzer's conductor judges every fuzzer and must never block;
/// a check-then-act around `ipc_send` or `ipc_receive_cap` races on a multicore machine.
///
/// Under one hold of `IPC_TABLES`: deliver `word` to a parked receiver; else collect one queued
/// sender as `ipc_receive_cap` would (a carried capability is filed in the caller's table); else
/// nothing. A speculative queue position is taken straight back under the same hold.
#[cfg(feature = "system_tests")]
pub enum ConductorMove {
    /// Nothing parked either way: nothing happened, nothing was left behind.
    None,
    /// A parked receiver took `[word, 0, 0]`.
    Sent,
    /// A queued sender was collected: its five words, shaped as `RECEIVE_CAP` returns them.
    Took([u64; 5]),
}

/// [`ConductorMove`]'s one operation. See the enum for the contract.
///
/// Context: takes `IPC_TABLES`.
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
            // Take the speculative sender position back under the same hold, so no receiver
            // can match it; the removal returns the token the receive needs.
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

/// Whether `tid` is still in the thread table (test support): the narrow question "was this
/// thread reaped". [`thread_count`] moves whenever any thread anywhere is reaped, so a test that
/// waits on it waits for the whole system to hold still. The name is generational, so `false`
/// means gone, never "replaced".
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn is_thread_present(tid: ThreadId) -> bool {
    IPC_TABLES
        .lock()
        .as_ref()
        .is_some_and(|s| s.threads.get(tid).is_some())
}

/// Arm a kill on the thread `tid` (test support), the single-thread form of what
/// [`reap_region_objects`] does to a region.
///
/// A bare thread spawned with [`spawn`] around `user::run` belongs to no region, so this is the
/// only way a test takes back one that never exits. Sets `killed`, §16 (object revocation)'s armed
/// kill: the thread becomes a corpse at its next preemption, so a caller that needs it gone waits
/// for [`is_thread_present`] to go false. Returns `false` if the name does not resolve, which for a
/// generational name means already gone.
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

// The ordinary boot and the system tests call this; the kernel's unit-test boot does not.
#[cfg_attr(test, allow(dead_code))]
pub fn thread_count() -> usize {
    IPC_TABLES.lock().as_ref().map_or(0, |s| s.threads.len())
}

/// Count the runnable threads that are neither `exclude` nor an idle thread (test support).
///
/// From a quiesced probe (yield until pending exits are reaped), a clean system's only runnable
/// threads are the idle threads and the probe, since legitimate work is `Blocked`, so this counts
/// leaked spinners.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn runnable_non_idle_count(&exclude: &ThreadId) -> usize {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return 0;
    };
    let mut idles = [u64::MAX; crate::cpu::MAX_CPUS];
    // Each online core's idle tid by set membership, not `0..count`: the VisionFive 2's online
    // harts are {1, 2, 3}.
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

/// Feed every live thread's stack into the high-water accounting. Long-lived service threads are
/// never reaped, so their stacks are seen only here, not in `KernelStack`'s `Drop`. A thread
/// running elsewhere is scanned as a snapshot, so a racing deepening is at worst under-reported
/// (`stack::high_water`).
///
/// Context: takes `IPC_TABLES`.
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

/// Print every thread's scheduler state, every rendezvous and notification with a waiter, and every
/// online core, for diagnosing a hang. Used by the test watchdog. What each field can claim is in
/// notes/scheduler/thread-dump.md.
///
/// Context: takes `IPC_TABLES`, which is free when the hang is a blocked thread.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn dump_threads() {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        crate::println!("  dump_threads: no scheduler");
        return;
    };
    // `pc` is a racing read for a thread on its core (printed `pc*`); the per-core lines are
    // relaxed reads. See notes/scheduler/thread-dump.md.
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
        // The space's root tells processes apart: every user program links at one base.
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
        // `Blocked` with `wait=-` means a state byte was written outside the block paths.
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
    // Rendezvous with a waiter: a deadlock shows as a sender with no receiver.
    for (name, &phys) in sched.rendezvous_table.iter() {
        // SAFETY: a live rendezvous page, direct-mapped, under IPC_TABLES.
        let ep = unsafe { &*(crate::arch::mmu::phys_to_virt(phys) as *const Rendezvous) };
        let (ns, nr, np) = ep.debug_counts();
        if ns != 0 || nr != 0 || np != 0 {
            crate::println!("  ep={name:#06x} senders={ns} receivers={nr} pending={np}");
        }
    }
    // Notifications with a waiter or a pending word.
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
    // The online set, not `0..count`.
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
            // Frozen across dumps: no timer traps, so the core is wedged.
            crate::arch::timer::ticks_on(c),
            // Claimed across dumps: the victim never reached a scheduler entry.
            pc.steal_request.peek(),
        );
        trace::dump(c);
    }
    // A parked core with a non-empty inbox holds a thread nothing will run.
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

/// How many senders are parked on the rendezvous `ep` (test support): the non-blocking look that
/// lets a test assert "and then nothing happened", which `RECEIVE` would block on.
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

/// Hold a corpse in the window between departing and leaving its core (milestone 321 (a corpse that
/// did not park, on four real cores)). Test support.
///
/// `depart` publishes a death before the core switches away, a window too short for a test to aim
/// at. With `hold`, sets `on_cpu` on a `Dead` thread whose core has already left it; without,
/// clears it. `false` if there was no such corpse (not `Dead`, gone, or still standing, whose own
/// `finish_switch` would clear the mark under the test).
///
/// Name: provisional, minted 2026-10-22 (UTC) by lane/321-reclaim-refused.
#[cfg(feature = "system_tests")]
pub fn hold_on_its_core(tid: ThreadId, hold: bool) -> bool {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return false;
    };
    let Some(t) = sched.threads.get_mut(tid) else {
        return false;
    };
    if t.handshake.state != State::Dead || (hold && t.handshake.on_cpu) {
        return false;
    }
    t.handshake.on_cpu = hold;
    true
}

/// The three fields [`thread_death_disposition`] reports. **Provisional name.**
///
/// A struct, so the `{:?}` in a panic message names each field for someone holding a serial log.
/// Every field is read only through `Debug`, which the dead-code lint does not count; the allow
/// below should shrink if code ever reads one.
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

/// What a death did to `tid`: its run state, its supervision rendezvous and its recorded wait, or
/// `None` once it is freed. Test support (milestone 321). **Provisional name.**
///
/// `Finished` with no `fault_ep` took the unsupervised path; `Dead` waiting as `Sender` parked and
/// was taken off again; `Dead` with no wait handed its message to a waiting receiver
/// ([`deliver_death`]); `Ready`, `Running` or `Blocked` never departed.
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

/// How many receivers are parked on the rendezvous `ep` (test support), the twin of
/// [`rendezvous_waiting_senders`]. A yield is not evidence a waiter has parked: since §28 it runs
/// on another core, so a test reads the queue itself.
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

/// Issue a bare `wake()` against `tid`, with nothing delivered (test support: the boot-8 injector).
///
/// It goes through the function every wake site funnels into, so whatever `wake()` does about an
/// undelivered wake is what this injects.
///
/// Context: takes `IPC_TABLES`.
#[cfg(any(test, feature = "system_tests"))]
pub fn wake_without_delivery(tid: ThreadId) {
    let mut guard = IPC_TABLES.lock();
    if let Some(sched) = guard.as_mut() {
        wake(sched, tid);
    }
}

/// The boot tour's last-reached stage, printed in every [`dump_threads`] header (name
/// provisional). A breadcrumb the periodic dump repeats survives a serial log that lost lines; the
/// number-to-step table is beside the tour in `main.rs`.
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

/// Read the tour stage back.
// No caller since milestone 295 (retire `components/src/builder.rs`); kept with the canary, for the
// reason written on `mod canary`.
#[allow(dead_code)]
pub(crate) fn boot_stage() -> u32 {
    BOOT_STAGE.load(Ordering::Relaxed)
}

/// Arm the [`canary`] over the thread table and the rendezvous registry, snapshotting under
/// `IPC_TABLES` so the baseline is a consistent cut. No caller; see the note on `mod canary`.
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

/// Disarm the [`canary`]. Quiesces: when it returns, no check pass is in flight on any core. No
/// caller; see the note on `mod canary`.
#[allow(dead_code)]
pub(crate) fn canary_disarm() {
    canary::disarm();
}

/// How many preemptions this machine has taken: the number that says preemption is real, read by
/// the preemption tests and printed by the milestone tour (not on `x86_64`).
///
/// A `bench` boot runs neither, which is why the allow names that feature.
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

/// Preemptions taken on this core. A window with interrupts masked takes none, which is what
/// `kernel/src/bench/spawn_map.rs`'s `map_new` is built on (milestone 541 (a timed window that
/// excludes preemption), `kernel/src/preemption_window_tests.rs`).
///
/// Its callers are only under `test` and `bench`; the counter is written unconditionally.
#[cfg_attr(
    not(any(test, feature = "system_tests", feature = "bench")),
    allow(dead_code)
)]
pub fn preemptions_here() -> u64 {
    preemptions_on(cpu::id())
}

/// Preemptions taken on core `id`, for a reader that must not follow a migrating thread:
/// [`preemptions_here`] reads whichever core runs now, and a preemption may move the reader.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub(crate) fn preemptions_on(id: usize) -> u64 {
    PREEMPTIONS_PER_CPU[id].load(Ordering::Relaxed)
}

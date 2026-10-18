//! **Interruptible jobs**, the shell's side of the two-tier `^C` path (DECISIONS §24
//! (interrupting the foreground process)): `spawn_interruptible`, `watch`, `forcible` and `reclaim`.
//!
//! Moved whole out of `components/src/swish.rs` by milestone 844 (the shell program is split
//! along its seams), as §273 (the shell program becomes a directory of modules) lays out.
//!
//! Name: provisional 2026-10-11 (UTC), milestone 844's lane. calef deferred every module name
//! in §273 to a name-ratification pass.

use super::*;

// ---- the two-tier interrupt path (DECISIONS §24) ----

/// Pages the construction untyped for a supervised child holds: its address space, code, stack, and TCB.
/// `interrupt_heeder` and `interrupt_ignorer` are tiny; this is generous. DESTROY returns these
/// pages to our budget.
const JOB_UNTYPED_PAGES: u64 = 32;
/// The range a supervised job's shared frame is mapped into, in our own space, one page per job.
///
/// **This range used to start at `0x00c0_0000`, which is [`OUT_VA`]**, and nothing said so. The two
/// agreed until 2026-08-01 (`abb44b67f`), when `OUT_VA` moved off [`FS_VA`] onto the job frames'
/// first page. From then on the first supervised job of every boot printed `could not map the job
/// frame` (the page was already mapped), and the second worked only because the cursor had advanced
/// past the collision on the way to failing. `script/swish-check` scripted no supervised job, so
/// nothing ran it for eight weeks. The assertion in `main.rs` is what now holds the range apart
/// from every fixed window this shell has, and the `interrupt_heeder` line in that script is what
/// runs it.
///
/// 16 MiB up is a pair page on the address-space map (milestone 206 (a program image has under 896 KiB)), clear of all four fixed
/// windows and the image window at 64 MiB, and a whole 2 MiB-aligned table's worth of pages sits
/// under it, so the first job costs one page-table page and the next 511 cost none. The program's
/// own image and stack are in bands of their own and cannot be reached from here.
pub(super) const JOBFRAME_WINDOWS: core::ops::Range<u64> =
    0x0000_0000_0100_0000..0x0000_0000_0120_0000;

/// The next job frame's address. It advances per job, because the shell holds no capability to its
/// own address space to `UNMAP` with (§249 (a running address space stays nameable) leaves that grant
/// to its builder, and the progenitor does not make it): each job
/// gets a fresh window and the old mapping is simply left behind (one page of address space, and one
/// frame from our budget, is the honest per-job cost). Past [`JOBFRAME_WINDOWS`]'s end a job is
/// refused rather than mapped over whatever lies beyond it.
static SH_JOBFRAME_NEXT: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(JOBFRAME_WINDOWS.start);

/// The terminal's `^C` count we have already accounted for. A watermark, not a per-job baseline, and
/// that distinction is load-bearing: a `^C` typed the instant after `run interrupt_heeder` is
/// counted by the terminal *before* the shell finishes spawning and starts watching (the input
/// driver runs first).
/// Diffing against a watermark carried across the session catches it anyway; a fresh baseline read at
/// watch-start would already include it and miss the interrupt. A prompt `^C` (a failed read) and a
/// finished job each advance the watermark, so neither leaks into the next job.
pub(super) static CONSUMED: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// Run a supervised foreground job: mint its resources from our own budget, direct the progenitor to build it
/// from a region we hold, then watch it under the two-tier `^C` escalation until it stops or we tear
/// it down. This is the whole of DECISIONS §24 on the shell's side.
pub(super) fn spawn_interruptible(e: Endowment) {
    // Mint the job's resources. RETYPE the shared frame first, then SPLIT the construction budget,
    // so the budget is the top of our watermark and DESTROY returns its pages cleanly (LIFO).
    let Some(job_fr) = retype_page_frame() else {
        failed();
        print(b"  this shell's memory budget is exhausted; nothing left to grant\n");
        return;
    };
    let Some(job_ut) = memory_region_split(JOB_UNTYPED_PAGES) else {
        cap_delete(job_fr);
        failed();
        print(b"  this shell's memory budget is exhausted; nothing left to grant\n");
        return;
    };

    // Map the shared frame into our own space so we can signal the job and read its status.
    let va = SH_JOBFRAME_NEXT.fetch_add(PAGE, core::sync::atomic::Ordering::Relaxed);
    if !JOBFRAME_WINDOWS.contains(&va) {
        cap_delete(job_fr);
        cap_delete(job_ut);
        failed();
        print(b"  this shell has run as many supervised jobs as it has room to watch\n");
        return;
    }
    if !map_page_frame(job_fr, va) {
        cap_delete(job_fr);
        cap_delete(job_ut);
        failed();
        print(b"  could not map the job frame\n");
        return;
    }
    // SAFETY: `map_page_frame` just mapped one page read/write at `va`, above, and it stays mapped for
    // the rest of this job's life (no unmap syscall exists), so the window's `new` asserts that
    // once here instead of at every jf_load/jf_store call site the way the two free functions
    // used to (milestone 139 round 3; "actually a *better* MappedWindow fit than the static case,
    // since `new` already takes a runtime base" is round 2's own framing for this cluster).
    let jf = unsafe { MappedWindow::new(va, PAGE) };
    // A freshly retyped frame is zeroed, but make the shared contract explicit.
    jf.write(job_page_frame::INTERRUPT as u64, 0u64);
    jf.write(job_page_frame::DONE as u64, 0u64);
    jf.write(job_page_frame::STATUS as u64, 0u64);
    jf.write(job_page_frame::HEARTBEAT as u64, 0u64);

    // Direct the progenitor: an interruptible request, then the job untyped, delegated with every
    // right (the progenitor builds from it, and since milestone 824 (a retype mints no right its
    // budget lacks) what it builds carries the budget's rights), and the job frame, WRITE|GRANT (the
    // progenitor maps it). We keep our own copies: the untyped to tear the job down, the frame to
    // signal it and read it.
    let (w0, w1, w2) = spawnproto::request(
        e.prog.id(),
        e.arg,
        e.mem_pages,
        spawnproto::Wiring {
            interruptible: true,
            sink: false,
            source: false,
            // A supervised job reports through the shared frame, not through a stream, and neither
            // demonstrator declares a second one. `OutputDeclaration::Silent` and a diagnostic rendezvous
            // would be a contradiction the manifest can already refuse.
            diagnostics: false,
            // Neither demonstrator declares a directory either, and a supervised job is built out of
            // *this shell's* untyped rather than the progenitor's pool, so there is no region a caretaker
            // could share with it (DECISIONS §92).
            dir: false,
            dir2: false,
            // A supervised job's completion signal is already the shared job frame's `DONE` flag,
            // not `result_ep`, so there is nothing here for DECISIONS §106's narrowing to replace.
            screen: false,
            image: false,
            run_unvouched: false,
            // No `std` manifest is interruptible, so no supervised job hears words.
            args: false,
            nameset: false,
            // An interruptible child is built with no capabilities at all (it reports through the
            // job frame), so there is nowhere to put the machine page.
            machine: false,
            // And nowhere to put display devices either, for that same reason; a `graphical_terminal`
            // session is a plain spawn, never a supervised job.
            graphics: false,
            keyboard: false,
        },
    );
    // **Out of raw mode before the job exists** (DECISIONS §227 (how Tab reaches the shell)
    // option D). [`watch`] learns of `^C` from the terminal's count, and the terminal counts only
    // in its line discipline. The watermark is taken after the switch, so a `^C` from here on is
    // this job's, and one pressed at the prompt, which was a byte to [`edit_line`], never is.
    leave_raw();
    CONSUMED.store(intr_count(), core::sync::atomic::Ordering::Relaxed);
    send(SPAWN, w0, w1, w2);
    delegate(job_ut, abi::rights::ALL);
    send_cap(job_fr);

    // The progenitor acks once the child is running: that is the shell's go-ahead to start watching.
    if receive(RESULT).0 != spawnproto::SPAWN_OK {
        failed();
        print(b"  could not spawn (the progenitor is out of memory)\n");
        cap_delete(job_fr);
        cap_delete(job_ut);
        return;
    }
    print(b"  running ");
    print(e.prog.name().as_bytes());
    print(b" in the foreground. ^C interrupts it.\n");

    watch(jf, job_ut);

    // Account for every ^C up to now, so the job's interrupts do not leak into the next one.
    CONSUMED.store(intr_count(), core::sync::atomic::Ordering::Relaxed);
    // Drop our caps. If the job was reclaimed, the untyped cap is already stale; cap_delete just
    // frees the slot.
    cap_delete(job_fr);
    cap_delete(job_ut);
}

/// Watch a running supervised job: poll its done flag and the terminal's `^C` count, drive the
/// escalation policy, and act. The busy-poll with `yield` is how one thread watches two things (the
/// job and `^C`) with only blocking primitives and no non-blocking receive (DECISIONS §24, wait A).
pub(super) fn watch(jf: MappedWindow, job_ut: u64) {
    // The baseline is the session watermark, not a fresh read: a ^C counted during the spawn is
    // already reflected in intr_count but not yet in CONSUMED, so diffing against CONSUMED sees it.
    let base = CONSUMED.load(core::sync::atomic::Ordering::Relaxed);
    let mut esc = Escalation::new();
    let mut fed: u64 = 0;
    loop {
        // Finished on its own (cooperatively or naturally)?
        if jf.read::<u64>(job_page_frame::DONE as u64) != 0 {
            let status = jf.read::<u64>(job_page_frame::STATUS as u64);
            let beats = jf.read::<u64>(job_page_frame::HEARTBEAT as u64);
            reclaim(job_ut);
            report_finished(status, beats);
            return;
        }
        // Fold in every ^C the terminal has seen since we started watching.
        let n = intr_count().wrapping_sub(base);
        let mut action = Action::None;
        while fed < n {
            fed += 1;
            let a = esc.on_interrupt();
            if !matches!(a, Action::None) {
                action = a; // Forcible (a second ^C) wins over Cooperative (the first)
            }
        }
        if matches!(action, Action::None) {
            action = esc.on_tick(); // the grace timeout: escalate a job that ignored the first ^C
        }
        match action {
            Action::Cooperative => {
                jf.write(job_page_frame::INTERRUPT as u64, 1u64);
                print(b"  ^C: asked the job to stop.\n");
            }
            Action::Forcible => {
                forcible(job_ut);
                return;
            }
            Action::None => {}
        }
        watch_pause();
    }
}

/// The (timer, notification) the watch loop sleeps on, retyped out of [`BUDGET`] on the first
/// supervised job and kept for the life of the shell. `0` is "not made yet"; `u64::MAX` is "the
/// budget refused", after which the loop yields as it did before milestone 106.
static WATCH_TIMER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static WATCH_NOTIFICATION: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// **One beat of the watch loop: a scheduler tick asleep** (milestone 106 (a wait that ends on
/// either the interrupt or the deadline)), rather than a `yield`.
///
/// Until 2026-09-26 this was `yield_now()`, which kept the shell runnable for the whole life of
/// every foreground job: a hart's worth of scheduler passes spent asking a shared page whether
/// anything had changed. Now the shell sleeps ten milliseconds between looks, which is 100 wakes a
/// second, each a page read and a `CALL` for the `^C` count. It is **not** the destination of
/// milestone 103 (`^C` stops spinning: the shell's interrupt watch, blocking), a watch that wakes
/// only when the job ends or `^C` arrives: that needs the job's exit and the terminal's `^C` to
/// signal a notification this shell waits on, which is 103's to build. See that milestone's block.
///
/// **The grace window changed meaning with it**, and is recorded rather than hidden:
/// `grant_plan::COOP_GRACE_TICKS` counts loop iterations, so 200 of them are now two seconds of
/// wall time where they were 200 yields, as long as a yield happened to take.
fn watch_pause() {
    use core::sync::atomic::Ordering::Relaxed;
    let mut t = WATCH_TIMER.load(Relaxed);
    if t == 0 {
        // Slots up to the directory's are the ones whose emptiness means something.
        let (timer, notification) =
            retype_sleeper(BUDGET, DIR_TERMINAL + 1).unwrap_or((u64::MAX, 0));
        WATCH_NOTIFICATION.store(notification, Relaxed);
        WATCH_TIMER.store(timer, Relaxed);
        t = timer;
    }
    if t == u64::MAX {
        yield_now();
        return;
    }
    let deadline = now() + cntfrq() / 100;
    if sleep_until(t, WATCH_NOTIFICATION.load(Relaxed), deadline) < 0 {
        yield_now();
    }
}

/// Report a job that stopped on its own.
fn report_finished(status: u64, beats: u64) {
    if status == job_page_frame::STATUS_INTERRUPTED {
        // Interrupted is a job that did not finish what it was asked to do, so `&&` must not carry
        // on past it. It is [`failed`] rather than [`refused`] because it ran: the shell agreed to
        // it, spawned it, and a person stopped it.
        failed();
        print(b"  the job caught the interrupt and stopped cleanly after ");
        print_num(beats);
        print(b" work units.\n");
    } else {
        print(b"  the job finished after ");
        print_num(beats);
        print(b" work units.\n");
    }
}

/// The forcible tier: tear the job's region down with the owner's `DESTROY`. The shell holds the
/// untyped the child was built from, so this reclaims its every object. Once the §16 amendment lands
/// (DESTROY force-kills a live resident thread), this ends even a runaway that ignored the first ^C.
fn forcible(job_ut: u64) {
    failed();
    print(b"  ^C again: tearing the job down.\n");
    if reclaim(job_ut) {
        print(b"  the job's process was torn down and its memory reclaimed.\n");
    } else {
        print(b"  teardown refused: DESTROY force-kills a live thread once the kernel amendment lands.\n");
    }
}

/// How long [`reclaim`] keeps retrying before it reports a refusal: a hundred of the kernel's 10 ms
/// timeslices. A force-killed thread is converted to a corpse at its own core's next preemption
/// (notes/object-revocation.md, "DESTROY force-kills a runaway"), so the wait that matters is one
/// timeslice, and a bound on time says that where a bound on attempts cannot.
const RECLAIM_PATIENCE_NS: u64 = 1_000_000_000;

/// Reclaim the job's region, retrying because a cooperatively-exiting child may still be finishing
/// its last instruction, and a force-killed one is still resident until its core's next tick
/// (DESTROY refuses while a thread is live). Returns whether it succeeded.
///
/// **Bounded by time, not by a count of 256 attempts, since 2026-09-25.** A yield returns at once
/// when this core has nothing else to run, so 256 of them could all land inside the one timeslice
/// the killed runaway still had on another core. Measured at a real prompt on aarch64: two of six
/// `interrupt_ignorer` teardowns printed "teardown refused", and each refusal also leaked the job's
/// pages from this shell's budget, which ran out on the seventh job.
pub(super) fn reclaim(job_ut: u64) -> bool {
    let start = monotonic_nanos();
    loop {
        if destroy_memory_region(job_ut) == 0 {
            return true;
        }
        if monotonic_nanos().wrapping_sub(start) > RECLAIM_PATIENCE_NS {
            return false;
        }
        yield_now();
    }
}

/// RETYPE one page of our budget into a `PageFrame` capability we hold. `None` when the budget is spent.
fn retype_page_frame() -> Option<u64> {
    let r = user_mode_runtime::retype_into_page_frame(BUDGET);
    if r < 0 { None } else { Some(r as u64) }
}

/// Map the frame in `slot` read/write at `va` in our own space; page tables come from our budget.
fn map_page_frame(slot: u64, va: u64) -> bool {
    user_mode_runtime::map_page_frame(slot, va, true, BUDGET)
}

/// Ask the terminal how many `^C` it has seen (a non-blocking poll; see `proto::OPERATION_INTRCOUNT`).
pub(super) fn intr_count() -> u64 {
    call(TERM, proto::req(proto::OPERATION_INTRCOUNT, 0), 0).0
}

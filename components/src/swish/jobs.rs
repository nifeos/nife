//! **Interruptible jobs**, the shell's side of the two-tier `^C` path (DECISIONS §24
//! (interrupting the foreground process)): `spawn_interruptible`, `watch`, `forcible` and `reclaim`.
//! And **job control** (milestone 48 (job control)): a trailing `&`, `jobs`, `wait`, `kill`, `fg`,
//! `bg` and `^Z`, all acting on the capabilities this shell holds for a supervised job, its TCB
//! included (DECISIONS §309 (a supervised job's `ThreadControlBlock` goes to the shell that asked
//! for it)).

//! # BUGS
//!
//! - Only a supervised job, the one kind this shell holds capabilities for, can run in the
//!   background; `&` on any other is refused with a sentence (notes/shell/the-program.md).
//! - A finished background job is reported when `jobs`, `wait` or `fg` next looks, not before the
//!   prompt, since the shell is blocked reading the line.
//! - Background jobs finishing out of order leave a hole in this shell's budget until the later
//!   ones go (notes/object-revocation.md, LIFO return of pages).
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
/// It must stay clear of every fixed window this shell has, [`OUT_VA`] and [`FS_VA`] among them: the
/// assertion in `wiring.rs` holds that, and the `interrupt_heeder` line in `script/swish-check`
/// runs a supervised job. 16 MiB up is a pair page on the address-space map (milestone 206 (a
/// program image has under 896 KiB)), clear of the image window at 64 MiB, with a 2 MiB-aligned
/// table's worth of pages under it: the first job costs one page-table page and the next 511 none.
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
pub(super) fn spawn_interruptible(jobs: &mut Jobs, e: Endowment) {
    let Some(h) = start_supervised(e, true) else {
        return;
    };
    print(b"  running ");
    print(h.program.name().as_bytes());
    print(b" in the foreground. ^C interrupts it. ^Z suspends it.\n");
    watch_to_the_end(jobs, h);
}

/// Watch a job in the foreground until it ends or is suspended. An ended job's `^C` are accounted
/// for and its capabilities given back; a suspended one goes into `jobs`, stopped, and the prompt
/// comes back.
fn watch_to_the_end(jobs: &mut Jobs, mut h: Held) {
    let suspended = watch(&h);
    // Account for every ^C and ^Z up to now, so this job's keys do not leak into the next one.
    CONSUMED.store(intr_count(), core::sync::atomic::Ordering::Relaxed);
    SUSPENDS_SEEN.store(suspend_count(), core::sync::atomic::Ordering::Relaxed);
    if !suspended {
        return release(h);
    }
    h.stopped = true;
    if h.started == 0 {
        jobs.started += 1;
        h.started = jobs.started;
    }
    let Some(i) = jobs.held.iter().position(Option::is_none) else {
        // Nowhere to list it, and a stopped job nobody can name would never run again.
        thread_control_block_resume(h.tcb);
        refused();
        print(
            b"  this shell holds as many jobs as it can list; the job runs on in the foreground\n",
        );
        h.stopped = false;
        return watch_to_the_end(jobs, h);
    };
    jobs.held[i] = Some(h);
    print_job(i + 1, &h);
    print(b" stopped\n");
}

/// Give back the slots this shell held for a job. If the job was reclaimed, every slot is already
/// stale and deleting it only frees the slot.
fn release(h: Held) {
    give_back(h.job_fr, h.job_ut);
    supervision_protocol::forget_thread_control_block(h.tcb);
}

/// Delete this shell's slots for a job's frame and region, on every path that ends with them.
fn give_back(job_fr: u64, job_ut: u64) {
    cap_delete(job_fr);
    cap_delete(job_ut);
}

/// **Mint, direct and start a supervised job**, and hand back what this shell holds for it once the
/// progenitor says it is running. `None` has already said why. `foreground` is whether the shell
/// will watch it now, which decides whether the terminal leaves raw mode before it exists.
fn start_supervised(e: Endowment, foreground: bool) -> Option<Held> {
    // Mint the job's resources. RETYPE the shared frame first, then SPLIT the construction budget,
    // so the budget is the top of our watermark and DESTROY returns its pages cleanly (LIFO).
    let Some(job_fr) = retype_page_frame() else {
        failed();
        print(b"  this shell's memory budget is exhausted; nothing left to grant\n");
        return None;
    };
    let Some(job_ut) = memory_region_split(JOB_UNTYPED_PAGES) else {
        cap_delete(job_fr);
        failed();
        print(b"  this shell's memory budget is exhausted; nothing left to grant\n");
        return None;
    };

    // Map the shared frame into our own space so we can signal the job and read its status.
    let va = SH_JOBFRAME_NEXT.fetch_add(PAGE, core::sync::atomic::Ordering::Relaxed);
    if !JOBFRAME_WINDOWS.contains(&va) {
        give_back(job_fr, job_ut);
        failed();
        print(b"  this shell has run as many supervised jobs as it has room to watch\n");
        return None;
    }
    if !map_page_frame(job_fr, va) {
        give_back(job_fr, job_ut);
        failed();
        print(b"  could not map the job frame\n");
        return None;
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
        e.program.id(),
        e.program_argument,
        e.memory_grant_pages,
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
            directory_grant: false,
            second_directory_grant: false,
            // A supervised job's completion signal is already the shared job frame's `DONE` flag,
            // not `result_ep`, so there is nothing here for DECISIONS §106's narrowing to replace.
            screen: false,
            image: false,
            run_unvouched: false,
            // No `std` manifest is interruptible, so no supervised job hears words.
            program_arguments: false,
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
    // A background job stays in raw mode: the prompt comes straight back, and a `^C` typed there
    // is a byte to the line editor, never this job's.
    if foreground {
        leave_raw();
        CONSUMED.store(intr_count(), core::sync::atomic::Ordering::Relaxed);
        SUSPENDS_SEEN.store(suspend_count(), core::sync::atomic::Ordering::Relaxed);
    }
    send(SPAWN, w0, w1, w2);
    delegate(job_ut, abi::rights::ALL);
    send_cap(job_fr);

    // The progenitor acks once the child is running: that is the shell's go-ahead to start watching.
    let answer = receive(RESULT).0;
    if answer != spawnproto::SPAWN_OK {
        failed();
        print(swish::spawn_failure_sentence(answer));
        give_back(job_fr, job_ut);
        return None;
    }
    // **The job's TCB, the one message after `SPAWN_OK`** (DECISIONS §309 (a supervised job's
    // `ThreadControlBlock` goes to the shell that asked for it)), received before anything else
    // because the progenitor is blocked sending it. `WRITE` without `GRANT`: this shell can
    // suspend and resume its job and cannot pass that on.
    let (_, tcb, _) = receive_cap(RESULT);
    Some(Held {
        jf,
        job_ut,
        job_fr,
        tcb,
        program: e.program,
        started: 0,
        stopped: false,
    })
}

/// Watch a running supervised job: poll its done flag and the terminal's `^C` and `^Z` counts,
/// drive the escalation policy, and act. The busy-poll with `yield` is how one thread watches
/// three things with only blocking primitives and no non-blocking receive (DECISIONS §24, wait A).
/// `true` means a `^Z` suspended the job, which is still held; `false` means it is over.
pub(super) fn watch(h: &Held) -> bool {
    let (jf, job_ut) = (h.jf, h.job_ut);
    // The baseline is the session watermark, not a fresh read: a ^C counted during the spawn is
    // already reflected in intr_count but not yet in CONSUMED, so diffing against CONSUMED sees it.
    let base = CONSUMED.load(core::sync::atomic::Ordering::Relaxed);
    let suspends = SUSPENDS_SEEN.load(core::sync::atomic::Ordering::Relaxed);
    let mut esc = Escalation::new();
    let mut fed: u64 = 0;
    loop {
        // Finished on its own (cooperatively or naturally)?
        if jf.read::<u64>(job_page_frame::DONE as u64) != 0 {
            let status = jf.read::<u64>(job_page_frame::STATUS as u64);
            let beats = jf.read::<u64>(job_page_frame::HEARTBEAT as u64);
            reclaim(job_ut);
            report_finished(status, beats);
            return false;
        }
        // **`^Z`: suspend it and give the prompt back** (milestone 48 (job control), §300 (the
        // paused state, a TCB verb, silent)). A refusal means the job ended in the same instant,
        // and the next pass finds it done.
        if suspend_count() != suspends && thread_control_block_suspend(h.tcb) == 0 {
            return true;
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
                return false;
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
/// either the interrupt or the deadline)), rather than a `yield`: ten milliseconds between looks,
/// 100 wakes a second, each a page read and a `CALL` for the `^C` count. A watch that wakes only
/// when the job ends or `^C` arrives needs both to signal a notification this shell waits on, which
/// is milestone 103 (`^C` stops spinning: the shell's interrupt watch, blocking).
/// `grant_plan::COOP_GRACE_TICKS` counts loop iterations, so its 200 are two seconds of wall time.
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

/// The terminal's `^Z` count already accounted for, [`CONSUMED`]'s twin: taken when a job comes to
/// the foreground, so only a `^Z` typed while it is watched suspends it.
static SUSPENDS_SEEN: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// Ask the terminal how many `^Z` it has seen (`proto::OPERATION_SUSPEND_COUNT`). A terminal that
/// does not count them answers `BAD_REQUEST`, read here as a count that never moves.
fn suspend_count() -> u64 {
    let n = call(TERM, proto::req(proto::OPERATION_SUSPEND_COUNT, 0), 0).0;
    if n == proto::BAD_REQUEST { 0 } else { n }
}

/// Ask the terminal how many `^C` it has seen (a non-blocking poll; see `proto::OPERATION_INTRCOUNT`).
pub(super) fn intr_count() -> u64 {
    call(TERM, proto::req(proto::OPERATION_INTRCOUNT, 0), 0).0
}

// ---- background jobs (milestone 48 (job control), phase one) ----

/// The most background jobs this shell holds at once. Each costs [`JOB_UNTYPED_PAGES`] plus a frame
/// of this shell's budget for as long as it runs, so the budget is the real bound; this is the
/// table's.
pub(super) const MAX_JOBS: usize = 8;

/// **What this shell holds for one supervised job**: the shared frame it signals and reads, the
/// region the job was built from, the frame's capability and the job's TCB. Nothing else is needed
/// to watch it, ask it to stop, pause it, or tear it down, which is why a job is exactly this.
#[derive(Clone, Copy)]
pub(super) struct Held {
    jf: MappedWindow,
    job_ut: u64,
    job_fr: u64,
    /// `WRITE` on the job's thread, for `SUSPEND` and `RESUME` (DECISIONS §309).
    tcb: u64,
    program: grant_plan::Program,
    /// Suspended by `^Z`, and not running until `bg` or `fg` resumes it.
    stopped: bool,
    /// When it went into the background, counted in jobs started, so `fg` with no operand takes
    /// the newest. Zero for a job that never left the foreground.
    started: u64,
}

impl Held {
    fn done(&self) -> bool {
        self.jf.read::<u64>(job_page_frame::DONE as u64) != 0
    }
}

/// **The background jobs**, numbered from one as `jobs` prints them. A finished job keeps its number
/// until it is reported, so `[2]` never silently becomes another job.
pub(super) struct Jobs {
    held: [Option<Held>; MAX_JOBS],
    started: u64,
}

impl Jobs {
    pub(super) const fn new() -> Self {
        Jobs {
            held: [None; MAX_JOBS],
            started: 0,
        }
    }

    fn get(&self, n: usize) -> Option<Held> {
        self.held.get(n.wrapping_sub(1)).copied().flatten()
    }

    fn take(&mut self, n: usize) -> Option<Held> {
        self.held.get_mut(n.wrapping_sub(1)).and_then(Option::take)
    }

    fn newest(&self) -> Option<usize> {
        (1..=MAX_JOBS)
            .filter_map(|n| self.get(n).map(|h| (h.started, n)))
            .max()
            .map(|(_, n)| n)
    }

    fn is_empty(&self) -> bool {
        self.held.iter().all(Option::is_none)
    }
}

/// **Run a line that ended in `&`** ([`swish::sequence::background`]). Exactly one command, and it
/// must plan to a supervised job; anything else is refused with nothing spawned.
pub(super) fn background(nav: &mut Nav, cmd: &[u8]) {
    let one = matches!(sequence::split(cmd), Ok(s) if s.len() == 1)
        && matches!(swish::route(cmd), Route::One(_));
    if cmd.is_empty() || !one {
        refused();
        print(b"  & runs one command in the background, not a chain or a pipeline\n");
        return;
    }
    let spec = match grant_plan::parse(cmd) {
        Command::Run(spec) if !spec.program.contains(&b'/') => spec,
        _ => return not_supervised(),
    };
    let expanded = match expansion(nav, &spec) {
        Ok(e) => e,
        Err(Say::Cannot(r)) => return refuse(spec, r),
        Err(said) => return say(said),
    };
    match grant_plan::plan(&spec, holdings(nav), expanded) {
        Err(r) => refuse(spec, r),
        Ok(endow) if endow.interruptible => spawn_background(&mut nav.jobs, endow),
        Ok(_) => not_supervised(),
    }
}

fn not_supervised() {
    refused();
    print(b"  & runs only a supervised job: for any other program this shell holds nothing to wait on or stop\n");
}

/// Start a supervised job and keep what this shell holds for it, without watching it.
fn spawn_background(jobs: &mut Jobs, e: Endowment) {
    // The slot is found before anything is minted, so a full table spawns nothing.
    let Some(i) = jobs.held.iter().position(Option::is_none) else {
        refused();
        print(b"  this shell already holds as many background jobs as it can list\n");
        return;
    };
    let Some(mut h) = start_supervised(e, false) else {
        return;
    };
    jobs.started += 1;
    h.started = jobs.started;
    jobs.held[i] = Some(h);
    print(b"  [");
    print_num(i as u64 + 1);
    print(b"] ");
    print(h.program.name().as_bytes());
    print(b" running in the background\n");
}

/// Reclaim a finished job's region and give back its slots.
fn collect(h: Held) -> bool {
    let ok = reclaim(h.job_ut);
    release(h);
    ok
}

fn no_such_job(word: &[u8]) {
    refused();
    print(b"  ");
    print(word);
    print(b": this shell holds no such job\n");
}

fn print_job(n: usize, h: &Held) {
    print(b"  [");
    print_num(n as u64);
    print(b"] ");
    print(h.program.name().as_bytes());
}

/// `jobs`: every background job, running or done. A done job is reported once and collected.
pub(super) fn list_jobs(jobs: &mut Jobs) {
    if jobs.is_empty() {
        print(b"  no jobs\n");
        return;
    }
    for n in 1..=MAX_JOBS {
        let Some(h) = jobs.get(n) else { continue };
        print_job(n, &h);
        if h.done() {
            let beats = h.jf.read::<u64>(job_page_frame::HEARTBEAT as u64);
            print(b" done after ");
            print_num(beats);
            print(b" work units\n");
            jobs.take(n);
            collect(h);
        } else if h.stopped {
            print(b" stopped\n");
        } else {
            print(b" running\n");
        }
    }
}

/// `wait [%N]`: until the named job finishes, or every job. A `^C` stops the waiting and leaves
/// the jobs running, which is bash's behavior and the only one that fits: the jobs are in the
/// background precisely so a keystroke at the terminal is not theirs.
pub(super) fn wait_jobs(jobs: &mut Jobs, tail: &[u8]) {
    let only = match grant_plan::job_operand(tail) {
        Ok(Some(n)) if jobs.get(n).is_none() => return no_such_job(b"wait"),
        Ok(only) => only,
        Err(()) => return no_such_job(b"wait"),
    };
    if jobs.is_empty() {
        print(b"  no jobs to wait for\n");
        return;
    }
    let waited = |n: usize| only.is_none_or(|m| m == n);
    leave_raw();
    let base = intr_count();
    CONSUMED.store(base, core::sync::atomic::Ordering::Relaxed);
    print(b"  waiting. ^C interrupts it.\n");
    loop {
        let mut left = 0;
        for n in (1..=MAX_JOBS).filter(|&n| waited(n)) {
            let Some(h) = jobs.get(n) else { continue };
            // A stopped job will not finish until it is resumed, so it is not waited for.
            if h.stopped {
                continue;
            }
            if !h.done() {
                left += 1;
                continue;
            }
            let status = h.jf.read::<u64>(job_page_frame::STATUS as u64);
            let beats = h.jf.read::<u64>(job_page_frame::HEARTBEAT as u64);
            jobs.take(n);
            collect(h);
            print_job(n, &h);
            print(b":\n");
            report_finished(status, beats);
        }
        if left == 0 {
            break;
        }
        if intr_count() != base {
            failed();
            print(b"  ^C: stopped waiting; the job keeps running in the background\n");
            break;
        }
        watch_pause();
    }
    CONSUMED.store(intr_count(), core::sync::atomic::Ordering::Relaxed);
}

/// `kill [-9] %N`: DECISIONS §24's two tiers, aimed at a background job.
pub(super) fn kill_job(jobs: &mut Jobs, tail: &[u8]) {
    let Ok((force, n)) = grant_plan::kill_operands(tail) else {
        refused();
        print(b"  kill: name a job, as kill %N or kill -9 %N\n");
        return;
    };
    let Some(h) = jobs.get(n) else {
        return no_such_job(b"kill");
    };
    if !force {
        h.jf.write(job_page_frame::INTERRUPT as u64, 1u64);
        // A stopped job is resumed so it can see the request, which is bash's `kill` on a stopped
        // job: the signal, then `SIGCONT`. It runs on in the background.
        if h.stopped
            && let Some(held) = jobs.held[n - 1].as_mut()
        {
            thread_control_block_resume(h.tcb);
            held.stopped = false;
        }
        print(b"  asked job ");
        print_num(n as u64);
        print(b" to stop\n");
        return;
    }
    jobs.take(n);
    print_job(n, &h);
    if collect(h) {
        print(b": its process was torn down and its memory reclaimed\n");
    } else {
        failed();
        print(b": teardown refused, and the job is no longer listed\n");
    }
}

/// `fg [%N]`: watch a background job as if it had been started in the foreground, resuming it
/// first if it is stopped. Nothing is revoked or granted, because no job holds terminal input;
/// foreground is the shell watching.
pub(super) fn foreground(jobs: &mut Jobs, tail: &[u8]) {
    let n = match grant_plan::job_operand(tail) {
        Ok(Some(n)) => Some(n),
        Ok(None) => jobs.newest(),
        Err(()) => None,
    };
    let Some(mut h) = n.and_then(|n| jobs.take(n)) else {
        return no_such_job(b"fg");
    };
    leave_raw();
    CONSUMED.store(intr_count(), core::sync::atomic::Ordering::Relaxed);
    SUSPENDS_SEEN.store(suspend_count(), core::sync::atomic::Ordering::Relaxed);
    if h.stopped {
        thread_control_block_resume(h.tcb);
        h.stopped = false;
    }
    print(b"  running ");
    print(h.program.name().as_bytes());
    print(b" in the foreground. ^C interrupts it. ^Z suspends it.\n");
    watch_to_the_end(jobs, h);
}

/// `bg [%N]`: resume a stopped job without watching it, the newest stopped job when no operand
/// names one. It runs on in the background, where `jobs`, `wait`, `kill` and `fg` find it.
pub(super) fn resume_in_background(jobs: &mut Jobs, tail: &[u8]) {
    let n = match grant_plan::job_operand(tail) {
        Ok(Some(n)) => Some(n),
        Ok(None) => (1..=MAX_JOBS)
            .filter_map(|n| jobs.get(n).filter(|h| h.stopped).map(|h| (h.started, n)))
            .max()
            .map(|(_, n)| n),
        Err(()) => None,
    };
    let Some((n, held)) = n.and_then(|n| jobs.held.get_mut(n - 1)?.as_mut().map(|h| (n, h))) else {
        return no_such_job(b"bg");
    };
    if !held.stopped {
        refused();
        print(b"  bg: that job is already running\n");
        return;
    }
    thread_control_block_resume(held.tcb);
    held.stopped = false;
    let h = *held;
    print_job(n, &h);
    print(b" running in the background\n");
}

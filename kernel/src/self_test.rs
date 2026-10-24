//! **The kernel proving it works on this machine, before it hands the machine to anybody.**
//!
//! The second rung of milestone 268 (every architecture boots the same way): every check runs on
//! every architecture, a failure prints and the boot carries on, and a program reads the verdict
//! (`boot_ladder::SELF_TEST`, matched by `crates/serial_console`). notes/boot-ladder.md has the
//! verdict's contract, and notes/self-test-checks.md each check's design and the full account of
//! the BUGS below.

//! # BUGS
//!
//! - Nothing proves an architecture ran the right set; review guards the list.
//! - The checks cost time on every boot: about a quarter of a second, and up to six spawns.
//! - [`blockdev`]'s driver is left running, holding disk 0.
//! - [`budget`]'s helper thread can be left parked if the process never reports.
//! - [`budget`] and [`teardown`] bracket the machine's used-frame count, sound only while nothing
//!   else allocates.
//! - A check whose operation never returns still hangs the boot; that needs a watchdog.

use core::sync::atomic::{AtomicU64, Ordering};

// The verdict's fixed strings come from `crates/boot_ladder`, so this module and
// `crates/serial_console` hold one copy of the contract (AGENTS.md rule 7).
use boot_ladder::{SELF_TEST as VERDICT, SELF_TEST_ABSENT as ABSENT, SELF_TEST_FAILED as FAILED};

use crate::{arch, memory, println, sched};

/// The timer the checks read. A module rather than `arch::timer` directly, so an injected build
/// can stop the counter these checks see without stopping the one the scheduler runs on.
mod timer {
    pub use crate::arch::timer::{frequency, ticks};

    /// The free-running counter, or a stopped one when the build injects a failure.
    pub(crate) fn now() -> u64 {
        if super::INJECT {
            0x0268
        } else {
            crate::arch::timer::now()
        }
    }
}

/// How many checks there are: the length of `boot_ladder::SELF_TEST_CHECKS`, and not a count of
/// the calls below. That difference is the fix for this module's old parity hole, where a check cut
/// from one architecture took its own evidence with it and the verdict still said *N of N*.
const CHECKS: usize = boot_ladder::SELF_TEST_CHECKS.len();

/// **Make one check report failure**, so the verdict can be seen red: `cargo xtask boot-check
/// --inject` builds with this on and asserts the gate goes red.
///
/// It also stops the counter the checks read (this module's `timer::now`), so every injected run
/// proves the waits are bounded. A `cfg!` rather than `#[cfg]`, so both arms compile on every
/// build.
///
/// Name: provisional (milestone 268 (every architecture boots the same way)).
const INJECT: bool = cfg!(feature = "self_test_injection");

/// The running count, and the names of whatever failed or was absent.
///
/// No allocation: the name lists are fixed arrays of `&'static str`, because this runs before the
/// heap is interesting and because a self-test that could fail to report a failure by running out
/// of memory would be reporting on the wrong thing.
struct Tally {
    passed: usize,
    ran: usize,
    /// Which of `boot_ladder::SELF_TEST_CHECKS` recorded an outcome, by index.
    seen: [bool; CHECKS],
    failed: [&'static str; CHECKS],
    failures: usize,
    absent: [&'static str; CHECKS],
    absences: usize,
}

impl Tally {
    const fn new() -> Self {
        Self {
            passed: 0,
            ran: 0,
            seen: [false; CHECKS],
            failed: [""; CHECKS],
            failures: 0,
            absent: [""; CHECKS],
            absences: 0,
        }
    }

    fn fail(&mut self, name: &'static str) {
        if self.failures < CHECKS {
            self.failed[self.failures] = name;
            self.failures += 1;
        }
    }

    /// Mark `name` as having recorded an outcome, and say whether it is on the shared list. A
    /// check not on it is a failure however it went: an architecture running a check the others do
    /// not is the same parity hole as one skipping a check they run.
    fn see(&mut self, name: &'static str) -> bool {
        self.ran += 1;
        match boot_ladder::SELF_TEST_CHECKS
            .iter()
            .position(|&n| n == name)
        {
            Some(i) => {
                self.seen[i] = true;
                true
            }
            None => {
                println!("  self-test       : {name} is not in boot_ladder::SELF_TEST_CHECKS");
                false
            }
        }
    }

    /// Record one check's outcome and print its line.
    ///
    /// The detail is `core::fmt::Arguments` rather than a `&str`, so a check can put the numbers it
    /// measured in its own line without formatting into a buffer first. The numbers are the
    /// diagnosis: `ok` on its own tells a reader nothing they can act on when the next boot says
    /// `FAILED`.
    fn record(&mut self, name: &'static str, ok: bool, detail: core::fmt::Arguments) {
        let ok = self.see(name) && ok;
        if ok {
            self.passed += 1;
        } else {
            self.fail(name);
        }
        println!(
            "  self-test       : {name:<10} {:<6}  {detail}",
            if ok { "ok" } else { "FAILED" },
        );
    }

    /// **Record that this machine has nothing for `name` to test**, with the reason
    /// (`boot_ladder::SELF_TEST_ABSENT`; calef's ruling of 2026-10-20 (UTC) on #2447).
    ///
    /// Absent is neither a pass nor a failure, and the verdict says so by name. Whether an absence
    /// is acceptable is a fact about the machine, which the kernel cannot know: `serial_console`'s
    /// board profiles name the checks that may not be absent on each board, and read an absence of
    /// one of those as a failure. The reason is stated because "no device" must be distinguishable
    /// from "no device this kernel looked for".
    fn absent(&mut self, name: &'static str, why: core::fmt::Arguments) {
        if !self.see(name) {
            self.fail(name);
            return;
        }
        if self.absences < CHECKS {
            self.absent[self.absences] = name;
            self.absences += 1;
        }
        println!("  self-test       : {name:<10} absent  {why}");
    }

    /// The verdict line. See [`VERDICT`] for why it is shaped the way it is.
    ///
    /// **The total is the shared list's length, not how many checks ran**, and a listed check that
    /// never recorded an outcome is named as a failure here. So a kernel that ran four of the five
    /// says `4 of 5 passed, 1 FAILED: scheduler` on every architecture, rather than `4 of 4 passed`.
    /// Absences follow the count and come before any failure, so a matcher that reads the names
    /// after `FAILED: ` to the end of the line reads only failures:
    /// `nife self-test: 9 of 13 passed, 3 absent: blockdev clock trng, 1 FAILED: entropy`.
    fn verdict(&mut self) {
        for (i, &name) in boot_ladder::SELF_TEST_CHECKS.iter().enumerate() {
            if !self.seen[i] {
                println!("  self-test       : {name:<10} FAILED  did not run on this architecture");
                self.fail(name);
            }
        }
        // Built by printing rather than by joining, so there is no buffer to size and no allocation
        // on the path that reports a broken machine.
        crate::print!("{VERDICT}{} of {CHECKS} passed", self.passed);
        if self.absences > 0 {
            crate::print!(", {} {ABSENT}", self.absences);
            print_names(&self.absent[..self.absences]);
        }
        if self.failures > 0 {
            crate::print!(", {} {FAILED}", self.failures);
            print_names(&self.failed[..self.failures]);
        }
        println!();
    }
}

/// Names separated by single spaces, with no newline.
fn print_names(names: &[&str]) {
    for (i, name) in names.iter().enumerate() {
        if i > 0 {
            crate::print!(" ");
        }
        crate::print!("{name}");
    }
}

/// **Run the set and print the verdict.**
///
/// Called from `kernel_main` on all three architectures, after the machine description and before
/// anything is handed to userspace. Everything it needs is up by then: paging, the frame allocator,
/// the timer with interrupts on, and the scheduler with its idle thread.
pub fn run() {
    let mut tally = Tally::new();

    // Each check notes its boot stage first, so a dump during a wedged check names it. `jh7110`
    // records both `clock` and `trng`, and `entropy` runs after it because on a JH7110 it draws
    // from the service that bring-up wired.
    let runs: [fn(&mut Tally); RUNS] = [
        exceptions, mapping, frames, timer, scheduler, preemption, userspace, budget, supervised,
        teardown, blockdev, deviceirq, jh7110, entropy,
    ];
    for (stage, check) in (FIRST_STAGE..).zip(runs) {
        sched::note_boot_stage(stage);
        check(&mut tally);
    }

    tally.verdict();
}

/// The boot stage the first check notes. Stages below it are the bring-up's, which notes none
/// today; the number is kept from the riscv64 tour this replaced so that a board log's stage reads
/// the same way across the change.
const FIRST_STAGE: u32 = 3;

/// How many check functions [`run`] calls: one per check, but one for `clock` and `trng` together.
const RUNS: usize = CHECKS - 1;

/// **The stage that means the self-test finished and the boot's middle stages ran to their end**,
/// noted beside `boot_ladder::TOUR` in `kernel_main`: the stage after the last check's. Board logs
/// from before the checks replaced the tours read 11 as finished, and 13-check kernels 15; this reads 17.
///
/// Name: provisional, minted 2026-10-20 (UTC) by lane `milestone/890-tours-to-self-test`.
pub const TOUR_STAGE: u32 = FIRST_STAGE + RUNS as u32;

/// **A breakpoint is caught and stepped over.**
///
/// The whole trap round trip: the vector is installed where the hardware accepts it, a synchronous
/// exception reaches the dispatcher, the cause is recognized, and the return lands after the
/// trapping instruction. Getting the last wrong is an infinite loop rather than a wrong answer.
fn exceptions(tally: &mut Tally) {
    let caught = arch::exceptions::self_test();
    let ok = caught >= 1 && !INJECT;
    if INJECT {
        tally.record(
            "exceptions",
            ok,
            format_args!("failure injected by the self_test_injection feature ({caught} caught)"),
        );
    } else {
        tally.record(
            "exceptions",
            ok,
            format_args!("a breakpoint was caught and stepped over ({caught})"),
        );
    }
}

/// **The kernel can map a page it did not have at boot, and unmap it again**, which is what the
/// kernel stack allocator stands on.
///
/// The address is computed from this machine's RAM, one gigabyte above the direct map's end,
/// rather than a constant that is free on one machine only. See notes/self-test-checks.md.
fn mapping(tally: &mut Tally) {
    use paging::Flags;

    let Some(ram_top) = memory::ram_regions()
        .map(|(start, size)| start + size)
        .max()
    else {
        tally.record(
            "mapping",
            false,
            format_args!("this machine described no RAM, so there is no free address to try"),
        );
        return;
    };
    // One gigabyte above the direct map's last byte: past everything `mmu::init` claimed, and
    // inside the kernel half on all three.
    let va = arch::mmu::phys_to_virt(ram_top + (1 << 30));
    let Some(frame) = memory::alloc() else {
        tally.record(
            "mapping",
            false,
            format_args!("the allocator had no frame to map at {va:#x}"),
        );
        return;
    };
    let pa = frame.addr();

    if let Err(e) = arch::mmu::map_page(va, pa, Flags::kernel_data()) {
        tally.record(
            "mapping",
            false,
            format_args!("map_page({va:#x} -> {pa:#x}) refused: {e:?}"),
        );
        return;
    }
    let translated = arch::mmu::translate(va);
    // SAFETY: the line above mapped this VA read/write in the kernel's own tables, and nothing else
    // in the kernel names it.
    unsafe { (va as *mut u64).write_volatile(WITNESS) };
    // SAFETY: the same VA the write above used, still mapped read/write.
    let read_back = unsafe { (va as *const u64).read_volatile() };
    let unmapped = arch::mmu::unmap_page(va);

    let ok = translated.map(|(p, _)| p) == Some(pa)
        && read_back == WITNESS
        && unmapped == Ok(pa)
        && arch::mmu::translate(va).is_none();
    if ok {
        tally.record(
            "mapping",
            ok,
            format_args!("{va:#x} -> {pa:#x}, read back {read_back:#x}, unmapped"),
        );
        memory::free(frame);
    } else {
        tally.record(
            "mapping",
            ok,
            format_args!(
                "{va:#x} -> {pa:#x}: translate {translated:x?}, read back {read_back:#x}, unmap {unmapped:x?}"
            ),
        );
        // Leaked, not freed: a frame whose mapping state is unknown must not go back to the pool.
    }
}

/// **How many reads of an unchanged counter mean it has stopped.** The slowest counter this
/// kernel reads changes every 250 ns, far faster than a million reads, and the count is in reads
/// because time is the thing in doubt.
const STALLED_READS: u32 = 1_000_000;

/// **How many looks with a yield between them mean the counter has stopped.** A yield can cost a
/// whole 10 ms tick with [`preemption`]'s spinners runnable, so a hundred unchanged looks is a
/// stopped counter, found within about a second.
const STALLED_YIELDS: u32 = 100;

/// **The free-running counter, read by something that notices when it stops.**
///
/// The two waits below are bounded by the counter, which is right on a working machine (it makes
/// the number mean the same thing on a 62 MHz generic timer and a 2 GHz TSC) and was a hang on a
/// broken one. This makes the broken case an answer instead.
struct Counter {
    last: u64,
    unchanged: u32,
    stalled_after: u32,
}

impl Counter {
    fn new() -> Self {
        Self::stalled_after(STALLED_READS)
    }

    /// A counter that reports a stall after `reads` unchanged reads, for a wait that does more than
    /// spin between them.
    fn stalled_after(reads: u32) -> Self {
        Self {
            last: timer::now(),
            unchanged: 0,
            stalled_after: reads,
        }
    }

    /// The counter now, or `None` once it has read the same value `stalled_after` times running.
    fn read(&mut self) -> Option<u64> {
        let now = timer::now();
        if now == self.last {
            self.unchanged += 1;
            if self.unchanged >= self.stalled_after {
                return None;
            }
        } else {
            self.last = now;
            self.unchanged = 0;
        }
        Some(now)
    }
}

/// The value written through the fresh mapping and read back. `0xc0ffee` is the RISC-V tour's, kept
/// so a board transcript from before milestone 268 and one from after it read the same.
const WITNESS: u64 = 0xc0ffee;

/// **A frame goes out of the allocator and comes back**, and the accounting agrees at both ends.
///
/// The cheapest check here and the one with the widest blast radius: every other allocation in the
/// kernel, including the page tables the check above builds, comes through this. What it catches is
/// a bitmap whose free and used counts have drifted apart, which is a leak that reports itself as
/// plenty of memory right up until there is none.
fn frames(tally: &mut Tally) {
    let Some(before) = memory::stats() else {
        tally.record(
            "frames",
            false,
            format_args!("the frame allocator is not up"),
        );
        return;
    };
    let Some(frame) = memory::alloc() else {
        tally.record(
            "frames",
            false,
            format_args!("no frame available ({} free)", before.free()),
        );
        return;
    };
    let addr = frame.addr();
    let held = memory::stats().map(|s| s.used);
    memory::free(frame);
    let after = memory::stats().map(|s| s.used);

    let ok = held == Some(before.used + 1) && after == Some(before.used);
    tally.record(
        "frames",
        ok,
        format_args!(
            "one frame out and back at {addr:#x}; used {} -> {:?} -> {:?}",
            before.used, held, after
        ),
    );
}

/// **The counter advances and the tick interrupt arrives.**
///
/// Two claims: a moving counter says the time source exists, and a moving tick count says the
/// controller routes the interrupt to this core and the handler runs. Bounded by the counter, so
/// the window means the same on every timer, and it spins through [`Counter`] rather than sleeping,
/// so a stopped counter or a missing interrupt ends the check instead of the boot.
fn timer(tally: &mut Tally) {
    let hz = timer::frequency();
    let mut counter = Counter::new();
    let start = counter.last;
    let ticks_before = timer::ticks();
    // 20 ms: two tick periods at the 100 Hz `TICK_HZ` every architecture uses, so a single missed
    // tick does not read as a broken interrupt path.
    let window = hz / 50;
    let advanced = loop {
        let Some(now) = counter.read() else {
            tally.record(
                "timer",
                false,
                format_args!(
                    "the counter stopped at {:#x}: {STALLED_READS} reads without a change",
                    counter.last
                ),
            );
            return;
        };
        let advanced = now.wrapping_sub(start);
        if advanced >= window {
            break advanced;
        }
        core::hint::spin_loop();
    };
    let ticks = timer::ticks() - ticks_before;

    let ok = advanced >= window && ticks >= 1;
    tally.record(
        "timer",
        ok,
        format_args!(
            "the {}MHz counter advanced {advanced} and {ticks} tick(s) arrived in ~20ms",
            hz / 1_000_000
        ),
    );
}

/// **A thread the kernel spawned runs, and carries what it captured**: the context switch, end
/// to end. The wait is bounded by the clock, not by a count of yields, because the new thread may
/// be placed on another core. See notes/self-test-checks.md.
fn scheduler(tally: &mut Tally) {
    static SAW: AtomicU64 = AtomicU64::new(0);

    // Distinctive rather than 1: a zero-initialized static that happened to be read as "it ran"
    // would pass this check on a kernel where nothing ran at all.
    const CAPTURED: u64 = 0x0268_0268_0268_0268;

    SAW.store(0, Ordering::SeqCst);
    let Some(tid) = sched::spawn(move || SAW.store(CAPTURED, Ordering::SeqCst)) else {
        tally.record(
            "scheduler",
            false,
            format_args!("the kernel could not spawn a thread at all"),
        );
        return;
    };

    // Two seconds is far beyond honest completion; the point is that the boot continues.
    let mut counter = Counter::new();
    let deadline = counter.last + 2 * timer::frequency();
    while SAW.load(Ordering::SeqCst) != CAPTURED {
        match counter.read() {
            Some(now) if now < deadline => sched::yield_now(),
            _ => break,
        }
    }

    let saw = SAW.load(Ordering::SeqCst);
    let ok = saw == CAPTURED;
    if ok {
        tally.record(
            "scheduler",
            ok,
            format_args!("thread {tid:?} ran and carried its captured state ({saw:#x})"),
        );
    } else {
        tally.record(
            "scheduler",
            ok,
            format_args!("thread {tid:?} did not run within 2s (saw {saw:#x})"),
        );
    }
}

/// **Wait up to `seconds` for `done`, yielding between looks**, bounded by the counter as every wait
/// here is, so a stopped counter ends the wait rather than making the deadline unreachable. `true`
/// once `done` held.
fn wait_until(seconds: u64, mut done: impl FnMut() -> bool) -> bool {
    let mut counter = Counter::stalled_after(STALLED_YIELDS);
    let deadline = counter.last + seconds * timer::frequency();
    loop {
        if done() {
            return true;
        }
        match counter.read() {
            Some(now) if now < deadline => sched::yield_now(),
            _ => return done(),
        }
    }
}

/// **Threads that never yield are taken off the CPU anyway, and none of them starves**
/// (DECISIONS §5 (preemptive threads with real stacks)).
///
/// One more spinner than there are cores online, so at least one core runs two and the timer has
/// to take the CPU from one to give it to the other. Every spinner must advance, and the global
/// preemption count must move.
fn preemption(tally: &mut Tally) {
    use core::sync::atomic::AtomicBool;

    const MAX: usize = crate::cpu::MAX_CPUS + 1;
    static PROGRESS: [AtomicU64; MAX] = [const { AtomicU64::new(0) }; MAX];
    static STOP: AtomicBool = AtomicBool::new(false);

    let spinners = (crate::smp::online_count() + 1).min(MAX);
    STOP.store(false, Ordering::SeqCst);
    let mut tids = [None; MAX];
    for (i, slot) in tids.iter_mut().enumerate().take(spinners) {
        PROGRESS[i].store(0, Ordering::Relaxed);
        *slot = sched::spawn(move || {
            while !STOP.load(Ordering::Relaxed) {
                PROGRESS[i].fetch_add(1, Ordering::Relaxed);
            }
        });
    }
    if tids[..spinners].iter().any(Option::is_none) {
        STOP.store(true, Ordering::SeqCst);
        tally.record(
            "preemption",
            false,
            format_args!("the kernel could not spawn {spinners} threads"),
        );
        return;
    }

    let p0 = sched::preemptions();
    // A fifth of a second spent yielding, so the spinners share this core as well.
    let start = timer::now();
    wait_until(1, || {
        timer::now().wrapping_sub(start) >= timer::frequency() / 5
    });
    STOP.store(true, Ordering::SeqCst);
    let preempted = sched::preemptions() - p0;
    let starved = PROGRESS[..spinners]
        .iter()
        .filter(|p| p.load(Ordering::Relaxed) == 0)
        .count();
    let gone = wait_until(2, || {
        tids[..spinners]
            .iter()
            .flatten()
            .all(|&t| !sched::is_thread_present(t))
    });

    let ok = starved == 0 && preempted > 0 && gone;
    tally.record(
        "preemption",
        ok,
        format_args!(
            "{spinners} never-yield threads on {} core(s): {} starved, {preempted} preemptions{}",
            crate::smp::online_count(),
            starved,
            if gone { "" } else { ", and they did not stop" },
        ),
    );
}

/// **A program runs at user level, reaches for a kernel address, and is killed; the kernel is
/// not.**
///
/// `kernel_test_subject` reads the kernel's own fault counter, a page that exists and is not its
/// own, so it takes a permission fault. Fails by name when the archive has no such program. It
/// waits for the thread to be reaped, because [`budget`], next, brackets the used-frame count and
/// a teardown in flight would read as the kernel allocating. See notes/self-test-checks.md.
fn userspace(tally: &mut Tally) {
    use crate::arch::exceptions::{USER_FAULTS, last_user_fault};
    use crate::user;

    let Some(image) = user::program("kernel_test_subject") else {
        tally.record(
            "userspace",
            false,
            format_args!(
                "the archive has no kernel_test_subject program, so no user program could run"
            ),
        );
        return;
    };
    let kernel_addr = &raw const USER_FAULTS as u64;
    let before = USER_FAULTS.load(Ordering::Acquire);
    let tid = sched::spawn(move || {
        user::run(
            image,
            user::Spawn {
                arg0: user::KERNEL_TEST_SUBJECT_READ_KERNEL,
                arg1: kernel_addr,
                arg2: 0,
                grants: &[],
                maps: &[],
            },
        )
    });
    let faulted = tid.is_some() && wait_until(2, || USER_FAULTS.load(Ordering::Acquire) > before);
    let at = last_user_fault().map(|(_, addr)| addr);
    let reaped = tid.is_some_and(|t| wait_until(2, || !sched::is_thread_present(t)));

    let ok = faulted && at == Some(kernel_addr) && reaped;
    tally.record(
        "userspace",
        ok,
        format_args!(
            "a program at user level read {kernel_addr:#x}: {}",
            match (tid.is_some(), faulted) {
                (false, _) => "it could not be spawned",
                (true, false) => "it was not stopped within 2s",
                (true, true) if ok => "killed, and the kernel survived",
                (true, true) if at != Some(kernel_addr) =>
                    "a fault was recorded at another address",
                (true, true) => "killed, but its thread was not reaped within 2s",
            }
        ),
    );
}

/// **A process spends a budget it was handed, and the kernel allocates nothing for it.**
///
/// `memory_region_depleter` reports once loaded and again with the pages it mapped, and the
/// kernel's used-frame count, read at both, must not move. The reports arrive on a helper thread,
/// because a receive has no deadline; then the process is killed.
fn budget(tally: &mut Tally) {
    use crate::user;

    const PAGES: u64 = 24;
    static READY_USED: AtomicU64 = AtomicU64::new(u64::MAX);
    static DONE_USED: AtomicU64 = AtomicU64::new(u64::MAX);
    static MAPPED: AtomicU64 = AtomicU64::new(u64::MAX);

    let Some(image) = user::program("memory_region_depleter") else {
        tally.record(
            "budget",
            false,
            format_args!("the archive has no memory_region_depleter program"),
        );
        return;
    };
    let Some((region, report, demo)) = user::memory_region_service::start(image, PAGES) else {
        tally.record(
            "budget",
            false,
            format_args!("no {PAGES}-page untyped region could be carved"),
        );
        return;
    };
    for s in [&READY_USED, &DONE_USED, &MAPPED] {
        s.store(u64::MAX, Ordering::SeqCst);
    }
    let used = || memory::stats().map_or(u64::MAX, |s| s.used as u64);
    let helper = sched::spawn(move || {
        sched::ipc_receive(report);
        READY_USED.store(used(), Ordering::SeqCst);
        let mapped = sched::ipc_receive(report)[0];
        DONE_USED.store(used(), Ordering::SeqCst);
        MAPPED.store(mapped, Ordering::SeqCst);
    });
    let reported = wait_until(2, || MAPPED.load(Ordering::SeqCst) != u64::MAX);
    // `teardown`, two checks on, brackets the same count, so the helper's own reap must land here.
    if reported && let Some(h) = helper {
        wait_until(2, || !sched::is_thread_present(h));
    }
    let (before, after, mapped) = (
        READY_USED.load(Ordering::SeqCst),
        DONE_USED.load(Ordering::SeqCst),
        MAPPED.load(Ordering::SeqCst),
    );
    let ended = sched::kill_thread(demo) && wait_until(2, || !sched::is_thread_present(demo));
    if ended {
        crate::memory_region::destroy(region);
    }

    let ok = reported && mapped > 0 && before == after;
    if reported {
        tally.record(
            "budget",
            ok,
            format_args!(
                "a process mapped {mapped} pages of a {PAGES}-page untyped; kernel frames used {before} -> {after}{}",
                if ended { "" } else { "; it outlived its kill" },
            ),
        );
    } else {
        tally.record(
            "budget",
            false,
            format_args!("the process did not report within 2s"),
        );
    }
}

/// **One supervised child that faults, its death received, and its region reaped.**
///
/// The child is `user::supervised_child`'s fault stub, built with its address space, code, stack,
/// TCB and supervision endpoint all in one 16-page region, so the reap gives every frame back.
/// The death is waited for on the corpse (`sched::corpse_fault_msg`) before the receive, because a
/// receive has no deadline and a corpse parked as the endpoint's sender makes it return at once.
/// Returns the child's thread id and the five words received, or which step refused.
fn supervised_round() -> Result<(u64, [u64; 5]), &'static str> {
    use crate::user::supervised_child;

    let region = crate::memory_region::create(16).ok_or("no 16-page region for the child")?;
    let Some(ep) = sched::create_rendezvous_from(region) else {
        crate::memory_region::destroy(region);
        return Err("no supervision endpoint in the child's region");
    };
    let fault = supervised_child::supervision_endpoint(ep);
    let child =
        match supervised_child::spawn(region, supervised_child::FAULT_STUB, &[], Some(fault)) {
            Ok(tid) => tid,
            Err(why) => {
                let _ = sched::reclaim_region(region);
                return Err(why);
            }
        };
    if !wait_until(2, || sched::corpse_fault_msg(child).is_some()) {
        let _ = sched::kill_thread(child);
        let _ = sched::reclaim_region(region);
        return Err("the child did not die within 2s");
    }
    let msg = sched::ipc_receive(ep);
    sched::reap_supervised(ep, child).map_err(|_| "the corpse refused to be reaped")?;
    Ok((child, msg))
}

/// **A faulting user-level thread's death reaches its supervisor as a message**, naming the
/// thread, the faulting pc and the address (DECISIONS §26 (the fault endpoint)).
///
/// The child is born supervised: its endpoint sits in the reserved fault slot when it starts. The
/// message must say `EVENT_FAULT`, the child's own thread id, the load instruction's pc and the
/// unmapped address it read. See notes/self-test-checks.md.
fn supervised(tally: &mut Tally) {
    use crate::user::supervised_child::{CODE_ADDRESS, FAULT_ADDRESS, FAULT_PC_OFFSET};

    let (child, msg) = match supervised_round() {
        Ok(r) => r,
        Err(why) => {
            tally.record("supervised", false, format_args!("{why}"));
            return;
        }
    };
    let pc = CODE_ADDRESS + FAULT_PC_OFFSET;
    let ok = msg[0] == abi::fault::EVENT_FAULT
        && msg[1] == child
        && msg[2] == pc
        && msg[3] == FAULT_ADDRESS;
    if ok {
        tally.record(
            "supervised",
            ok,
            format_args!(
                "thread {child:#x} faulted at {pc:#x} reading {FAULT_ADDRESS:#x}; its supervisor received all three in a FAULT event, and reaped it"
            ),
        );
    } else {
        tally.record(
            "supervised",
            ok,
            format_args!(
                "thread {child:#x} faulted; its supervisor received event {} tid {:#x} pc {:#x} addr {:#x}, wanting {} {child:#x} {pc:#x} {FAULT_ADDRESS:#x}",
                msg[0],
                msg[1],
                msg[2],
                msg[3],
                abi::fault::EVENT_FAULT,
            ),
        );
    }
}

/// **Tearing a process down returns every frame it cost**: a second round of spawn and teardown
/// moves the machine's used-frame count by zero.
///
/// Two rounds of [`supervised_round`], and only the second is judged. A first round pays first-use
/// carves that are not leaks (a kernel stack the recycler has not held yet, the kernel's own
/// object budget), and a machine at its steady state charges the repeat nothing. Like [`budget`]
/// it brackets the used-frame count, so it is sound only while nothing else allocates in its
/// window. See notes/self-test-checks.md.
fn teardown(tally: &mut Tally) {
    let used = || memory::stats().map(|s| s.used as i64);
    let (Some(before), first) = (used(), supervised_round()) else {
        tally.record(
            "teardown",
            false,
            format_args!("the frame allocator is not up"),
        );
        return;
    };
    let after_first = used().unwrap_or(before);
    let second = supervised_round();
    let after_second = used().unwrap_or(after_first);
    let (first_cost, second_cost) = (after_first - before, after_second - after_first);
    match (first, second) {
        (Ok(_), Ok(_)) => tally.record(
            "teardown",
            second_cost == 0,
            format_args!(
                "two rounds of a supervised child spawned, faulted and reaped: the first cost {first_cost} frame(s), the second {second_cost}"
            ),
        ),
        (Err(why), _) | (_, Err(why)) => {
            tally.record("teardown", false, format_args!("a round failed: {why}"));
        }
    }
}

/// **A user-level driver reads a file off a virtio disk, woken by the device's interrupt.**
///
/// The driver gets the registers, a DMA page and the interrupt by grant, parses nifefs itself and
/// reports `motd`'s first eight bytes; the kernel issues no virtio command. Disk 0, MMIO first,
/// else the PCIe virtio-blk; the file service, installer and surveyor use disks of their own.
/// Absent on a machine with no virtio disk, which is every boot `boot-check` makes and every board.
fn blockdev(tally: &mut Tally) {
    use crate::user;

    static WORD: AtomicU64 = AtomicU64::new(0);
    static DONE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

    let mmio = crate::virtio::find_block_device().is_some();
    if !mmio && crate::pci::find_block_device().is_none() {
        tally.absent(
            "blockdev",
            format_args!("no virtio block device on this machine, over MMIO or PCIe"),
        );
        return;
    }
    let transport = if mmio { "virtio-mmio" } else { "virtio-pci" };
    let Some(image) = user::program("block_driver") else {
        tally.record(
            "blockdev",
            false,
            format_args!("a {transport} disk, but the archive has no block_driver program"),
        );
        return;
    };
    let report = if mmio {
        user::virtio_service::start(image)
    } else {
        user::virtio_service::start_pci(image)
    };
    let Some(report) = report else {
        tally.record(
            "blockdev",
            false,
            format_args!("a {transport} disk, but its driver could not be wired"),
        );
        return;
    };
    DONE.store(false, Ordering::SeqCst);
    sched::spawn(move || {
        WORD.store(sched::ipc_receive(report)[0], Ordering::SeqCst);
        DONE.store(true, Ordering::SeqCst);
    });
    let reported = wait_until(5, || DONE.load(Ordering::SeqCst));
    let head = WORD.load(Ordering::SeqCst).to_le_bytes();
    let ok = reported && &head == b"nife: re";
    if !reported {
        tally.record(
            "blockdev",
            false,
            format_args!("a user-level driver on the {transport} disk did not report within 5s"),
        );
    } else {
        tally.record(
            "blockdev",
            ok,
            format_args!(
                "a user-level driver read motd off the {transport} disk through its own nifefs parse: {head:?}"
            ),
        );
    }
}

/// **A device's interrupt line reaches the CPU through the interrupt controller.**
///
/// Different from [`fn@timer`]'s claim, which is about an interrupt the CPU's own timer raises:
/// this one comes from a device outside the CPU, through the controller's routing. `x86_64` proves
/// the PIT's line through the IO APIC at bring-up (`arch::irq::init`, the only window where the
/// scheduler is up and the tick not yet armed) and this reads the count it took. **Absent on
/// aarch64 and riscv64**, which have no device the kernel can make interrupt without input or a
/// disk; there the line is proved by the input driver at the hand-over and by [`blockdev`].
fn deviceirq(tally: &mut Tally) {
    match arch::irq::device_line_interrupts() {
        Some(n) => tally.record(
            "deviceirq",
            n > 0,
            format_args!("{n} interrupts from a device line in ~0.2s at bring-up"),
        ),
        None if cfg!(target_arch = "x86_64") => tally.absent(
            "deviceirq",
            format_args!("the MADT named no IO APIC, so no device line was routed"),
        ),
        None => tally.absent(
            "deviceirq",
            format_args!(
                "no device on this port interrupts without input or a disk; the input driver and blockdev prove a line where one exists"
            ),
        ),
    }
}

/// **The JH7110's clock controller ungates its TRNG, and the TRNG serves a client, timed.**
///
/// One bring-up gives both answers: `clock` is the controller's before and after (clocks running,
/// reset released), and `trng` two device generations that differ, drawn through the confined
/// driver, with bring-up and draw timed. Absent on every machine but a JH7110.
fn jh7110(tally: &mut Tally) {
    #[cfg(target_arch = "riscv64")]
    jh7110_on_riscv64(tally);
    #[cfg(not(target_arch = "riscv64"))]
    for name in ["clock", "trng"] {
        tally.absent(name, format_args!("no JH7110 on this architecture"));
    }
}

#[cfg(target_arch = "riscv64")]
fn jh7110_on_riscv64(tally: &mut Tally) {
    use crate::user;

    let Some(device) = user::entropy_service::jh7110_trng_device() else {
        let why = "this machine's tree names no JH7110 TRNG (neither starfive,jh7110-trng nor the vendor U-Boot's starfive,trng)";
        tally.absent("clock", format_args!("{why}, so no clock was ungated"));
        tally.absent("trng", format_args!("{why}"));
        return;
    };
    let Some(image) = user::program("jh7110_entropy") else {
        for name in ["clock", "trng"] {
            tally.record(
                name,
                false,
                format_args!(
                    "JH7110 TRNG at {:#x}, but the archive has no jh7110_entropy program",
                    device.reg_base
                ),
            );
        }
        return;
    };
    let wire_start = arch::timer::now();
    let Some(w) = user::entropy_service::ensure(image, user::entropy_service::Bus::Jh7110) else {
        for name in ["clock", "trng"] {
            tally.record(
                name,
                false,
                format_args!(
                    "JH7110 TRNG at {:#x}, but its service would not wire",
                    device.reg_base
                ),
            );
        }
        return;
    };
    let wire_ticks = arch::timer::now().wrapping_sub(wire_start);
    jh7110_clock(tally, w.clock);
    let draw = draw_twice(w, 5);
    let ok = draw.as_ref().is_some_and(Draw::is_good);
    match draw {
        None => tally.record(
            "trng",
            false,
            format_args!("JH7110 TRNG at {:#x}: its driver did not answer within 5s", device.reg_base),
        ),
        Some(d) => tally.record(
            "trng",
            ok,
            format_args!(
                "JH7110 TRNG at {:#x} (tree says {}): report {:#x}, STAT {:#010x} ({}), draws {}/{} bytes, first {:02x}{:02x}{:02x}{:02x}.., differ {}; bring-up {} us, {} bytes in {} us ({} bytes/s)",
                device.reg_base,
                if device.status_okay { "okay" } else { "disabled" },
                d.report[0],
                (d.report[1] >> 32) as u32,
                mode_note((d.report[1] >> 32) as u32),
                d.na,
                d.nb,
                d.head[0],
                d.head[1],
                d.head[2],
                d.head[3],
                d.differ,
                micros(wire_ticks + d.ready_ticks),
                d.na + d.nb,
                micros(d.draw_ticks),
                bytes_per_second((d.na + d.nb) as u64, d.draw_ticks),
            ),
        ),
    }
}

/// The clock-and-reset controller's before-and-after, as the `clock` check.
#[cfg(target_arch = "riscv64")]
fn jh7110_clock(tally: &mut Tally, clock: Option<jh7110_clock_and_reset::Report>) {
    let Some(report) = clock else {
        tally.record(
            "clock",
            false,
            format_args!(
                "this tree names a TRNG but no clock-and-reset window was mapped, so nothing was ungated"
            ),
        );
        return;
    };
    let found = crate::user::entropy_service::jh7110_crg_window();
    let ok = report.has_clocks_running() && report.released;
    tally.record(
        "clock",
        ok,
        format_args!(
            "JH7110 STG CRG at {:#x} ({}): clocks {:#010x},{:#010x} -> {:#010x},{:#010x}, reset {} after {} polls{}",
            found.map_or(0, |f| f.base),
            if found.is_some_and(|f| f.from_tree) { "from the tree" } else { "the published constant" },
            report.clock_before[0],
            report.clock_before[1],
            report.clock_after[0],
            report.clock_after[1],
            if report.released { "released" } else { "STILL HELD" },
            report.polls,
            if report.was_already_up() { "; the firmware had already done all of it" } else { "" },
        ),
    );
}

/// What [`draw_twice`] measured: the service's readiness report (zeros when this caller did not
/// wire it), the two draws' byte counts and whether they differ, the first draw's head, and the
/// readiness and draw times in counter ticks.
struct Draw {
    report: [u64; 5],
    na: usize,
    nb: usize,
    zeros: bool,
    differ: bool,
    head: [u8; 4],
    /// The two timings, which only the JH7110's `trng` check prints.
    #[cfg_attr(not(target_arch = "riscv64"), allow(dead_code))]
    ready_ticks: u64,
    #[cfg_attr(not(target_arch = "riscv64"), allow(dead_code))]
    draw_ticks: u64,
}

impl Draw {
    /// Ready, two full 32-byte draws, not all zeros, and different from each other: one draw proves
    /// only that something was returned, and a stuck register file, a driver serving its buffer
    /// twice and a device that never started all present as a repeat. A report of zeros means a
    /// second caller, whom the service already reported to, and that is not a failure.
    fn is_good(&self) -> bool {
        (self.report[0] == entropy_contract::READY || self.report[0] == 0)
            && self.na == 32
            && self.nb == 32
            && !self.zeros
            && self.differ
    }
}

/// **Wait for an entropy service to be ready, then draw two 32-byte buffers from it as a client**,
/// on a helper thread so the wait keeps a deadline. Each draw is four `entropy_contract` round
/// trips (`MAX_BYTES` is 8), and the second empties a different device generation than the first.
/// `None` if the service did not answer within `seconds`.
fn draw_twice(w: crate::user::entropy_service::Wiring, seconds: u64) -> Option<Draw> {
    use core::sync::atomic::AtomicBool;

    static REPORT: [AtomicU64; 5] = [const { AtomicU64::new(0) }; 5];
    static COUNTS: AtomicU64 = AtomicU64::new(0);
    static HEAD: AtomicU64 = AtomicU64::new(0);
    static FLAGS: AtomicU64 = AtomicU64::new(0);
    static TICKS: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];
    static DONE: AtomicBool = AtomicBool::new(false);

    DONE.store(false, Ordering::SeqCst);
    sched::spawn(move || {
        let start = arch::timer::now();
        let report = w.wait_for_ready().unwrap_or([0; 5]);
        let ready_at = arch::timer::now();
        let (mut a, mut b) = ([0u8; 32], [0u8; 32]);
        let (na, nb) = (w.fill(&mut a), w.fill(&mut b));
        let drawn_at = arch::timer::now();
        for (slot, word) in REPORT.iter().zip(report) {
            slot.store(word, Ordering::Relaxed);
        }
        COUNTS.store(((na as u64) << 32) | nb as u64, Ordering::Relaxed);
        HEAD.store(
            u32::from_le_bytes([a[0], a[1], a[2], a[3]]) as u64,
            Ordering::Relaxed,
        );
        let zeros = a.iter().all(|&x| x == 0);
        FLAGS.store(
            u64::from(zeros) | (u64::from(a != b) << 1),
            Ordering::Relaxed,
        );
        TICKS[0].store(ready_at.wrapping_sub(start), Ordering::Relaxed);
        TICKS[1].store(drawn_at.wrapping_sub(ready_at), Ordering::Relaxed);
        DONE.store(true, Ordering::SeqCst);
    })?;
    if !wait_until(seconds, || DONE.load(Ordering::SeqCst)) {
        return None;
    }
    let counts = COUNTS.load(Ordering::Relaxed);
    let flags = FLAGS.load(Ordering::Relaxed);
    Some(Draw {
        report: core::array::from_fn(|i| REPORT[i].load(Ordering::Relaxed)),
        na: (counts >> 32) as usize,
        nb: (counts & 0xffff_ffff) as usize,
        zeros: flags & 1 != 0,
        differ: flags & 2 != 0,
        head: (HEAD.load(Ordering::Relaxed) as u32).to_le_bytes(),
        ready_ticks: TICKS[0].load(Ordering::Relaxed),
        draw_ticks: TICKS[1].load(Ordering::Relaxed),
    })
}

/// **The machine's entropy source serves a client random bytes.**
///
/// The source the hand-over would use: a JH7110 TRNG (already wired by [`jh7110`]), else the CPU's
/// seed instruction (`RDSEED`, `RNDRRS`), through the same entropy service the hand-over grants the
/// progenitor, shared through `entropy_service::ensure`. Two 32-byte draws that differ.
///
/// **Not virtio-rng**, which the hand-over hands to the progenitor as a raw device so that it
/// builds its own service; a kernel-built service on the same device would fight it for the queue.
/// A machine whose only source is virtio-rng reports this check absent and says why, and so does a
/// machine with no source at all.
fn entropy(tally: &mut Tally) {
    use crate::user::{self, entropy_service};

    let jh7110 = entropy_service::jh7110_trng_device().is_some();
    let (bus, program, source) = if jh7110 {
        (
            entropy_service::Bus::Jh7110,
            "jh7110_entropy",
            "the JH7110 TRNG",
        )
    } else {
        (
            entropy_service::Bus::Instruction,
            "entropy",
            "the CPU's seed instruction",
        )
    };
    let Some(image) = user::program(program) else {
        tally.record(
            "entropy",
            false,
            format_args!("the archive has no {program} program to serve {source}"),
        );
        return;
    };
    let Some(w) = entropy_service::ensure(image, bus) else {
        if jh7110 {
            tally.record(
                "entropy",
                false,
                format_args!("{source} is named by the tree, but its service would not wire"),
            );
        } else if crate::virtio::find_entropy_device().is_some() {
            tally.absent(
                "entropy",
                format_args!(
                    "no seed instruction or TRNG; this machine's virtio-rng is the progenitor's to drive after the hand-over"
                ),
            );
        } else {
            tally.absent(
                "entropy",
                format_args!(
                    "no seed instruction, no JH7110 TRNG and no virtio-rng on this machine"
                ),
            );
        }
        return;
    };
    match draw_twice(w, 5) {
        None => tally.record(
            "entropy",
            false,
            format_args!("{source}'s service did not answer within 5s"),
        ),
        Some(d) => tally.record(
            "entropy",
            d.is_good(),
            format_args!(
                "{source} served {}+{} bytes to a client, first {:02x}{:02x}{:02x}{:02x}.., the two draws differ: {}",
                d.na, d.nb, d.head[0], d.head[1], d.head[2], d.head[3], d.differ,
            ),
        ),
    }
}
/// **Counter ticks as microseconds**, for the `trng` check's timings: comparable across boards,
/// whose counters run at different rates. Truncating, so work too short to measure reads zero.
///
/// Provisional name (an architect's call): `micros`, with `micros_between` and `bytes_per_second`
/// beside it.
#[cfg(target_arch = "riscv64")]
#[allow(dead_code)]
fn micros(ticks: u64) -> u64 {
    // `frequency()` asserts a nonzero rate. `saturating_mul` bounds the scaling: a gap long enough
    // to overflow is one nobody is timing.
    ticks.saturating_mul(1_000_000) / arch::timer::frequency()
}

/// The two-timestamp form of [`micros`]. `wrapping_sub` because the `time` CSR is a free-running
/// 64-bit counter and this is the arithmetic the rest of this file already uses on it (see the
/// JH7110 entropy step above); at 10 MHz it wraps after about 58,000 years.
#[cfg(target_arch = "riscv64")]
#[allow(dead_code)]
fn micros_between(start: u64, end: u64) -> u64 {
    micros(end.wrapping_sub(start))
}

/// **Bytes per second over a measured span**, from ticks so it does not inherit the microsecond
/// figure's truncation; zero for a span too short to measure. It counts the IPC round trips and
/// the driver as well as the device, so it is not comparable to a Linux `hwrng` figure
/// (notes/self-test-checks.md).
#[cfg(target_arch = "riscv64")]
#[allow(dead_code)]
fn bytes_per_second(bytes: u64, ticks: u64) -> u64 {
    if ticks == 0 {
        return 0;
    }
    bytes.saturating_mul(arch::timer::frequency()) / ticks
}

/// **Say what the TRNG's `STAT` says about its output width**: in 128-bit mode only `RAND0..RAND3`
/// carry an answer, so 16 of 32 reported bytes would not be device output. An all-zero `STAT` is a
/// register window that answered nothing. See notes/self-test-checks.md.
// riscv64-only because the JH7110 is; `allow(dead_code)` because the boot tour that calls it is
// itself compiled out of the shell and bench builds, the same way `image_for_virtio` is.
#[cfg(target_arch = "riscv64")]
#[allow(dead_code)]
fn mode_note(stat: u32) -> &'static str {
    if stat == 0 {
        "the whole status register read zero, so this says nothing about the mode"
    } else if stat & jh7110_entropy::STAT_R256 != 0 {
        "256-bit: all eight RAND words are the answer"
    } else {
        "128-BIT: only RAND0..3 are the answer, so 16 of every 32 bytes are not device output"
    }
}

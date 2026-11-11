//! **The kernel proving its own internals work on this machine, before it hands the machine to
//! anybody.**
//!
//! The kernel's half of the boot self-test (milestones 268 and 1063): the checks of what must run
//! at EL1, run on every architecture, each printing its line, a failure printing and the boot
//! carrying on. The checks that need a user program run in `boot_self_test`, which the progenitor
//! starts before the shell; the kernel hands it this half's tally in the page
//! `boot_ladder::kernel_half` describes, and that program prints the one verdict
//! (`boot_ladder::SELF_TEST`, matched by `crates/serial_console`). notes/boot-ladder.md has the
//! verdict's contract, and notes/self-test-checks.md each check's design and the full account of
//! the BUGS below.

//! # BUGS
//!
//! - Nothing proves an architecture ran the right set; review guards the list.
//! - The checks cost time on every boot: about a quarter of a second.
//! - A measurement boot hands nothing to a progenitor, so it prints this half's lines and no
//!   verdict.
//! - A check whose operation never returns still hangs the boot; that needs a watchdog.

use core::sync::atomic::{AtomicU64, Ordering};

use boot_ladder::verdict::{CheckLine, Outcome, Tally, check};

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

/// **Make one check in each half report failure**, so the verdict can be seen red: `cargo xtask
/// boot-check --inject` builds with this on and asserts the gate goes red, naming `exceptions` here
/// and `userspace` in `boot_self_test`, which reads it from the kernel's half
/// (`boot_ladder::kernel_half::flag::INJECT`).
///
/// It also stops the counter the checks here read (this module's `timer::now`), so every injected
/// run proves the waits are bounded. A `cfg!` rather than `#[cfg]`, so both arms compile on every
/// build.
///
/// Name: provisional (milestone 268 (every architecture boots the same way)).
const INJECT: bool = cfg!(feature = "self_test_injection");

/// Whether this build injects a failure ([`INJECT`]), for the hand-over to tell `boot_self_test`.
///
/// Name: provisional.
pub const fn injects() -> bool {
    INJECT
}

/// **The kernel's half of the tally**, kept from [`run`] until the hand-over writes it into the
/// page the progenitor passes on (`boot_ladder::kernel_half`) or, with no progenitor to hand it
/// to, prints the verdict itself ([`verdict_without_userspace`]). Three words:
/// `boot_ladder::verdict::Tally`'s.
static HALF: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];

/// **This half's running tally**, which prints each check's line as it records it.
struct Checks(Tally);

impl Checks {
    /// Record `check`'s outcome and print its line. The detail is `core::fmt::Arguments`, so a
    /// check puts the numbers it measured in its own line without a buffer.
    fn record(
        &mut self,
        (name, index): (&'static str, usize),
        ok: bool,
        detail: core::fmt::Arguments,
    ) {
        let outcome = if ok { Outcome::Passed } else { Outcome::Failed };
        self.0.record(index, outcome);
        println!(
            "{}",
            CheckLine {
                name,
                outcome,
                detail
            }
        );
    }

    /// **Record that this machine has nothing for the check to test**, with the reason
    /// (`boot_ladder::SELF_TEST_ABSENT`; calef's ruling of 2026-10-20 (UTC) on #2447). Whether an
    /// absence is acceptable is the board's fact: `serial_console`'s board profiles say.
    fn absent(&mut self, (name, index): (&'static str, usize), why: core::fmt::Arguments) {
        self.0.record(index, Outcome::Absent);
        println!(
            "{}",
            CheckLine {
                name,
                outcome: Outcome::Absent,
                detail: why
            }
        );
    }
}

/// `check!(name)`: the name and its compile-time index in `boot_ladder::SELF_TEST_CHECKS`, so a
/// check the list does not name does not build.
macro_rules! check {
    ($name:literal) => {
        ($name, const { check($name) })
    };
}

/// **Run this half's checks and keep its tally for the hand-over.**
///
/// Called from `kernel_main` on all three architectures, after the machine description and before
/// anything is handed to userspace. Everything it needs is up by then: paging, the frame allocator,
/// the timer with interrupts on, and the scheduler with its idle thread.
pub fn run() {
    let mut checks = Checks(Tally::new());

    // Each check notes its boot stage first, so a dump during a wedged check names it.
    let runs: [fn(&mut Checks); RUNS] = [
        exceptions,
        mapping,
        frames,
        timer,
        scheduler,
        preemption,
        device_interrupt,
    ];
    for (stage, check) in (FIRST_STAGE..).zip(runs) {
        sched::note_boot_stage(stage);
        check(&mut checks);
    }

    for (slot, word) in HALF.iter().zip(checks.0.to_words()) {
        slot.store(word, Ordering::Release);
    }
}

/// **This half's tally, as [`run`] left it**, for the page the hand-over writes.
///
/// Name: provisional.
pub fn kernel_tally() -> Tally {
    Tally::from_words(core::array::from_fn(|i| HALF[i].load(Ordering::Acquire)))
}

/// **Print the verdict from this half alone**, for a boot with no progenitor to run the other: no
/// archive, or one whose progenitor would not load. Every check of the userspace half fails by
/// name, because it did not run.
///
/// Name: provisional.
pub fn verdict_without_userspace(why: &str) {
    let mut tally = kernel_tally();
    for name in tally.fail_unrecorded() {
        println!(
            "{}",
            CheckLine {
                name,
                outcome: Outcome::Failed,
                detail: format_args!("did not run: {why}"),
            }
        );
    }
    println!("{}", tally.verdict());
}

/// The boot stage the first check notes. Stages below it are the bring-up's, which notes none
/// today; the number is kept from the riscv64 tour this replaced so that a board log's stage reads
/// the same way across the change.
const FIRST_STAGE: u32 = 3;

/// How many check functions [`run`] calls: one per check of this half.
const RUNS: usize = 7;

/// **The stage that means this half finished and the boot's middle stages ran to their end**,
/// noted beside `boot_ladder::TOUR` in `kernel_main`: the stage after the last check's.
///
/// Name: provisional, minted 2026-10-20 (UTC) by lane `milestone/890-tours-to-self-test`.
pub const TOUR_STAGE: u32 = FIRST_STAGE + RUNS as u32;

/// **A breakpoint is caught and stepped over.**
///
/// The whole trap round trip: the vector is installed where the hardware accepts it, a synchronous
/// exception reaches the dispatcher, the cause is recognized, and the return lands after the
/// trapping instruction. Getting the last wrong is an infinite loop rather than a wrong answer.
fn exceptions(tally: &mut Checks) {
    let caught = arch::exceptions::self_test();
    let ok = caught >= 1 && !INJECT;
    if INJECT {
        tally.record(
            check!("exceptions"),
            ok,
            format_args!("failure injected by the self_test_injection feature ({caught} caught)"),
        );
    } else {
        tally.record(
            check!("exceptions"),
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
fn mapping(tally: &mut Checks) {
    use paging::Flags;

    let Some(ram_top) = memory::ram_regions()
        .map(|(start, size)| start + size)
        .max()
    else {
        tally.record(
            check!("mapping"),
            false,
            format_args!("this machine described no RAM, so there is no free address to try"),
        );
        return;
    };
    // One gigabyte above the direct map's last byte: past everything `mmu::initialize` claimed, and
    // inside the kernel half on all three.
    let va = arch::mmu::phys_to_virt(ram_top + (1 << 30));
    let Some(frame) = memory::alloc() else {
        tally.record(
            check!("mapping"),
            false,
            format_args!("the allocator had no frame to map at {va:#x}"),
        );
        return;
    };
    let pa = frame.addr();

    if let Err(e) = arch::mmu::map_page(va, pa, Flags::kernel_data()) {
        tally.record(
            check!("mapping"),
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
            check!("mapping"),
            ok,
            format_args!("{va:#x} -> {pa:#x}, read back {read_back:#x}, unmapped"),
        );
        memory::free(frame);
    } else {
        tally.record(
            check!("mapping"),
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
fn frames(tally: &mut Checks) {
    let Some(before) = memory::stats() else {
        tally.record(
            check!("frames"),
            false,
            format_args!("the frame allocator is not up"),
        );
        return;
    };
    let Some(frame) = memory::alloc() else {
        tally.record(
            check!("frames"),
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
        check!("frames"),
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
fn timer(tally: &mut Checks) {
    let hertz = timer::frequency();
    let mut counter = Counter::new();
    let start = counter.last;
    let ticks_before = timer::ticks();
    // 20 ms: two tick periods at the 100 Hz `TICK_HERTZ` every architecture uses, so a single missed
    // tick does not read as a broken interrupt path.
    let window = hertz / 50;
    let advanced = loop {
        let Some(now) = counter.read() else {
            tally.record(
                check!("timer"),
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
        check!("timer"),
        ok,
        format_args!(
            "the {}MHz counter advanced {advanced} and {ticks} tick(s) arrived in ~20ms",
            hertz / 1_000_000
        ),
    );
}

/// **A thread the kernel spawned runs, and carries what it captured**: the context switch, end
/// to end. The wait is bounded by the clock, not by a count of yields, because the new thread may
/// be placed on another core. See notes/self-test-checks.md.
fn scheduler(tally: &mut Checks) {
    static SAW: AtomicU64 = AtomicU64::new(0);

    // Distinctive rather than 1: a zero-initialized static that happened to be read as "it ran"
    // would pass this check on a kernel where nothing ran at all.
    const CAPTURED: u64 = 0x0268_0268_0268_0268;

    SAW.store(0, Ordering::SeqCst);
    let Some(tid) = sched::spawn(move || SAW.store(CAPTURED, Ordering::SeqCst)) else {
        tally.record(
            check!("scheduler"),
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
            check!("scheduler"),
            ok,
            format_args!("thread {tid:?} ran and carried its captured state ({saw:#x})"),
        );
    } else {
        tally.record(
            check!("scheduler"),
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
fn preemption(tally: &mut Checks) {
    use core::sync::atomic::AtomicBool;

    const MAX: usize = crate::cpu::MAXIMUM_CPUS + 1;
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
            check!("preemption"),
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
        check!("preemption"),
        ok,
        format_args!(
            "{spinners} never-yield threads on {} core(s): {} starved, {preempted} preemptions{}",
            crate::smp::online_count(),
            starved,
            if gone { "" } else { ", and they did not stop" },
        ),
    );
}

/// **A device's interrupt line reaches the CPU through the interrupt controller.**
///
/// Different from [`fn@timer`]'s claim, which is about an interrupt the CPU's own timer raises:
/// this one comes from a device outside the CPU, through the controller's routing. `x86_64` proves
/// the PIT's line through the IO APIC at bring-up (`arch::interrupt_controller::initialize`, the
/// only window where
/// the scheduler is up and the tick not yet armed) and this reads the count it took. **Absent on
/// aarch64 and riscv64**, which have no device the kernel can make interrupt without input or a
/// disk; there the line is proved by the input driver at the hand-over and by `boot_self_test`'s
/// `blockdev`.
#[doc(alias = "irq")]
fn device_interrupt(tally: &mut Checks) {
    match arch::interrupt_controller::device_line_interrupts() {
        Some(n) => tally.record(
            check!("device_interrupt"),
            n > 0,
            format_args!("{n} interrupts from a device line in ~0.2s at bring-up"),
        ),
        None if cfg!(target_arch = "x86_64") => tally.absent(
            check!("device_interrupt"),
            format_args!("the MADT named no IO APIC, so no device line was routed"),
        ),
        None => tally.absent(
            check!("device_interrupt"),
            format_args!(
                "no device on this port interrupts without input or a disk; the input driver and blockdev prove a line where one exists"
            ),
        ),
    }
}

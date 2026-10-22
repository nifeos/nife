//! **The kernel proving it works on this machine, before it hands the machine to anybody.**
//!
//! Milestone 268 (every architecture boots the same way)'s second rung: say what the machine is
//! ([`crate::print_machine_description`]), prove the kernel works on it (here), then hand over.
//! The checks are the claims the per-port tours made, on every port (calef's ruling of 2026-10-19
//! (UTC) on #2434). Every check runs on every architecture; where an architecture differs is
//! `arch::exceptions::self_test`, `arch::irq::device_line_interrupts` and the JH7110 bring-up.
//!
//! # It reports, it does not gate
//!
//! calef, 2026-09-09: *"I could see building tools to diagnose what broke but we can only run them
//! if there is a prompt"*. A failing check prints its verdict and the boot carries on to userspace.
//! **So the verdict is read by a program**: `boot_ladder::SELF_TEST` is the contract,
//! `crates/serial_console` matches it, and `cargo xtask boot-check` boots all three architectures
//! and fails on a red one in CI.
//!
//! ```text
//! nife self-test: 10 of 13 passed, 3 absent: blockdev clock trng
//! nife self-test: 9 of 13 passed, 3 absent: blockdev clock trng, 1 FAILED: exceptions
//! ```
//!
//! - **The prefix is always present and identical on every architecture**: a missing marker looks
//!   exactly like a slow board.
//! - **The counts print on success as well as on failure**, so "ran nothing and passed" is
//!   distinguishable from "ran the set and passed".
//! - **A check whose device this machine lacks is `absent`, with its reason**, neither a pass nor a
//!   failure (calef's ruling of 2026-10-20 (UTC) on #2447). Which absences a board allows is
//!   `serial_console`'s board profile's to say, and an absence its profile forbids is a failure
//!   there: radon may not be without `entropy`, `trng` or `clock`, xenon without `entropy` or
//!   `deviceirq`.
//!
//! The wording is **provisional**: a line two programs agree on is an architect's.
//! `serial_console::progress` is tested against this module's own constant.
//!

//! # BUGS
//!
//! - **Nothing here proves an architecture ran the right set.** The list is a compile-time fact,
//!   and a check deleted from it takes its own evidence with it; the count is the partial defense,
//!   and review guards the set.
//! - **The checks cost time on every boot.** [`fn@timer`] waits about 20 ms for a tick and
//!   [`preemption`] spends a fifth of a second; [`userspace`], [`budget`] and, where there is a
//!   disk, [`blockdev`] each spawn a process from the archive and wait for it. The deep proofs are
//!   still `script/test`'s.
//! - **[`blockdev`]'s driver is left running**, as it was in the tour it came from: it holds disk 0,
//!   which nothing after it uses, and `virtio_service::start` returns no thread to end.
//! - **[`budget`]'s helper thread can be left parked.** A receive has no deadline, so the reports
//!   are taken on a thread of their own; a process that never reports leaves that thread blocked,
//!   and the check fails by its own deadline.
//! - **[`budget`] still brackets the machine's used-frame count**, so it is sound only while
//!   nothing on any core allocates or frees a frame in its window. That is a rung below a
//!   mechanism, and deliberately so: the claim is that the kernel allocates nothing, which no
//!   single object can be asked. It holds because each earlier check that spawns waits for its
//!   threads to be reaped ([`preemption`], [`userspace`]) or ends long before ([`scheduler`]'s
//!   thread, a fifth of a second earlier). A check added before it that returns with a teardown
//!   in flight reopens the red `notes/load-sensitive-assertions.md` records.
//! - **A check can still hang the boot if the operation it measures never returns.** Both waits
//!   read the counter through [`Counter`], which reports a stopped counter rather than waiting on
//!   it, and [`fn@timer`] spins rather than sleeping. A `sched::yield_now` or `mmu::map_page` that
//!   never comes back is unbounded; that needs a watchdog this module does not own.
//!   `arch::timer::spin_for` still hangs on a stopped counter.

use core::sync::atomic::{AtomicU64, Ordering};

// **The verdict's two fixed strings come from `crates/boot_ladder`**, not from literals here, so
// that this module and `crates/serial_console` cannot hold two copies of one contract (`AGENTS.md`
// rule 7). Each carries its own trailing space, so a matcher that finds one has found a boundary
// rather than the start of a longer word.
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

/// **Make one check report failure**, so the verdict can be *seen* red rather than reasoned about.
///
/// A verdict that has never been red is a verdict nobody has tested, and milestone 268's proof
/// condition is that an injected failure turns the line red and fails CI. `cargo xtask boot-check
/// --inject` builds each architecture with this on and asserts the gate goes red; without it the
/// only way to know would be to break the kernel for real.
///
/// It is a `cfg!` rather than a `#[cfg]` block so that both arms compile on every build: a fault
/// injector that only type-checks when it is switched on is an injector that has rotted by the time
/// somebody needs it.
///
/// **It injects two failures, and the second is a gate for the first fix.** Besides failing
/// `exceptions` it stops the counter the checks read (this module's `timer::now`), which is the
/// fault that used to hang [`fn@timer`] forever. So every `--inject` run also proves the waits are
/// bounded: a regression there turns a red verdict back into a boot that goes quiet, and
/// `boot-check --inject` fails on that.
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

    // Each check notes its boot stage before it runs, so a thread dump during a wedged check says
    // which one (the breadcrumb `sched::dump_threads` repeats every couple of seconds survives a
    // lossy serial log; notes/visionfive2.md, fifth stop). Stage 3 is the first check and the
    // last is 2 plus the list's length; `TOUR_STAGE` follows the verdict.
    // One run per check except `jh7110`, which records `clock` and `trng` from one bring-up; the
    // verdict names any listed check that recorded nothing, so the list and these runs cannot
    // drift apart unseen. `entropy` runs after `jh7110` because on a JH7110 it draws from the
    // service that bring-up wired.
    let runs: [fn(&mut Tally); RUNS] = [
        exceptions, mapping, frames, timer, scheduler, preemption, userspace, budget, blockdev,
        deviceirq, jh7110, entropy,
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
/// from before the checks replaced the tours read 11 as finished; this reads 15.
///
/// Name: provisional, minted 2026-10-20 (UTC) by lane `milestone/890-tours-to-self-test`.
pub const TOUR_STAGE: u32 = FIRST_STAGE + RUNS as u32;

/// **A breakpoint is caught and stepped over.**
///
/// The oldest self-test in this tree, and the one milestone 268 collected rather than invented:
/// riscv64 and `x86_64` have called `arch::exceptions::self_test` from `main.rs` since their ports
/// were written, and `notes/riscv-port.md` already calls it "a boot self-test". aarch64 had the
/// machinery (`BRK_COUNT`, and a handler that advances past a `brk`) and no function; milestone 268
/// gave it one.
///
/// What it proves is the whole trap round trip: the vector is installed at an address the hardware
/// accepts, a synchronous exception reaches our dispatcher, the dispatcher recognises the cause,
/// and the return lands on the instruction *after* the trapping one rather than on it. Getting the
/// last of those wrong is an infinite loop rather than a wrong answer.
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

/// **The kernel can map a page it did not have at boot, and unmap it again.**
///
/// The second self-test milestone 268 collected: it was an unnamed block in the RISC-V tour
/// (`kmap test`), and it is what the kernel stack allocator stands on, so a kernel where it fails
/// cannot spawn a thread.
///
/// **The address is computed, not a constant**, and that is the part worth keeping. A constant here
/// was the third QEMU-shaped address the VisionFive 2 caught in one bench session (2026-08-14); the
/// direct map ends at the top of whatever RAM this machine has, so one gigabyte above that clears
/// any alignment the mapper rounds to and is free on every machine rather than on one.
///
/// `phys_to_virt` rather than each architecture's own arithmetic: the three spell the direct map
/// differently (`pa | KERNEL_VA_BASE` on aarch64, `pa + KERNEL_VA_BASE` on riscv64, `pa +
/// DIRECT_MAP_BASE` on `x86_64`, whose kernel *image* base is a different constant entirely), and the
/// function is the one thing all three agree on.
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
        // Deliberately leaked rather than freed: a frame whose mapping state is unknown must not go
        // back into the pool, where the next allocation would inherit whatever went wrong.
    }
}

/// **How many reads of an unchanged counter mean it has stopped.**
///
/// Every counter this kernel reads is free-running and fast: `CNTVCT_EL0` at 62.5 MHz on QEMU's
/// aarch64, `rdtime` at 10 MHz on QEMU's riscv64 and 4 MHz on the JH7110, the TSC on `x86_64`. The
/// slowest of those changes every 250 ns, and a million reads take far longer than that on any core
/// this runs on, so a healthy counter cannot read the same value this many times running. Counted
/// in reads rather than in time for the obvious reason: time is the thing in doubt.
const STALLED_READS: u32 = 1_000_000;

/// **How many looks with a yield between them mean the counter has stopped.** A yield is a pass
/// through the scheduler, and with [`preemption`]'s spinners runnable it can be a whole 10 ms tick
/// before this thread runs again, so a healthy counter changes between every two looks by a wide
/// margin. A hundred unchanged looks is a stopped counter, found within about a second even when
/// every look waits a tick; [`STALLED_READS`]' million would be three hours.
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
/// Two claims in one window, and they are different claims. A free-running counter that moves says
/// the time source exists; a tick count that moves says the interrupt controller routes its
/// interrupt to this core, the core has interrupts unmasked, and the handler runs. The second is
/// what preemption stands on, and the first is what everything that waits stands on, including the
/// wait below.
///
/// **Bounded by the counter rather than by an iteration count**, which is what makes the number
/// mean the same thing on a 62 MHz aarch64 generic timer, a 10 MHz RISC-V `mtime` and a 2 GHz TSC.
///
/// **It spins rather than sleeping, and reads through [`Counter`]**, and both are the fix for the
/// hang this module's `BUGS` used to record. The loop was `wait_for_interrupt` until the counter
/// passed the window, so a counter that never advanced sat here forever, and so did a machine whose
/// interrupts never arrived, because the sleep had nothing to wake it. Spinning with interrupts on
/// still lets the tick arrive, which is half of what this check measures.
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

/// **A thread the kernel spawned runs, and carries what it captured.**
///
/// The context switch, end to end: `spawn` builds a thread out of the kernel's own budget,
/// `switch_to` leaves this stack and the trampoline arrives on the new one, the closure runs with
/// the environment it was built with, and the thread exits back into the scheduler. A kernel that
/// fails this cannot run a driver, a service or a process.
///
/// **Clock-bounded rather than yield-bounded**, which the RISC-V tour learned the hard way: the
/// secondaries are online by this point, so placement can put the new thread on another core, and a
/// fixed count of `yield_now` on this core elapses in microseconds, long before another core has
/// scheduled it. Boot 12 on the VisionFive 2 printed "1 of 2" and boot 13 "0 of 2" against a
/// yield-bounded wait (notes/visionfive2.md).
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

    // Two seconds is far beyond any honest completion and is the same bound the RISC-V tour
    // settled on; the point of the deadline is that the boot continues rather than that the number
    // is tight. Read through [`Counter`], so a stopped counter ends the wait instead of making the
    // deadline unreachable.
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

/// **Threads that never yield are taken off the CPU anyway, and none of them starves.**
///
/// The property that separates a kernel from a runtime (DECISIONS §5 (preemptive threads with real stacks)): a thread whose whole body is
/// a loop, with no yield, no syscall and not even a function call, owns its core under any
/// cooperative scheduler. These run inside the kernel and are still preempted, which is the stronger
/// form of the claim; `system_tests`' `a_user_program_that_never_yields_is_preempted_anyway` makes
/// it for a program at user level.
///
/// **One more spinner than there are cores online**, so at least one core runs two of them and the
/// timer has to take the CPU from one to give it to the other. Two spinners on a four-core machine
/// can each own a core and never be preempted, which proves nothing; that was the shape of the
/// aarch64 and riscv64 tours this check replaces. Every spinner has to advance, and the global
/// preemption count has to move.
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
/// The privilege boundary, both halves. `kernel_test_subject` is handed the address of the kernel's
/// own fault counter, which is certainly mapped and certainly not the program's, so the fault it
/// takes is a permission fault on a page that exists rather than a translation fault on one that
/// does not. The program has to run at user level to take it, and the counter it reached for is the
/// one that records it. The kernel then executes the next line, which is the other half.
///
/// Needs the archive, which every boot that can hand over has. A boot without one fails this check
/// by name rather than passing it: no user program ran.
///
/// **It waits for the killed thread to be reaped before it returns.** The fault is counted before
/// the process is torn down, and the teardown returns about twenty frames on whichever core the
/// thread was on. [`budget`], next, brackets a process run with the machine's used-frame count, so
/// a teardown still in flight lands in its window and reads as the kernel allocating. Under TCG
/// with two cores the thread was still present when this returned on 22 of 24 loaded boots
/// (2026-10-20); `notes/load-sensitive-assertions.md` has the red it caused.
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
/// Milestone 11 (untyped memory)'s claim, and the reason a process cannot exhaust the kernel: every page it maps
/// comes out of the untyped region it holds, so the kernel's own used-frame count stands still
/// while the process maps until the region is spent. `memory_region_depleter` reports once it is
/// loaded (its ELF and stack are the kernel's, as any process's are) and again with how many pages
/// it mapped; the count is read at both reports.
///
/// The process spins after its second report so the count it reports against holds still, so this
/// ends it. The reports are received on a helper thread, because a receive has no deadline and
/// this check's waits must all have one; a process that never reports leaves that helper parked,
/// which is the failure path's whole cost.
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
    sched::spawn(move || {
        sched::ipc_receive(report);
        READY_USED.store(used(), Ordering::SeqCst);
        let mapped = sched::ipc_receive(report)[0];
        DONE_USED.store(used(), Ordering::SeqCst);
        MAPPED.store(mapped, Ordering::SeqCst);
    });
    let reported = wait_until(2, || MAPPED.load(Ordering::SeqCst) != u64::MAX);
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

/// **A user-level driver reads a file off a virtio disk, woken by the device's interrupt.**
///
/// The kernel enumerates the bus, grants the driver the device's registers, a DMA page and its
/// interrupt, and receives what the driver read. Every one of those is an authority the driver did
/// not have until the kernel granted it, and the kernel issues no virtio command and touches no DMA
/// itself. The driver reads `motd` through a nifefs superblock it parses, and reports its first
/// eight bytes.
///
/// Disk 0, the MMIO one, or the PCIe virtio-blk when there is no MMIO disk. The hand-over's file
/// service takes disk 1 and the installer and surveyor their own, so this driver collides with
/// none of them. **Absent on a machine with no virtio disk**, which is every boot `boot-check`
/// makes and every board.
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
/// Two checks from one wiring, because one bring-up produces both answers. `clock` is the
/// clock-and-reset controller's own before-and-after (milestone 220 (this kernel drives no clock
/// or reset controller, and the first real device will need one)): the TRNG's clocks running and its reset released. `trng` is the TRNG, through its
/// confined user-level driver, drawing two device generations that differ, with the bring-up and
/// the draw timed, which is fatal risk 6's "at real speed" (milestone 306 (time the hw entropy
/// step)). The kernel never reads a `RAND` register.
///
/// **Absent on every machine but a JH7110**, and radon's board profile in `serial_console` names
/// both as checks that may not be absent there.
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
/// **Counter ticks as microseconds** (design/roadmap/0306-time-the-hw-entropy-step.md), for
/// the `trng` check's timings.
///
/// Microseconds rather than milliseconds because the interesting half of the number is an IPC round
/// trip, and rather than raw ticks because a tick is a different amount of time on every machine
/// this runs on: QEMU's riscv64 `virt` counts at 10 MHz and the JH7110 at 4 MHz, so a transcript
/// quoting ticks could not be compared against another board's without the rate beside it. The rate
/// itself came out of this machine's own device tree (`arch::timer::init_frequency`), which is why
/// there is no constant here to go stale.
///
/// Truncating division, deliberately: this is an instrument, and a figure that rounded up would
/// report a nonzero duration for work that took no measurable time at all.
///
/// Provisional name (an architect's call): `micros`, with `micros_between` and `bytes_per_second`
/// beside it.
#[cfg(target_arch = "riscv64")]
#[allow(dead_code)]
fn micros(ticks: u64) -> u64 {
    // `frequency()` asserts the rate is nonzero rather than returning a poison value, and it has
    // been read from the tree since `bring_up_machine`, long before this tour, so there is no
    // pre-init case to guard here. `saturating_mul` bounds the scaling rather than the duration: a
    // gap long enough to overflow is one nobody is timing.
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

/// **Bytes per second over a measured span**, for the same line.
///
/// Computed from ticks rather than from the microsecond figure beside it, so the rate does not
/// inherit that figure's truncation. Zero for a span too short to measure, which is the honest
/// answer: a rate derived from a zero-length interval is not a large number, it is no number.
///
/// **What it counts, which has to be stated before it is quoted.** Everything between the readiness
/// report and the last byte landing: the `entropy_contract` round trips (one per
/// `entropy_contract::MAX_BYTES`, so four per 32-byte draw), the two context switches each one costs,
/// the driver's own poll loop, and the device. It does **not** count the process spawn or the
/// bring-up, which are the once-per-boot cost reported separately. It is therefore not comparable
/// to a Linux `hwrng` throughput figure, which is a read from an already-running kernel driver with
/// no IPC in it at all; `notes/benchmarks.md`'s rule is to say so where the number is printed.
#[cfg(target_arch = "riscv64")]
#[allow(dead_code)]
fn bytes_per_second(bytes: u64, ticks: u64) -> u64 {
    if ticks == 0 {
        return 0;
    }
    bytes.saturating_mul(arch::timer::frequency()) / ticks
}

/// **Say what the TRNG's `STAT` says about its output width** (milestone 159), for the boot tour's
/// `hw entropy` line.
///
/// This is the one bit that decides whether the 32 bytes that line reports are 32 bytes of device
/// output. In 128-bit mode only `RAND0..RAND3` carry a generation's answer, so a driver that
/// assembles all eight words is serving 16 real bytes and 16 of whatever the upper registers hold.
/// The driver writes `MODE.R256` during bring-up precisely because the width a given JH7110 resets
/// to is a build-time parameter of the silicon; `STAT.R256` is the read-back that says it took, and
/// nothing has ever read it on hardware.
///
/// An all-zero `STAT` is called out separately because it is not a mode report at all: it is the
/// signature of a register window that answered with nothing, which milestone 220's clock line
/// above is what explains.
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

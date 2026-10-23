//! The QEMU test harness.
//!
//! `cargo test` builds the kernel with `cfg(test)`, the runner in
//! `.cargo/config.toml` boots it under QEMU, and we report pass/fail by asking QEMU
//! to exit with a status code via semihosting. Cargo reads that status and calls it
//! a pass or a failure.
//!
//! Set up on day one, per DECISIONS §7 (testing: QEMU harness + host-testable crates, from commit
//! one). The watchdogs, the frame ledger, skips, filters and repeats are in
//! notes/kernel-test-harness.md.

use core::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, AtomicU64, AtomicUsize, Ordering};

// The per-test budget table lives in `crates/test_times` (`DEFAULT_BUDGET_SECS`, `SLOW_TESTS`),
// because `xtask`'s host watchdog reads the same numbers (rule 7).
use test_times::budget_secs_for;

use crate::arch::semihosting;
use crate::{print, println};

// The hang watchdog: a no-progress heartbeat for a lost wakeup and a per-test wall-clock ceiling
// for a livelock, both off the boot core's timer IRQ. notes/kernel-test-harness.md has what each
// can and cannot see.
/// **Which tests this binary runs**: a substring of the full test path, or empty for the whole
/// suite. Baked in by `kernel/build.rs` from `NIFE_TEST_FILTER`, which `cargo xtask test --test`
/// sets; a compile-time constant because that is the one channel identical on all three
/// architectures. A filter change costs a kernel relink.
///
/// # BUGS
///
/// A filtered run is not the suite: whole-suite ceilings cannot fail in it, and a test that passes
/// only after another can fail alone. See notes/kernel-test-harness.md.
const TEST_FILTER: &str = env!("NIFE_TEST_FILTER");

/// **Did this run name a test by filter?** For the few tests too expensive for the whole suite,
/// which skip unless somebody asked for them by name (`script/test --test <name>`); each says why
/// in its own skip reason. Name provisional (milestone 604 (the builder's scratch cursor is
/// bounded)).
#[cfg(any(test, feature = "system_tests"))]
#[allow(dead_code)]
pub fn run_was_filtered() -> bool {
    !TEST_FILTER.is_empty()
}

/// Set by `cargo xtask test --test` (through `kernel/build.rs`) when it boots more than one test
/// image per leg and counts the selections itself (milestone 609 (the system tests leave the kernel
/// crate)). Then an image with no matching test exits cleanly and says so, and the harness fails
/// the leg if the images together selected nothing. Unset, the rule below holds per image.
const FILTER_COUNTED_ACROSS_IMAGES: bool = !env!("NIFE_TEST_FILTER_ACROSS_IMAGES").is_empty();

/// **How many times the runner repeats the filtered tests in one boot**, from `NIFE_TEST_REPEAT`
/// (name provisional; empty means once, and a value that is not a whole number of at least 1 fails
/// the build). The first failing iteration ends the boot, and [`note_repeat_failure`] names it.
/// See notes/kernel-test-harness.md.
///
/// # BUGS
///
/// - A repeat is refused without a filter: the suite keeps services for the boot.
/// - The leak check sees frames and the runner's slots, not every table.
const TEST_REPEAT: u32 = parse_repeat(env!("NIFE_TEST_REPEAT"));

const fn parse_repeat(text: &str) -> u32 {
    let bytes = text.as_bytes();
    if bytes.is_empty() {
        return 1;
    }
    let mut n: u32 = 0;
    let mut i = 0;
    while i < bytes.len() {
        assert!(
            bytes[i].is_ascii_digit(),
            "NIFE_TEST_REPEAT must be a whole number"
        );
        n = n * 10 + (bytes[i] - b'0') as u32;
        i += 1;
    }
    assert!(n >= 1, "NIFE_TEST_REPEAT must be at least 1");
    n
}

/// The iteration running now, from 1, or 0 when the run is not repeating. Read by
/// [`note_repeat_failure`] on the way out of a failed run.
static REPEAT_ITERATION: AtomicU32 = AtomicU32::new(0);

/// **Which iteration failed**, said before the failure's own message so a console that drops the
/// panic's second line (xenon's did, milestone 321) still carries it. Called by the panic handler
/// and both watchdogs; a no-op when the run is not repeating.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub(crate) fn note_repeat_failure(message: &str) {
    let iteration = REPEAT_ITERATION.load(Ordering::Relaxed);
    if iteration == 0 {
        return;
    }
    println!();
    println!(
        "repeat result: FAILED. {iteration} runs of {TEST_REPEAT}, {} passed, first failure at \
         run {iteration}; its message is {message}",
        iteration - 1
    );
}

/// The readings the repeat loop compares between iterations: what one test run can leak, and
/// what a leak runs out of.
#[derive(Clone, Copy)]
struct RepeatReading {
    frames: usize,
    run: usize,
    slots: usize,
    slot_capacity: usize,
}

impl RepeatReading {
    fn now() -> RepeatReading {
        let (slots, slot_capacity) = crate::sched::current_capability_slots();
        RepeatReading {
            frames: crate::memory::free_page_frames(),
            run: crate::memory::largest_free_run(),
            slots,
            slot_capacity,
        }
    }
}

/// **Refuse a repeat that would run dry and blame the test for it.**
///
/// The baseline is the second iteration, not the first: a first run may wire a service the boot
/// then keeps (the frame ledger's held list), which is a one-time cost rather than a leak. From
/// there the average loss per iteration is projected over the iterations left, and the run is
/// refused as soon as the projection reaches the floor. Refused, not warned: a test that fails in
/// iteration 140 with `OutOfPageFrames` reads exactly like the rare failure this mode exists to
/// count, which is the misreport the refusal prevents.
fn refuse_a_repeat_that_runs_dry(iteration: u32, baseline: RepeatReading, now: RepeatReading) {
    // Iterations since the baseline, and the ones still to run including this one.
    let since = (iteration - 2) as usize;
    let left = (TEST_REPEAT - iteration + 1) as usize;
    let frames_lost = baseline.frames.saturating_sub(now.frames);
    let frames_needed = frames_lost.div_ceil(since) * left;
    let slots_gained = now.slots.saturating_sub(baseline.slots);
    let slots_needed = slots_gained.div_ceil(since) * left;
    let frames_short = now.frames < frames_needed + SUITE_MIN_FREE_RUN;
    let run_short = now.run < SUITE_MIN_FREE_RUN;
    let slots_short = now.slots + slots_needed > now.slot_capacity;
    if !frames_short && !run_short && !slots_short {
        return;
    }
    println!();
    println!(
        "REPEAT REFUSED at run {iteration} of {TEST_REPEAT}: the selected tests do not give back \
         what they take, so the remaining runs would fail by running out rather than by anything \
         they test, and that failure would read as theirs."
    );
    println!(
        "  frames: {} free at run 2, {} now ({frames_lost} lost over {since} runs); the {left} runs \
         left need about {frames_needed}, plus the {SUITE_MIN_FREE_RUN} the ledger keeps. Longest \
         free run {}.",
        baseline.frames, now.frames, now.run
    );
    println!(
        "  capability slots: {} held at run 2, {} now, of {}.",
        baseline.slots, now.slots, now.slot_capacity
    );
    println!(
        "  Fix the test's teardown, or lower NIFE_TEST_REPEAT. See kernel/src/testing.rs, \
         TEST_REPEAT."
    );
    semihosting::exit(semihosting::EXIT_FAILURE);
}

static HEARTBEAT: AtomicU64 = AtomicU64::new(0);
static WATCH_LAST_HB: AtomicU64 = AtomicU64::new(0);
static WATCH_STALL_TICKS: AtomicU64 = AtomicU64::new(0);

/// When the running test started, in `arch::timer::now()` counter units, or 0 for "no test is
/// running" (during boot, and between tests), which disables the ceiling.
static TEST_START: AtomicU64 = AtomicU64::new(0);

/// The running test's wall-clock budget in counter units, and its name, for the failure report. The
/// name is a `&'static str` split into pointer and length so the tick path never takes a lock; it is
/// only reassembled on the failure path.
static TEST_BUDGET: AtomicU64 = AtomicU64::new(0);
static TEST_NAME_PTR: AtomicPtr<u8> = AtomicPtr::new(core::ptr::null_mut());
static TEST_NAME_LEN: AtomicUsize = AtomicUsize::new(0);

/// **The current test's skip reason**, set by [`skip!`] when the fixture a test needs is not on
/// this machine: a third outcome beside pass (return) and fail (panic). `skip!` is a macro so its
/// `return` leaves the test itself. See notes/kernel-test-harness.md.
pub static SKIP_REASON: AtomicPtr<u8> = AtomicPtr::new(core::ptr::null_mut());
pub static SKIP_REASON_LEN: AtomicUsize = AtomicUsize::new(0);
/// **A test printed the word "skip" while it was running**, set by the console writer and read
/// once the test returns. See [`note_printed`] and the check in `Testable::run`.
#[cfg(any(test, feature = "system_tests"))]
static PRINTED_A_SKIP_WORD: AtomicBool = AtomicBool::new(false);

/// **Note a console fragment written while a test runs**, so a test that prints "skip" and then
/// returns without a skip reason fails rather than counting as a pass (milestone 214 (a test that
/// prints "skipping" and returns is counted as passed)). It reads what was printed, so it needs no
/// allow-list. A test that prints "skip" for another reason and passes is a false positive; the
/// tree has none. See notes/kernel-test-harness.md.
#[cfg(any(test, feature = "system_tests"))]
pub(crate) fn note_printed(fragment: &str) {
    if fragment.contains("skip") {
        PRINTED_A_SKIP_WORD.store(true, Ordering::Relaxed);
    }
}

/// How many tests this run has skipped, for the final line. Read once, at the end; never
/// compared against anything today, which is milestone 145's own open question (a run that
/// skips more over time is a fact worth someone eventually gating on, not silently absorbing).
static SKIPPED: AtomicUsize = AtomicUsize::new(0);

/// **Skip the current test**, because the fixture it needs is not attached to this boot.
///
/// `reason` should name the missing thing the way the old `.expect(...)` message did ("no
/// virtio-rng device on the mmio bus"), because that string is now the only record of why the
/// test did not run. Must be called from directly inside a `#[test_case]` function (it returns
/// from its caller); calling it from a nested helper returns out of the helper instead, which
/// is a bug at the call site, not in the macro.
// Exported, with its statics at the crate root, because most callers are in `system_tests/`,
// where `$crate::testing` is a private module.
#[cfg(any(test, feature = "system_tests"))]
#[allow(unused_macros)]
#[macro_export]
macro_rules! skip {
    ($reason:expr) => {{
        let reason: &'static str = $reason;
        $crate::SKIP_REASON.store(
            reason.as_ptr() as *mut u8,
            core::sync::atomic::Ordering::Relaxed,
        );
        $crate::SKIP_REASON_LEN.store(reason.len(), core::sync::atomic::Ordering::Relaxed);
        return;
    }};
}
#[cfg(any(test, feature = "system_tests"))]
#[allow(unused_imports)]
pub use crate::skip;

// The frame ledger: what each test never gave back, and the longest run still free at the end.
// One bitmap scan per test, between tests. See notes/kernel-test-harness.md.

/// Free frames when the first test started: the ledger's opening balance.
static FRAMES_AT_START: AtomicUsize = AtomicUsize::new(0);
/// Whether [`FRAMES_AT_START`] has been stamped (0 is a legal reading, so a flag rather than a
/// sentinel).
static FRAMES_STAMPED: AtomicBool = AtomicBool::new(false);
/// The reading taken at the top of the test now running, and that test's name. The charge against a
/// test is the drop from *its* reading to the *next* one, which is why both are carried forward.
static FRAMES_AT_PREV: AtomicUsize = AtomicUsize::new(0);
static PREV_NAME_PTR: AtomicPtr<u8> = AtomicPtr::new(core::ptr::null_mut());
static PREV_NAME_LEN: AtomicUsize = AtomicUsize::new(0);
/// The worst single spender so far, and its name, for the closing summary. Same pointer/length
/// trick as the test name above: no allocation, no lock.
static WORST_SPEND: AtomicUsize = AtomicUsize::new(0);
static WORST_NAME_PTR: AtomicPtr<u8> = AtomicPtr::new(core::ptr::null_mut());
static WORST_NAME_LEN: AtomicUsize = AtomicUsize::new(0);

/// Report a test's frame cost once it reaches this many frames. Below it, silence: a test that
/// spends two pages on an endpoint is not news, and a number on every line buries the ones that
/// are. Sixteen frames is 64 KiB, about the smallest thing a service-shaped test takes.
const PAGE_FRAME_REPORT_MIN: usize = 16;

/// **What the whole suite may leave unreturned, in frames**, checked once at the end of the run.
///
/// Not zero: [notes/frame-budget.md](../../notes/frame-budget.md) prices what a boot keeps on
/// purpose. Just above the measured total, so a test that keeps its memory fails here, naming
/// itself, rather than later as `OutOfPageFrames` in something innocent.
///
/// # BUGS
///
/// - Two test sites keep their endpoints for the boot on purpose (notes/kernel-test-harness.md).
/// - The kernel's `user::*_service` wiring still uses the never-freed pool, once per test.
const SUITE_PAGE_FRAME_BUDGET: usize = 24_322;

/// **The longest run of free frames the boot must still have at the end**, in frames.
///
/// Loading a program needs a contiguous run, and a suite can pass the residue ceiling while
/// fragmented: one boot ended with 216 free and no run longer than 117, and every larger program
/// failed to load in whichever test came next. 1024 frames (4 MiB) is well above the largest
/// program this boot loads and well under the 14,080 measured after reclamation.
const SUITE_MIN_FREE_RUN: usize = 1024;

/// **Charge the test that just finished, and open an account for the one about to start.**
///
/// The readings partition the run: a test is charged the drop from its own reading to the next
/// one, so every frame lost between the first test and the last lands on exactly one test.
/// Reading around the body instead misses a service left running, which on the first measured
/// aarch64 boot was 17,362 of 29,091 frames. The cost is a one-test lag in the transcript.
fn charge_previous(now: usize, next: &'static str) {
    let prev_ptr = PREV_NAME_PTR.load(Ordering::Relaxed);
    if !prev_ptr.is_null() {
        let spent = FRAMES_AT_PREV.load(Ordering::Relaxed).saturating_sub(now);
        if spent >= PAGE_FRAME_REPORT_MIN {
            println!("    [that test kept {spent} frames]");
        }
        if spent > WORST_SPEND.load(Ordering::Relaxed) {
            WORST_SPEND.store(spent, Ordering::Relaxed);
            WORST_NAME_PTR.store(prev_ptr, Ordering::Relaxed);
            WORST_NAME_LEN.store(PREV_NAME_LEN.load(Ordering::Relaxed), Ordering::Relaxed);
        }
    }
    FRAMES_AT_PREV.store(now, Ordering::Relaxed);
    PREV_NAME_PTR.store(next.as_ptr() as *mut u8, Ordering::Relaxed);
    PREV_NAME_LEN.store(next.len(), Ordering::Relaxed);
}

/// The worst spender's name, reassembled. Only called by the closing summary.
fn worst_spender_name() -> Option<&'static str> {
    let ptr = WORST_NAME_PTR.load(Ordering::Relaxed);
    let len = WORST_NAME_LEN.load(Ordering::Relaxed);
    if ptr.is_null() || len == 0 {
        return None;
    }
    // SAFETY: the pair was stored from a `&'static str` (`core::any::type_name`), which lives for
    // the whole program, and is only ever overwritten by another such pair.
    unsafe { core::str::from_utf8(core::slice::from_raw_parts(ptr as *const u8, len)).ok() }
}

/// Print the ledger, and fail the run if the boot ends with no usable contiguous run
/// ([`SUITE_MIN_FREE_RUN`]) or with more kept than is accounted for ([`SUITE_PAGE_FRAME_BUDGET`]).
fn report_page_frame_ledger() {
    if !FRAMES_STAMPED.load(Ordering::Relaxed) {
        return; // no test ran; nothing was spent
    }
    let end = crate::memory::free_page_frames();
    // Close the last test's account, so the charges partition the whole run with nothing left over.
    charge_previous(end, "");
    let start = FRAMES_AT_START.load(Ordering::Relaxed);
    let run = crate::memory::largest_free_run();
    let spent = start.saturating_sub(end);
    println!(
        "frames: {start} free before the first test, {end} after the last ({spent} never returned); \
         longest free run {run}"
    );
    if let Some(name) = worst_spender_name() {
        println!(
            "  the biggest single spender was {name} at {} frames",
            WORST_SPEND.load(Ordering::Relaxed)
        );
    }
    if run < SUITE_MIN_FREE_RUN {
        println!();
        println!(
            "FRAME LEDGER: the boot ends with no free run longer than {run} frames, under the \
             {SUITE_MIN_FREE_RUN} this gate requires. This is the failure rather than a warning \
             about one: loading a program takes a contiguous run, so the next test to load anything \
             substantial would fail as Unmappable(OutOfPageFrames), and it would do so in whichever \
             test happened to be next rather than in the one that spent the memory. Read the \
             `[that test kept N frames]` lines above for who grew. See notes/frames.md."
        );
        semihosting::exit(semihosting::EXIT_FAILURE);
    }
    if spent > SUITE_PAGE_FRAME_BUDGET {
        println!();
        println!(
            "FRAME LEDGER: the suite kept {spent} frames against a budget of {SUITE_PAGE_FRAME_BUDGET}. \
             A test built a service and did not hand its memory back. Read the \
             `[that test kept N frames]` lines above for who grew; the reclaim path is \
             `kill_thread` + `sched::reclaim_region`, wrapped as `user::holding::Holding`. Do not \
             raise the budget without an account of what is permanent and why. See notes/frames.md."
        );
        semihosting::exit(semihosting::EXIT_FAILURE);
    }
}

/// **How close the boot came to the thread table's ceiling**, printed beside the frame ledger.
///
/// It reports and does not gate: what fills the table is the services earlier tests meant to keep,
/// so a climbing peak is usually a boot doing more, and `sched::thread_leak_police` catches the
/// wrong kind of growth. See `sched::MAX_THREADS`.
fn report_thread_peak() {
    let peak = crate::sched::peak_thread_count();
    let max = crate::sched::MAX_THREADS;
    println!(
        "threads: {peak} live at the peak, of {max} the image allows ({} spare). \
         See sched::MAX_THREADS.",
        max.saturating_sub(peak)
    );
}

/// **The test that last raised the region table's high-water mark**, as a `&'static str` split into
/// pointer and length the way [`WORST_NAME_PTR`] is. Null when the peak was set before the first
/// test (the boot's own services), or never moved during one.
static REGION_PEAK_NAME_PTR: AtomicPtr<u8> = AtomicPtr::new(core::ptr::null_mut());
static REGION_PEAK_NAME_LEN: AtomicUsize = AtomicUsize::new(0);

/// **How close the boot came to the region table's ceiling**, and the test during which the peak
/// was set: where to start reading, not who is to blame, since a leak many tests earlier can set
/// it. Reports and does not gate, for [`report_thread_peak`]'s reason; `memory_region::MAX_REGIONS`
/// says who holds what.
fn report_region_peak() {
    let peak = crate::memory_region::peak_region_count();
    let max = crate::memory_region::MAX_REGIONS;
    let during = stored_name(&REGION_PEAK_NAME_PTR, &REGION_PEAK_NAME_LEN);
    println!(
        "regions: {peak} live at the peak, of {max} the image allows ({} spare), set during {}. \
         See memory_region::MAX_REGIONS.",
        max.saturating_sub(peak),
        during.unwrap_or("the boot, before the first test"),
    );
}

/// **The test during which the rendezvous registry last reached a new peak.**
static RENDEZVOUS_PEAK_NAME_PTR: AtomicPtr<u8> = AtomicPtr::new(core::ptr::null_mut());
static RENDEZVOUS_PEAK_NAME_LEN: AtomicUsize = AtomicUsize::new(0);

/// **How close the boot came to the rendezvous registry's ceiling**, printed beside the region
/// peak, for the same reason and in the same shape. It adds the one split that decides what can
/// be done about a high number: how many rendezvous sit on the kernel's own chunks, which are
/// never freed, against the ones retyped from a region that goes when its region does. The ledger
/// is at `sched::PEAK_RENDEZVOUS`.
fn report_rendezvous_peak() {
    let (peak, kernel) = crate::sched::rendezvous_pressure();
    let max = crate::sched::MAX_RENDEZVOUS;
    println!(
        "rendezvous: {peak} live at the peak, of {max} the image allows ({} spare), set during {}; \
         {kernel} were created on kernel chunks, which are never freed. See sched::MAX_RENDEZVOUS.",
        max.saturating_sub(peak),
        stored_name(&RENDEZVOUS_PEAK_NAME_PTR, &RENDEZVOUS_PEAK_NAME_LEN)
            .unwrap_or("the boot, before the first test"),
    );
}

/// **The test during which free frames last reached a new low**, and the test during which the
/// allocator first refused a request. Same shape as [`REGION_PEAK_NAME_PTR`].
static FRAME_LOW_NAME_PTR: AtomicPtr<u8> = AtomicPtr::new(core::ptr::null_mut());
static FRAME_LOW_NAME_LEN: AtomicUsize = AtomicUsize::new(0);
static FIRST_REFUSAL_NAME_PTR: AtomicPtr<u8> = AtomicPtr::new(core::ptr::null_mut());
static FIRST_REFUSAL_NAME_LEN: AtomicUsize = AtomicUsize::new(0);

/// A test name stored as a pointer and length, reassembled, or `None` if none was stored.
fn stored_name(ptr: &AtomicPtr<u8>, len: &AtomicUsize) -> Option<&'static str> {
    let (ptr, len) = (ptr.load(Ordering::Relaxed), len.load(Ordering::Relaxed));
    if ptr.is_null() || len == 0 {
        return None;
    }
    // SAFETY: every pair is stored from a `&'static str` (`core::any::type_name`) by
    // `Testable::run`, and is only ever overwritten by another such pair.
    unsafe { core::str::from_utf8(core::slice::from_raw_parts(ptr, len)).ok() }
}

/// **The fewest frames free at any moment, and what the allocator refused**, printed beside the
/// region peak. The ledger reads only the run's ends, so a dip to nothing in the middle passes it
/// while a later test fails far from the spender; this names the test the low came in. Reports and
/// does not gate. `memory::FREE_LOW_WATER` carries the ledger.
fn report_frame_pressure() {
    let (low, refused, largest) = crate::memory::allocation_pressure();
    if low == usize::MAX {
        return; // nothing allocated after the allocator came up; nothing to say
    }
    let during = stored_name(&FRAME_LOW_NAME_PTR, &FRAME_LOW_NAME_LEN)
        .unwrap_or("the boot, before the first test");
    println!("frames: at the lowest {low} were free, during {during}. See memory::FREE_LOW_WATER.");
    if refused > 0 {
        println!(
            "  {refused} allocations were refused, the largest a request for {largest} frames; \
             the first during {}",
            stored_name(&FIRST_REFUSAL_NAME_PTR, &FIRST_REFUSAL_NAME_LEN)
                .unwrap_or("the boot, before the first test"),
        );
    }
}

/// Report a test's duration once it reaches this many seconds. Below it, silence: most tests are
/// milliseconds and a duration on every line would bury the signal. Above it, the number is what makes
/// a `test_times::SLOW_TESTS` entry an evidence-based declaration rather than a guess: until this existed, the
/// only way to learn a test's real cost was to set its budget too low on purpose and read the failure.
const SLOW_REPORT_SECS: u64 = 5;

/// **How many tests one image can record a time for** (milestone 807 (the kernel suite reports
/// what each test cost)). The largest image held 320 tests on 2026-10-06 (riscv64's
/// `system_tests`), so this is three times that. A fixed table rather than a `Vec` because the
/// record must not allocate: the frame ledger and the heap tests measure exactly what an
/// allocation here would disturb. 4 KiB of `.bss`. An image past it fails before its first test,
/// in [`runner`], rather than printing a record with a hole in it.
const TIME_RECORD_CAPACITY: usize = 1024;

/// Each selected test's elapsed milliseconds, by its index in the runner's slice. `u32` holds 49
/// days, which no test survives: the ceiling is minutes.
static TEST_MILLISECONDS: [AtomicU32; TIME_RECORD_CAPACITY] =
    [const { AtomicU32::new(0) }; TIME_RECORD_CAPACITY];

/// What the test that just returned cost, in milliseconds, set by [`Testable::run`] on both of its
/// exits (a pass and a skip) and read by [`runner`] straight after. A static rather than a return
/// value so `Testable` keeps the signature every `#[test_case]` already satisfies.
static LAST_TEST_MILLISECONDS: AtomicU64 = AtomicU64::new(0);

/// Record one step of forward progress for the hang watchdog (test builds only). Cheap enough to sit
/// on the wake and console paths: one relaxed increment. See the module note above.
#[inline]
pub(crate) fn note_progress() {
    HEARTBEAT.fetch_add(1, Ordering::Relaxed);
}

/// Is any online core running a real thread (not its idle fallback)? A lost-wakeup hang leaves every
/// core parked on idle; a slow-but-live test (a userspace CPU-bound loop like `std_net`'s smoltcp
/// poll) always has one running. Read-only across the per-CPU blocks; racy by nature, which a
/// heartbeat sampled once per tick tolerates.
fn any_cpu_running_real_work() -> bool {
    // The online set, not `0..count`: an online set of {1,2,3} has no core 0, and a count-as-index
    // scan would never look at core 3.
    crate::smp::online_cpus().any(|c| {
        let pc = crate::cpu::of(c);
        let cur = pc.current.load(Ordering::Relaxed);
        cur != crate::cpu::NO_TID && cur != pc.idle.load(Ordering::Relaxed)
    })
}

/// Called from the timer IRQ each tick (test builds only; see `timer::tick`). Only the boot core
/// watches, so any dump happens once. The boot core is `arch::boot_cpu_id()` (0 on aarch64, but on
/// RISC-V whichever hart QEMU booted), which is also the one hart that ticks in a single-hart test.
pub(crate) fn watchdog_tick() {
    if crate::cpu::id() != crate::arch::boot_cpu_id() {
        return;
    }

    // Mechanism 2 first: a test over its budget fails even while it is making progress, which is the
    // whole point (a livelock doing IPC keeps the heartbeat below perfectly healthy).
    check_test_ceiling();

    const STALL_LIMIT: u64 = 6000; // ticks at 100 Hz = 60 s with no progress at all
    let hb = HEARTBEAT.load(Ordering::Relaxed);
    let progress = hb != WATCH_LAST_HB.load(Ordering::Relaxed) || any_cpu_running_real_work();
    if progress {
        WATCH_LAST_HB.store(hb, Ordering::Relaxed);
        WATCH_STALL_TICKS.store(0, Ordering::Relaxed);
        return;
    }
    if WATCH_STALL_TICKS.fetch_add(1, Ordering::Relaxed) + 1 == STALL_LIMIT {
        println!();
        println!(
            "WATCHDOG: no progress for ~60 s. Every core idle, every thread blocked: a lost-wakeup hang."
        );
        crate::sched::dump_threads();
        note_repeat_failure("above");
        semihosting::exit(semihosting::EXIT_FAILURE);
    }
}

/// **Has the running test exceeded its wall-clock budget?** (mechanism 2; see the module note.) Fails
/// the run with the test's name, how long it actually ran, and its budget, so a future livelock reads
/// as "this test exceeded its budget while making progress" and not as an anonymous timeout.
fn check_test_ceiling() {
    let start = TEST_START.load(Ordering::Relaxed);
    if start == 0 {
        return; // no test running: boot, or between tests
    }
    let budget = TEST_BUDGET.load(Ordering::Relaxed);
    let elapsed = crate::arch::timer::now().wrapping_sub(start);
    if budget == 0 || elapsed <= budget {
        return;
    }

    // Over budget. Disarm first, so anything that prints or wakes from here (the dump) cannot
    // re-enter this path and report twice.
    TEST_START.store(0, Ordering::Relaxed);

    let hz = crate::arch::timer::frequency().max(1);
    println!();
    println!(
        "WATCHDOG: test exceeded its {} s budget (ran {} s) WHILE MAKING PROGRESS: a livelock, not a \
         lost wakeup. The no-progress heartbeat cannot see this, which is why the per-test ceiling \
         exists. If this test is honestly this slow, give it an entry in test_times::SLOW_TESTS.",
        budget / hz,
        elapsed / hz,
    );
    if let Some(name) = current_test_name() {
        println!("  test: {name}");
    }
    crate::sched::dump_threads();
    note_repeat_failure("above");
    semihosting::exit(semihosting::EXIT_FAILURE);
}

/// The running test's name, reassembled from the pointer/length pair stamped by [`Testable::run`].
/// Only called on the failure path.
fn current_test_name() -> Option<&'static str> {
    let ptr = TEST_NAME_PTR.load(Ordering::Relaxed);
    let len = TEST_NAME_LEN.load(Ordering::Relaxed);
    if ptr.is_null() || len == 0 {
        return None;
    }
    // SAFETY: the pair was stored from a `&'static str` (`core::any::type_name`), which lives for the
    // whole program, and is only ever overwritten by another such pair.
    unsafe {
        let bytes = core::slice::from_raw_parts(ptr as *const u8, len);
        core::str::from_utf8(bytes).ok()
    }
}

/// **A budget in timer ticks delivered to this core, for a test that has to wait.**
///
/// A descheduled emulator misses deadlines, so on a busy host fewer ticks arrive and the budget
/// stretches with the guest's work, where a wall-clock one would shrink. A change of core
/// re-anchors, since tick counts are per core. See notes/kernel-test-harness.md.
///
/// # EXAMPLES
///
/// ```ignore
/// // Two seconds' worth of preemption opportunities on a quiet host, and more wall clock than that
/// // on a busy one.
/// let mut budget = TickBudget::new(2 * crate::arch::timer::TICK_HZ);
/// while !done() {
///     assert!(!budget.is_expired(), "the workers never drained");
///     crate::sched::yield_now();
/// }
/// ```
#[cfg(any(test, feature = "system_tests"))]
pub struct TickBudget {
    cpu: usize,
    start: u64,
    ticks: u64,
}

#[cfg(any(test, feature = "system_tests"))]
impl TickBudget {
    /// Start a budget of `ticks` timer ticks on whatever core is running now.
    pub fn new(ticks: u64) -> Self {
        let (cpu, start) = Self::sample();
        Self { cpu, start, ticks }
    }

    /// Has the budget run out? Re-anchors and returns `false` if this thread changed core.
    pub fn is_expired(&mut self) -> bool {
        let (cpu, now) = Self::sample();
        if cpu != self.cpu {
            self.cpu = cpu;
            self.start = now;
            return false;
        }
        now - self.start >= self.ticks
    }

    /// Read the core id and that core's tick count as a pair, re-reading if we moved between the two
    /// reads. Without this the pair can name one core's id and another's counter, which is the
    /// migration hazard this type exists to handle arriving inside the handler for it.
    fn sample() -> (usize, u64) {
        loop {
            let cpu = crate::cpu::id();
            let ticks = crate::arch::timer::ticks();
            if crate::cpu::id() == cpu {
                return (cpu, ticks);
            }
        }
    }
}

/// Lets us print a test's name before running it. `core::any::type_name` gives us
/// the full path of the function, which is close enough to a test name.
pub trait Testable {
    fn run(&self);

    /// This test's name, without running it. [`runner`] needs it to decide whether a filter
    /// selects this test, and that decision has to happen *before* `run` prints a line or charges
    /// the frame ledger, so it cannot be the `type_name` call inside `run`.
    fn name(&self) -> &'static str;
}

impl<T: Fn()> Testable for T {
    fn name(&self) -> &'static str {
        core::any::type_name::<T>()
    }

    fn run(&self) {
        let name = self.name();

        // Before the name is printed: this closes the previous test's account under its own `ok`
        // line. The opening balance is the first test's, since boot is not a test's doing.
        let frames_now = crate::memory::free_page_frames();
        if !FRAMES_STAMPED.swap(true, Ordering::Relaxed) {
            FRAMES_AT_START.store(frames_now, Ordering::Relaxed);
        }
        charge_previous(frames_now, name);

        print!("test {name} ... ");
        HEARTBEAT.fetch_add(1, Ordering::Relaxed); // tell the watchdog this test started

        // Arm the per-test wall-clock ceiling (mechanism 2). The name goes first and the start time
        // last, because a non-zero start is what arms the check: this way the watchdog never sees a
        // live budget with a stale name.
        TEST_NAME_PTR.store(name.as_ptr() as *mut u8, Ordering::Relaxed);
        TEST_NAME_LEN.store(name.len(), Ordering::Relaxed);
        TEST_BUDGET.store(
            budget_secs_for(name) * crate::arch::timer::frequency(),
            Ordering::Relaxed,
        );
        let start = crate::arch::timer::now();
        // `now()` could legitimately be 0 on the very first tick of a fresh counter, and 0 is the
        // disarmed sentinel, so nudge it. One counter unit of slack is nothing against a 90 s budget.
        TEST_START.store(if start == 0 { 1 } else { start }, Ordering::Relaxed);

        // Cleared HERE rather than before the name is printed, so a test whose own name contains
        // "skip" does not accuse itself. See note_printed.
        #[cfg(any(test, feature = "system_tests"))]
        PRINTED_A_SKIP_WORD.store(false, Ordering::Relaxed);

        // Attribute the high-water marks to the test that moved them. Read around the body rather
        // than inside the kernel, so the instruments know nothing about tests.
        let region_peak_before = crate::memory_region::peak_region_count();
        let (rendezvous_peak_before, _) = crate::sched::rendezvous_pressure();
        let (frames_low_before, refused_before, _) = crate::memory::allocation_pressure();
        self();
        let mark = |ptr: &AtomicPtr<u8>, len: &AtomicUsize| {
            ptr.store(name.as_ptr() as *mut u8, Ordering::Relaxed);
            len.store(name.len(), Ordering::Relaxed);
        };
        if crate::memory_region::peak_region_count() > region_peak_before {
            mark(&REGION_PEAK_NAME_PTR, &REGION_PEAK_NAME_LEN);
        }
        if crate::sched::rendezvous_pressure().0 > rendezvous_peak_before {
            mark(&RENDEZVOUS_PEAK_NAME_PTR, &RENDEZVOUS_PEAK_NAME_LEN);
        }
        let (frames_low_after, refused_after, _) = crate::memory::allocation_pressure();
        if frames_low_after < frames_low_before {
            mark(&FRAME_LOW_NAME_PTR, &FRAME_LOW_NAME_LEN);
        }
        if refused_after > refused_before
            && FIRST_REFUSAL_NAME_PTR.load(Ordering::Relaxed).is_null()
        {
            mark(&FIRST_REFUSAL_NAME_PTR, &FIRST_REFUSAL_NAME_LEN);
        }

        // Disarm: between tests there is no budget to exceed, and the next test arms its own.
        TEST_START.store(0, Ordering::Relaxed);

        // A test that called skip!() left a reason here instead of returning normally. Report it
        // and stop: the frame-ledger charge and the stack-intact check below still apply (a
        // skipped test can still smash the stack on its way out), but there is no "ok" to print.
        let skip_ptr = SKIP_REASON.swap(core::ptr::null_mut(), Ordering::Relaxed);
        if !skip_ptr.is_null() {
            let len = SKIP_REASON_LEN.swap(0, Ordering::Relaxed);
            // SAFETY: skip!() only ever stores the pointer and length of a &'static str it holds
            // for the duration of the call, and the swap above is the only reader, so this runs
            // at most once per store.
            let reason = unsafe {
                core::str::from_utf8_unchecked(core::slice::from_raw_parts(skip_ptr, len))
            };
            SKIPPED.fetch_add(1, Ordering::Relaxed);

            let ticks = crate::arch::timer::now().saturating_sub(start);
            record_milliseconds(ticks);
            let elapsed = ticks / crate::arch::timer::frequency();
            if elapsed >= SLOW_REPORT_SECS {
                print!("[{elapsed} s] ");
            }
            assert!(
                crate::stack::is_intact(),
                "this test smashed the stack (headroom: {})",
                crate::stack::headroom()
            );
            println!("skipped: {reason}");
            return;
        }

        // A test that said "skip" and returned without a reason misreports itself (milestone 214).
        #[cfg(any(test, feature = "system_tests"))]
        if PRINTED_A_SKIP_WORD.swap(false, Ordering::Relaxed) {
            panic!(
                "this test printed \"skip\" and then returned, so the run counts it as a PASS. \
                 A test whose fixture is absent calls testing::skip!(reason), which returns and \
                 puts it in the skipped column; a println! and a `return` are indistinguishable \
                 from proving the claim. See design/roadmap/0214-print-and-return-skips.md.",
            );
        }

        // Every figure goes to the record the runner prints after the suite; only a slow one shows
        // on the `ok` line.
        let ticks = crate::arch::timer::now().saturating_sub(start);
        record_milliseconds(ticks);
        let elapsed = ticks / crate::arch::timer::frequency();
        if elapsed >= SLOW_REPORT_SECS {
            print!("[{elapsed} s] ");
        }

        // Here, so an overflow is blamed on the test that did it, not a later one (notes/stack.md).
        assert!(
            crate::stack::is_intact(),
            "this test smashed the stack (headroom: {})",
            crate::stack::headroom()
        );

        println!("ok");
    }
}

/// Keep the test that just ran's cost for [`runner`] (milestone 807). The stamp is the one the
/// ceiling already takes, so the figure costs one conversion; `test_times::milliseconds`
/// does it in 128 bits, because a TSC's frequency times a long test can outgrow 64.
fn record_milliseconds(ticks: u64) {
    LAST_TEST_MILLISECONDS.store(
        test_times::milliseconds(ticks, crate::arch::timer::frequency()),
        Ordering::Relaxed,
    );
}

/// **The per-test time record** (milestone 807 (the kernel suite reports what each test cost),
/// §254 (a gate prints what each item cost), Fork 1): one `time <milliseconds> <full test path>` line for every test this image ran,
/// in the order it ran them, under `test_times::HEADING`.
///
/// It comes after the other reports and before `test result:`, so every line `xtask`,
/// `script/falsifications` and the HVF leg already parse is where it was. `xtask` reads it, checks
/// that it names every test the transcript names, and cross-checks its sum against the host's
/// clock; `crates/test_times` holds the grammar both sides use.
fn report_test_times(tests: &[&dyn Testable], filter: &str) {
    println!("{}", test_times::HEADING);
    for (index, test) in tests.iter().enumerate() {
        if filter.is_empty() || test.name().contains(filter) {
            let line = test_times::Line {
                milliseconds: u64::from(TEST_MILLISECONDS[index].load(Ordering::Relaxed)),
                path: test.name(),
            };
            println!("{line}");
        }
    }
}

/// **The livelock probe** (feature `watchdog_probe`, EXPECTED TO FAIL the run): it loops forever
/// doing a full IPC rendezvous each pass, so the heartbeat sees a healthy kernel and only the
/// per-test ceiling can catch it.
///
/// ```text
/// helpers/qemu-bounded.sh 200 cargo test -p kernel \
///     --features watchdog_probe --target aarch64-unknown-none-softfloat
/// ```
///
/// Expected: the run fails after the 90 s default budget, naming this test.
#[cfg(all(any(test, feature = "system_tests"), feature = "watchdog_probe"))]
#[test_case]
fn a_livelock_that_keeps_doing_ipc_trips_the_per_test_ceiling() {
    let ep = crate::sched::create_rendezvous();

    // A partner that answers forever. Both sides rendezvous every pass, so every pass is a wake, and
    // a wake is "progress" as far as the heartbeat is concerned.
    crate::sched::spawn(move || {
        loop {
            let _ = crate::sched::ipc_receive(ep);
        }
    })
    .expect("probe: could not spawn the partner");

    loop {
        crate::sched::ipc_send(ep, [1, 2, 3]);
    }
}

/// Runs every `#[test_case]` in the crate, then exits QEMU.
///
/// A panic anywhere in here lands in the panic handler, which exits with a failure
/// status instead. So there is no "count the failures" logic: the first failing
/// assertion terminates the run. Crude, but a kernel with a failed invariant has no
/// business continuing anyway.
pub fn runner(tests: &[&dyn Testable]) {
    // A substring of the full path, as `cargo test <name>` matches; a full path is already unique,
    // so there is no `--exact`.
    let filter = TEST_FILTER;
    let selected = if filter.is_empty() {
        tests.len()
    } else {
        tests.iter().filter(|t| t.name().contains(filter)).count()
    };

    println!();
    if filter.is_empty() {
        println!("running {selected} tests");
    } else {
        println!(
            "running {selected} of {} tests (filter: {filter})",
            tests.len()
        );
    }
    println!();

    // A filter that selects nothing fails the run: "ok. 0 passed" for a typo would prove nothing.
    if selected == 0 && FILTER_COUNTED_ACROSS_IMAGES {
        // Not a verdict: another image in this leg may hold the test, and `cargo xtask test` sums
        // the `running` lines. The result line keeps the shape its readers parse.
        println!("no test in this image matches the filter `{filter}`; the harness counts the leg");
        println!("test result: ok. 0 passed");
        semihosting::exit(semihosting::EXIT_SUCCESS)
    }
    if selected == 0 {
        println!("no test matches the filter `{filter}`");
        // `--test` selects tests, not architectures (DECISIONS §19 (architectural parity is a
        // tenet)), so an aarch64-only test lands here on the other legs.
        println!("  (a test only this architecture lacks? `--test` runs every leg; add `--arch`)");
        semihosting::exit(semihosting::EXIT_FAILURE)
    }

    // Fail now rather than print a record with a hole that `xtask` fails at the suite's end.
    assert!(
        tests.len() <= TIME_RECORD_CAPACITY,
        "this image has {} tests and the per-test time record holds {TIME_RECORD_CAPACITY}; \
         raise TIME_RECORD_CAPACITY in kernel/src/testing.rs",
        tests.len(),
    );

    run_selected(tests, filter);

    println!();
    #[cfg(any(test, feature = "system_tests"))]
    // the runner itself is compiled in every build; the instrument only exists in test
    crate::stack::report_high_water();
    report_page_frame_ledger();
    report_thread_peak();
    report_region_peak();
    report_rendezvous_peak();
    report_frame_pressure();

    println!();
    report_test_times(tests, filter);

    println!();
    // A skipped test is not a pass, and the result line keeps the two apart (milestone 145 (a test
    // that needs hardware the boot doesn't have can say so)).
    let skipped = SKIPPED.load(Ordering::Relaxed);
    if skipped > 0 {
        println!(
            "test result: ok. {} passed, {skipped} skipped",
            selected - skipped
        );
    } else {
        println!("test result: ok. {selected} passed");
    }

    semihosting::exit(semihosting::EXIT_SUCCESS)
}

/// **Run every selected test, `TEST_REPEAT` times over**, reading the leak instruments between runs
/// when it is more than once. See [`TEST_REPEAT`].
fn run_selected(tests: &[&dyn Testable], filter: &str) {
    let repeating = TEST_REPEAT > 1;
    if repeating && filter.is_empty() {
        println!(
            "NIFE_TEST_REPEAT={TEST_REPEAT} without a filter: a repeat runs named tests, not the \
             suite, whose services stay up for the boot. Add `--test <name>`. See TEST_REPEAT."
        );
        semihosting::exit(semihosting::EXIT_FAILURE)
    }
    if repeating {
        println!(
            "{} {TEST_REPEAT} times (NIFE_TEST_REPEAT)",
            test_times::REPEAT_ANNOUNCEMENT
        );
    }

    let mut baseline = None;
    let mut previous_frames = None;
    for iteration in 1..=TEST_REPEAT {
        if repeating {
            let now = announce_repeat_run(iteration, previous_frames);
            previous_frames = Some(now.frames);
            match baseline {
                None if iteration == 2 => baseline = Some(now),
                Some(base) => refuse_a_repeat_that_runs_dry(iteration, base, now),
                None => {}
            }
        }
        for (index, test) in tests.iter().enumerate() {
            if filter.is_empty() || test.name().contains(filter) {
                test.run();
                // The record holds the sum over every run; `xtask` knows a repeated run by
                // `test_times::REPEAT_ANNOUNCEMENT` and reads it as one.
                let ms = LAST_TEST_MILLISECONDS.load(Ordering::Relaxed);
                let sum = u64::from(TEST_MILLISECONDS[index].load(Ordering::Relaxed)) + ms;
                TEST_MILLISECONDS[index]
                    .store(u32::try_from(sum).unwrap_or(u32::MAX), Ordering::Relaxed);
            }
        }
    }
    if repeating {
        let end = RepeatReading::now();
        println!();
        println!(
            "repeat result: ok. {TEST_REPEAT} runs, {TEST_REPEAT} passed, no failure; {} frames \
             free after the last run, {} capability slots held",
            end.frames, end.slots,
        );
        // Off the iteration, so a failure in the reports below is not blamed on the last run.
        REPEAT_ITERATION.store(0, Ordering::Relaxed);
    }
}

/// Open repeated run `iteration`: mark it for [`note_repeat_failure`] and print what it starts
/// with, which is what the next run's line is compared against.
fn announce_repeat_run(iteration: u32, previous_frames: Option<usize>) -> RepeatReading {
    REPEAT_ITERATION.store(iteration, Ordering::Relaxed);
    let now = RepeatReading::now();
    print!(
        "repeat run {iteration} of {TEST_REPEAT}: {} frames free",
        now.frames
    );
    // Negative when a run handed back more than it took, which a reclaim landing late on another
    // core does; signed so that reads as what it is.
    if let Some(p) = previous_frames {
        print!(
            " ({:+} since the last run)",
            now.frames as isize - p as isize
        );
    }
    println!(
        ", longest free run {}, {} of {} capability slots held, {} threads",
        now.run,
        now.slots,
        now.slot_capacity,
        crate::sched::thread_count(),
    );
    now
}

/// **A root region's run of frames, named while the region is alive**, so a frame-return test asks
/// whether those frames came back rather than whether nothing else in the machine allocated.
///
/// A root region's pages come from the frame allocator and go back to it, so their used bits are
/// the property itself, and nothing outside the region moves them. A frame a reclaim never
/// returned stays used, so a leak fails every run. `memory_region::usage` is the instrument for a
/// child region. See notes/kernel-test-harness.md.
///
/// Name: provisional (this lane; calef names the interfaces).
pub struct RegionRun {
    base: u64,
    pages: u64,
}

impl RegionRun {
    /// Name the run `region` spans, and prove the naming is not vacuous.
    ///
    /// The bounds have to be taken while the region is alive, because `region_bounds` goes stale
    /// with the name. The `Some(true)` guard is what keeps the later "these frames came back"
    /// check honest: it says these addresses are frames this allocator actually owns, so a wrong
    /// base fails loudly here rather than reading as "not used" after the reclaim.
    ///
    /// # Panics
    /// If `region` is not a live region, or if any frame in its run is not marked used.
    pub fn of(region: u64) -> RegionRun {
        let (base, size) =
            crate::memory_region::region_bounds(region).expect("no bounds for a live region");
        let run = RegionRun {
            base,
            pages: size / page_frames::FRAME_SIZE,
        };
        run.assert_all(
            true,
            "a live region's pages should be marked used by the allocator",
        );
        run
    }

    /// Every frame in the run is back with the allocator.
    ///
    /// # Panics
    /// If any frame in the run is still marked used.
    pub fn assert_returned(&self, what: &str) {
        self.assert_all(false, what);
    }

    /// Every frame in the run is still marked used: nothing gave them back.
    ///
    /// # Panics
    /// If any frame in the run has been freed.
    pub fn assert_held(&self, what: &str) {
        self.assert_all(true, what);
    }

    fn assert_all(&self, used: bool, what: &str) {
        for i in 0..self.pages {
            let frame = page_frames::PageFrame::from_addr(self.base + i * page_frames::FRAME_SIZE);
            assert_eq!(
                crate::memory::is_page_frame_used(frame),
                Some(used),
                "{what}: frame {:#x} (page {i} of the {}-page run at {:#x})",
                frame.addr(),
                self.pages,
                self.base,
            );
        }
    }
}

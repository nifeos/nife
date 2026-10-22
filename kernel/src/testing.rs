//! The QEMU test harness.
//!
//! `cargo test` builds the kernel with `cfg(test)`, the runner in
//! `.cargo/config.toml` boots it under QEMU, and we report pass/fail by asking QEMU
//! to exit with a status code via semihosting. Cargo reads that status and calls it
//! a pass or a failure.
//!
//! Set up on day one on purpose. The alternative is debugging by `println!` for a
//! year (DECISIONS §7).

use core::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, AtomicU64, AtomicUsize, Ordering};

// The per-test budget table lives in `crates/test_times` (`DEFAULT_BUDGET_SECS`, `SLOW_TESTS`),
// because `xtask`'s host watchdog reads the same numbers (rule 7).
use test_times::budget_secs_for;

use crate::arch::semihosting;
use crate::{print, println};

// The hang watchdog: **two independent mechanisms, because there are two ways a test never
// finishes.** Both run off the timer IRQ, so they cost a couple of atomic loads per tick and cannot
// perturb the scheduling they watch. Either one firing fails the run with a thread dump.
//
// **1. The no-progress heartbeat (a lost wakeup).** [`note_progress`] bumps a counter on every
// observable kernel step: a completed IPC rendezvous or device-IRQ wake (`sched::wake` /
// `wake_load_aware`) and every line of console output (`console::_print`, which covers each test's
// "ok"). Progress is also credited when any online core is running a real, non-idle thread
// ([`any_cpu_running_real_work`]), so a CPU-bound userspace poll with no wake and no output counts.
// If none of that happens for ~60 s the run fails: every thread `Blocked`, every core idle.
//
// **2. The per-test wall-clock ceiling (a livelock).** The heartbeat above cannot see a livelock that
// *makes progress* (a loop spinning while still doing IPC resets it on every rendezvous).
// [`Testable::run`] stamps a start time and a budget per test, and this fails the run when the
// budget is exceeded **even though progress is being made**.
//
// What each can and cannot see, stated plainly:
//
//   - The heartbeat catches a total stall (deadlock, lost wakeup) fast, and catches it wherever it
//     happens, including before any test starts. It is blind to any loop that keeps doing IPC.
//   - The ceiling catches any test that does not terminate, livelock included, regardless of how busy
//     it looks. It cannot fire faster than the budget, so it is a backstop, not a diagnostic.
//   - Neither can tell a livelock from slow-but-correct work *while it is running*; only the budget,
//     which is a human declaration of expected cost, separates them. That is the honest limit.
//   - **The heartbeat credits work by ANY thread, including leftovers from earlier tests**, so a
//     test whose server died can look busy; the kernel cannot attribute a thread to a test. The
//     defense is the ceiling, plus reading the thread dump's **address-space roots** rather than
//     its program counters, which tells a leftover spinner from the process under test.
//   - **A ceiling failure reports the ceiling, not the cost.** "ran 900 s" against a 900 s budget
//     says nothing about how long the work would take, and raising a budget returns only the new
//     budget.
//   - Both run off the boot core's timer IRQ, so a kernel that loses it loses both. The backstop
//     for that is xtask's host watchdog (`time_record::HOST_GRACE_SECS`, notes/scheduler.md), which
//     kills QEMU at the test's budget plus a grace. This ceiling still fires first; its dump says more.
/// **Which tests this binary runs** (milestone 210): a substring of the full test path, or empty
/// for the whole suite.
///
/// Baked in by `kernel/build.rs` from `NIFE_TEST_FILTER`, which `cargo xtask test --test <s>` sets.
/// It is a compile-time constant rather than a boot argument because that is the only channel
/// identical on all three architectures: two of them boot with a device tree this kernel does not
/// parse, and the third boots through PVH with no device tree at all. The cost is a kernel relink
/// per filter change (about 2.3 s), against the ~53 s of aarch64 suite it replaces.
///
/// # BUGS
///
/// A filtered run **is not the suite**, and nothing here says so twice. Tests are not independent:
/// the frame ledger's kept-frames ceiling, the thread peak and the stack high-water are all
/// whole-suite numbers, and a one-test run's readings are far under them, so they cannot fail here
/// and a filtered run proves nothing about them. Order coupling runs the other way too: a test that
/// only passes because an earlier one wired a service will fail alone, which is a true finding
/// about the test rather than about the code, and is worth reading as one.
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

/// **How many times the runner repeats the filtered tests in one boot**, baked in by
/// `kernel/build.rs` from `NIFE_TEST_REPEAT` (name provisional), which `cargo xtask test --repeat`
/// and `cargo xtask uefi-test --repeat` set. Empty means once, and then nothing about the run's
/// output changes.
///
/// It exists for a failure that shows once in many boots on a bench where each boot costs a
/// person a trip with a USB stick (milestone 321 (a corpse that did not park, on four real
/// cores)). A frequency is the finding there, and two hundred runs in one boot measure it where
/// two hundred boots would not be attempted.
///
/// Parsed at compile time, so a value that is not a whole number of at least 1 is a build error
/// rather than a run that quietly repeats once.
///
/// # A failure stops the run
///
/// The first failing iteration ends the boot exactly as a single run's failure does: the panic
/// handler or the watchdog prints its message and its dump, and [`note_repeat_failure`] adds which
/// iteration it was and how many passed before it. Continuing and counting would need the kernel
/// to survive a failed invariant and a test's half-built state, which is what the panic handler
/// exists to refuse, and a second failure read after the first could be the first one's wreckage.
/// So a frequency comes from the iteration the first failure lands on, and a run of N that passes
/// says "no failure in N".
///
/// # BUGS
///
/// - **A repeat is refused without a filter.** The whole suite keeps services it built for the
///   boot (the frame ledger's `SUITE_PAGE_FRAME_BUDGET` prices them), so repeating it measures that
///   residue rather than the tests.
/// - **The leak check sees frames and the runner's own capability slots, not every table.** A
///   test that leaks a rendezvous or a thread table entry per run fails in whichever iteration
///   runs out, and its message will read as the test's own. The per-iteration line prints the
///   live thread count so the trend is at least on the page.
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

/// **A test's fixture is not there, on this machine, and that is not this test's fault.**
///
/// Milestone 145: the on-board test-suite exit (milestone 16a) ran the `#[test_case]` suite on
/// the VisionFive 2 for the first time, and found roughly thirty tests that correctly expect a
/// synthetic device only `xtask`'s QEMU runners attach (virtio-rng, virtio-gpu, an NVMe
/// controller, ...). None of them was wrong; none of them had ever needed to run anywhere else.
/// `Testable::run` had no way to record a third outcome besides pass (return) and fail (panic),
/// so a test with no fixture had exactly one honest option: crash the whole suite, which is what
/// `non_volatile_memory_express.rs`'s end-to-end test did.
///
/// The boot tour already has the shape this borrows: `main.rs` prints "skipped (no 'outlaw'
/// program in the initial ramdisk)" instead of asserting a fixture that may not be there. `skip!()`
/// is the same move inside a `#[test_case]`: call it where the old code called `.expect(...)`, and
/// it prints the same message a passing test would have, tagged `skipped` instead of `ok`, then
/// returns from the calling function. The macro (not a function) is what makes the early return
/// reach the test: a function can only return `()` and hand back control, not unwind its caller.
pub static SKIP_REASON: AtomicPtr<u8> = AtomicPtr::new(core::ptr::null_mut());
pub static SKIP_REASON_LEN: AtomicUsize = AtomicUsize::new(0);
/// **A test printed the word "skip" while it was running**, set by the console writer and read
/// once the test returns. See [`note_printed`] and the check in `Testable::run`.
#[cfg(any(test, feature = "system_tests"))]
static PRINTED_A_SKIP_WORD: AtomicBool = AtomicBool::new(false);

/// **The mechanism that keeps [`skip!`] from being optional**, from milestone 214
/// (design/roadmap/0214-print-and-return-skips.md), on a test that prints "skipping" and returns
/// being counted as passed.
///
/// `skip!()` existed and was one macro away, and eighty-odd `#[test_case]`s reached for
/// `println!("... skipping)"); return;` instead. That is indistinguishable from a pass to
/// everything that reads the counts, which is the only thing anyone reads: milestone 164 watched
/// `x86_64` go from "200 passed, 55 skipped" to "211 passed, 44 skipped" without a single test
/// running that had not run before.
///
/// So the console tells the harness what a test said. Every fragment written while a test is
/// running is checked for `skip`, and a test that says the word and then returns **without** a
/// skip reason fails the run. It is not a lint over the source: it fires on what was actually
/// printed, at the moment the machine printed it, which is why it needs no allow-list and cannot
/// be defeated by a format string the source does not show whole.
///
/// **What it costs and what it can get wrong.** One `str::contains` on each console fragment in
/// test builds only. It fires on a test that prints "skip" for some other reason and then passes,
/// which is a false positive; the whole tree today has none (measured, all three architectures),
/// and a test that genuinely wants the word can print it in a message it also asserts on, or fail
/// loudly rather than returning. The other direction is silent: a test that returns early having
/// proved nothing and printed nothing is invisible to this, which is why the sweep also went
/// looking for early returns with no line at all.
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
// Exported (milestone 609 (the system tests leave the kernel crate)) because most of its callers now
// live in `system_tests/`, and its statics are named at the crate root for the same reason: from
// another crate, `$crate::testing` is a private module.
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

// ---------------------------------------------------------------------------------------------
// The frame ledger.
//
// **A boot has one pool of physical frames and no test gives an account of what it took.** That is
// how the aarch64 suite spent its way to `Unmappable(OutOfPageFrames)` in whatever test happened to
// spawn last, one run in three, three milestones in a row blaming the wrong code (notes/frames.md).
// The instrument that settled it in milestone 107 was four lines in `memory_region::create`, thrown away
// after one run; this is the same idea kept, so the next person reads a number instead of building
// one.
//
// Two numbers per test, because they answer different questions and only the second fails a boot:
// how many frames the test never gave back, and the longest run still allocatable afterwards. A
// suite can hold a comfortable free total and refuse a 128-page request; 137 free with no run of
// 128 is the measured case.
//
// Costs one bitmap scan per test (O(total), ~32k frames), between tests, off every hot path.
// ---------------------------------------------------------------------------------------------

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
/// Not zero, and the difference is accounted rather than shrugged at. A boot keeps some of what
/// its tests built on purpose: the shared services, `root_supervisor`'s unreclaimable budget, a
/// page per kernel endpoint, a current-CPU page per live address space, the logins and credential
/// stores, the file servers' windows and heaps, the terminals and compositors tests leave running.
/// [notes/frame-budget.md](../../notes/frame-budget.md) prices each one and holds the history of
/// the number; [notes/frames.md](../../notes/frames.md) lists the objects.
///
/// What must not *grow* is the per-test residue, and this ceiling sits just above the measured
/// total so that a new service-shaped test which forgets to hand its memory back fails **here**,
/// naming itself, instead of three tests later as `OutOfPageFrames` in something innocent.
///
/// The conventions every change follows, each with its story in the note: measure the merged
/// tree's real run, never sum two branches; the tighter architecture carries the headroom; the
/// +32 covers a measured two-frame local-against-CI divergence; and raising or lowering the
/// number is a decision, so read the `[that test kept N frames]` lines and be able to say why.
///
/// Current: `24_322`, the +32 over the 24,290 the riscv64 suite keeps (aarch64 24,267; CI run
/// 39450063569, 2026-10-20 UTC, with milestone 142 (a text display good enough that people use it
/// instead of a GUI)'s four faces). Part of that residue is program image size rather than any
/// test's memory. Each of `c_seam_tests`' three confiner spawns keeps a space sized by
/// `c_confiner`'s loadable segments, 15 pages in a debug build. Spaces that map the whole initial
/// ramdisk keep revocation-log pages sized by it, so a larger archive moves this number too. A
/// change that grows a program can therefore move it by a few frames with no leak behind it; the
/// `[that test kept N frames]` lines and the programs' segment pages tell the two apart.
///
/// # BUGS
///
/// - **Two test sites still keep their endpoints for the boot, deliberately.** `pipeline_service`
///   (its `spawn_service_stub` is a kernel thread that loops forever) and `user::tests`'
///   `a_user_client_moves_data_through_shared_memory` (a client that spins forever): reclaiming
///   either would leave a thread spinning on a stale endpoint. Each takes its pages from a region
///   of its own, so the cost is charged to that test and not to a later one.
/// - **The kernel's own `user::*_service` wiring still uses the never-freed pool**, and much of it
///   is called once per test rather than once per boot (`fs_service`, `ntp_service`,
///   `display_service`, among others), so those tests still move this ledger in +32 steps.
///   `create_rendezvous` being `pub(crate)` does not reach them, because they are in this crate.
///   Milestone 670 (test helpers give their clients a region to retype from) and milestone 671
///   (tests retype their rendezvous from their own region) are that work.
const SUITE_PAGE_FRAME_BUDGET: usize = 24_322;

/// **The longest run of free frames the boot must still have at the end**, in frames.
///
/// The other half of the gate, and the half that names the actual failure. Loading any program calls
/// `AddressSpace::new`, which calls `memory_region::create`, which calls `alloc_contiguous`: what a boot
/// runs out of is not memory, it is a **contiguous run**, and the two can be far apart. Milestone
/// 107 measured 137 frames free with no run of 128; the boot this gate was written for ended with
/// 216 free and no run longer than **117**, so any test loading a program bigger than that failed as
/// `Unmappable(OutOfPageFrames)`, in whichever test happened to be next rather than in the one that
/// spent the memory. [`SUITE_PAGE_FRAME_BUDGET`] alone would not have caught that, because a suite can
/// pass a residue ceiling and still be fragmented into uselessness.
///
/// 1024 frames is 4 MiB, comfortably more than the largest program this boot loads (`redoxfs_server` and
/// `net_stack` are the big ones, a few hundred pages with their page tables) and comfortably under
/// the 14080 measured after reclamation. It is a floor with room, not a target.
const SUITE_MIN_FREE_RUN: usize = 1024;

/// **Charge the test that just finished, and open an account for the one about to start.**
///
/// Called at the top of every test, and once more from the ledger, so the readings **partition the
/// run**: what a test is charged is the drop between its own reading and the next one, and every
/// frame lost between the first test and the last is charged to exactly one test. That matters more
/// than it sounds. Reading free frames before and after the test body instead attributes only what
/// the test spent *while it was running*, and a test that spawns a service and returns as soon as it
/// has its report leaves the service still mapping its heap: on the first measured aarch64 boot that
/// under-attribution was **17362 of the 29091 frames**, a clear majority landing nowhere.
///
/// The cost is a one-test lag in the transcript, which is why the line is printed after the previous
/// test's `ok` rather than on it.
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
/// The same posture as that ledger and deliberately one rung weaker: it reports and does not gate.
/// A frame budget can be gated because a suite that keeps more frames than it accounts for has a
/// leak, and the number the gate compares against is the tree's own claim about what is permanent.
/// A thread peak is not that. What fills this table is the services earlier tests started and
/// meant to keep (`notes/frames.md`'s "held" list), so a peak that climbs is usually a boot doing
/// more rather than a boot doing something wrong, and a gate here would fire on every milestone
/// that adds a service. `sched::thread_leak_police` is the check that does catch the wrong kind of
/// growth, and it is a different question (runnable spinners, not occupancy).
///
/// What this buys instead is that nobody has to instrument the kernel to learn the number again.
/// It was invisible until a spawn was refused, and then the investigation cost two full runs. See
/// `sched::MAX_THREADS`, whose doc comment reads these lines the way the frame budget's reads the
/// `[that test kept N frames]` ones.
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

/// **How close the boot came to the region table's ceiling**, printed beside the thread peak.
///
/// [`report_thread_peak`]'s twin, one rung weaker than the frame ledger for its reason: what fills
/// this table is mostly the services earlier tests left running on purpose, one region for each
/// address space and more for each spawner's `SPLIT`, so a peak that climbs is usually a boot doing
/// more. It adds one thing the thread line does not have, **the test during which the peak was
/// set**, because that is the question the lane of milestone 152 (durable delegation) had to
/// answer with a temporary print: the table reached 252 of 256 in `timetable_tests`, and the cause
/// was a leak many tests earlier that no single test's behaviour showed. The name says where to start reading, not who is to
/// blame; the ledger at `memory_region::MAX_REGIONS` says who holds what.
///
/// Reports and does not gate, and the reason is recorded at `MAX_REGIONS` rather than here.
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

/// **The fewest frames that were free at any moment, and what the allocator refused**, printed
/// beside the region peak.
///
/// The frame ledger above reads the allocator at the first test and the last. Both its gates are
/// about that end state, so a run that dips to nothing in the middle and recovers passes them, and
/// the test that meets the dip fails with `Unmappable(OutOfPageFrames)` or hangs far from whoever
/// spent the memory. That is what the lane of milestone 198 (a package manager) hit: two more
/// login tests, and aarch64's `std_net` and riscv64's CPU matrix fell over. This line is the
/// reading that says how close the run came (`memory::FREE_LOW_WATER`) and, when it went over,
/// where it went over first.
///
/// Reports and does not gate, for [`report_thread_peak`]'s reason. On 2026-09-26 no allocation was
/// refused on any of the three architectures, so the refusal line normally does not print, and a
/// run where it does has met the ceiling somewhere; the named test is where to start reading.
/// `memory::FREE_LOW_WATER` carries the ledger.
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
    // The online set, not `0..count` (first-silicon sweep, 2026-08-14): with the VisionFive 2's
    // {1,2,3} online, the count-as-index scan read parked slot 0's statics and never looked at
    // cpu 3, so a suite whose only live work sat on cpu 3 would read as hung.
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

/// **A budget denominated in timer ticks delivered to this core, for a test that has to wait.**
///
/// The unit is the whole point (milestone 62). A `timer::now()` deadline keeps running whether the
/// guest executes an instruction or not, so on a contended host a wall-clock budget shrinks in the
/// only currency a test cares about, which is guest work. Delivered ticks move the other way: a
/// descheduled emulator misses deadlines, so **fewer** ticks arrive over the same stretch of wall
/// clock and the budget stretches under exactly the conditions that made the wall-clock one wrong.
/// This is the same move the drift twins made in `arch/*/timer.rs` (assert the law, not the rate),
/// spent on a wait instead of on a clock.
///
/// It is a budget rather than a wait loop because the two callers poll differently and both are
/// right to. `sched::tests::within_ticks` must not yield, since the thing it waits for is a
/// preemption. `smp::tests::a_migrated_kernel_thread_keeps_its_hart_pointer` must yield, because it
/// is draining workers that share its core, and it checks a *second* assertion on every turn, so it
/// cannot be written as a condition handed to somebody else's loop. What the two share is the
/// budget's arithmetic, and that is all this holds.
///
/// **A change of core re-anchors instead of subtracting.** `timer::ticks()` is per core (§11) and a
/// steal can move this thread between two reads (§28.3), so comparing the counters either side of a
/// migration compares two unrelated numbers. Re-anchoring is also the honest reading: the budget is
/// "how many preemption opportunities this core gave me", and after a migration the answer starts
/// again. The cost is that a thread migrating repeatedly could stretch the budget without bound,
/// which is why nothing here is the last line of defence: the harness's per-test wall-clock ceiling
/// is (see the module note, mechanism 2), and it fails the run whatever the ticks say.
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

        // The frame ledger, before this test's name is printed: the reading closes the *previous*
        // test's account (and prints its charge under its own `ok` line) and opens this one's. The
        // opening balance is taken at the first test rather than at boot, because what the kernel
        // spends coming up is not a test's doing and is not what this measures.
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

        // **The test said "skip" and then returned as a pass.** That is milestone 214's defect
        // and there is no honest reading of it: either the test skipped, in which case the branch
        // above should have run, or it did not, in which case it printed something misleading
        // about itself. Fail the run rather than let the final line carry it.
        #[cfg(any(test, feature = "system_tests"))]
        if PRINTED_A_SKIP_WORD.swap(false, Ordering::Relaxed) {
            panic!(
                "this test printed \"skip\" and then returned, so the run counts it as a PASS. \
                 A test whose fixture is absent calls testing::skip!(reason), which returns and \
                 puts it in the skipped column; a println! and a `return` are indistinguishable \
                 from proving the claim. See design/roadmap/0214-print-and-return-skips.md.",
            );
        }

        // Report what a test actually cost, if it cost anything worth knowing. The ceiling already
        // needs a start time, so this is free, and it closes a real gap: a SLOW_TESTS budget is a
        // human declaration of expected cost, and until now there was no way to learn the cost except
        // by setting a budget too low on purpose and reading the failure. That is a bad way to find
        // out, and it is exactly the position I was in when std_fs tripped the 90 s ceiling with no
        // number attached. Anything under the threshold stays silent on the `ok` line; every figure,
        // this one included, goes to the record block the runner prints after the suite (milestone
        // 807), which is where a reader asking "what did each test cost" looks.
        let ticks = crate::arch::timer::now().saturating_sub(start);
        record_milliseconds(ticks);
        let elapsed = ticks / crate::arch::timer::frequency();
        if elapsed >= SLOW_REPORT_SECS {
            print!("[{elapsed} s] ");
        }

        // A test that overflows the stack corrupts the kernel and then fails somewhere
        // else entirely, often in a *later* test, or by hanging with no output at all.
        // Checking here pins the blame on the test that actually did it.
        //
        // This is not hypothetical. It is how milestone 3 went. See notes/stack.md.
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

/// **The livelock probe** (feature `watchdog_probe`, EXPECTED TO FAIL the run). Proof that the
/// per-test ceiling catches what the no-progress heartbeat cannot: this loops forever while doing a
/// full IPC rendezvous every iteration, so [`note_progress`] fires constantly and the heartbeat sees
/// a perfectly healthy kernel. That is the shape of the RedoxFS repeat-write livelock (spinning in an
/// allocator commit while still serving blk IPC), which turned a loud 60 s watchdog trip into an
/// infinite silent hang at ~400% CPU.
///
/// Run it with:
///
/// ```text
/// helpers/qemu-bounded.sh 200 cargo test -p kernel \
///     --features watchdog_probe --target aarch64-unknown-none-softfloat
/// ```
///
/// Expected: the run FAILS after this test's 90 s default budget, naming the test and its runtime.
/// Without the ceiling it hangs until the outer bound kills it, which is the regression this guards.
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
    // **The filter** (milestone 210), baked in at compile time by `kernel/build.rs` from
    // `NIFE_TEST_FILTER`. Empty means the whole suite, which is the default and the only thing CI
    // and `script/test` ever produce.
    //
    // The match is a substring of the test's full path, the same shape `cargo test <name>` uses, so
    // `--test frames` selects a module's worth and
    // `--test kernel::memory::tests::frames_are_zeroed` selects exactly one. There is no `--exact`
    // here because there is nothing to disambiguate: a full path is already unique.
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

    // **A filter that selects nothing fails the run.** Reporting "ok. 0 passed" for a typo would be
    // a green result that proves nothing, which is exactly the manufactured fact the `skip!()`
    // accounting and the NIFE_DISK check elsewhere in this tree exist to refuse.
    if selected == 0 && FILTER_COUNTED_ACROSS_IMAGES {
        // Not a verdict on its own: the other image in this leg may carry the test, and
        // `cargo xtask test` adds the `running` lines up. The result line keeps the shape every
        // reader of these transcripts (the HVF leg, `script/falsifications`) already parses.
        println!("no test in this image matches the filter `{filter}`; the harness counts the leg");
        println!("test result: ok. 0 passed");
        semihosting::exit(semihosting::EXIT_SUCCESS)
    }
    if selected == 0 {
        println!("no test matches the filter `{filter}`");
        // The likeliest cause on a multi-leg run, named here because the reader is looking at one
        // leg's transcript and cannot see that another leg passed. `--test` selects tests, not
        // architectures (DECISIONS §19), so a filter naming an aarch64-only test runs the riscv64
        // and x86_64 legs too and lands here on both. Say `--arch` as well when that is what you
        // meant.
        println!("  (a test only this architecture lacks? `--test` runs every leg; add `--arch`)");
        semihosting::exit(semihosting::EXIT_FAILURE)
    }

    // **The time record's table must hold every test, or the run fails before it starts** (milestone
    // 807). A record with a hole in it would read as a test that cost nothing, and `xtask`'s gap
    // check would then fail the leg at the end of a whole suite for a reason this line knows now.
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
    // "passed" counts only the tests that actually ran to an "ok"; a skipped test (skip!(), no
    // fixture on this boot) is not a pass, and rolling it into either bucket silently would hide
    // exactly the fact milestone 145 exists to keep visible. A board run says "244 passed, 31
    // skipped" rather than a bare 275 that looks identical to QEMU's full count.
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

/// **A root region's run of frames, named while the region is alive.**
///
/// The instrument the frame-return tests in `sched.rs`, `user/child_thread_tests.rs` and
/// `user/force_kill_tests.rs` ask their question through, and it exists because the question they
/// used to ask was wider than the property. `memory::free_page_frames()` counts every free frame in
/// the machine, so bracketing it across a reclaim asserts that *nothing else in the kernel
/// allocated or freed* for the length of the window. No test in this suite has any business making
/// that claim: the windows contain child threads, remote-core reaps and a neighbouring test's late
/// teardown, and two of them were proved wrong on CI before this type existed (pull requests #1101
/// and #1120, notes/load-sensitive-assertions.md).
///
/// A root region's pages come from the frame allocator and go back to it
/// (`memory_region::destroy`), so every frame in the run is marked used from `create` until the
/// reclaim and free afterwards. Those bits are the property itself, and nothing outside the region
/// can move them. `memory_region::usage` is the sibling instrument, for a *child* region whose
/// pages return to a parent rather than to the allocator, and whose frames therefore stay used
/// either way.
///
/// **Leak sensitivity is sharper, not merely preserved.** A frame a reclaim never returned stays
/// marked used forever, so the defect fails on every run rather than on the runs where the
/// arithmetic happens to be visible, and it can no longer be masked by somebody else freeing the
/// same number of frames in the same window.
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

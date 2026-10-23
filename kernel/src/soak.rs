//! The workload that does not stop, and the thing that watches it.
//!
//! With `--features soak_test`, [`run`] replaces the boot tour's halt: it spawns user-mode workers
//! (`fixtures/src/soaker.rs`), prints a beat every [`BEAT_SECONDS`], and fails the run on a
//! refusal, a wrong reply or a stall. [`signal_waiters`] makes it cross cores, §138 (how a
//! saturated workload is made to hand threads across cores). See notes/soak.md.
//!
//! # BUGS
//! A green soak is never proof the concurrency is correct. notes/soak/limitations.md and
//! notes/soak/supervisor.md have the rest.

// aarch64 has no watchdog to arm yet; a build that quietly never armed would look like a board
// that never wedged. See notes/soak/supervisor.md.
#[cfg(all(feature = "watchdog_soak_test", target_arch = "aarch64"))]
compile_error!(
    "--features watchdog_soak_test arms the Intel TCO on x86_64 (kernel/src/arch/x86_64/tco.rs) \
     and the JH7110 watchdog on riscv64 (kernel/src/drivers/jh7110_watchdog.rs); aarch64 has none \
     yet. argon's Tegra WDT is a later step of design/roadmap/0593-a-wedged-kernel-resets-itself.md."
);

// Every x86_64 and riscv64 soak arms the watchdog unless it opts out (calef, 2026-09-26 UTC).
// This restates the rule `build.rs` implements, so an edit there that drops the default fails here.
#[cfg(all(
    feature = "soak_test",
    any(target_arch = "x86_64", target_arch = "riscv64"),
    not(feature = "unwatched_soak_test"),
    not(soak_watchdog)
))]
compile_error!(
    "an x86_64 or riscv64 soak_test build must arm the watchdog unless it names \
     unwatched_soak_test (calef, 2026-09-26 UTC); kernel/build.rs no longer sets the soak_watchdog \
     cfg for it"
);

// The opt-out and the insistence cannot both be meant. `wedge_soak_test` implies
// `watchdog_soak_test`, so a wedge with no watchdog to reset it is refused here too.
#[cfg(all(feature = "unwatched_soak_test", feature = "watchdog_soak_test"))]
compile_error!(
    "--features unwatched_soak_test and watchdog_soak_test (or wedge_soak_test) contradict each \
     other: one opts out of the watchdog and the other insists on it"
);

use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use paging::Flags;
use soak_page::MAX_WORKERS;

use crate::cap::{Rights, irq_cap, rendezvous_cap};
use crate::user::{self, Mapping, Spawn};
use crate::{arch, memory, println, sched, smp};

/// How often the supervisor speaks: three beats to `serial_console`'s default quiet window. See
/// notes/soak/supervisor.md.
const BEAT_SECONDS: u64 = 5;

/// The line that tells the watcher a soak has begun.
///
/// `crates/serial_console`'s recognizer matches this prefix, and its tests fail if the two differ.
/// A console marker takes the spelling of the command that produced it, `script/soak-test` (calef,
/// 2026-09-14 UTC).
const START_MARKER: &str = "soak-test: started";

/// Wire the pool and never come back.
///
/// The caller is the boot thread at the end of the tour, and this replaces its `arch::halt()`.
pub fn run() -> ! {
    let cpus = smp::online_count();
    let groups = topology_groups(cpus);
    let callers = CALLERS_PER_GROUP;

    // Through the trust root, because this boot has no progenitor to measure it; that also keeps
    // it in the image for `uefi_loader`'s seal and `script/card-check`.
    let Some(image) = crate::trust::require_program("soaker") else {
        println!("soak-test: FAILED: no 'soaker' program in the initial ramdisk; nothing to run");
        arch::halt(arch::HaltReason::measurement_boot());
    };

    // Zeroed, so a first sample cannot read stale RAM as progress that already happened.
    let Some(frame) = memory::alloc_zeroed() else {
        println!("soak-test: FAILED: no frame for the shared progress page");
        arch::halt(arch::HaltReason::measurement_boot());
    };
    let shared = frame.addr();

    // One tick rendezvous per group: a shared one starves all but one waiter on a loaded host.
    let mut tick_endpoints = [0u64; MAX_GROUPS];
    for slot in tick_endpoints.iter_mut().take(groups) {
        *slot = sched::create_rendezvous();
    }
    // Routed before any waiter exists, so none meets an unbound route; signaling starts last.
    if !bind_tick_routes(&tick_endpoints[..groups]) {
        println!(
            "soak-test: FAILED: one of interrupts {}..={TICK_INTID_TOP} is already routed, so a tick \
             route would steal it; see TICK_INTID_TOP in kernel/src/soak.rs",
            tick_intid(groups - 1)
        );
        arch::halt(arch::HaltReason::measurement_boot());
    }

    let (tids, placed) = spawn_workers(image, shared, groups);

    // After every worker exists, so the first beat does not measure a setup backlog.
    arm_tick_hook(groups);

    let workers = groups * MEMBERS_PER_GROUP;
    print_start_banner(groups, callers, workers, cpus);

    #[cfg(feature = "reboot_soak_test")]
    arm_reboot();

    print_spawn_census(groups, &placed[..workers]);

    watch(shared, workers, &tids, &placed)
}

/// Spawn every group's workers, responder first.
///
/// Returns the thread each worker got and the core it was placed on (`u8::MAX` if unknown), kept
/// for the census. A waiter is granted only its `Irq` capability, a responder read and a caller
/// write on the group's rendezvous. Halts the boot if a spawn fails.
fn spawn_workers(
    image: &'static [u8],
    shared: u64,
    groups: usize,
) -> ([u64; MAX_WORKERS], [u8; MAX_WORKERS]) {
    let mut tids = [0u64; MAX_WORKERS];
    let mut placed = [u8::MAX; MAX_WORKERS];

    for group in 0..groups {
        let endpoint = sched::create_rendezvous();
        for member in 0..MEMBERS_PER_GROUP {
            let role = role_of(member);
            let index = worker_index(group, member);
            let grant = if role == ROLE_WAITER {
                irq_cap(tick_intid(group))
            } else if role == ROLE_RESPONDER {
                rendezvous_cap(endpoint, Rights::READ)
            } else {
                rendezvous_cap(endpoint, Rights::WRITE)
            };
            let started = sched::spawn_reporting_placement(move || {
                user::run(
                    image,
                    Spawn {
                        arg0: role,
                        arg1: index as u64,
                        // Distinct per worker, or that pair locks into one interleaving.
                        arg2: (index as u64)
                            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                            .wrapping_add(1),
                        grants: &[grant],
                        maps: &[Mapping {
                            va: soak_page::VA,
                            phys: shared,
                            flags: Flags::user_data(),
                        }],
                    },
                )
            });
            let Some((tid, cpu)) = started else {
                println!(
                    "soak-test: FAILED: could not spawn worker {index} of {}",
                    groups * MEMBERS_PER_GROUP
                );
                arch::halt(arch::HaltReason::measurement_boot());
            };
            tids[index] = tid;
            // Saturating: a cpu id past `u8` reads as the census's "no answer", not a wrong core.
            placed[index] = u8::try_from(cpu).unwrap_or(u8::MAX);
        }
    }
    (tids, placed)
}

/// The start marker and the lines that say how to read the beat that follows.
fn print_start_banner(groups: usize, callers: usize, workers: usize, cpus: usize) {
    println!();
    println!(
        "{START_MARKER} {groups} groups of one responder, {callers} callers, \
         {GRINDERS_PER_GROUP} grinder and {WAITERS_PER_GROUP} tick waiter ({workers} user \
         threads) on {cpus} online core(s), beating every {BEAT_SECONDS}s"
    );
    println!(
        "soak-test: silence longer than a few beats is a hang, not a slow run; the beat is on the wall \
         clock and does not depend on the workload making progress"
    );
    println!(
        "soak-test: a clean run is a number to compare against, NOT evidence that the concurrency is \
         correct (design/roadmap/0219-a-workload-that-does-not-stop.md)"
    );
    println!(
        "soak-test: the threads that cross cores are the tick waiters, NOT the rendezvous pairs; a \
         rising crossings count is the wake protocol sustained across cores under load, not the \
         IPC workload migrating (design/roadmap/0221-a-soak-that-crosses-cores.md)"
    );
    println!(
        "soak-test: rounds counts IPC round trips and wakes counts tick-route wakes; they are separate \
         because they are separate quantities, and neither rate compares with a soak built \
         differently"
    );
}

/// The spawn-time census, and what a reader should and should not take from it.
fn print_spawn_census(groups: usize, placed: &[u8]) {
    print_census(
        "where the kernel placed each worker at spawn",
        groups,
        placed,
    );
    println!(
        "{CENSUS_MARKER} this is the boot-time lottery and NOT where the run settles: a rendezvous \
         wake queues the peer on the waker's own core (DECISIONS 138, how a saturated workload is \
         made to hand threads across cores), so each group converges onto one core within a few \
         exchanges"
    );
    println!(
        "{CENSUS_MARKER} the beat line's drifted= is how many responders, callers and grinders are \
         no longer on the core the census above put them on; while it reads 0 that census is \
         current, and a nonzero one is followed by the census that replaces it"
    );
    println!(
        "{CENSUS_MARKER} tick waiters are excluded from drifted= because their movement is the \
         point rather than a surprise, and it is already crossings= \
         (design/roadmap/0221-a-soak-that-crosses-cores.md)"
    );
}

/// The prefix every census line carries.
///
/// Not `soak-test:`, which `crates/serial_console`'s recognizer reads. See
/// notes/soak/placement-census.md.
///
/// Name: provisional (milestone 240 (the soak reports what happened and not where)): calef names
/// what a reader meets. Respelled from `soak-census:` by milestone 297 (`soak` becomes
/// `soak-test`), tracking the command.
const CENSUS_MARKER: &str = "soak-test-census:";

/// Which role the `member`-th thread of a group plays.
///
/// The spawn loop and the census both ask this, so they cannot disagree.
fn role_of(member: usize) -> u64 {
    if member == 0 {
        ROLE_RESPONDER
    } else if member <= CALLERS_PER_GROUP {
        ROLE_CALLER
    } else if member <= CALLERS_PER_GROUP + GRINDERS_PER_GROUP {
        ROLE_GRINDER
    } else {
        ROLE_WAITER
    }
}

/// The one letter a census spends on a role. See [`print_census`].
fn role_letter(role: u64) -> char {
    match role {
        ROLE_RESPONDER => 'R',
        ROLE_CALLER => 'C',
        ROLE_GRINDER => 'G',
        _ => 'W',
    }
}

/// Print which roles are on which core, one line per online core, empty ones included.
///
/// `what` heads the block, `cpus[i]` is where worker `i` is (`u8::MAX` for not yet known), and
/// `groups` is printed beside any shortfall. Each token is `<letter><group>`. It draws no
/// conclusion from the placement. See notes/soak/placement-census.md.
fn print_census(what: &str, groups: usize, cpus: &[u8]) {
    println!(
        "{CENSUS_MARKER} {what}: R=responder, C=caller, G=grinder, W=tick waiter, and the number \
         after each letter is its group"
    );
    let mut accounted = 0usize;
    for cpu in smp::online_cpus() {
        let here = u8::try_from(cpu).unwrap_or(u8::MAX);
        let n = cpus.iter().filter(|&&c| c == here).count();
        accounted += n;
        crate::print!("{CENSUS_MARKER} core={cpu} threads={n}");
        for (i, &c) in cpus.iter().enumerate() {
            if c == here {
                crate::print!(
                    " {}{}",
                    role_letter(role_of(i % MEMBERS_PER_GROUP)),
                    i / MEMBERS_PER_GROUP
                );
            }
        }
        println!();
    }
    // A worker not yet run, or on a core outside the online set (a placement bug).
    let missing = cpus.len() - accounted;
    if missing != 0 {
        let unknown = cpus.iter().filter(|&&c| c == u8::MAX).count();
        println!(
            "{CENSUS_MARKER} unplaced={missing} of which not-yet-run={unknown} (groups={groups}, \
             online={})",
            smp::online_count()
        );
    }
}

/// `arg0` for the half that answers, matching `fixtures/src/soaker.rs`.
const ROLE_RESPONDER: u64 = 0;
/// `arg0` for the half that calls.
const ROLE_CALLER: u64 = 1;
/// `arg0` for the half that only computes.
const ROLE_GRINDER: u64 = 2;
/// `arg0` for the role that blocks on the tick route (milestone 221).
const ROLE_WAITER: u64 = 3;

/// How many callers share one responder.
const CALLERS_PER_GROUP: usize = 3;

/// How many pure-compute threads share a group.
const GRINDERS_PER_GROUP: usize = 1;

/// How many tick waiters share a group: one per core, since [`topology_groups`] builds a group per
/// core. See notes/soak/supervisor.md.
const WAITERS_PER_GROUP: usize = 1;

/// How many worker threads one group has.
///
/// One responder, its callers, its grinders and its waiter: the divisor in [`worker_index`], the
/// multiplier in the spawn loop and the bound [`topology_groups`] fits under.
const MEMBERS_PER_GROUP: usize = 1 + CALLERS_PER_GROUP + GRINDERS_PER_GROUP + WAITERS_PER_GROUP;

/// The highest interrupt number the tick routes are bound to; group `g` uses `TICK_INTID_TOP - g`.
///
/// They need only be `sched::irq_route` slots nothing else in a soak boot claims, and
/// [`bind_tick_routes`] refuses to start a soak whose routes would steal one. Why no architecture
/// delivers them is in notes/soak/tick-route.md.
///
/// Name: provisional (milestone 221 (the soak never crosses cores)): calef names public items.
const TICK_INTID_TOP: u32 = 255;

/// The prefix every line about the reboot loop carries, so one grep of a fifty-boot log says why
/// the series stopped. Not `soak-test:`, for [`CENSUS_MARKER`]'s reason.
///
/// Name: provisional (milestone 249): calef names public items. Respelled from `soak-reboot:` by
/// milestone 297, tracking the command.
#[cfg(feature = "reboot_soak_test")]
const REBOOT_MARKER: &str = "soak-test-reboot:";

/// How this architecture asks for a cold reboot, in the banner's words; `arch::reboot` prints the
/// detail of each attempt.
#[cfg(all(feature = "reboot_soak_test", target_arch = "riscv64"))]
const REBOOT_ROUTE: &str = "SBI SRST system_reset, reset type 1";
#[cfg(all(feature = "reboot_soak_test", target_arch = "aarch64"))]
const REBOOT_ROUTE: &str = "PSCI SYSTEM_RESET, on the conduit /psci names";
#[cfg(all(feature = "reboot_soak_test", target_arch = "x86_64"))]
const REBOOT_ROUTE: &str =
    "the FADT reset register, then port 0xcf9 full reset, then the 8042 reset pulse";

/// How long one boot soaks before it draws the placement lottery again.
///
/// Long enough for the placement to converge, short enough for fifty draws in an evening. See
/// notes/soak/supervisor.md.
#[cfg(feature = "reboot_soak_test")]
const REBOOT_AFTER_SECONDS: u64 = 120;

/// The last chance to stop the loop, after the window and before the reset: one beat's time.
#[cfg(feature = "reboot_soak_test")]
const REBOOT_GRACE_SECONDS: u64 = 5;

/// Arm the reboot loop: drain the console, then announce the hazard and how to stop it.
///
/// Called once, from [`run`], after the workers exist and before the first census. The drain drops
/// a byte U-Boot's countdown left, which would fire the escape on the first boot. A kernel that
/// will reboot its machine says so, with the escape, on the only channel it has. See
/// notes/soak/rebooting-soak.md.
#[cfg(feature = "reboot_soak_test")]
fn arm_reboot() {
    crate::console::discard_rx();
    // What ended the previous boot, from the PMIC's side, which the console cannot show.
    #[cfg(target_arch = "riscv64")]
    crate::reboot::pmic_report(REBOOT_MARKER);
    println!(
        "{REBOOT_MARKER} THIS BUILD REBOOTS THE BOARD. It soaks for {REBOOT_AFTER_SECONDS}s, then \
         asks for a cold reboot ({REBOOT_ROUTE}) and draws the thread-placement lottery again, \
         forever."
    );
    println!(
        "{REBOOT_MARKER} to stop the loop: press any key on this console. The check is a poll of \
         the UART's data-ready bit once every {BEAT_SECONDS}s and again {REBOOT_GRACE_SECONDS}s \
         before each reset, and the bit is sticky, so a keypress at any moment is found. Stopping \
         disarms the reboot and leaves the soak running; it does not end the run."
    );
    println!(
        "{REBOOT_MARKER} the fallback that needs no cooperation from this kernel is the firmware's \
         own autoboot countdown or boot menu on the next boot, and after that, the boot medium."
    );
}

/// The window has run out: announce, offer the grace period, and reset.
///
/// Returns only if somebody typed or every reset route was refused; the caller disarms on both.
/// Called after the beat's verdict, never before, so a failed run never resets over its evidence.
/// See notes/soak/supervisor.md.
#[cfg(feature = "reboot_soak_test")]
fn draw_again(elapsed: u64) {
    println!(
        "{REBOOT_MARKER} window reached at t={elapsed}s. Cold-rebooting in \
         {REBOOT_GRACE_SECONDS}s to draw the placement lottery again; press any key on this \
         console to stop the loop."
    );

    // Polled through the whole window, yielding the core between checks.
    let hz = arch::timer::frequency();
    let deadline = arch::timer::now().wrapping_add(hz.wrapping_mul(REBOOT_GRACE_SECONDS));
    while arch::timer::now().wrapping_sub(deadline) > u64::MAX / 2 {
        if crate::console::is_byte_waiting() {
            println!(
                "{REBOOT_MARKER} DISARMED in the grace window: a byte arrived on this console. \
                 This board will not reboot itself again. The soak keeps running and keeps \
                 beating; power it off when you are done with it."
            );
            return;
        }
        sched::yield_now();
    }

    // Drained before the call: a reset stops the UART, and later bytes may never reach the wire.
    println!("{REBOOT_MARKER} rebooting now ({REBOOT_ROUTE}).");
    crate::console::drain();
    let refused = crate::reboot::cold_reset(REBOOT_MARKER);
    println!(
        "{REBOOT_MARKER} FAILED ({refused:?}): every reset route was refused (the lines above say \
         how), so an unattended series is not available on this machine by this route. The soak \
         keeps running; nothing has been damaged and no further reset is attempted."
    );
}

/// The prefix every line about the watchdog carries, which `script/soak-test --watchdog` and
/// `--wedge` read. Name provisional.
#[cfg(soak_watchdog)]
const WATCHDOG_MARKER: &str = "soak-test-watchdog:";

/// Seconds from the last pet to the reset: twelve beats, counted by both watchdogs as two halves.
/// See notes/soak/supervisor.md.
#[cfg(soak_watchdog)]
const WATCHDOG_RESET_AFTER_SECONDS: u64 = 60;

/// Print a pet once every this many beats, and at the first.
#[cfg(soak_watchdog)]
const WATCHDOG_REPORT_EVERY_BEATS: u64 = 6;

/// When the test-only wedge happens: after the log has shown the watchdog petted.
#[cfg(feature = "wedge_soak_test")]
const WEDGE_AFTER_SECONDS: u64 = 30;

/// Find the Intel TCO watchdog and arm it, announcing the hazard and its escape first.
///
/// Returns the armed driver, or `None` with the reason on the console; the soak runs either way.
#[cfg(all(soak_watchdog, target_arch = "x86_64"))]
fn arm_watchdog() -> Option<Watchdog> {
    discard_stale_bytes();
    let tco = match arch::tco::find() {
        Ok(tco) => tco,
        Err(why) => {
            println!(
                "{WATCHDOG_MARKER} NOT ARMED: {why}. The soak runs without a watchdog, and a wedge \
                 on this machine waits for a person."
            );
            return None;
        }
    };
    println!(
        "{WATCHDOG_MARKER} found the Intel TCO watchdog: {:?} layout, TCO registers at I/O port \
         {:#x}",
        tco.generation(),
        tco.base()
    );
    match tco.arm(WATCHDOG_RESET_AFTER_SECONDS) {
        Ok(armed) => {
            if armed.previous_boot_reset_by_watchdog {
                println!(
                    "{WATCHDOG_MARKER} the previous boot was ended by this watchdog: \
                     SECOND_TO_STS was set when this boot found it, and is now cleared"
                );
            } else {
                println!(
                    "{WATCHDOG_MARKER} the previous boot was not ended by this watchdog \
                     (SECOND_TO_STS clear)"
                );
            }
            println!(
                "{WATCHDOG_MARKER} ARMED. THIS BUILD RESETS THE MACHINE IF THE SOAK STOPS BEATING: \
                 {} ticks of 0.6s counted twice, so a reset {}.{}s after the last pet, which comes \
                 every {BEAT_SECONDS}s from the beat.",
                armed.ticks,
                armed.reset_after_tenths / 10,
                armed.reset_after_tenths % 10,
            );
            print_watchdog_escape();
            Some(tco)
        }
        Err(why) => {
            println!("{WATCHDOG_MARKER} NOT ARMED: {why}.");
            None
        }
    }
}

/// Drop what U-Boot left in the receive buffer before the watchdog arms.
///
/// Only a soak that does not reboot drains here; a rebooting one drained at its banner, and a
/// second drain would discard `script/board-console --stop`'s escape.
#[cfg(soak_watchdog)]
fn discard_stale_bytes() {
    #[cfg(not(feature = "reboot_soak_test"))]
    crate::console::discard_rx();
}

/// The watchdog each architecture's soak arms.
#[cfg(all(soak_watchdog, target_arch = "x86_64"))]
type Watchdog = arch::tco::Tco;
#[cfg(all(soak_watchdog, target_arch = "riscv64"))]
type Watchdog = crate::drivers::jh7110_watchdog::Jh7110Watchdog;

/// What the reset waits for and how to stop it, the second half of every `ARMED` banner.
#[cfg(soak_watchdog)]
fn print_watchdog_escape() {
    println!(
        "{WATCHDOG_MARKER} a soak that FAILS prints its verdict and dump and then stops \
         petting, so the reset follows the evidence and never replaces it. To stop the \
         watchdog: press any key on this console. It is halted at the next beat and the soak \
         keeps running."
    );
}

/// Find the JH7110's watchdog, start its clocks, arm it and see it count.
///
/// Returns the armed driver, or `None` with the reason on the console. Refuses a tree that does not
/// name the watchdog and its clocks, and a counter whose measured rate disagrees with
/// `CORE_CLOCK_HZ`. See notes/soak/supervisor.md.
#[cfg(all(soak_watchdog, target_arch = "riscv64"))]
fn arm_watchdog() -> Option<Watchdog> {
    use jh7110_watchdog::CORE_CLOCK_HZ;
    discard_stale_bytes();
    let Some((found, sys)) = crate::memory::jh7110_watchdog() else {
        println!(
            "{WATCHDOG_MARKER} NOT ARMED: this machine's device tree names no JH7110 watchdog \
             (starfive,jh7110-wdt or starfive,dskit-wdt). The soak runs without a watchdog, and a \
             wedge on this machine waits for a person."
        );
        return None;
    };
    println!(
        "{WATCHDOG_MARKER} found the JH7110 watchdog: {} at {:#x}, its clocks and resets in the \
         SYS CRG at {:#x} ({})",
        core::str::from_utf8(found.compatible).unwrap_or("?"),
        found.base,
        sys.base,
        if sys.from_tree {
            "named by this machine's tree"
        } else {
            "NOT named by this tree: the constant both published trees agree on"
        },
    );
    if !start_watchdog_clocks(&found, &sys) {
        return None;
    }
    let load = jh7110_watchdog::load_for(WATCHDOG_RESET_AFTER_SECONDS, CORE_CLOCK_HZ)?;
    // SAFETY: `memory::init` recorded this window only for a tree that names the watchdog, and
    // `mmu::map_everything` mapped its first page device-typed in the direct map.
    // `start_watchdog_clocks` has just read both clocks running and both resets released.
    let mut dog = unsafe { Watchdog::new(arch::mmu::phys_to_virt(found.base) as usize) };
    dog.arm(load);
    let hz = measure_watchdog_rate(&dog);
    if !jh7110_watchdog::rate_agrees(hz, CORE_CLOCK_HZ) {
        dog.disarm();
        let halted = !dog.is_counting();
        println!(
            "{WATCHDOG_MARKER} NOT ARMED: the counter ran at {hz} Hz against this kernel's timer, \
             not the {CORE_CLOCK_HZ} Hz its count was computed for, so it would reset at the wrong \
             time or never. It is {}.",
            if halted {
                "halted"
            } else {
                "STILL COUNTING (the enable bit did not clear)"
            }
        );
        return None;
    }
    let tenths = jh7110_watchdog::reset_after_tenths(load, CORE_CLOCK_HZ);
    println!(
        "{WATCHDOG_MARKER} the counter runs at {hz} Hz, measured against this kernel's timer. The \
         JH7110 keeps no record this kernel knows of how its previous boot ended, so a watchdog \
         reset shows only as this kernel starting again."
    );
    println!(
        "{WATCHDOG_MARKER} ARMED. THIS BUILD RESETS THE MACHINE IF THE SOAK STOPS BEATING: \
         {load} ticks of the {CORE_CLOCK_HZ} Hz core clock counted twice, so a reset {}.{}s after \
         the last pet, which comes every {BEAT_SECONDS}s from the beat.",
        tenths / 10,
        tenths % 10,
    );
    print_watchdog_escape();
    Some(dog)
}

/// Walk the watchdog node's own clocks and resets, print what each word said, and answer whether
/// the block can now count.
#[cfg(all(soak_watchdog, target_arch = "riscv64"))]
fn start_watchdog_clocks(
    found: &jh7110_watchdog::Discovered,
    sys: &jh7110_clock_and_reset::Found,
) -> bool {
    use jh7110_clock_and_reset::Step;
    let plan = found.clocks_and_resets;
    if !plan
        .plan()
        .iter()
        .any(|s| matches!(s, Step::EnableClock(_)))
    {
        println!(
            "{WATCHDOG_MARKER} NOT ARMED: the watchdog's node names no SYS-domain clock this \
             kernel can turn on ({} specifier(s) skipped).",
            plan.skipped
        );
        return false;
    }
    // SAFETY: `memory::init` recorded this window only for a tree that names the watchdog, and
    // `mmu::map_everything` mapped it device-typed in the direct map. The plan's identifiers were
    // bounded by `SYS` when it was built, and `bring_up` bounds them again.
    let report = unsafe {
        crate::drivers::jh7110_clock_and_reset::bring_up(
            arch::mmu::phys_to_virt(sys.base) as usize,
            &jh7110_clock_and_reset::SYS,
            plan.plan(),
        )
    };
    let clocks = plan.plan().iter().filter_map(|s| match s {
        Step::EnableClock(i) => Some(*i),
        Step::DeassertReset(_) | Step::SelectParent { .. } => None,
    });
    for (n, index) in clocks.enumerate().take(report.clocks) {
        println!(
            "{WATCHDOG_MARKER} clock {index} {:#010x} -> {:#010x}",
            report.clock_before[n], report.clock_after[n]
        );
    }
    println!(
        "{WATCHDOG_MARKER} {} of {} reset(s) released, {} step(s) rejected, {} specifier(s) \
         skipped{}",
        report.resets_released,
        report.resets,
        report.rejected,
        plan.skipped,
        if report.was_already_up() {
            "; firmware had already started it"
        } else {
            ""
        },
    );
    let ready =
        report.rejected == 0 && report.has_clocks_running() && report.every_reset_released();
    if !ready {
        println!(
            "{WATCHDOG_MARKER} NOT ARMED: a clock did not read back running or a reset did not \
             release, so the watchdog's registers may not answer."
        );
    }
    ready
}

/// The watchdog counter's rate over a tenth of a second of this kernel's timer. Spun rather than
/// yielded, so the window is the timer's and not the scheduler's.
#[cfg(all(soak_watchdog, target_arch = "riscv64"))]
fn measure_watchdog_rate(dog: &Watchdog) -> u64 {
    let hz = arch::timer::frequency();
    let start = arch::timer::now();
    let before = dog.ticks_left();
    while arch::timer::now().wrapping_sub(start) < hz / 10 {
        core::hint::spin_loop();
    }
    let after = dog.ticks_left();
    let elapsed = arch::timer::now().wrapping_sub(start);
    jh7110_watchdog::measured_hz(before, after, elapsed, hz)
}

/// Stop beating, on purpose, for `--features wedge_soak_test`.
///
/// Masks interrupts and spins, so no tick preempts the supervisor and nothing pets the watchdog.
#[cfg(feature = "wedge_soak_test")]
fn wedge(elapsed: u64) -> ! {
    println!(
        "{WATCHDOG_MARKER} WEDGING NOW at t={elapsed}s, deliberately (--features wedge_soak_test): \
         this core masks interrupts and spins, the beat stops, and nothing pets the watchdog. The \
         next thing this console should show is this kernel booting again."
    );
    arch::interrupts::disable();
    loop {
        core::hint::spin_loop();
    }
}

/// The most groups the shared page has room for, and so the most tick routes there can be.
const MAX_GROUPS: usize = MAX_WORKERS / MEMBERS_PER_GROUP;

/// The interrupt number group `g`'s waiter holds. See [`TICK_INTID_TOP`].
const fn tick_intid(g: usize) -> u32 {
    TICK_INTID_TOP - g as u32
}

/// The rendezvous [`signal_waiters`] signals, one per group, each plus one; zero means empty.
///
/// Plain atomics, because the reader is an interrupt handler on every core. Written once each, by
/// the boot thread. See notes/soak/supervisor.md.
static TICK_ROUTES: [AtomicU64; MAX_GROUPS] = [const { AtomicU64::new(0) }; MAX_GROUPS];

/// How many of [`TICK_ROUTES`] are armed. Zero until [`arm_tick_hook`], which keeps the hook inert
/// during setup; [`signal_waiters`] loads it first.
static TICK_GROUPS: AtomicUsize = AtomicUsize::new(0);

/// Which route the next tick signals, anywhere on the machine. See [`signal_waiters`].
static TICK_CURSOR: AtomicUsize = AtomicUsize::new(0);

/// Signal one tick route, round-robin across the groups, through `sched::irq_notify`.
///
/// Inert until [`arm_tick_hook`] runs, after every worker exists. One route per group, because a
/// shared rendezvous starves all but one waiter. See notes/soak/tick-route.md.
///
/// Context: called from `sched::on_tick`, in interrupt context, on every core. Takes `IPC_TABLES`
/// through `irq_notify`, an `IrqSafeMutex` (§9 (locking: `IrqSafeMutex`, plus a discipline)), so
/// the interrupted code cannot hold it.
///
/// Name: provisional (milestone 221): calef names public items.
pub fn signal_waiters() {
    let groups = TICK_GROUPS.load(Ordering::Acquire);
    if groups == 0 {
        return;
    }
    // Relaxed: the routes are equivalent, so a skipped or repeated number under contention is fine.
    let which = TICK_CURSOR.fetch_add(1, Ordering::Relaxed) % groups;
    match TICK_ROUTES[which].load(Ordering::Relaxed) {
        0 => {}
        route => sched::irq_notify(route - 1),
    }
}

/// Bind one tick route per group to `endpoints`.
///
/// Returns `false`, binding nothing, if there are none, too many, or any interrupt number is
/// already routed. Called before the first waiter is spawned, so none meets `WrongObject` at
/// `Irq::WAIT`; [`arm_tick_hook`] starts the signaling later. See notes/soak/tick-route.md.
fn bind_tick_routes(endpoints: &[sched::RendezvousId]) -> bool {
    if endpoints.is_empty() || endpoints.len() > MAX_GROUPS {
        return false;
    }
    if (0..endpoints.len()).any(|g| sched::irq_route(tick_intid(g)).is_some()) {
        return false;
    }
    for (g, &ep) in endpoints.iter().enumerate() {
        sched::bind_irq(tick_intid(g), ep);
        TICK_ROUTES[g].store(ep + 1, Ordering::Relaxed);
    }
    true
}

/// Start the signalling: from here on, every tick on every core signals a route.
///
/// Release, pairing with [`signal_waiters`]'s acquire: a core that reads a nonzero count sees every
/// route [`bind_tick_routes`] stored and the `bind_irq` behind it.
fn arm_tick_hook(groups: usize) {
    TICK_GROUPS.store(groups, Ordering::Release);
}

/// How many groups to build on a machine with `cores` cores.
///
/// One group per core, and at least two so a single-core QEMU leg still runs something with more
/// than one endpoint in it. Capped by what the shared page has slots for.
fn topology_groups(cpus: usize) -> usize {
    let groups = cpus.max(2);
    groups.min(MAX_WORKERS / MEMBERS_PER_GROUP)
}

/// Which slot of the shared page belongs to this group's `member`.
///
/// Members are laid out consecutively, member 0 the responder, so an index names its group.
fn worker_index(group: usize, member: usize) -> usize {
    group * MEMBERS_PER_GROUP + member
}

/// Read one `u64` out of the shared page through the direct map.
fn read_counter(shared: u64, offset: u64) -> u64 {
    // SAFETY: `shared` is a frame this module allocated and never freed, the direct map names it,
    // and `offset` comes from `soak_page`, whose host tests prove every offset it produces lands
    // inside one page.
    unsafe { core::ptr::read_volatile((arch::mmu::phys_to_virt(shared) + offset) as *const u64) }
}

/// Beat, sample, judge; forever, or until something is wrong.
///
/// `tids` and `placed` are each worker's thread and spawn core, against which every beat reports
/// drift. Pets the watchdog and asks the reboot questions after each verdict. See
/// notes/soak/supervisor.md.
fn watch(shared: u64, workers: usize, tids: &[u64; MAX_WORKERS], placed: &[u8; MAX_WORKERS]) -> ! {
    let hz = arch::timer::frequency();
    let started = arch::timer::now();
    let mut previous = [0u64; MAX_WORKERS];
    let mut here = [u8::MAX; MAX_WORKERS];
    // The arrangement the log currently claims, which starts as the spawn placement `run` printed.
    let mut census = *placed;
    let mut last_total = 0u64;
    let mut last_wakes = 0u64;
    let mut last_at = started;
    let mut beat = 0u64;
    // Whether this boot will still reboot itself: until a keypress or a refused reset.
    #[cfg(feature = "reboot_soak_test")]
    let mut armed = true;
    #[cfg(soak_watchdog)]
    let mut watchdog = arm_watchdog();

    loop {
        // Yielded rather than spun, so the supervisor does not take a core from the workload.
        let due = arch::timer::now().wrapping_add(hz.wrapping_mul(BEAT_SECONDS));
        while arch::timer::now().wrapping_sub(due) > u64::MAX / 2 {
            sched::yield_now();
        }

        beat += 1;
        let now = arch::timer::now();
        let elapsed = now.wrapping_sub(started) / hz.max(1);
        let window = (now.wrapping_sub(last_at) / hz.max(1)).max(1);

        let BeatSample {
            total,
            wakes,
            mismatches,
            stalled,
            first_stalled,
        } = sample_workers(shared, &mut previous, workers);

        let drifted = count_drift(tids, workers, &mut here, &census);

        let refused = sched::wake_refusals();
        let rate = total.wrapping_sub(last_total) / window;
        let wake_rate = wakes.wrapping_sub(last_wakes) / window;
        println!(
            "soak-test: t={elapsed}s beat={beat} rounds={total} rate={rate}/s wakes={wakes} \
             wakerate={wake_rate}/s workers={workers} refused={refused} mismatch={mismatches} \
             stalled={stalled} drifted={drifted} crossings={} remote={} steals={} deferred={}",
            sched::migrations(),
            sched::remote_placements(),
            sched::steals_served(),
            sched::wakes_deferred(),
        );
        last_total = total;
        last_wakes = wakes;
        last_at = now;

        if drifted != 0 {
            print_census(
                "where the workers are NOW, the arrangement above having changed",
                workers / MEMBERS_PER_GROUP,
                &here[..workers],
            );
            census = here;
        }

        let verdict = verdict_of(refused, mismatches, stalled, beat);

        if let Some(why) = verdict {
            fail_run(elapsed, beat, why, first_stalled, workers, &here[..workers]);
        }

        // After the verdict, which diverges on a failure, so a reset never replaces evidence.
        #[cfg(feature = "reboot_soak_test")]
        if armed {
            ask_reboot_questions(&mut armed, elapsed);
        }

        #[cfg(soak_watchdog)]
        tend_watchdog(&mut watchdog, elapsed, beat);
        #[cfg(feature = "wedge_soak_test")]
        if elapsed >= WEDGE_AFTER_SECONDS {
            wedge(elapsed);
        }
    }
}

/// One beat's sums over every worker's counters, and which workers made no progress.
struct BeatSample {
    total: u64,
    wakes: u64,
    mismatches: u64,
    stalled: usize,
    first_stalled: usize,
}

/// Read every worker's counters, sum them, and count the workers whose progress did not move.
///
/// A worker's progress is its `rounds` plus its `wakes`, which covers every role because each
/// leaves one of them at zero and neither falls.
fn sample_workers(shared: u64, previous: &mut [u64; MAX_WORKERS], workers: usize) -> BeatSample {
    let mut total = 0u64;
    let mut wakes = 0u64;
    let mut mismatches = 0u64;
    let mut stalled = 0usize;
    let mut first_stalled = usize::MAX;
    for (i, last) in previous.iter_mut().take(workers).enumerate() {
        let rounds = read_counter(shared, soak_page::rounds(i));
        let woken = read_counter(shared, soak_page::wakes(i));
        mismatches = mismatches.wrapping_add(read_counter(shared, soak_page::mismatches(i)));
        total = total.wrapping_add(rounds);
        wakes = wakes.wrapping_add(woken);
        let progress = rounds.wrapping_add(woken);
        if progress == *last {
            stalled += 1;
            if first_stalled == usize::MAX {
                first_stalled = i;
            }
        }
        *last = progress;
    }
    BeatSample {
        total,
        wakes,
        mismatches,
        stalled,
        first_stalled,
    }
}

/// Read where each worker is now into `here`, and count those not where the last census put them.
///
/// Measured against the last census, not spawn, so a nonzero count means a newer census follows.
/// Tick waiters, whose movement is `crossings=`, and threads not yet run are not counted. See
/// notes/soak/supervisor.md.
fn count_drift(
    tids: &[u64; MAX_WORKERS],
    workers: usize,
    here: &mut [u8; MAX_WORKERS],
    census: &[u8; MAX_WORKERS],
) -> usize {
    sched::last_cpus(&tids[..workers], &mut here[..workers]);
    (0..workers)
        .filter(|&i| {
            role_of(i % MEMBERS_PER_GROUP) != ROLE_WAITER
                && here[i] != u8::MAX
                && here[i] != census[i]
        })
        .count()
}

/// The beat's verdict, if it is a failure: a wake refused, then a mismatch, then a stall.
///
/// The first beat is exempt from the stall check only, since its workers may not have started.
fn verdict_of(refused: u64, mismatches: u64, stalled: usize, beat: u64) -> Option<&'static str> {
    if refused != 0 {
        Some(
            "the wake gate refused a wake: a waker made a parked receiver Ready with nothing \
              delivered (sched::wake_load_aware, the boot-8 gate). This is the defect \
              design/fatal-risks/README.md risk 5 is about.",
        )
    } else if mismatches != 0 {
        Some(
            "a caller got back a word that was not the answer to what it sent, or a CALL \
              arrived without its reply capability. The IPC path delivered the wrong message.",
        )
    } else if stalled != 0 && beat > 1 {
        Some(
            "a worker made no progress for a whole beat while the machine kept running: it \
              is wedged, or it died.",
        )
    } else {
        None
    }
}

/// Report a failed beat with its census and the thread dump, then panic.
///
/// The dump comes before the panic because the panic handler prints none.
fn fail_run(
    elapsed: u64,
    beat: u64,
    why: &str,
    first_stalled: usize,
    workers: usize,
    here: &[u8],
) -> ! {
    println!("soak-test: FAILED at t={elapsed}s beat={beat}: {why}");
    if first_stalled != usize::MAX {
        println!(
            "soak-test: first stalled worker is {first_stalled}: group {}, member {} of \
             {MEMBERS_PER_GROUP} (member 0 is the responder, then {CALLERS_PER_GROUP} \
             callers, then {GRINDERS_PER_GROUP} grinder, then the tick waiter)",
            first_stalled / MEMBERS_PER_GROUP,
            first_stalled % MEMBERS_PER_GROUP,
        );
    }
    print_census(
        "where the workers were when this run failed",
        workers / MEMBERS_PER_GROUP,
        here,
    );
    sched::dump_threads();
    panic!("soak failed at t={elapsed}s beat={beat}: {why}");
}

/// The rebooting soak's two questions for this beat: has anyone typed, and is the window up.
#[cfg(feature = "reboot_soak_test")]
fn ask_reboot_questions(armed: &mut bool, elapsed: u64) {
    if crate::console::is_byte_waiting() {
        *armed = false;
        println!(
            "{REBOOT_MARKER} DISARMED at t={elapsed}s: a byte arrived on this console. \
             This board will not reboot itself again. The soak keeps running and keeps \
             beating; power it off when you are done with it."
        );
    } else if elapsed >= REBOOT_AFTER_SECONDS {
        draw_again(elapsed);
        *armed = false;
    }
}

/// Pet the watchdog for this beat, or halt it if a byte arrived on the console.
#[cfg(soak_watchdog)]
fn tend_watchdog(watchdog: &mut Option<Watchdog>, elapsed: u64, beat: u64) {
    if let Some(tco) = watchdog.take() {
        if crate::console::is_byte_waiting() {
            // The TCO answers from its halt bit; the JH7110's enable bit is read back after.
            #[cfg(target_arch = "x86_64")]
            let halted = tco.disarm();
            #[cfg(target_arch = "riscv64")]
            let halted = {
                tco.disarm();
                !tco.is_counting()
            };
            println!(
                "{WATCHDOG_MARKER} DISARMED at t={elapsed}s: a byte arrived on this console. The \
                 watchdog is {} and will not reset this machine. The soak keeps running.",
                if halted {
                    "halted"
                } else {
                    "STILL COUNTING (the halt bit did not take)"
                }
            );
        } else {
            let left = tco.ticks_left();
            tco.pet();
            if beat % WATCHDOG_REPORT_EVERY_BEATS == 1 {
                println!(
                    "{WATCHDOG_MARKER} petted at t={elapsed}s beat={beat}: the count had {left} \
                     of {} ticks left",
                    tco.reload()
                );
            }
            *watchdog = Some(tco);
        }
    }
}

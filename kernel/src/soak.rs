//! **The workload that does not stop, and the thing that watches it.** The fifth entry of
//! `design/fatal-risks/README.md` says the concurrency may be wrong in ways QEMU cannot show, and
//! names sustained multi-core stress with the load-sensitive assertions live as its experiment.
//! With `--features soak_test` the boot tour does not halt: it builds a pool of user-mode workers
//! and watches them forever. See notes/soak.md.
//!
//! **The workload is a user program and the detection is in the kernel.** The defect this risk
//! produced (a receiver made `Ready` without a delivery) is causable from userspace through the
//! real syscall path, so a kernel-mode stress loop would test an artefact of the test; and a user
//! program cannot assert about kernel internals, so the assertions stay here with the trace
//! counters. `fixtures/src/soaker.rs` is the workload; this file wires the pairs, samples them, and
//! decides.
//!
//! **The tick route.** A saturated workload never crosses cores: a rendezvous wake is local, and
//! `sched::wake_load_aware` is reachable only from an interrupt (§138 (how a saturated workload is
//! made to hand threads across cores)). So [`signal_waiters`] signals a rendezvous from
//! `sched::on_tick` on every core, and one worker role blocks on it through `Irq::WAIT`, running
//! the device-interrupt sequence: `sched::irq_notify` -> `Rendezvous::signal` ->
//! `handshake.serve` -> `wake_load_aware` -> `place_on` -> the reschedule IPI. What crosses is the
//! waiters, not the pairs, and the heartbeat says so on every run.
//!
//! Every [`BEAT_SECONDS`] of wall clock it prints a round-trip total with the counters that make it
//! interpretable (crossings, contention, gate refusals), for comparison between architectures,
//! between QEMU and silicon, and with the same machine later. A slow machine still prints; only a
//! wedged printer is silent. `crates/serial_console` reports `WentQuiet` after `--quiet-after`, and
//! [`START_MARKER`] re-arms that check, which a finished boot tour suppresses.
//!
//! # BUGS
//!
//! - **A green soak is never proof the concurrency is correct**: it licenses only "this machine did
//!   N cross-core round trips without a refusal, a wrong reply or a stall".
//! - The supervisor yields in a loop (no kernel sleep), so its rate is not `script/bench`'s.
//! - A panicked worker is reported "stalled", not "died", and is not restarted.
//! - The round-trip total is sampled while workers write it (`soak_page`'s `BUGS`).
//! - The tick route skips the interrupt controller's claim, mask and complete, and a run with it on
//!   is a different soak, comparable only with the same build.
//! - Nothing sets a duration; the watcher ends the run. [`REBOOT_AFTER_SECONDS`] is one draw of the
//!   placement lottery under `reboot_soak_test`, and many draws do not replace one long run.
//! - The escape polls one LSR bit nothing can prove is wired; the bench procedure confirms
//!   `DISARMED` before walking away (notes/soak.md).

// **The rebooting soak runs on all three architectures** (milestone 249 (the boot lottery is sampled by a person walking to the board)'s parity half,
// 2026-09-24). It was riscv64's alone, and a `compile_error!` said so, for a reason that still
// stands: a build that compiled and quietly never rebooted would be the worst available failure,
// indistinguishable from a board that drew the same placement fifty times. What changed is that
// each architecture now has a real reset behind `arch::reboot` (SBI SRST, PSCI `SYSTEM_RESET`, the
// FADT reset register and the two legacy ports) and a real escape behind `console::is_byte_waiting`
// (the NS16550's and PL011's sticky data-ready bits), and `script/soak-test --reboot` proves under
// QEMU, per architecture, that the machine came back rather than that the call returned.

// **The watchdog soak has no aarch64 watchdog to arm, and the compiler says so** (milestone 593
// (a wedged kernel resets itself), provisional number). x86_64 drives the Intel TCO and riscv64
// the JH7110's watchdog. The failure this prevents is milestone 249's worst one turned around: a
// build that compiled and quietly never armed would look exactly like a board that never wedged.
#[cfg(all(feature = "watchdog_soak_test", target_arch = "aarch64"))]
compile_error!(
    "--features watchdog_soak_test arms the Intel TCO on x86_64 (kernel/src/arch/x86_64/tco.rs) \
     and the JH7110 watchdog on riscv64 (kernel/src/drivers/jh7110_watchdog.rs); aarch64 has none \
     yet. argon's Tegra WDT is a later step of design/roadmap/0593-a-wedged-kernel-resets-itself.md."
);

// **Every x86_64 and riscv64 soak arms the watchdog unless it opts out**, calef's ruling of
// 2026-09-26 (UTC): "A soak should reset itself when it freezes." `build.rs` decides the
// `soak_watchdog` cfg, and this restates the rule it must implement, so a build-script edit that
// drops the default fails the `soak_test` build rather than shipping xenon or radon a soak that
// waits for a person.
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

/// How often the supervisor speaks.
///
/// Five seconds, against `serial_console`'s fifteen-second default quiet window: three missed beats
/// before a run is called a hang. Two would be tight on a board whose console is shared with
/// U-Boot's leftovers; four would take a minute to notice a wedge.
const BEAT_SECONDS: u64 = 5;

/// The line that tells the watcher a soak has begun.
///
/// `crates/serial_console`'s recogniser matches this prefix, so the two agree by construction
/// rather than by both being remembered. Changing it means changing that recogniser, and its tests
/// will say so.
///
/// **Spelled for the command, not for this module** (milestone 297, calef's ruling 2026-09-14): a
/// console marker takes the spelling of the command a reader typed to produce it, and the reader's
/// path to this string runs through `script/soak-test`. It read `soak: started` until then.
const START_MARKER: &str = "soak-test: started";

/// Wire the pool and never come back.
///
/// The caller is the boot thread at the end of the tour, and this replaces its `arch::halt()`.
pub fn run() -> ! {
    let cpus = smp::online_count();
    let groups = topology_groups(cpus);
    let callers = CALLERS_PER_GROUP;

    // Measured through the chain an ordinary boot's progenitor runs, because this boot has none
    // (milestone 563 (a seal check that reads bytes cannot see a check that was dropped)): a soak
    // card is left running unattended for hours, the last kind of boot that should enter bytes
    // nobody checked, and using the trust root is what keeps it in the image for `uefi_loader`'s
    // seal and `script/card-check` to find.
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

    // **One tick rendezvous per group** (milestone 221), for the reason `signal_waiters` gives at
    // length: a shared one lets whichever waiter is already running drain a burst of signals through
    // `Rendezvous`'s `pending` path while its queued peers starve, and a loaded host produces
    // exactly those bursts. Measured rather than feared: the shared version failed a run.
    let mut tick_endpoints = [0u64; MAX_GROUPS];
    for slot in tick_endpoints.iter_mut().take(groups) {
        *slot = sched::create_rendezvous();
    }
    // Routed before any waiter exists, so none can meet an unbound route; the signalling itself is
    // still switched on last, below. See `bind_tick_routes`.
    if !bind_tick_routes(&tick_endpoints[..groups]) {
        println!(
            "soak-test: FAILED: one of interrupts {}..={TICK_INTID_TOP} is already routed, so a tick \
             route would steal it; see TICK_INTID_TOP in kernel/src/soak.rs",
            tick_intid(groups - 1)
        );
        arch::halt(arch::HaltReason::measurement_boot());
    }

    let (tids, placed) = spawn_workers(image, shared, groups);

    // **Last, and after every worker exists** (milestone 221). Signalling earlier would hand the
    // first waiter a backlog to drain before it ever blocked, so the first beat would measure setup
    // rather than the machine. Harmless either way (a signal is counted, not lost), which is why
    // this is the half that waits and the routing above is the half that does not.
    arm_tick_hook(groups);

    let workers = groups * MEMBERS_PER_GROUP;
    print_start_banner(groups, callers, workers, cpus);

    #[cfg(feature = "reboot_soak_test")]
    arm_reboot();

    print_spawn_census(groups, &placed[..workers]);

    watch(shared, workers, &tids, &placed)
}

/// Spawn every group's workers, responder first, and keep the thread and core each was given.
fn spawn_workers(
    image: &'static [u8],
    shared: u64,
    groups: usize,
) -> ([u64; MAX_WORKERS], [u8; MAX_WORKERS]) {
    // **What milestone 240 needs and nothing else records.** The kernel decides where every worker
    // goes and then forgets; these two arrays are that decision kept, so the census below can be
    // printed from what happened rather than from what the placement policy usually does.
    let mut tids = [0u64; MAX_WORKERS];
    let mut placed = [u8::MAX; MAX_WORKERS];

    for group in 0..groups {
        let endpoint = sched::create_rendezvous();
        // The responder first. A caller whose responder is not yet receiving simply parks on the
        // rendezvous, so the order is not load-bearing; starting the answering half first keeps a
        // fresh boot's first few beats from reading as a stall.
        for member in 0..MEMBERS_PER_GROUP {
            let role = role_of(member);
            let index = worker_index(group, member);
            // A waiter holds an `Irq` capability and nothing else; it never sends or receives on
            // the group's endpoint and must not be able to. That is the same least-authority rule
            // the other three roles get from their rights, one object over.
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
                        // A distinct, odd, well-spread seed per worker: the jitter is what keeps
                        // the pairs from locking into one interleaving, and two workers handed the
                        // same seed would defeat it for that pair.
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
            // `pick_spawn_target` returns an online cpu id, and `MAX_CPUS` is what bounds one, so
            // this cannot truncate. Saturating rather than `as` so that a future topology which
            // broke that assumption would report `u8::MAX`, the census's own "no answer" value,
            // instead of a wrong core number a reader would believe.
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

/// **The prefix every census line carries** (milestone 240).
///
/// Deliberately not `soak-test:`. `crates/serial_console`'s recogniser matches two substrings on
/// that prefix, [`START_MARKER`] and `soak-test: t=`, and a census is neither the start of a run nor a
/// heartbeat; giving it its own word keeps a block of census lines from having to be proven
/// harmless against a recogniser it has nothing to do with. It is also what a reader greps for,
/// which is the whole point of printing it.
///
/// Name: provisional (milestone 240 (the soak reports what happened and not where)): calef names
/// what a reader meets. Respelled from `soak-census:` by milestone 297 (`soak` becomes
/// `soak-test`), tracking the command.
const CENSUS_MARKER: &str = "soak-test-census:";

/// Which role the `member`-th thread of a group plays.
///
/// One function rather than the ladder it replaced, because the spawn loop and the census have to
/// agree about it: a census that labelled the roles by a second copy of this arithmetic would be
/// an instrument that could disagree with the thing it measures.
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

/// **Print which roles are on which core, one line per core** (milestone 240).
///
/// `cpus[i]` is where worker `i` is, and `u8::MAX` means nothing knows yet. The tokens are
/// `<letter><group>`, so `G0 G3` on one line is that core drawing two grinders, which is the shape
/// milestone 240's block names and the one a reader of a log alone could not previously see.
///
/// A core with no workers gets a line too. That is the informative case rather than the empty one:
/// four cores online and one of them idle is an explanation, and a census that printed only the
/// occupied cores would hide it.
///
/// This states where the threads are and draws no conclusion from it, which is the milestone's own
/// constraint: if a series of boots shows the rate does not follow the placement, that is the
/// result, and this must not have prejudged it.
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
    // Everything the per-core loop did not name. Two causes, and both are worth a line rather than
    // a silent shortfall: a worker that has not been switched to yet (its `last_cpu` is still
    // unset), and a core id that is not in the online set, which would be a bug in placement rather
    // than in this census and which `groups` is printed beside so the arithmetic can be checked.
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

/// How many tick waiters share a group (milestone 221).
///
/// One, and the number is doing real work rather than being a placeholder. Every waiter is blocked
/// on the *same* rendezvous, and a tick raises one signal, which wakes one of them; so the number
/// of waiters is the number of threads the wake protocol has to choose between, and it wants to be
/// comparable with the core count so that `pick_wake_target` has somewhere to put them. One per
/// group is one per core, since [`topology_groups`] builds a group per core.
const WAITERS_PER_GROUP: usize = 1;

/// How many worker threads one group has.
///
/// One responder, its callers, its grinders and its waiter. Spelled once because it is the divisor
/// in [`worker_index`], the multiplier in the spawn loop, and the bound [`topology_groups`] fits
/// under: three places that were three copies of the same sum before milestone 221 added a fourth
/// term to it.
const MEMBERS_PER_GROUP: usize = 1 + CALLERS_PER_GROUP + GRINDERS_PER_GROUP + WAITERS_PER_GROUP;

/// **The highest interrupt number the tick routes are bound to** (milestone 221). Group `g` uses
/// `TICK_INTID_TOP - g`.
///
/// These name no hardware and none of the three architectures can deliver them, which is the whole
/// requirement: `sched::bind_irq` writes a slot in a flat array that `sched::irq_route` reads, and
/// these only have to be slots nothing else in a soak boot claims.
///
/// **Counting down from 255, and each architecture rules the band out for its own reason.** On
/// aarch64 and riscv64 a routed interrupt is delivered only if something enabled it at the
/// controller, and nothing enables these; the highest number either architecture's device tree
/// actually hands out is two orders of magnitude below them. On `x86_64` the top of the band is
/// `arch::x86_64::irq::SPURIOUS_VECTOR`, which the trap handler answers in its own arm before
/// `irq_route` is ever asked, and the rest of it sits at the far end of the MSI band, which is
/// allocated upward from `MSI_VECTOR_BASE` (0xc0) and would need more than fifty devices to reach.
///
/// **None of that is what makes this safe**, and saying so is the point: the arithmetic is an
/// argument and [`arm_tick_routes`] is a mechanism. It asks `sched::irq_route` about every number
/// it is about to take, and refuses to start a soak whose tick routes would steal somebody else's
/// interrupt. A soak boot runs the whole tour first, so every device that is going to claim an
/// interrupt has already claimed it by the time that check runs.
///
/// Name: provisional (milestone 221 (the soak never crosses cores)): calef names public items.
const TICK_INTID_TOP: u32 = 255;

/// **The prefix every line about the reboot loop carries** (milestone 249).
///
/// Its own word, for [`CENSUS_MARKER`]'s reason and one more of its own. `crates/serial_console`'s
/// recogniser matches two substrings on `soak-test: `, and neither of them is anything this loop
/// says.
/// The extra reason is that these are the lines a person greps a fifty-boot log for when they want
/// to know why the series stopped, and a prefix that means exactly "the reboot loop said something"
/// answers that in one command.
///
/// Name: provisional (milestone 249): calef names public items. Respelled from `soak-reboot:` by
/// milestone 297, tracking the command.
#[cfg(feature = "reboot_soak_test")]
const REBOOT_MARKER: &str = "soak-test-reboot:";

/// **How this architecture asks for a cold reboot, in the words the banner uses** (milestone 249).
/// `arch::reboot` prints the detail of each attempt; this is the one-phrase version an operator
/// reads before walking away.
#[cfg(all(feature = "reboot_soak_test", target_arch = "riscv64"))]
const REBOOT_ROUTE: &str = "SBI SRST system_reset, reset type 1";
#[cfg(all(feature = "reboot_soak_test", target_arch = "aarch64"))]
const REBOOT_ROUTE: &str = "PSCI SYSTEM_RESET, on the conduit /psci names";
#[cfg(all(feature = "reboot_soak_test", target_arch = "x86_64"))]
const REBOOT_ROUTE: &str =
    "the FADT reset register, then port 0xcf9 full reset, then the 8042 reset pulse";

/// **How long one boot soaks before it draws the placement lottery again** (milestone 249).
///
/// Two minutes, and both bounds on the number are measured rather than picked.
///
/// **The floor is when the arrangement is knowable.** notes/soak.md records that on radon the spawn
/// placement holds for about twenty-five seconds and is then replaced, in a single drift event, by
/// an arrangement that does not change again. A window shorter than that would record the lottery's
/// *first* draw and not the one the rate is a property of, which is the reading the whole
/// distribution is for. Thirty seconds is the minimum that can be right and leaves no margin at all
/// for a boot that converges more slowly.
///
/// **The ceiling is that fifty boots have to fit in an evening**, which is this milestone's own
/// premise: nine hand-cycled draws were a whole one. At two minutes plus the twenty-odd seconds a
/// boot takes, fifty draws is about two hours unattended, and a hundred is an overnight run.
///
/// Two minutes then buys roughly twenty beats after convergence, which is enough for the rate to be
/// read off several of them rather than off the one that happened to be last. It is a constant
/// rather than a knob because there is no configuration path to a board kernel: the card carries a
/// build, and changing this means building another one.
#[cfg(feature = "reboot_soak_test")]
const REBOOT_AFTER_SECONDS: u64 = 120;

/// **The last chance to stop the loop, after the window and before the reset** (milestone 249).
///
/// Five seconds, matching [`BEAT_SECONDS`], so a person who reads the announcement on a console has
/// the same amount of time to act that a beat takes to arrive. It is not the main escape and is not
/// load-bearing: [`REBOOT_AFTER_SECONDS`] worth of beats have already asked the same question, and
/// this window exists for the case where somebody walks up mid-run and wants the board back without
/// having to guess where in the cycle it is.
#[cfg(feature = "reboot_soak_test")]
const REBOOT_GRACE_SECONDS: u64 = 5;

/// **Arm the reboot loop, and say so in the place a reader meets it** (milestone 249).
///
/// Called once, from [`run`], after the workers exist and before the first census. Two things
/// happen and the announcement is the more important of them.
///
/// The **drain** is `Ns16550::discard_rx`'s job and its own comment explains what it is for: a
/// console that has been sitting in front of a person may hold a byte U-Boot's countdown collected,
/// and firing the escape on boot 1 of 50 because of it would produce no distribution and no
/// explanation.
///
/// The **banner** is rung three of AGENTS.md's ladder, put where it cannot be missed. A kernel that
/// is going to reboot the machine it is running on must say so on the only channel it has, before
/// it does it, in words that include how to stop it. Somebody who inherits a card and boots it
/// finds out what it is from the first screen, not from a milestone document.
#[cfg(feature = "reboot_soak_test")]
fn arm_reboot() {
    crate::console::discard_rx();
    // What ended the previous boot, from the PMIC's side: E27's reset printed nothing here.
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

/// **The window has run out: announce, offer the grace period, and reset** (milestone 249).
///
/// Returns in exactly two cases, and the caller disarms on both: somebody typed, or the firmware
/// refused. It cannot return having rebooted, because a successful reset does not come back.
///
/// **It is called after the beat's verdict and never before it**, which is the ordering that
/// matters most in this file. A soak that has just failed panics, and a panic diverges, so a run
/// that found something can never reboot over its own evidence. The whole point of fifty boots is
/// the one that fails, and a loop that tidied it away by resetting would be an instrument that
/// destroys its own best result.
#[cfg(feature = "reboot_soak_test")]
fn draw_again(elapsed: u64) {
    println!(
        "{REBOOT_MARKER} window reached at t={elapsed}s. Cold-rebooting in \
         {REBOOT_GRACE_SECONDS}s to draw the placement lottery again; press any key on this \
         console to stop the loop."
    );

    // Poll the escape for the whole grace window rather than only at its end, so a key pressed on
    // reading the line above is honoured immediately. `yield_now` rather than a spin, for `watch`'s
    // reason: this thread is one more contender and should give its core back between checks.
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

    // Every attempt prints its own line before it is made, because a reset stops the UART
    // draining and anything after the call may never reach the wire. `cold_reset` returns only
    // when every route was refused, board routes before firmware routes (the direct AXP15060
    // write milestone 592 (radon's cold reboot dies in OpenSBI's PMIC write) adds is the first on
    // a JH7110), each refusal already on the console.
    println!("{REBOOT_MARKER} rebooting now ({REBOOT_ROUTE}).");
    crate::console::drain();
    let refused = crate::reboot::cold_reset(REBOOT_MARKER);
    println!(
        "{REBOOT_MARKER} FAILED ({refused:?}): every reset route was refused (the lines above say \
         how), so an unattended series is not available on this machine by this route. The soak \
         keeps running; nothing has been damaged and no further reset is attempted."
    );
}

/// **The prefix every line about the watchdog carries** (milestone 593, provisional number). One
/// grep finds how the watchdog was found, armed, petted, disarmed or deliberately starved, and
/// `script/soak-test --watchdog` and `--wedge` read these lines. Name provisional.
#[cfg(soak_watchdog)]
const WATCHDOG_MARKER: &str = "soak-test-watchdog:";

/// **Seconds from the last pet to the reset** (milestone 593).
///
/// Sixty, as the proposal this milestone came from priced it: twelve beats of margin, so a beat
/// delayed by a loaded machine cannot trip it, and short enough that a wedged board is back before a
/// person would have walked to it. Both watchdogs count twice before they reset, so this is two
/// counts of 30 s: 50 TCO ticks each, or 720,000,000 ticks of the JH7110's 24 MHz core clock.
#[cfg(soak_watchdog)]
const WATCHDOG_RESET_AFTER_SECONDS: u64 = 60;

/// Print a pet once every this many beats, and at the first. Every beat would double the log; the
/// first line is the one that proves the counter moves, and the rest say it still does.
#[cfg(soak_watchdog)]
const WATCHDOG_REPORT_EVERY_BEATS: u64 = 6;

/// **When the test-only wedge happens** (milestone 593): six beats in, so the log shows the watchdog
/// being petted before it shows it being starved.
#[cfg(feature = "wedge_soak_test")]
const WEDGE_AFTER_SECONDS: u64 = 30;

/// **Find the chipset's watchdog and arm it, saying so first** (milestone 593).
///
/// Returns the armed driver, or `None` with the reason on the console. A soak without a watchdog
/// still runs: the soak is the experiment and the watchdog only decides who recovers the machine.
///
/// The banner is milestone 249's rule applied again: a build that will reset the machine it runs on
/// says so on the only channel it has, with how to stop it, before it can happen.
#[cfg(all(soak_watchdog, target_arch = "x86_64"))]
fn arm_watchdog() -> Option<Watchdog> {
    // A stale byte must not halt the watchdog at the first beat; see `arm_reboot`.
    crate::console::discard_rx();
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
            // Crossed: the build's hazard, then how to stop it, the same order as milestone 249's.
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

/// The watchdog each architecture's soak arms (milestone 593).
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

/// **Find the JH7110's watchdog, start its clocks, arm it and see it count** (milestone 593).
///
/// The x86_64 twin's banner rule holds: the hazard and its escape are on the console before the
/// reset can happen. Two refusals it does not need: the tree must name the watchdog and its
/// clocks (there is no constant to fall back on), and the counter must run at the rate its count
/// was computed for, measured against this kernel's own timer. A counter on a gated clock reads a
/// frozen count, and arming it would print `ARMED` over a timer that never fires.
#[cfg(all(soak_watchdog, target_arch = "riscv64"))]
fn arm_watchdog() -> Option<Watchdog> {
    use jh7110_watchdog::CORE_CLOCK_HZ;
    crate::console::discard_rx();
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

/// **Stop beating, on purpose** (milestone 593's test-only wedge).
///
/// The wedge a watchdog exists for is a core that will never run the supervisor again. Masking
/// interrupts and spinning is that, on the one core that matters: no tick can preempt this thread,
/// so the beat never comes back, and nothing else in the kernel knows how to pet the watchdog.
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

/// **The rendezvous [`signal_waiters`] signals, one per group, each plus one; zero means empty.**
///
/// Plus one for the same reason `sched::IRQ_ROUTES` does it: a rendezvous name may legitimately be
/// zero, and this needs a value that means "nothing here" without a second word to say so.
///
/// Plain atomics rather than anything under a lock, because the reader is an interrupt handler on
/// every core: taking a lock to find out *whether there is anything to do* is a thing that can go
/// wrong, and a single load cannot be. Written once each, by the boot thread, after every worker
/// exists.
static TICK_ROUTES: [AtomicU64; MAX_GROUPS] = [const { AtomicU64::new(0) }; MAX_GROUPS];

/// How many of [`TICK_ROUTES`] are armed. Zero until [`arm_tick_routes`] has bound them all, which
/// is what keeps the hook inert during setup and what [`signal_waiters`] loads first.
static TICK_GROUPS: AtomicUsize = AtomicUsize::new(0);

/// Which route the next tick signals, anywhere on the machine. See [`signal_waiters`].
static TICK_CURSOR: AtomicUsize = AtomicUsize::new(0);

/// **Signal one tick route. Called from `sched::on_tick`, in interrupt context, on every core.**
///
/// This is the whole of milestone 221's kernel-side mechanism, and it is short because the path it
/// wants was already built: `sched::irq_notify` is the function a device interrupt goes through, it
/// documents itself as safe from interrupt context (`IPC_TABLES` is an `IrqSafeMutex`, DECISIONS
/// §9, so the interrupted code on this core cannot have been holding it), and everything
/// interesting is downstream of it.
///
/// Not armed on an ordinary boot, because there is no ordinary boot: the whole module is behind
/// `--features soak_test`. Within a soak boot it stays inert until [`arm_tick_routes`] runs, which is
/// after every worker has been spawned, so the ticks that land during setup are not counted against
/// a waiter that does not exist yet.
///
/// # One route per group, round-robin, and it is a fix rather than a flourish
///
/// The first version signalled **one** rendezvous that every waiter blocked on, and it failed a run
/// on a loaded host: three of four waiters made no progress for a whole beat and the soak reported
/// them as wedged. The cause is in `crates/inter_process_communication`'s `Rendezvous`, and it is
/// correct behaviour there: `receive` takes a **pending** signal before it looks at the receiver
/// queue, because a driver must never miss an interrupt that already happened. So when ticks arrive
/// in a burst, which is what a guest on a busy host sees, whichever waiter is already running
/// drains the whole backlog through the `pending` path and never queues, while its peers sit at the
/// head of a queue nothing pops. One waiter is fed and the rest starve.
///
/// A rendezvous per group removes the shared resource, so a backlog can only ever belong to the
/// waiter it accumulated for. The cursor is global rather than derived from `cpu::id()` so that
/// every route is covered whatever the topology: [`topology_groups`] builds at least two groups even
/// on a single-core machine, where a scheme keyed on the core would leave the second waiter to
/// starve for the whole run.
///
/// Name: provisional (milestone 221): calef names public items.
pub fn signal_waiters() {
    let groups = TICK_GROUPS.load(Ordering::Acquire);
    if groups == 0 {
        return;
    }
    // Relaxed: all this counter has to do is differ from its neighbours over time. It may skip or
    // repeat a number under contention without anything being wrong, because the routes are
    // equivalent and the fairness it buys is statistical rather than exact.
    let which = TICK_CURSOR.fetch_add(1, Ordering::Relaxed) % groups;
    match TICK_ROUTES[which].load(Ordering::Relaxed) {
        0 => {}
        route => sched::irq_notify(route - 1),
    }
}

/// Bind one tick route per group. `false` if any of the interrupt numbers is already spoken for,
/// which is a refusal to start rather than a thing to work around.
///
/// Every number is checked **before** any of them is bound, so a refusal leaves the tree as it found
/// it rather than half-routed.
///
/// **Called before the first waiter is spawned, and the order is a bug fix.** It was called after,
/// so that ticks would not accumulate against waiters that did not exist yet, and that reasoning was
/// right about the backlog and wrong about the race: a waiter that reached `Irq::WAIT` before its
/// route existed got `WrongObject`, and a refused waiter has nowhere to report to, so it stopped
/// counting and the run failed one beat later with four workers "wedged". Binding first closes the
/// window entirely, because [`TICK_GROUPS`] is what actually starts the signalling and that is still
/// stored last. The backlog this used to avoid is now bounded and harmless: each route is its own,
/// its waiter drains it at once, and the first beat absorbs it.
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
/// Release against [`signal_waiters`]'s acquire, and it is the edge that matters: a core that reads
/// a nonzero group count must also see every route [`bind_tick_routes`] stored and every `bind_irq`
/// behind them, or a tick could signal a slot that is still empty.
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
/// Members are laid out consecutively, member 0 being the group's responder, so a stalled index in
/// a report names its group by arithmetic a reader can do in their head.
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
/// `tids` and `placed` are milestone 240's: the thread and the core each worker was given at spawn,
/// so that every beat can say whether the census printed at the top of the log is still true.
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
    // **Is this boot still going to reboot itself?** (Milestone 249.) True until somebody types on
    // the console or the firmware refuses a reset. A plain local rather than a static because
    // exactly one thread ever asks: the supervisor is the only caller of both halves.
    #[cfg(feature = "reboot_soak_test")]
    let mut armed = true;
    // **The watchdog, armed as the beat begins** (milestone 593): from here on, a beat that does not
    // come is a reset about a minute later.
    #[cfg(soak_watchdog)]
    let mut watchdog = arm_watchdog();

    loop {
        // Yield until the beat is due. `yield_now` rather than a spin: this thread is one more
        // contender for the machine, and one that gives its core back between checks contends
        // usefully instead of stealing a core from the workload.
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
        // One line, one beat, every field named. A log a person greps six weeks later is worth more
        // than a table that lines up on a terminal nobody kept.
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

        // **Re-census when the answer changed, and only then** (milestone 240). A block every beat
        // would double an already dense log; a block never would leave the start census standing
        // after it stopped being true, which the aarch64 QEMU leg showed happens inside the first
        // five seconds. Printing on the change carries a current census whenever one exists and is
        // quiet the rest of the time, and `drifted=` is what makes the quiet readable.
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

        // **The rebooting soak's two questions, in this order, and only after the verdict**
        // (milestone 249). After, because `panic!` above diverges: a run that found something keeps
        // its evidence on the console and the board instead of resetting over it.
        //
        // The escape is asked first and unconditionally, so that a keypress arriving in the same
        // beat as the deadline stops the loop rather than racing it. LSR's data-ready bit is
        // sticky, cleared only by reading the byte out, so this poll every [`BEAT_SECONDS`] cannot
        // miss one: the question is "has anyone typed since the soak armed", not "is anyone typing
        // right now".
        #[cfg(feature = "reboot_soak_test")]
        if armed {
            ask_reboot_questions(&mut armed, elapsed);
        }

        // **Pet the watchdog, after the verdict** (milestone 593). After, for `draw_again`'s reason:
        // a failed beat panics above and never gets here, so the watchdog resets a failed soak only
        // once its dump is on the console. The escape is the same sticky console byte the reboot
        // loop reads, asked first, so a keypress halts the watchdog rather than racing it.
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
        // **One question for every role** (milestone 221): did *either* of this worker's two
        // progress counters move? A waiter leaves `rounds` at zero forever and the other three
        // roles leave `wakes` at zero forever, so summing them is a progress measure that needs
        // no table of who plays which part, and cannot be fooled: neither counter ever goes
        // down, so a sum that is unchanged means both terms were.
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
fn count_drift(
    tids: &[u64; MAX_WORKERS],
    workers: usize,
    here: &mut [u8; MAX_WORKERS],
    census: &[u8; MAX_WORKERS],
) -> usize {
    // **Is the census the log last printed still true?** (milestone 240.) One lock acquisition
    // and `workers` comparisons once every [`BEAT_SECONDS`], which is what lets a census be
    // printed on a change instead of on every beat: while this reads zero the reader knows the
    // block above them describes the machine right now, and they are told rather than trusting.
    //
    // **Measured against the last census rather than against spawn**, and the difference is the
    // whole value. Against spawn it becomes a constant a few beats in (11 of 20 on the aarch64
    // QEMU leg) and then says nothing further; against the census it returns to zero after each
    // reprint, so a nonzero one always means "there is a newer block below".
    //
    // Counted over the roles whose movement is news. A tick waiter migrating is milestone 221's
    // whole point and is already `crossings=`, so folding it in would make this rise on a
    // healthy run and mean nothing. A thread reading `u8::MAX` has not run yet and has no
    // second core to have gone to, which is not drift either.
    sched::last_cpus(&tids[..workers], &mut here[..workers]);
    (0..workers)
        .filter(|&i| {
            role_of(i % MEMBERS_PER_GROUP) != ROLE_WAITER
                && here[i] != u8::MAX
                && here[i] != census[i]
        })
        .count()
}

/// The beat's verdict, if it is a failure.
fn verdict_of(refused: u64, mismatches: u64, stalled: usize, beat: u64) -> Option<&'static str> {
    // The three verdicts, in the order a reader would want them: the one that is a finding
    // about the kernel, then the one that is a finding about delivery, then the one that is a
    // finding about liveness.
    //
    // The first beat is exempt from the stall check and only from that one: workers that have
    // not been scheduled yet have not stalled, they have not started.
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
        // Group and member, not a partner index. The old line printed `first_stalled ^ 1`
        // and called it the partner, which was arithmetic from a two-member group that no
        // longer exists; naming the group points a reader at every thread that could have
        // wedged this one, which is what they actually need.
        println!(
            "soak-test: first stalled worker is {first_stalled}: group {}, member {} of \
             {MEMBERS_PER_GROUP} (member 0 is the responder, then {CALLERS_PER_GROUP} \
             callers, then {GRINDERS_PER_GROUP} grinder, then the tick waiter)",
            first_stalled / MEMBERS_PER_GROUP,
            first_stalled % MEMBERS_PER_GROUP,
        );
    }
    // The census before the dump, because a failure is the one moment a reader has the
    // whole log in front of them and wants to know the shape of the machine that produced
    // it. It costs four lines once, on a run that is ending anyway.
    print_census(
        "where the workers were when this run failed",
        workers / MEMBERS_PER_GROUP,
        here,
    );
    // The thread dump before the panic, because the panic handler does not print one and
    // the per-core event rings are the whole reason they exist: a soak failure with no path
    // into the wedge is the end state alone, which is the thing first-silicon diagnostics
    // were built because nobody could read.
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
        // Returns only if somebody typed in the grace window or the firmware refused the
        // reset, and both mean the same thing here: stop asking.
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

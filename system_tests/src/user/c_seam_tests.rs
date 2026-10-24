use super::test_programs::spawn_progenitor_shaped;
use crate::sched;

/// The report protocol, matching `crates/c_seam`. Userspace owns the definition; the test
/// mirrors it, the same convention `authority_tests` and the net client's selectors follow.
const RPT_RAN: u64 = 1;
const RPT_DEATH: u64 = 2;
const RPT_SITE: u64 = 3;
const RPT_VERDICT: u64 = 4;
const RPT_FAILED: u64 = 9;

/// The verdict bits, matching `crates/c_seam`'s `checks` module.
const IN_GRANT_WRITE_LANDED: u64 = 1 << 0;
const WITNESS_RO_INTACT: u64 = 1 << 1;
const WITNESS_FAR_INTACT: u64 = 1 << 2;
const FAULT_ADDR_AS_EXPECTED: u64 = 1 << 3;
const OUTPUT_CORRECT: u64 = 1 << 4;

/// What a run of one faulting attempt must report: the component's own store landed, both
/// witnesses survived, and the fault was where the C code pointed.
const CONFINED: u64 =
    IN_GRANT_WRITE_LANDED | WITNESS_RO_INTACT | WITNESS_FAR_INTACT | FAULT_ADDR_AS_EXPECTED;
/// What the honest attempt must report: all of the above, plus a correct answer.
const CONFINED_AND_CORRECT: u64 = CONFINED | OUTPUT_CORRECT;

/// The attempts, matching `crates/c_seam`.
const ATTEMPTS: usize = 4;
const ATTEMPT_HONEST: u64 = 2;

/// The confiner's budget. It builds three instances of the shim, one at a time, reaping each
/// before the next, so this covers one instance region plus its own scratch mappings rather
/// than three.
const CONFINER_BUDGET_PAGES: u64 = 512;

/// Four reports per attempt (ran, death, site, verdict).
const EXPECTED_REPORTS: usize = 4 * ATTEMPTS;

/// **Spawn the confiner the way the kernel spawns the progenitor**, and return the report endpoint
/// every process in the run holds a WRITE view of. Deliberately the same endowment
/// `spawn_elf_child_builder` gives (the archive read-only at `INITIAL_RAMDISK_VA`, an untyped in
/// slot 0, a report endpoint in slot 1), so what is under test is the seam rather than a privileged
/// shortcut.
fn spawn_confiner() -> (sched::RendezvousId, u64) {
    // From a region the run owns and reclaims, as `live_swap_tests::spawn_swapper` does.
    let report_region = crate::memory_region::create(1).expect("no region for the report endpoint");
    let report = sched::create_rendezvous_from(report_region).expect("no report rendezvous");
    spawn_progenitor_shaped(
        "c_confiner",
        CONFINER_BUDGET_PAGES,
        crate::cap::Rights::ALL,
        report,
        crate::cap::Rights::WRITE.union(crate::cap::Rights::GRANT),
        &[],
        None,
        0,
    );
    (report, report_region)
}

/// **Wait, with a deadline, for the next report to be parked on the endpoint.**
///
/// The confiner reports a death only when one is delivered, so under a broken confinement (the
/// out-of-bounds store landing instead of faulting) a bare blocking receive here has nothing to
/// take, and the test would fail for the right reason as a watchdog livelock, saying the wrong
/// thing (`design/roadmap/0202-confinement-claims-falsified.md`). Thirty seconds against a
/// 90-second per-test budget: a healthy run's largest gap, a shim build and start, is well under a
/// second, and a stalled run fails here with a sentence about confinement.
fn wait_for_report(report: sched::RendezvousId) -> bool {
    let deadline = crate::arch::timer::now() + 30 * crate::arch::timer::frequency();
    while crate::arch::timer::now() < deadline {
        if sched::rendezvous_waiting_senders(report) > 0 {
            return true;
        }
        sched::yield_now();
    }
    sched::rendezvous_waiting_senders(report) > 0
}

/// **Run one confiner from spawn to quiescence**, returning every report it made.
///
/// Run to the end rather than stopping at the first interesting message, for the reason
/// `authority_tests::run_tree` records: a half-run harness keeps building processes in the
/// background, and a test that leaves work running is a test that fails somebody else.
fn run_seam() -> [[u64; 5]; EXPECTED_REPORTS] {
    let (report, report_region) = spawn_confiner();
    let mut msgs = [[0u64; 5]; EXPECTED_REPORTS];
    for (i, slot) in msgs.iter_mut().enumerate() {
        assert!(
            wait_for_report(report),
            "report {i} of {EXPECTED_REPORTS} never arrived, so the run stalled rather than \
             finishing. The likeliest cause is that an attempt did not fault: the confiner \
             reports a death only when one is delivered, so a store that was supposed to be \
             refused and was not leaves this receive with nothing to take. That is the \
             confinement failing, and it is reported here rather than as a watchdog timeout \
             because a timeout says livelock and means nothing to whoever reads it.",
        );
        let msg = sched::ipc_receive(report);
        assert_ne!(
            msg[0], RPT_FAILED,
            "the C seam harness could not be built: stage {}. Stages 1-2 are the archive and \
             the c_shim ELF, 3-6 the shared pages, 7 the supervision endpoint, 10-13 building, \
             starting, and reaping an instance.",
            msg[1],
        );
        *slot = msg;
    }

    // Let it settle, then prove there is nothing more to say. A parked sender here means a
    // thirteenth report exists, which is how "the supervisor did not restart the instance that
    // finished" is proven without a blocking receive that would hang when the code is right.
    for _ in 0..400 {
        sched::yield_now();
    }
    assert_eq!(
        sched::rendezvous_waiting_senders(report),
        0,
        "the run made more than {EXPECTED_REPORTS} reports: the supervisor acted after the \
         honest attempt finished",
    );
    sched::reclaim_region(report_region).expect("the report endpoint's region did not come back");
    msgs
}

/// Every report of one kind, in arrival order.
fn of_kind(msgs: &[[u64; 5]; EXPECTED_REPORTS], kind: u64) -> impl Iterator<Item = &[u64; 5]> {
    msgs.iter().filter(move |m| m[0] == kind)
}

/// **A C out-of-bounds write faults the process and touches nothing outside its grant.** The
/// milestone's one load-bearing test.
///
/// The off-by-one store lands on a page mapped read-only into the component and read/write into
/// the confiner: the same frame, present in the offender's tables, and provably unchanged. The
/// wild store lands on a virtual address the component has no mapping for and the confiner does:
/// another frame, unchanged. The verdict bitmap is asserted for **equality**: a missing bit is a
/// broken confinement, and a superset is the checker answering a question nobody asked.
///
/// Falsification: replayable `system_tests/falsifications/user.c_seam_tests.a_c_out_of_bounds_write_faults_and_changes_nothing_outside_its_grant.patch`
#[test_case]
fn a_c_out_of_bounds_write_faults_and_changes_nothing_outside_its_grant() {
    let msgs = run_seam();

    let mut verdicts = of_kind(&msgs, RPT_VERDICT);
    for attempt in 0..2u64 {
        let v = verdicts
            .next()
            .unwrap_or_else(|| panic!("no verdict for attempt {attempt}"));
        assert_eq!(v[1], attempt, "verdicts arrived out of order");
        assert_eq!(
            v[2],
            CONFINED,
            "attempt {attempt}: the C component was not confined. Bits that should be set and \
             are not: in-grant store landed {}, read-only witness intact {}, unmapped witness \
             intact {}, fault address as expected {}.",
            v[2] & IN_GRANT_WRITE_LANDED != 0,
            v[2] & WITNESS_RO_INTACT != 0,
            v[2] & WITNESS_FAR_INTACT != 0,
            v[2] & FAULT_ADDR_AS_EXPECTED != 0,
        );
    }

    // The deaths themselves: a real fault, not a cooperative exit, and a tid the supervisor can
    // trust because the kernel is the only sender on this path (§26.5).
    let mut deaths = of_kind(&msgs, RPT_DEATH);
    for attempt in 0..2u64 {
        let d = deaths
            .next()
            .unwrap_or_else(|| panic!("attempt {attempt} produced no death message"));
        assert_eq!(
            d[2],
            abi::fault::EVENT_FAULT,
            "attempt {attempt} did not fault: the out-of-bounds write was tolerated, or the \
             process exited some other way",
        );
        assert_ne!(
            d[1], 0,
            "attempt {attempt}'s fault message carried no tid: the supervisor cannot tell who \
             died",
        );
    }

    // And the fault sites are real addresses in real code, not zeroed placeholders. The
    // *equality* to the intended address is checked inside the confiner (which knows the
    // layout); this is the sanity check that the kernel filled both words at all.
    let mut sites = of_kind(&msgs, RPT_SITE);
    for attempt in 0..2u64 {
        let s = sites
            .next()
            .unwrap_or_else(|| panic!("attempt {attempt} produced no fault site"));
        assert_ne!(
            s[1], 0,
            "attempt {attempt}: the kernel reported no faulting pc"
        );
        assert_ne!(
            s[2], 0,
            "attempt {attempt}: the kernel reported no faulting address"
        );
    }
}

/// **The supervisor restarts the crashed C component, and the restarted component does real
/// work.**
///
/// The restart half of §26: two instances crash, and the third computes a checksum and a transform
/// in C into the shared grant and exits. The confiner checks that output against an independent
/// Rust implementation (`OUTPUT_CORRECT`), because "the C produced the right answer after two
/// crashes" is the claim. The clean exit must arrive as `EVENT_EXIT` and must **not** be restarted
/// (§26.3), which ends the run.
#[test_case]
fn the_supervisor_restarts_the_faulted_c_component_and_the_restart_does_real_work() {
    let msgs = run_seam();

    // Every attempt reached its C call, in order, including the two that came after a crash.
    // (No `Vec` here: this is the kernel, and there is no allocator in a test.)
    let mut ran = [u64::MAX; ATTEMPTS];
    let mut count = 0usize;
    for m in of_kind(&msgs, RPT_RAN) {
        if count < ATTEMPTS {
            ran[count] = m[1];
        }
        count += 1;
    }
    assert_eq!(
        count, ATTEMPTS,
        "expected {ATTEMPTS} instances of the C component to run, saw {count}: the supervisor \
         did not restart a crashed child, or restarted one it should have left finished",
    );
    for (i, &attempt) in ran.iter().enumerate() {
        assert_eq!(
            attempt, i as u64,
            "instance {i} reported itself as attempt {attempt}: the supervisor's restart policy \
             ran with the wrong state",
        );
    }

    // The honest attempt: confined like the others, and correct.
    let honest = of_kind(&msgs, RPT_VERDICT)
        .find(|v| v[1] == ATTEMPT_HONEST)
        .expect("the honest attempt produced no verdict");
    assert_eq!(
        honest[2],
        CONFINED_AND_CORRECT,
        "the restarted C component was reached but did not produce the right answer (output \
         correct: {}). A restart that revives a process which computes nothing is not a restart.",
        honest[2] & OUTPUT_CORRECT != 0,
    );

    // A clean exit reads as EXIT, which is what lets a userspace policy tell "finished" from
    // "crashed" without guessing. The honest attempt is the third child, and since the underrun
    // was added it is no longer the last, so its death is taken by position rather than by
    // `last()`.
    let honest_death = of_kind(&msgs, RPT_DEATH)
        .nth(2)
        .expect("no death messages at all");
    assert_eq!(
        honest_death[2],
        abi::fault::EVENT_EXIT,
        "the honest attempt exited cleanly, so its supervisor must see EXIT, not FAULT",
    );
}

/// **The underrun: a store a whole page below the grant** (an eighth outsider pass at the
/// confinement claim, claim 20). The cited bugs both aim upward, past the end; this one aims
/// down, the direction a negative index or a `size_t` underflow reaches for, at a page nothing in
/// the component maps. It must fault as a translation fault at exactly that address, with the
/// in-grant store landed and both witnesses untouched: the same receipt the overruns produce, for
/// the direction nothing had probed.
///
/// Falsification: unfalsified. Written by the eighth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn a_c_underrun_below_the_grant_faults_and_changes_nothing_outside_it() {
    let msgs = run_seam();

    let verdict = of_kind(&msgs, RPT_VERDICT)
        .find(|v| v[1] == 3)
        .expect("no verdict for the underrun attempt");
    assert_eq!(
        verdict[2], CONFINED,
        "the underrun attempt was not confined: bits {:#x} against {:#x}",
        verdict[2], CONFINED
    );

    // The fault deaths, in attempt order, are the overruns and the underrun (the honest attempt
    // exits cleanly), so the underrun's is the third.
    let underrun_death = of_kind(&msgs, RPT_DEATH)
        .filter(|d| d[2] == abi::fault::EVENT_FAULT)
        .nth(2)
        .expect("the underrun attempt produced no fault death");
    assert_ne!(
        underrun_death[1], 0,
        "the underrun's fault message carried no tid"
    );

    // The site: exactly one page below the grant, the address the C computed and nothing else.
    let underrun_site = of_kind(&msgs, RPT_SITE)
        .find(|s| s[2] == address_space_map::service_window(0x5000_0000) - page_frames::FRAME_SIZE)
        .expect("no fault site one page below the grant: the underrun faulted elsewhere");
    assert_ne!(underrun_site[1], 0, "no faulting pc for the underrun");
}

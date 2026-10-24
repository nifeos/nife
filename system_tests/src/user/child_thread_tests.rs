//! `child_thread_tests`: a process builds, starts and runs a child thread, reclaim frees the
//! child's regions, spawn-to-reap repeats without leaking, and a process is composed from two
//! capabilities.
//!
//! Name: provisional, minted 2026-10-12 (UTC) by the lane for milestone 873 (the catch-all
//! userspace test file is split by topic).

use super::*;
use crate::arch::timer;
use crate::sched;

/// **Milestone 19c.3, the whole point: one process builds and starts another, and it runs.**
/// The kernel drives the four verbs the way the progenitor eventually will: retype an address space and
/// a TCB, map a code page (containing a hand-assembled EL0 stub) and a stack into the space,
/// insert a report rendezvous into the child's capability table, configure the TCB (entry, stack, space),
/// and START it. The child, code no wiring wrote and a thread no `spawn` created, drops to
/// EL0, invokes the capability it was granted to SEND a word home, and exits. Receiving that
/// word proves every verb: the retype, the maps, the cap insert, the configure, the start,
/// and a real EL0 thread built from parts.
#[test_case]
fn a_process_can_build_start_and_run_a_child_thread() {
    const CODE_VA: u64 = address_space_map::IMAGE_BASE;
    const STACK_VA: u64 = address_space_map::STACK_TOP_PAGE;
    // The child's program: SEND(slot 0, rendezvous::SEND, REPORT_WORD) then EXIT, nine
    // instructions, with the child's first granted cap in slot 0: `supervision_tests`' pair
    // (aarch64 and RISC-V), so the tests below run on either machine.
    let code = super::supervision_tests::REPORT_STUB;
    let expect_word = super::supervision_tests::REPORT_WORD;

    // The child's address space, and a region to carve its code and stack frames from.
    let as_region = crate::memory_region::create(8).expect("no address space region");
    let aspace = user_address_space_create(as_region).expect("no aspace");
    let frames_region = crate::memory_region::create(2).expect("no frame region");

    let code_phys = code_page(frames_region, code);
    user_address_space_map(
        aspace,
        CODE_VA,
        code_phys,
        Flags::user_code(),
        crate::revoke::PageMapSource::NoCapability,
    )
    .expect("map code");

    let stack_phys = crate::memory_region::retype_page(frames_region).expect("no stack frame");
    user_address_space_map(
        aspace,
        STACK_VA,
        stack_phys,
        Flags::user_data(),
        crate::revoke::PageMapSource::NoCapability,
    )
    .expect("map stack");

    // The child's one authority: WRITE on a report rendezvous, so it can SEND but not receive.
    let endpoints = crate::memory_region::create(1).expect("no endpoint region");
    let report = crate::sched::create_rendezvous_from(endpoints).expect("no report rendezvous");
    let report_cap = crate::cap::rendezvous_cap(
        report,
        crate::cap::Rights::WRITE.union(crate::cap::Rights::GRANT),
    );

    // Build the thread from parts.
    let thread_control_block_region = crate::memory_region::create(2).expect("no tcb region");
    let tid =
        crate::sched::create_thread_control_block(thread_control_block_region).expect("no tcb");
    let slot =
        crate::sched::thread_control_block_insert_cap(tid, report_cap, None).expect("cap insert");
    assert_eq!(
        slot, 0,
        "the child's first cap must land in slot 0 (the code assumes it)"
    );

    // Not before it is whole: START must refuse an unconfigured embryo.
    assert!(
        crate::sched::start_thread_control_block(tid, [0; 3]).is_err(),
        "START ran a half-built thread (no address space, no entry)",
    );

    crate::sched::configure_thread_control_block(
        tid,
        CODE_VA,
        STACK_VA + page_frames::FRAME_SIZE,
        aspace,
    )
    .expect("configure");
    crate::sched::start_thread_control_block(tid, [0; 3]).expect("start");

    // And starting twice must refuse: it is no longer an embryo.
    assert!(
        crate::sched::start_thread_control_block(tid, [0; 3]).is_err(),
        "START ran a thread that was already running",
    );

    let got = crate::sched::ipc_receive(report)[0];
    assert_eq!(
        got, expect_word,
        "the child never reported: a built-from-parts thread did not reach EL0 and run",
    );
    crate::sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

/// **Object revocation, piece 3: a started thread and its bound address space are reclaimed
/// after it exits.** The child's code and stack come from the *same* region as its address space,
/// so one region holds its whole world (root, tables, code, stack) and its TCB is in another. Once
/// it has run, exited and been reaped, both regions reclaim *exactly*: the bound space died with
/// the thread (the `Drop` chain), leaving its region object-free for `reclaim_region`.
///
/// Never try to reclaim the TCB region while the child is live: a refused reclaim arms §16 (object
/// revocation)'s kill on its threads (§24 (interrupting the foreground process)), which can reap
/// the child before its `SEND`. `force_kill_tests` covers the refusal; see
/// notes/system-test-modules.md.
#[test_case]
fn reclaim_frees_a_started_then_exited_childs_regions() {
    const CODE_VA: u64 = address_space_map::IMAGE_BASE;
    const STACK_VA: u64 = address_space_map::STACK_TOP_PAGE;
    // SEND(slot 0, rendezvous::SEND, REPORT_WORD) then EXIT, the shared stub (see the test above).
    let code = super::supervision_tests::REPORT_STUB;
    let expect_word = super::supervision_tests::REPORT_WORD;

    // The report rendezvous lives in a region of its own, reclaimed last, so it is not part of what
    // this test accounts for: the accounting below is per-region, and the frames it spends cannot
    // be confused with the child's.
    let endpoints = crate::memory_region::create(1).expect("no endpoint region");
    let report = crate::sched::create_rendezvous_from(endpoints).expect("no report rendezvous");

    // The child's whole address space in one region: root, tables, code, and stack.
    let as_region = crate::memory_region::create(8).expect("no address space region");
    let aspace = user_address_space_create(as_region).expect("no aspace");

    let code_phys = code_page(as_region, code);
    user_address_space_map(
        aspace,
        CODE_VA,
        code_phys,
        Flags::user_code(),
        crate::revoke::PageMapSource::NoCapability,
    )
    .expect("map code");

    let stack_phys = crate::memory_region::retype_page(as_region).expect("no stack frame");
    user_address_space_map(
        aspace,
        STACK_VA,
        stack_phys,
        Flags::user_data(),
        crate::revoke::PageMapSource::NoCapability,
    )
    .expect("map stack");

    let report_cap = crate::cap::rendezvous_cap(
        report,
        crate::cap::Rights::WRITE.union(crate::cap::Rights::GRANT),
    );
    let thread_control_block_region = crate::memory_region::create(2).expect("no tcb region");
    // The two runs this test is responsible for, named while their regions are alive. A
    // machine-wide free-frame delta could not survive this window: the child runs on whichever
    // core §28 (SMP placement: two random choices at spawn) placed it on, the reaper frees its
    // address space from that core's switch, and a neighbouring test's own late teardown lands
    // here too. These ten frames cannot be moved by any of that. See `testing::RegionRun` and
    // notes/load-sensitive-assertions.md.
    let as_run = crate::testing::RegionRun::of(as_region);
    let tcb_run = crate::testing::RegionRun::of(thread_control_block_region);
    let tid =
        crate::sched::create_thread_control_block(thread_control_block_region).expect("no tcb");
    crate::sched::thread_control_block_insert_cap(tid, report_cap, None).expect("cap insert");
    crate::sched::configure_thread_control_block(
        tid,
        CODE_VA,
        STACK_VA + page_frames::FRAME_SIZE,
        aspace,
    )
    .expect("configure");
    crate::sched::start_thread_control_block(tid, [0; 3]).expect("start");

    // **Nothing may touch `thread_control_block_region` between here and the child's report.** A refused
    // `reclaim_region` arms §16's kill on the child and dooms it; see this test's own note above.

    // Let it run: it SENDs the word and exits. Receiving proves it reached EL0.
    let got = crate::sched::ipc_receive(report)[0];
    assert_eq!(got, expect_word, "the child never reported");

    // Wait on the clock for this child's ThreadId to go. A Finished thread is removed when its
    // own core switches away from it, and DECISIONS §28's placement can put it on ANOTHER core,
    // so yields on THIS core cannot hurry it. A child never reaped still times out and fails. A
    // thread-table headcount would also wait on every other test's teardown in flight.
    assert!(
        wait_for(|| !crate::sched::is_thread_present(tid)),
        "the exited child was never reaped",
    );

    // Both regions reclaim now: the TCB's, and the address space's (its bound space died with
    // the thread, so the region is object-free, needing only unpin and free).
    crate::sched::reclaim_region(thread_control_block_region)
        .expect("reclaim the TCB region after exit");
    crate::sched::reclaim_region(as_region).expect("reclaim the address-space region after exit");

    tcb_run.assert_returned("the child's TCB region did not give its frames back");
    as_run.assert_returned("the child's address-space region did not give its frames back");
    crate::sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

/// **Spawn-to-reap repeats without leaking: the whole milestone's payoff.** Build, start, run,
/// exit, reap, and reclaim a region-backed EL0 child, in a loop. Every iteration returns the
/// free-frame count to the same baseline, and the region slots are reused (generational), so the
/// loop neither leaks memory nor exhausts the region table. This is the property "spawn's
/// prerequisite" was always about: not retype (that had shipped), but reclamation, so a workload
/// can come and go over and over. A few iterations under TCG is enough to catch any per-cycle
/// leak; the real magnitudes wait on the EL0 `lat_proc` benchmark.
#[test_case]
fn spawn_to_reap_repeats_without_leaking() {
    const CODE_VA: u64 = address_space_map::IMAGE_BASE;
    const STACK_VA: u64 = address_space_map::STACK_TOP_PAGE;
    let code = super::supervision_tests::REPORT_STUB;
    let expect_word = super::supervision_tests::REPORT_WORD;

    let endpoints = crate::memory_region::create(1).expect("no endpoint region");
    let report = crate::sched::create_rendezvous_from(endpoints).expect("no report rendezvous");

    for round in 0..6 {
        let as_region = crate::memory_region::create(8).expect("address space region");
        let aspace = user_address_space_create(as_region).expect("aspace");
        let code_phys = code_page(as_region, code);
        user_address_space_map(
            aspace,
            CODE_VA,
            code_phys,
            Flags::user_code(),
            crate::revoke::PageMapSource::NoCapability,
        )
        .expect("map code");
        let stack_phys = crate::memory_region::retype_page(as_region).expect("stack frame");
        user_address_space_map(
            aspace,
            STACK_VA,
            stack_phys,
            Flags::user_data(),
            crate::revoke::PageMapSource::NoCapability,
        )
        .expect("map stack");

        let report_cap = crate::cap::rendezvous_cap(
            report,
            crate::cap::Rights::WRITE.union(crate::cap::Rights::GRANT),
        );
        let thread_control_block_region = crate::memory_region::create(2).expect("tcb region");
        // This round's own ten frames, by name. The leak this test hunts is a per-cycle one, so the
        // question is per round; the machine's free total can be moved by any other core in a
        // window that holds a child running, a remote reap and two reclaims.
        let as_run = crate::testing::RegionRun::of(as_region);
        let tcb_run = crate::testing::RegionRun::of(thread_control_block_region);
        let tid =
            crate::sched::create_thread_control_block(thread_control_block_region).expect("tcb");
        crate::sched::thread_control_block_insert_cap(tid, report_cap, None).expect("cap insert");
        crate::sched::configure_thread_control_block(
            tid,
            CODE_VA,
            STACK_VA + page_frames::FRAME_SIZE,
            aspace,
        )
        .expect("configure");
        crate::sched::start_thread_control_block(tid, [0; 3]).expect("start");

        assert_eq!(
            crate::sched::ipc_receive(report)[0],
            expect_word,
            "round {round}: the child never reported"
        );
        // On the clock, not on yields, and on THIS child rather than on a headcount, both for
        // the reasons spelled out in the test above: §28 can place the child on another core and
        // only that core's switch reaps it, and `thread_count()` is the size of the whole table,
        // so waiting for it to return to a baseline is waiting for the rest of the system.
        // A lagging reap here would surface as the reclaim below refusing a live thread.
        assert!(
            wait_for(|| !crate::sched::is_thread_present(tid)),
            "round {round}: the child was never reaped",
        );
        crate::sched::reclaim_region(thread_control_block_region).expect("reclaim tcb region");
        crate::sched::reclaim_region(as_region).expect("reclaim address space region");

        // The frame's address is in the failure, which is what says which round leaked; the
        // whole-boot half of "nothing leaked anywhere" is the frame ledger's job
        // (`testing::report_page_frame_ledger`), measured where it is not load-sensitive.
        tcb_run.assert_returned("spawn-to-reap leaked the cycle's TCB region");
        as_run.assert_returned("spawn-to-reap leaked the cycle's address-space region");
    }
    crate::sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

/// **A process composed from two capabilities runs in the space it was built in** (milestone 19b
/// (run a real workload), extended by §185 (what carries the claim that userspace composes a process
/// from an authority you can count on one hand)).
///
/// The witness holds a memory region and a report line. It reads a child out of the archive, mints
/// a rendezvous, builds the child's space, maps a frame into it (refused twice at one address) and
/// starts it. Each bit is one verb the kernel allowed; `CHILD_ANSWERED` says a thread a process
/// built ran in the space it built. The wait has a deadline, for `c_seam_tests::wait_for_report`'s.
///
/// Falsification: replayable `system_tests/falsifications/user.child_thread_tests.a_process_composed_from_two_capabilities_runs_in_the_space_it_built.patch`
#[cfg(initial_ramdisk)]
#[test_case]
fn a_process_composed_from_two_capabilities_runs_in_the_space_it_built() {
    use capability_witness_contract::process_composition as bits;
    let witness = process_composition_service::wire();
    let report = witness.report;
    // Each predicate is asked once per pass and its first `true` is the answer: the reclaim below
    // is a predicate with a side effect, and asking it again after it succeeded would ask about a
    // region that no longer exists.
    let within_30_s = |done: &mut dyn FnMut() -> bool| {
        let deadline = timer::now() + 30 * timer::frequency();
        loop {
            if done() {
                return true;
            }
            if timer::now() >= deadline {
                return false;
            }
            sched::yield_now();
        }
    };
    assert!(
        within_30_s(&mut || sched::rendezvous_waiting_senders(report) > 0),
        "the witness sent no verdict in 30 s: it is still waiting for a child it started, so the \
         thread it built never ran or never answered on the capability it was given",
    );
    let verdict = sched::ipc_receive(report)[0];

    // Hand the memory back before judging, so a wrong verdict does not also read as a leak in the
    // frame ledger. The witness exits after its send; its space is bound to it and goes with it.
    // The child exited before the witness sent anything, so the budget it was built from is free
    // once the reaper has it; the retry is for that reap, which may land on another core.
    assert!(
        within_30_s(&mut || !sched::is_thread_present(witness.tid)),
        "the witness sent its verdict and was never reaped",
    );
    assert!(
        within_30_s(&mut || sched::reclaim_region(witness.budget).is_ok()),
        "the witness's budget would not reclaim: something it built is still alive",
    );
    sched::reclaim_region(witness.thread_control_block_region)
        .expect("reclaim the witness's thread control block region");
    sched::reclaim_region(witness.report_region).expect("reclaim the witness's report line");

    assert_eq!(
        verdict,
        bits::WHOLE,
        "process composition verdict {verdict:#08b}: bit0 child read from the archive, bit1 space \
         built and child laid into it, bit2 own frame mapped into it, bit3 double map refused, bit4 \
         configured and started, bit5 the child answered",
    );
}

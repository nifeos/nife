use super::test_programs::spawn_progenitor_shaped;
use crate::sched;

/// The report protocol, matching user/src/suptree.rs (the same convention the net client's
/// selectors follow: userspace owns the definition, the test mirrors it).
const REPORT_INIT_DROPPED: u64 = 1;
const REPORT_SERVER_RAN: u64 = 2;
const REPORT_SUP_SAW_DEATH: u64 = 3;
const REPORT_SUP_GAVE_UP: u64 = 4;
const REPORT_FAILED: u64 = 9;
/// `sub_server_supervisor`'s two children: the label it gives each, and the attempt each starts
/// at. Mirrors `supervision_protocol::SUB_SERVER_LABELS` (milestone 105 (the two forks)). The first starts at the
/// attempt `flaky` crashes on and the second at one it finishes, so the two must be told apart for
/// the right one to be restarted.
const LABEL_CRASHES: u64 = 0x1abe_0a01;
const LABEL_FINISHES: u64 = 0x1abe_0b02;
const FIRST_ATTEMPT_FINISHES: u64 = 10;

/// Pages in `root_supervisor`'s construction budget. It builds two servers out of this, splits the
/// spawner's budget from it, and then deletes it; the spawner's split is the only memory the tree
/// spends afterwards.
const ROOT_BUDGET_PAGES: u64 = 1024;

/// **Spawn the tree's root the way the kernel spawns the progenitor**, and return the report endpoint every
/// process in the tree holds a WRITE view of.
///
/// Deliberately the same endowment `spawn_elf_child_builder` gives (`INITIAL_RAMDISK_VA`, an
/// untyped in slot 0, a report endpoint in slot 1) so what is being tested is `root_supervisor`'s
/// *choices*, not a privileged shortcut.
fn spawn_tree() -> (sched::RendezvousId, u64) {
    // From a region the run owns and reclaims, as `live_swap_tests::spawn_swapper` does.
    let report_region = crate::memory_region::create(1).expect("no region for the report endpoint");
    let report = sched::create_rendezvous_from(report_region).expect("no report rendezvous");
    spawn_progenitor_shaped(
        "root_supervisor",
        ROOT_BUDGET_PAGES,
        crate::cap::Rights::ALL,
        report,
        crate::cap::Rights::WRITE.union(crate::cap::Rights::GRANT),
        &[],
        None,
        0,
    );
    (report, report_region)
}

/// How many reports a healthy run of the tree makes: the progenitor's drop; the crashing child's first
/// instance running, its crash reaching the supervisor, its replacement running, and the
/// replacement's clean exit reaching the supervisor; and the finishing child running once and its
/// clean exit reaching the supervisor. Exactly seven, which is itself an assertion: an eighth would
/// mean the supervisor restarted something it should have left finished, or a tier-one server died.
const EXPECTED_REPORTS: usize = 7;

/// **Run one tree from spawn to quiescence**, returning every report it made.
///
/// Runs it to the end rather than stopping at the first interesting message, for a reason worth
/// recording: a half-run tree keeps building processes in the background, and the first version of
/// these tests left one running, which broke a *later* test's thread accounting
/// (`destroy_force_kills_a_runaway` counts threads before and after). A test that leaves work
/// running is a test that fails somebody else.
///
/// The order of the seven is not fixed (the progenitor's drop races the sub-server's first run), so callers
/// filter by kind; within a kind the order is causal and asserted.
fn run_tree() -> [[u64; 5]; EXPECTED_REPORTS] {
    let (report, report_region) = spawn_tree();
    let mut msgs = [[0u64; 5]; EXPECTED_REPORTS];
    for slot in msgs.iter_mut() {
        let msg = sched::ipc_receive(report);
        assert_ne!(
            msg[0], REPORT_FAILED,
            "the supervision tree could not be built: stage {}",
            msg[1]
        );
        assert_ne!(
            msg[0], REPORT_SUP_GAVE_UP,
            "the supervisor exhausted its retry budget ({} restarts): the replacement should \
             have survived",
            msg[1],
        );
        *slot = msg;
    }

    // Let the tree settle, then prove it has nothing more to say. A parked sender here means a
    // sixth report exists, which is how "the supervisor did not restart a finished server" is
    // proven without a blocking receive that would hang when the code is right.
    for _ in 0..400 {
        sched::yield_now();
    }
    assert_eq!(
        sched::rendezvous_waiting_senders(report),
        0,
        "the tree made more than {EXPECTED_REPORTS} reports: something acted after the \
         sub-server finished",
    );
    sched::reclaim_region(report_region).expect("the report endpoint's region did not come back");
    msgs
}

/// Every report of one kind, in arrival order.
fn of_kind(msgs: &[[u64; 5]; EXPECTED_REPORTS], kind: u64) -> impl Iterator<Item = &[u64; 5]> {
    msgs.iter().filter(move |m| m[0] == kind)
}

/// **The progenitor drops its construction authority, and the drop is real.**
///
/// `root_supervisor` builds its two servers, deletes the wiring capabilities and then the untyped budget
/// itself, and immediately tries the two primitives that build things: retype a page, and retype a
/// kernel object. Both must fail, and they must fail with `NoSuchSlot` (there is nothing there)
/// rather than `NotPermitted` (there is something there and you may not use it), because the
/// capability is *gone*, not narrowed. That distinction is the whole difference between "we asked
/// the progenitor not to" and "progenitor cannot."
///
/// It is reported from inside the process on purpose: what matters is what the *holder* can do,
/// and only the holder can ask.
///
/// Falsification: replayable `system_tests/falsifications/user.authority_tests.the_root_supervisor_drops_its_construction_authority_and_cannot_build_again.patch`
#[test_case]
fn the_root_supervisor_drops_its_construction_authority_and_cannot_build_again() {
    let msgs = run_tree();
    let dropped = of_kind(&msgs, REPORT_INIT_DROPPED)
        .next()
        .expect("progenitor never reported dropping its budget");
    assert_eq!(
        dropped[1], 1,
        "progenitor still built a page or a kernel object after deleting its untyped: the authority \
         was not actually dropped",
    );
    assert_eq!(
        dropped[2], 1,
        "using the dropped budget failed with error {} (negated), not NoSuchSlot: the slot \
         should be empty, not merely restricted",
        dropped[2],
    );
}

/// **A dead sub-server is restarted by its own supervisor, in userspace, and the progenitor cannot have
/// helped.**
///
/// The sequence, for the child the supervisor starts at attempt 0: it crashes on a load from an
/// unmapped address; its supervisor receives the kernel's fault message, reaps the corpse (§32 (a supervisor may collect a corpse)), and
/// asks for attempt 1; attempt 1 runs and exits cleanly; the supervisor reads EXIT as "finished" and
/// does **not** restart it again. Every decision in that paragraph is code in an unprivileged
/// process that holds no memory at all, and the kernel's whole contribution is one message per death.
///
/// **How "without the progenitor's involvement" is proven, and why it is not a timing argument.** The progenitor has
/// no construction authority by then: it deleted its untyped, and the companion test above
/// confirms it can no longer use it. A process that cannot retype a page cannot have built the
/// replacement. Authority, not scheduling order, is the evidence.
///
/// **It is also where the real tree proves labels** (milestone 105). `sub_server_supervisor` has two
/// children on one endpoint, identical programs told nothing but an attempt number; one crashes and
/// one finishes, and it restarts by label alone. The check lives here rather than in a test of its
/// own because every `run_tree` leaves a parked tree of about 1,100 frames for the rest of the suite,
/// and a third would put the suite over `SUITE_PAGE_FRAME_BUDGET`, which CI measured on 2026-10-05.
/// Dropping the label fails this in `run_tree` (stage 23), not at an assertion below, so the kernel
/// claim's falsification lives in `supervision_tests::a_supervisor_tells_two_dead_children_apart_by_label`.
///
/// Falsification: unfalsified. Renamed by milestone 405 (the nine `INIT` roles), which ran it green
/// on all three architectures but did not break the code it guards to watch it go red.
#[test_case]
fn a_dead_sub_server_is_restarted_by_its_supervisor_not_by_the_root() {
    let msgs = run_tree();

    let attempts = |n: u64| {
        of_kind(&msgs, REPORT_SERVER_RAN)
            .filter(|m| m[1] == n)
            .count()
    };
    assert_eq!(
        attempts(0),
        1,
        "the crashing child's first instance should run once"
    );
    assert_eq!(
        attempts(1),
        1,
        "the crashed sub-server was not restarted as attempt 1 exactly once: the supervisor's \
         restart policy did not run, or ran with the wrong state",
    );
    assert_eq!(
        of_kind(&msgs, REPORT_SERVER_RAN).count(),
        3,
        "an unexpected instance ran: the supervisor restarted a server that had finished",
    );

    let mut crashing = of_kind(&msgs, REPORT_SUP_SAW_DEATH).filter(|m| m[1] == LABEL_CRASHES);
    let crash = crashing.next().expect("the supervisor saw no death");
    assert_eq!(
        crash[2],
        abi::fault::EVENT_FAULT,
        "the crash should reach the supervisor as a FAULT event",
    );
    // The other half of §26's "both events flow": a clean exit must arrive as EXIT, because that
    // is what lets a userspace policy tell "finished" from "crashed" without guessing.
    let finished = crashing
        .next()
        .expect("the replacement's clean exit never reached the supervisor");
    assert_eq!(
        finished[2],
        abi::fault::EVENT_EXIT,
        "attempt 1 exited cleanly, so the supervisor must see EXIT, not FAULT",
    );
    assert!(
        crashing.next().is_none(),
        "the crashing child died more than twice under its label",
    );

    // The supervisor told its two children apart by label alone (milestone 105, §148 (resolves by
    // asking the kernel) as amended). The finishing child, whose first attempt was 10, must have
    // run once and died once under its own label. A supervisor that confused the two would have
    // restarted it.
    let mut finishing = of_kind(&msgs, REPORT_SUP_SAW_DEATH).filter(|m| m[1] == LABEL_FINISHES);
    assert_eq!(
        finishing.next().map(|m| m[2]),
        Some(abi::fault::EVENT_EXIT),
        "the finishing child's clean exit did not arrive under its label",
    );
    assert!(
        finishing.next().is_none(),
        "the finishing child died more than once"
    );
    assert_eq!(
        of_kind(&msgs, REPORT_SERVER_RAN)
            .filter(|m| m[1] == FIRST_ATTEMPT_FINISHES)
            .count(),
        1,
        "the finishing child ran other than once: the supervisor confused it with the one that crashed",
    );
}

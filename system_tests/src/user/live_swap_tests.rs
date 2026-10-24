use super::test_programs::spawn_progenitor_shaped;
use super::*;
use crate::sched;

/// The report protocol, matching user/src/swap.rs. Userspace owns the definition; the test
/// mirrors it, the same convention `authority_tests` and `c_seam_tests` follow.
const RPT_UP: u64 = 1;
const RPT_QUIESCED: u64 = 2;
const RPT_PROBE_SURVIVED: u64 = 3;
const RPT_STEP: u64 = 4;
const RPT_LOG: u64 = 5;
const RPT_CLIENT: u64 = 6;
const RPT_ATTACK: u64 = 7;
const RPT_DEATH: u64 = 8;
const RPT_SITE: u64 = 9;
const RPT_DRAINED: u64 = 10;
const RPT_REFUSED: u64 = 11;
const RPT_SURVEY: u64 = 12;
const RPT_UNCOLLECTABLE: u64 = 13;
const RPT_UNANSWERED: u64 = 17;
const RPT_RECLAIMED: u64 = 18;
const RPT_DEPENDENTS: u64 = 15;
/// The confused-deputy attacker's out-of-log `OPERATION_PUT` result (milestone 633, third pass).
const RPT_DEPUTY: u64 = 16;
const RPT_FAILED: u64 = 99;

/// `capability_plan::Refusal::Unprovided`'s wire code: the supervisor routes nothing to a role the
/// component's manifest declares. Mirrored here for the same reason every other constant in this
/// block is: userspace owns the definition.
const REFUSAL_UNPROVIDED: u64 = 1;

/// The operator's steps.
const STEP_BUILT: u64 = 1;
const STEP_DRAINED: u64 = 2;
const STEP_REVOKED: u64 = 3;
const STEP_STARTED: u64 = 4;
const STEP_ABSORBED: u64 = 6;
const STEP_ROLLED_BACK: u64 = 7;

/// The operator's verdict bits (`swap::log_checks`).
const LOG_NO_GAP: u64 = 1 << 0;
const LOG_MONOTONE: u64 = 1 << 1;
const LOG_BOTH_VERSIONS: u64 = 1 << 2;
const LOG_REVOKE_ENFORCED: u64 = 1 << 3;

/// The client's verdict bits (`swap::client_checks`).
const CL_ALL_REPLIED: u64 = 1 << 0;
const CL_SEQ_ECHOED: u64 = 1 << 1;
const CL_DIGEST_CORRECT: u64 = 1 << 2;
const CL_ONE_TRANSITION: u64 = 1 << 3;
const CL_SPANNED_SWAP: u64 = 1 << 4;
const CL_WAS_BUFFERED: u64 = 1 << 5;
const CL_NONE_REFUSED: u64 = 1 << 6;
const CL_WAS_GONE: u64 = 1 << 7;

/// The roles, and the two versions.
const ROLE_DIRECT: u64 = 0;
const ROLE_QUEUED: u64 = 1;
const ROLE_HUNG: u64 = 2;
const ROLE_HANDOFF: u64 = 3;
const ROLE_UNWARNED: u64 = 4;
const ROLE_LATE_WARNING: u64 = 5;
/// The confused-deputy channel (milestone 633, third pass).
const ROLE_DEPUTY: u64 = 6;
/// The hung channel with no device page to lend (`x86_64`).
const ROLE_HUNG_WITHOUT_DEVICE: u64 = 7;
/// The pages one instance's region holds (`hot_swap_contract::INSTANCE_PAGES`). The hung test asserts
/// the operator's budget got exactly one instance back, so both sides name one constant.
const INSTANCE_PAGES: u64 = 32;
/// The one blob layout any build in this tree writes (`hot_swap_contract::LAYOUT_1`). The refusing
/// replacement names it as the layout it could not read, so both sides name one constant.
const LAYOUT_1: u64 = 1;
const V1: u64 = 1;
const V2: u64 = 2;
const REQUESTS: u64 = 64;

/// The request a wedging instance swallows (`hot_swap_contract::WEDGE_SEQ`). The test asserts the swap
/// landed here, so both sides name one constant.
const WEDGE_SEQ: u64 = 24;

/// The device's virtual address in every component, matching `swap::DEV_VA`. The test asserts
/// the kernel's reported fault address against this, which is why both sides name one constant.
const DEV_VA: u64 = address_space_map::pair_page(0x0310_0000);

/// The console UART's physical address, matching `crate::console`: the device the operator lends,
/// takes back, and lends again. Taken from `user::UART_PHYS` (milestone 161 (the x86_64 kernel
/// port)), whose `x86_64` arm is zero: COM1 is in the I/O port space, so there is no page for a
/// device capability to map ([DECISIONS §121](../../../design/decisions/0121-port-io-capability.md)
/// (what a device capability is when the device has no page), PROPOSED). That zero is what
/// [`NO_DEVICE_PAGE`] tests for.
use crate::user::UART_PHYS;
/// This file needs a device that is a page, and `x86_64` has none.
///
/// The strongest assertion is that the outgoing instance faults inside [`DEV_VA`] after the revoke.
/// Lending physical page zero instead would still pass (a real-mode interrupt-vector byte, then a
/// fault): a passing test about nothing. `hot_swap_contract::probe_device`'s x86 arm refuses for
/// the same reason; `user::machine_has_no_device_page_for_the_console` is the one definition.
use crate::user::{NO_UART_PAGE, machine_has_no_device_page_for_the_console};

/// The operator's budget: five instance regions of forty pages plus its own scratch mappings and
/// their page tables.
///
/// Tight on purpose: `memory_region::create` takes a contiguous run, the suite runs several of
/// these systems beside a dozen tests parking an eight-megabyte progenitor region, and a generous
/// budget fragments the allocator until a later, unrelated test cannot get its region.
const SWAPPER_BUDGET_PAGES: u64 = 224;

/// How many reports one run can make before the test gives up waiting for the operator's final
/// verdict. Only a tripwire: the loop stops at `RPT_LOG`. A role that adds reports must raise it,
/// or the run loses the operator's verdict and fails for the wrong reason.
const MAX_REPORTS: usize = 42;

/// Spawn the operator the way the kernel spawns the progenitor, and return the report rendezvous
/// every process in the run holds a WRITE view of.
///
/// The endowment is `spawn_elf_child_builder`'s (the archive read-only at `INITIAL_RAMDISK_VA`, an
/// untyped in slot 0, a report rendezvous in slot 1) plus a `WRITE|GRANT` device capability in slot
/// 2, as the progenitor gets at boot, so what is tested is the operator's choices.
fn spawn_swapper(role: u64) -> (sched::RendezvousId, u64, u64, u64) {
    // From a region this run reclaims, not the kernel's pool, which grows in 32-page chunks it
    // never frees (the BUGS on `testing::SUITE_PAGE_FRAME_BUDGET`). It holds the report rendezvous
    // and the notification and timer that bound the operator's un-park of the plant.
    let report_region = crate::memory_region::create(4).expect("no region for the report endpoint");
    let report = sched::create_rendezvous_from(report_region).expect("no report rendezvous");
    let wake = sched::create_notification_from(report_region).expect("no notification for swapper");
    let retransmit = sched::create_timer_from(report_region).expect("no timer for swapper");
    let started = spawn_progenitor_shaped(
        "swapper",
        SWAPPER_BUDGET_PAGES,
        crate::cap::Rights::ALL,
        report,
        crate::cap::Rights::WRITE.union(crate::cap::Rights::GRANT),
        &[
            (
                crate::cap::device_frame_cap(
                    UART_PHYS,
                    crate::cap::Rights::WRITE.union(crate::cap::Rights::GRANT),
                ),
                "device capability",
            ),
            (
                crate::cap::notification_cap(wake, crate::cap::Rights::ALL),
                "notification",
            ),
            (
                crate::cap::timer_cap(retransmit, crate::cap::Rights::ALL),
                "timer",
            ),
        ],
        Some(wake),
        role,
    );
    (
        report,
        started.budget,
        started.thread_control_block_region,
        report_region,
    )
}

/// **Run one swap system to its own verdict**, returning every report it made.
///
/// Run to the end rather than stopping at the first interesting message, for the reason
/// `authority_tests::run_tree` records: a half-run system keeps building processes in the
/// background, and a test that leaves work running is a test that fails somebody else. The
/// operator's `RPT_LOG` is always its last word, so that is the stop condition.
fn run_swap(role: u64) -> ([[u64; 5]; MAX_REPORTS], usize) {
    let (report, budget, thread_control_block_region, report_region) = spawn_swapper(role);
    let mut msgs = [[0u64; 5]; MAX_REPORTS];
    let mut n = 0;
    while n < MAX_REPORTS {
        let msg = sched::ipc_receive(report);
        assert_ne!(
            msg[0], RPT_FAILED,
            "the swap system could not be built: stage {}. Stages 1-4 are the archive and the \
             four program images, 5-10 the endpoints and the witness page, 11-16 the incumbent \
             and the client, 20-27 the swap itself, 30-37 the attacker and its plant (30 the \
             plant, 31 the attacker, 33 the note it waits on, 34 the retire, 35-36 the plant's \
             drain, 37 the slow-plant bail: the plant had not sent when the bound fired on an \
             honest kernel), \
             40-51 the queued rung, \
             60-63 the component manifests (60 means an unsatisfiable declaration was WIRED), \
             64 the dependency graph query (more live instances than MAX_LIVE), 70-87 and 110-119 \
             the hung-component rung (81 means the incumbent answered the bounded quiesce, so \
             it never hung; 117 and 119 a region DESTROY that never succeeded), 90-108 state handoff (92 means a \
             stateful component was wired by a supervisor with no handoff page to give it, 100 \
             means the refusing replacement did not refuse, 106 means the absorbing one did not \
             absorb).",
            msg[1],
        );
        assert_ne!(
            msg[0], RPT_PROBE_SURVIVED,
            "the outgoing instance read the device AFTER the operator revoked it and the read \
             succeeded: the revoke did not take, so there was a window with two owners of one \
             device's registers",
        );
        msgs[n] = msg;
        n += 1;
        if msg[0] == RPT_LOG {
            break;
        }
    }
    assert!(
        n < MAX_REPORTS,
        "the operator never reached its final verdict in {MAX_REPORTS} reports",
    );

    // A parked sender after settling would mean something acted after the operator finished,
    // proved without a blocking receive that would hang when the code is right.
    for _ in 0..400 {
        sched::yield_now();
    }
    assert_eq!(
        sched::rendezvous_waiting_senders(report),
        0,
        "the swap system had more to say after the operator's final verdict",
    );

    // The system reclaims itself, asserted (notes/live-replacement.md, "The system reclaims
    // itself"): §16 refuses a region with children carved out of it, so the reclaim succeeds only
    // if every collected instance (§32) came home. `sched::reclaim_region`, the `DESTROY` syscall's
    // entry point, because the budget is pinned by the objects retyped from it.
    let (_, region_size) =
        crate::memory_region::usage(budget).expect("the operator's budget should exist");
    assert_eq!(
        region_size, SWAPPER_BUDGET_PAGES,
        "the operator's budget should be exactly {SWAPPER_BUDGET_PAGES} pages"
    );
    sched::reclaim_region(budget).expect(
        "the operator's budget would not reclaim: a child region is still carved out of it, so \
         the swap system leaked one of its components",
    );
    // A stale name is the measurement: under §16 (object revocation: reclaim the objects a process
    // built) it means every child was gone and all `SWAPPER_BUDGET_PAGES` went back, and nothing
    // outside this region can perturb it, as it could a global free-frame count.
    assert!(
        crate::memory_region::usage(budget).is_none(),
        "the operator's budget still exists after reclaim, so its {SWAPPER_BUDGET_PAGES} pages did \
         not go back to the parent",
    );

    // Hygiene, not asserted: the operator's own space and TCB come home through the reaper, which
    // runs when the core it died on next schedules (DECISIONS §28 (SMP placement)), and a
    // free-frame assertion here flakes on other tests' teardowns (notes/live-replacement.md's
    // BUGS).
    let _ = sched::reclaim_region(thread_control_block_region);
    // Last, after every thread that held the report endpoint is gone or going. Hygiene like the
    // line above, for the same reason it is not asserted on.
    let _ = sched::reclaim_region(report_region);
    (msgs, n)
}

/// Every report of one kind, in arrival order.
fn of_kind(msgs: &[[u64; 5]], kind: u64) -> impl Iterator<Item = &[u64; 5]> {
    msgs.iter().filter(move |m| m[0] == kind)
}

/// Did the operator report this step?
fn had_step(msgs: &[[u64; 5]], step: u64) -> bool {
    of_kind(msgs, RPT_STEP).any(|m| m[1] == step)
}

/// A component this operator cannot provide for is refused before anything is built (milestone
/// 23's component manifest; `crates/capability_plan`, notes/component-manifest.md).
///
/// On both channels, each with the other role's real manifest: the direct channel routes neither
/// of the broker's `requests` and `backend`, the queued one no `uart`. The refusal must be the typed
/// one (a role went unrouted) and arrive ahead of every build step and started instance, so a
/// manifest is a request the supervisor may refuse, not an instruction carried out half way.
fn a_component_the_operator_cannot_provide_for_was_refused_first(msgs: &[[u64; 5]]) {
    let at = msgs.iter().position(|m| m[0] == RPT_REFUSED).expect(
        "the operator never reported a refusal, so nothing shows that an unsatisfiable \
             manifest is refused rather than wired",
    );
    assert_eq!(
        msgs[at][1], REFUSAL_UNPROVIDED,
        "the manifest was refused for the wrong reason (code {}, wanted Unprovided={}): the \
         refusal has to be \"this supervisor routes nothing to a role it declares\" or it is not \
         evidence about routing at all",
        msgs[at][1], REFUSAL_UNPROVIDED,
    );
    let first_build = msgs
        .iter()
        .position(|m| m[0] == RPT_STEP || m[0] == RPT_UP)
        .unwrap_or(usize::MAX);
    assert!(
        at < first_build,
        "the refusal arrived at report {at}, after the first build at report {first_build}: a \
         component that cannot be provided for must be refused before any authority has moved",
    );
}

/// The dependency graph a supervisor would compute agrees with what this channel ran (milestone
/// 23's dependency-aware-orchestration residual; `crates/capability_plan`'s `dependents`).
///
/// One `RPT_DEPENDENTS` per swap target: `w1` how many live instances `dependents` returned, `w2`
/// the first one's id (0 if none). It is the graph's verdict from `Requirements::depends_on` before
/// any step runs, so a mismatch means the graph and the hand-written sequencing drifted apart.
fn the_dependency_graph_matches_what_this_channel_ran(
    msgs: &[[u64; 5]],
    want_len: u64,
    want_id: u64,
) {
    let dep = of_kind(msgs, RPT_DEPENDENTS)
        .next()
        .expect("the operator never reported the dependency graph's verdict");
    assert_eq!(
        dep[1], want_len,
        "the dependency graph named {} live instances that must be warned before this swap, \
         wanted {want_len}",
        dep[1],
    );
    assert_eq!(
        dep[2], want_id,
        "the dependency graph's first named instance was {}, wanted {want_id}",
        dep[2],
    );
}

/// A component is replaced under a client that is talking to it. See notes/live-replacement.md.
///
/// The four steps happen in an order the operator chose, then two witnesses in two address spaces
/// agree the conversation was unbroken. Neither is consulted about the other's half.
#[test_case]
fn a_client_keeps_talking_while_the_server_underneath_it_is_replaced() {
    if machine_has_no_device_page_for_the_console() {
        crate::testing::skip!(NO_UART_PAGE);
    }
    let (msgs, n) = run_swap(ROLE_DIRECT);
    let msgs = &msgs[..n];

    // Before the four steps: every component in this run was wired from its own declaration, and one
    // that this channel cannot provide for was refused with nothing built.
    a_component_the_operator_cannot_provide_for_was_refused_first(msgs);

    // Nothing on this channel needs warning before the console is swapped: `CLIENT` is a pure
    // consumer (no `Serve` need), so §41's sender-queue argument already covers it and the graph's
    // own answer is a real empty set, not an untested one.
    the_dependency_graph_matches_what_this_channel_ran(msgs, 0, 0);

    // The four steps, each on machinery that existed before this milestone.
    for (step, what) in [
        (STEP_BUILT, "build the replacement"),
        (STEP_DRAINED, "drain the incumbent"),
        (STEP_REVOKED, "revoke the device"),
        (STEP_STARTED, "start the replacement"),
    ] {
        assert!(
            had_step(msgs, step),
            "the operator never got as far as: {what}",
        );
    }

    // Both instances ran, and both could reach the device they were endowed with. The second
    // half matters as much as the first: an instance that answered every request while the
    // registers went unowned would look like a perfect swap.
    let mut ups = of_kind(msgs, RPT_UP);
    let first = ups.next().expect("the incumbent never started");
    let second = ups.next().expect("the replacement never started");
    assert!(ups.next().is_none(), "a third instance started");
    assert_eq!(first[1], V1, "the incumbent should be version 1");
    assert_eq!(second[1], V2, "the replacement should be version 2");
    assert!(
        first[2] == 1 && second[2] == 1,
        "an instance could not read the device it was endowed with, so the registers were not \
         where the swap thinks they were",
    );

    // The incumbent served a real share of the conversation before it was drained: a swap that
    // happened before anyone was talking would prove nothing.
    let quiesced = of_kind(msgs, RPT_QUIESCED)
        .next()
        .expect("the incumbent never acknowledged the drain");
    assert!(
        quiesced[2] > 0 && quiesced[2] < REQUESTS,
        "the incumbent served {} of {REQUESTS} requests: the swap did not land inside the \
         conversation",
        quiesced[2],
    );

    // Witness one: the client, from its own replies, in its own address space.
    let client = of_kind(msgs, RPT_CLIENT)
        .next()
        .expect("the client never reported a verdict");
    const CLIENT_UNBROKEN: u64 =
        CL_ALL_REPLIED | CL_SEQ_ECHOED | CL_DIGEST_CORRECT | CL_ONE_TRANSITION | CL_SPANNED_SWAP;
    assert_eq!(
        client[1] & CLIENT_UNBROKEN,
        CLIENT_UNBROKEN,
        "the client's stream was broken (verdict {:#x}): missing {:#x}. ALL_REPLIED={}, \
         SEQ_ECHOED={}, DIGEST_CORRECT={}, ONE_TRANSITION={}, SPANNED_SWAP={}",
        client[1],
        CLIENT_UNBROKEN & !client[1],
        client[1] & CL_ALL_REPLIED != 0,
        client[1] & CL_SEQ_ECHOED != 0,
        client[1] & CL_DIGEST_CORRECT != 0,
        client[1] & CL_ONE_TRANSITION != 0,
        client[1] & CL_SPANNED_SWAP != 0,
    );

    // Witness two: the operator, from the shared page, after every writer is dead.
    let log = of_kind(msgs, RPT_LOG)
        .next()
        .expect("the operator never reported its verdict");
    const LOG_CLEAN: u64 = LOG_NO_GAP | LOG_MONOTONE | LOG_BOTH_VERSIONS | LOG_REVOKE_ENFORCED;
    assert_eq!(
        log[1] & LOG_CLEAN,
        LOG_CLEAN,
        "the operator's log says the swap was not clean (verdict {:#x}): NO_GAP={} (a request \
         nobody served), MONOTONE={} (the old instance answered after the new one, so two \
         owners), BOTH_VERSIONS={}, REVOKE_ENFORCED={} (the post-revoke device read did not \
         fault where it should have)",
        log[1],
        log[1] & LOG_NO_GAP != 0,
        log[1] & LOG_MONOTONE != 0,
        log[1] & LOG_BOTH_VERSIONS != 0,
        log[1] & LOG_REVOKE_ENFORCED != 0,
    );
    // The two witnesses agree on *where* the swap happened, which is the cross-check that makes
    // each of them evidence rather than a self-report.
    assert_eq!(
        log[2], client[2],
        "the operator's log and the client's replies disagree about which request the \
         replacement took over at ({} vs {})",
        log[2], client[2],
    );

    // The control: the outgoing instance died faulting on the device it no longer had, at the
    // device's own virtual address. `run_swap` has already refused a run in which that read
    // succeeded.
    let death = of_kind(msgs, RPT_DEATH)
        .next()
        .expect("the outgoing instance never died");
    assert_eq!(
        death[2],
        abi::fault::EVENT_FAULT,
        "the outgoing instance should have faulted on the revoked device, not exited cleanly",
    );
    let site = of_kind(msgs, RPT_SITE)
        .next()
        .expect("no fault site was reported");
    assert_eq!(
        site[1] & !(FRAME_SIZE - 1),
        DEV_VA,
        "the outgoing instance faulted at {:#x}, which is not in the device page {DEV_VA:#x}: \
         it died of something other than the revoke, which would make the rest of this test \
         vacuous",
        site[1],
    );
}

/// The attacker holds a real capability to the stable rendezvous and still cannot be the server.
///
/// `SEND` and `RECEIVE` take different rights on one object, so a rendezvous handed out two ways is
/// a one-way pipe; the attacker holds exactly the client's endowment, so the refusal is rights, not
/// wiring. A plant makes it fail rather than hang (milestone 800 (a non-Anthropic model attacks the
/// confinement claim), fourth pass): with the replacement retired, a `swap_boundary_witness` parks
/// a message that a let-open `RECEIVE_CAP` returns as the report's marker word; the operator drains
/// it otherwise. Claim 26's record, beside its kernel-side twin in `confinement_attack_tests`.
///
/// Falsification: replayable `system_tests/falsifications/user.live_swap_tests.a_client_of_the_stable_rendezvous_cannot_become_its_server.patch`
#[test_case]
fn a_client_of_the_stable_rendezvous_cannot_become_its_server() {
    if machine_has_no_device_page_for_the_console() {
        crate::testing::skip!(NO_UART_PAGE);
    }
    let (msgs, n) = run_swap(ROLE_DIRECT);
    let attack = of_kind(&msgs[..n], RPT_ATTACK)
        .next()
        .expect("the attacker never reported");
    assert_eq!(
        attack[1],
        (-(abi::Error::NotPermitted as i64)) as u64,
        "a client of the stable rendezvous received on it (or was refused for the wrong reason): \
         error {}, wanted NotPermitted. If this succeeded, any holder of a request capability \
         could impersonate the component.",
        attack[1] as i64,
    );
}

/// A confined client cannot drive the server to write past its log, into a device page it was
/// never granted (milestone 633 (an outside agent attacks the confinement claim), third pass).
///
/// Claim 26's other half: the server's `OPERATION_PUT` writes at `LOG_VA + log_base + arg`, `arg`
/// being the client's word, so an unbounded `arg` reaches the device page the server holds, a
/// confused deputy rather than a kernel gate. `hot_swap_contract::log_put` refuses an offset
/// outside the log page, and the server answers `PUT_REFUSED`.
///
/// Falsification: replayable `system_tests/falsifications/user.live_swap_tests.a_confined_client_drives_the_server_to_write_past_its_log.patch`
#[test_case]
fn a_confined_client_drives_the_server_to_write_past_its_log() {
    if machine_has_no_device_page_for_the_console() {
        crate::testing::skip!(NO_UART_PAGE);
    }
    // `log_put` writes at `LOG_VA + log_base + arg`; with `log_base = 0` this offset lands at
    // `DEV_VA`, the first register of the device page. Mirrored arithmetic rather than a
    // `hot_swap_contract` dependency, the convention every constant in this file follows.
    const DEPUTY_PROBE: u64 = DEV_VA - address_space_map::pair_page(0x0300_0000);

    let (msgs, n) = run_swap(ROLE_DEPUTY);
    let attack = of_kind(&msgs[..n], RPT_ATTACK)
        .next()
        .expect("the attacker never reported its RECEIVE_CAP refusal");
    assert_eq!(
        attack[1],
        (-(abi::Error::NotPermitted as i64)) as u64,
        "the attacker's RECEIVE_CAP was not refused, so this run proves nothing about the deputy",
    );
    let deputy = of_kind(&msgs[..n], RPT_DEPUTY)
        .next()
        .expect("the attacker never reported its deputy call");
    // `tag` is `(version << 32) | seq`, so the low word is the offset the server echoed. If the
    // server served the out-of-log PUT, it echoes `DEPUTY_PROBE`.
    let served_offset = deputy[2] & 0xFFFF_FFFF;
    assert_ne!(
        served_offset,
        DEPUTY_PROBE & 0xFFFF_FFFF,
        "CONFINEMENT ESCAPE: a WRITE-only client made the swap server serve OPERATION_PUT at \
         offset {:#x}, far past its one log page, writing into the device register page the client \
         holds no capability for. The server echoed the offset (tag {:#x}) rather than refusing it.",
        DEPUTY_PROBE,
        deputy[2],
    );
    // And the server said so, rather than answering something else that happens not to echo the
    // offset. `hot_swap_contract::PUT_REFUSED`, mirrored.
    assert_eq!(
        (deputy[1], deputy[2]),
        (u64::MAX, u64::MAX),
        "the server neither served nor refused the out-of-log PUT as the protocol says",
    );
}

/// The opt-in rung: a producer keeps producing while no backend exists at all.
///
/// On the direct rung a caller blocks through the down window; here `broker` takes custody, at
/// one extra hop per request (`broker_rtt` in bench/baseline-aarch64.txt), so it is chosen per
/// channel. Unlike the direct test, the backend does not exist for a window (quiesced, dead,
/// collected), the producer keeps calling, and every item reaches the new backend's log in order.
#[test_case]
fn a_producer_never_blocks_on_an_absent_consumer_and_loses_nothing() {
    // No `NO_UART_PAGE` skip: this channel routes no device.
    let (msgs, n) = run_swap(ROLE_QUEUED);
    let msgs = &msgs[..n];

    // The manifest mechanism's other side: on this channel the console component's declaration is
    // the one that cannot be satisfied, because no device is routed here.
    a_component_the_operator_cannot_provide_for_was_refused_first(msgs);

    // `broker` declares `depends_on: &["backend"]`, so the graph names it (id 2) as the one
    // instance to warn before the swap, and it decides the warning. The warning is §231's page and
    // signal, which the operator never waits on.
    the_dependency_graph_matches_what_this_channel_ran(msgs, 1, 2);

    let producer = of_kind(msgs, RPT_CLIENT)
        .next()
        .expect("the producer never reported a verdict");
    const PRODUCER_OK: u64 =
        CL_ALL_REPLIED | CL_SEQ_ECHOED | CL_DIGEST_CORRECT | CL_NONE_REFUSED | CL_WAS_BUFFERED;
    assert_eq!(
        producer[1] & PRODUCER_OK,
        PRODUCER_OK,
        "the queued producer's run was not clean (verdict {:#x}): NONE_REFUSED={} (the queue \
         overflowed or a request was rejected), WAS_BUFFERED={} (nothing was ever buffered, so \
         the producer never actually spanned a window with no backend)",
        producer[1],
        producer[1] & CL_NONE_REFUSED != 0,
        producer[1] & CL_WAS_BUFFERED != 0,
    );

    // The broker's own account: it drained everything it ever took custody of.
    let drained = of_kind(msgs, RPT_DRAINED)
        .next()
        .expect("the broker never reported a drain");
    assert!(drained[1] > 0, "the broker drained nothing");
    assert_eq!(
        drained[1], producer[2],
        "the broker drained {} items but the producer said it had handed over {}",
        drained[1], producer[2],
    );

    // And the backend's log, read by the operator after both backends are gone: every item, in
    // order, served by somebody, with the version changing exactly where the swap was.
    let log = of_kind(msgs, RPT_LOG)
        .next()
        .expect("the operator never reported its verdict");
    const LOG_CLEAN: u64 = LOG_NO_GAP | LOG_MONOTONE | LOG_BOTH_VERSIONS;
    assert_eq!(
        log[1] & LOG_CLEAN,
        LOG_CLEAN,
        "the queued channel lost or reordered work (verdict {:#x}): NO_GAP={}, MONOTONE={}, \
         BOTH_VERSIONS={}",
        log[1],
        log[1] & LOG_NO_GAP != 0,
        log[1] & LOG_MONOTONE != 0,
        log[1] & LOG_BOTH_VERSIONS != 0,
    );
}

// ===============================================================================================
// Milestone 23 (a capability-routed component OS with live replacement)'s third residual: a
// component that stops answering **without dying**.
// ===============================================================================================

/// `abi::survey`'s state codes, unpacked from `RPT_SURVEY` by `hot_swap_contract::survey_counts`'s layout.
/// Mirrored here for the reason every other constant in this file is: userspace owns the definition.
fn survey_blocked(w: u64) -> u64 {
    (w >> 16) & 0xff
}
fn survey_dead(w: u64) -> u64 {
    (w >> 24) & 0xff
}
fn survey_awake(w: u64) -> u64 {
    (w & 0xff) + ((w >> 8) & 0xff)
}

/// A supervisor notices a component that stops answering without dying, restores the service, and
/// reclaims it (milestone 23's third residual; notes/hung-component/the-wedge-test.md argues each).
///
/// The wedge is silent: one swallowed request, no death message (DECISIONS §26 (the fault
/// endpoint)), no corpse. The six results are the helpers below, from the operator's unanswered
/// quiesce to the stranded caller's `Gone`. `x86_64` runs `ROLE_HUNG_WITHOUT_DEVICE`, where all but
/// the take-back hold. The operator's bound is not asserted; whether it would convict a healthy
/// incumbent is measured in `components/src/swapper.rs`.
///
/// Falsification: replayable `system_tests/falsifications/user.live_swap_tests.a_supervisor_notices_a_hung_component_and_reclaims_it.patch`
#[test_case]
fn a_supervisor_notices_a_hung_component_and_reclaims_it() {
    let device = !machine_has_no_device_page_for_the_console();
    let (msgs, n) = run_swap(if device {
        ROLE_HUNG
    } else {
        ROLE_HUNG_WITHOUT_DEVICE
    });
    let msgs = &msgs[..n];

    the_operator_noticed_the_hang_on_its_own(msgs);

    a_hang_reads_as_an_idle_domain(msgs);

    no_member_of_a_hung_domain_can_be_collected(msgs);

    let (client, log) = the_hung_service_is_restored(msgs, device);

    the_hung_component_is_reclaimed_while_still_hung(msgs);

    the_stranded_caller_sees_gone(client, log);
}

/// Result 1 of the hung case: the operator notices on its own. Its `OPERATION_QUIESCE` went
/// unanswered within its bound (milestone 106 (a wait that ends on either the interrupt or the
/// deadline)'s timed wait), and it never drained.
fn the_operator_noticed_the_hang_on_its_own(msgs: &[[u64; 5]]) {
    let unanswered = of_kind(msgs, RPT_UNANSWERED).next().expect(
        "the operator never reached a hang verdict, so its quiesce was answered or it hung itself",
    );
    assert!(
        unanswered[1] > 0 && unanswered[2] >= unanswered[1],
        "the operator called the incumbent hung after waiting {} ticks of a {}-tick bound: the \
         verdict came before the deadline, so something other than the timed wait ended the wait",
        unanswered[2],
        unanswered[1],
    );
    // The deadline is not asserted further, but it is printed, so a run's log carries the figure
    // the bound in `swapper.rs` was chosen against.
    crate::println!(
        "  hung channel: quiesce unanswered after {} of a {}-tick bound",
        unanswered[2],
        unanswered[1]
    );
    assert!(
        !had_step(msgs, STEP_DRAINED),
        "the operator reported a drain: OPERATION_QUIESCE needs the incumbent to answer, which is \
         the one thing a hung component does not do. A run that drained did not test a hang.",
    );
    assert!(
        of_kind(msgs, RPT_QUIESCED).all(|m| m[1] != V1),
        "the incumbent reported answering a quiesce, so it was not hung",
    );
    // The verdict precedes everything the operator did about it.
    let at_verdict = msgs.iter().position(|m| m[0] == RPT_UNANSWERED).unwrap();
    for (kind, what) in [(RPT_SURVEY, "surveyed"), (RPT_RECLAIMED, "reclaimed")] {
        let at = msgs.iter().position(|m| m[0] == kind).unwrap_or(usize::MAX);
        assert!(
            at_verdict < at,
            "the operator {what} (report {at}) before its quiesce went unanswered (report \
             {at_verdict})",
        );
    }
}

/// Result 2 of the hung case: the domain does not report a hang. Every member is `BLOCKED` and none
/// `DEAD`, exactly what a healthy idle system reads as.
fn a_hang_reads_as_an_idle_domain(msgs: &[[u64; 5]]) {
    let survey = of_kind(msgs, RPT_SURVEY)
        .next()
        .expect("the operator never surveyed its domain");
    assert_ne!(
        survey[1],
        u64::MAX,
        "the operator was refused its own domain (error {}): it retyped this rendezvous out of its \
         own budget, so it holds ENUMERATE and this is a bug in the survey rather than a finding",
        survey[2],
    );
    assert_eq!(
        survey[1], 3,
        "the domain should hold exactly the incumbent, the client and the stand-in carrying the \
         quiesce ({} reported). The replacement is built but never started, and a thread's \
         supervision rendezvous is recorded at START, so an embryo is not yet a member.",
        survey[1],
    );
    assert_eq!(
        survey_blocked(survey[2]),
        3,
        "the survey found {} blocked members and {} awake, of 3. **This assertion is the finding**: \
         the hung component, the caller it stranded and the quiesce queued behind it all read as \
         BLOCKED, which is exactly what a healthy server waiting for work and its clients read as. \
         A member reading READY or RUNNING here was still on its way to its park when the bound \
         expired, which at the bound swapper.rs uses means the bound is far too tight.",
        survey_blocked(survey[2]),
        survey_awake(survey[2]),
    );
    assert_eq!(
        survey_dead(survey[2]),
        0,
        "the survey reported {} dead members: a hang is not a death, and if the kernel is calling \
         it one then the rest of this test is asserting the wrong thing",
        survey_dead(survey[2]),
    );

    // No death message had arrived by then either, which is the other half of "nothing noticed":
    // the operator reports every death it collects, and the first of those must come after the
    // survey. A supervisor blocked in RECEIVE on its supervision rendezvous would simply never wake.
    let at_survey = msgs.iter().position(|m| m[0] == RPT_SURVEY).unwrap();
    let first_death = msgs
        .iter()
        .position(|m| m[0] == RPT_DEATH)
        .unwrap_or(usize::MAX);
    assert!(
        at_survey < first_death,
        "a death reached the operator (report {first_death}) before it surveyed the hang (report \
         {at_survey}): something died, so this was not the hung case",
    );
}

/// Result 3 of the hung case: every member refuses to be collected, `StillAlive`. §32 (a supervisor
/// may collect a corpse) authorizes collecting a corpse, and a hung component is not one.
fn no_member_of_a_hung_domain_can_be_collected(msgs: &[[u64; 5]]) {
    let uncollectable = of_kind(msgs, RPT_UNCOLLECTABLE)
        .next()
        .expect("the operator never tried to collect its domain");
    assert_eq!(
        uncollectable[1], 3,
        "the operator asked about {} members, expected 3",
        uncollectable[1],
    );
    assert_eq!(
        uncollectable[2], uncollectable[1],
        "{} of {} members refused collection with StillAlive. Every one must: Rendezvous::REAP \
         authorizes collecting a corpse and refuses a live thread (DECISIONS §32), so a supervisor \
         facing a hang holds a verb with nothing to apply it to. A member that was collectable \
         here would mean something had died.",
        uncollectable[2], uncollectable[1],
    );
}

/// Result 4 of the hung case: the service comes back with no new authority, and the client's stream
/// is unbroken. Returns the client's verdict and the operator's log, which result 6 reads again.
fn the_hung_service_is_restored(msgs: &[[u64; 5]], device: bool) -> (&[u64; 5], &[u64; 5]) {
    for (step, what) in [
        (STEP_BUILT, "build the replacement"),
        (
            STEP_STARTED,
            "start the replacement on the stable rendezvous",
        ),
    ] {
        assert!(
            had_step(msgs, step),
            "the operator never got as far as: {what}"
        );
    }
    assert_eq!(
        had_step(msgs, STEP_REVOKED),
        device,
        "the device take-back from the live, wedged holder should run exactly when there is a \
         device page to take back",
    );

    // Both instances ran, and where there is a device both reached it, so the registers were where
    // the swap thinks they were on each side of a revoke the outgoing holder never consented to.
    let mut ups = of_kind(msgs, RPT_UP);
    let first = ups.next().expect("the incumbent never started");
    let second = ups.next().expect("the replacement never started");
    assert_eq!(first[1], V1);
    assert_eq!(second[1], V2);
    assert!(
        first[2] == device as u64 && second[2] == device as u64,
        "an instance's device reach ({}, {}) does not match the channel's (device page: {device})",
        first[2],
        second[2],
    );

    // The client's own verdict, from its own replies, in its own address space. Every check the
    // healthy channel makes still holds: a hang cost this client nothing but a detour.
    let client = of_kind(msgs, RPT_CLIENT)
        .next()
        .expect("the client never reported a verdict, so it is still parked inside its call");
    const CLIENT_UNBROKEN: u64 =
        CL_ALL_REPLIED | CL_SEQ_ECHOED | CL_DIGEST_CORRECT | CL_ONE_TRANSITION | CL_SPANNED_SWAP;
    assert_eq!(
        client[1] & CLIENT_UNBROKEN,
        CLIENT_UNBROKEN,
        "the client's stream was broken across the hang (verdict {:#x}): missing {:#x}",
        client[1],
        CLIENT_UNBROKEN & !client[1],
    );

    // The operator's independent witness: the shared page, read after every writer is gone. No
    // REVOKE_ENFORCED here: the holder the device was taken from never runs again, so nothing can
    // fault on the registers to prove the take-back. The direct channel carries that receipt.
    let log = of_kind(msgs, RPT_LOG)
        .next()
        .expect("the operator never reported its verdict");
    const LOG_CLEAN: u64 = LOG_NO_GAP | LOG_MONOTONE | LOG_BOTH_VERSIONS;
    assert_eq!(
        log[1] & LOG_CLEAN,
        LOG_CLEAN,
        "the hung channel's log is not clean (verdict {:#x}): NO_GAP={} (a request nobody ever \
         served, so the swallowed one was never asked again), MONOTONE={} (the wedged instance \
         answered after the replacement had, so there were two owners), BOTH_VERSIONS={}",
        log[1],
        log[1] & LOG_NO_GAP != 0,
        log[1] & LOG_MONOTONE != 0,
        log[1] & LOG_BOTH_VERSIONS != 0,
    );
    (client, log)
}

/// Result 5 of the hung case: the incumbent's region came back while it was still hung, after the
/// service did. `DESTROY` after `STARTED` finishes it in place (milestone 133 (ending a permanently
/// blocked thread, and deciding who may)): the budget's live children fall by one instance, and it
/// never dies.
fn the_hung_component_is_reclaimed_while_still_hung(msgs: &[[u64; 5]]) {
    let reclaimed = of_kind(msgs, RPT_RECLAIMED)
        .next()
        .expect("the operator never destroyed the hung incumbent's region");
    assert_eq!(
        reclaimed[1].checked_sub(reclaimed[2]),
        Some(INSTANCE_PAGES),
        "the operator's budget had {} pages in live child regions before the DESTROY and {} \
         after; exactly one instance ({INSTANCE_PAGES} pages) should have come back. Without \
         milestone 133 the DESTROY of a region holding a Blocked thread is refused for the life \
         of the machine.",
        reclaimed[1],
        reclaimed[2],
    );
    let at_started = msgs
        .iter()
        .position(|m| m[0] == RPT_STEP && m[1] == STEP_STARTED)
        .unwrap();
    let at_reclaimed = msgs.iter().position(|m| m[0] == RPT_RECLAIMED).unwrap();
    assert!(
        at_started < at_reclaimed,
        "the incumbent was destroyed (report {at_reclaimed}) before the replacement started \
         (report {at_started})",
    );
    // Two deaths, the client's exit and the replacement's retirement, and neither a fault. A third
    // would be the incumbent dying, which would mean it ran again after the DESTROY: milestone
    // 133 finishes a Blocked resident without waking it, and a finished thread sends no death.
    assert_eq!(
        of_kind(msgs, RPT_DEATH).count(),
        2,
        "the operator collected {} deaths; the hung incumbent should have left none",
        of_kind(msgs, RPT_DEATH).count(),
    );
    assert!(
        of_kind(msgs, RPT_DEATH).all(|m| m[2] != abi::fault::EVENT_FAULT),
        "a child faulted: the incumbent ran again after its region was destroyed",
    );
}

/// Result 6 of the hung case: the caller the incumbent stranded was freed by the kernel with `Gone`
/// (milestone 254 (a caller stranded by a server that died)) and asked the replacement again, the
/// composition milestone 133's block argued, run.
fn the_stranded_caller_sees_gone(client: &[u64; 5], log: &[u64; 5]) {
    assert_ne!(
        client[1] & CL_WAS_GONE,
        0,
        "the client never saw abi::Error::Gone (verdict {:#x}). Either its call was answered \
         normally, and it was never stranded, which proves nothing about a hang; or the DESTROY \
         finished the incumbent without answering the callers it held reply capabilities for, \
         and the client would still be parked (milestone 254's sched::strand_callers_of).",
        client[1],
    );
    // The swap landed exactly at the swallowed request, on both witnesses. Not a coincidence and
    // not a range: the request the incumbent took and never answered is the first one the
    // replacement served, because it is the one the client asked again for.
    assert_eq!(
        client[2], WEDGE_SEQ,
        "the client says the version changed at request {}, but the incumbent swallowed request \
         {WEDGE_SEQ}",
        client[2],
    );
    assert_eq!(
        log[2], client[2],
        "the operator's log and the client's replies disagree about where the replacement took \
         over ({} vs {})",
        log[2], client[2],
    );
}

/// A component's state survives its replacement, and a replacement that cannot take the state
/// over replaces nothing (DECISIONS §209 (state handoff is an opaque blob over a granted frame, and
/// it is optional); milestone 23's last residual; notes/state-handoff.md).
///
/// The component answers each request with its tally. The swap is tried against a replacement
/// reading a layout nobody wrote, then one reading the incumbent's. The client's `SEQ_ECHOED` holds
/// over 64 requests only if no state was lost; the operator's refused attempt rolls back naming the
/// layout, and the absorbed tally equals the quiesced one; the page shows one version change, no
/// gap, never backwards. It needs no device, so it runs on `x86_64`, where the slot-2 device
/// capability `spawn_swapper` grants names physical page zero and is never mapped.
#[test_case]
fn a_component_keeps_its_state_across_a_swap_and_a_swap_that_cannot_absorb_it_does_not_commit() {
    let (msgs, n) = run_swap(ROLE_HANDOFF);
    let msgs = &msgs[..n];

    // Before anything is built: a supervisor with no handoff page to route (the client's routing
    // table) is refused the stateful contract, rather than wiring it and losing the state silently.
    a_component_the_operator_cannot_provide_for_was_refused_first(msgs);

    // Two drains, one refusal, one absorb, in that order.
    let steps: [u64; 4] = {
        let mut out = [0u64; 4];
        let mut k = 0;
        for m in of_kind(msgs, RPT_STEP) {
            if matches!(m[1], STEP_DRAINED | STEP_ROLLED_BACK | STEP_ABSORBED) && k < 4 {
                out[k] = m[1];
                k += 1;
            }
        }
        out
    };
    assert_eq!(
        steps,
        [STEP_DRAINED, STEP_ROLLED_BACK, STEP_DRAINED, STEP_ABSORBED],
        "the operator should drain, roll back, drain again, then commit; it did {steps:?} \
         (2 = drained, 7 = rolled back, 6 = absorbed)",
    );
    let rolled_back = of_kind(msgs, RPT_STEP)
        .find(|m| m[1] == STEP_ROLLED_BACK)
        .expect("checked above");
    assert_eq!(
        rolled_back[2], LAYOUT_1,
        "the refusing replacement should name the layout it could not read, the incumbent's",
    );

    // The state that left is the state that arrived.
    let mut drains = of_kind(msgs, RPT_STEP).filter(|m| m[1] == STEP_DRAINED);
    let first_drain = drains.next().expect("checked above")[2];
    let second_drain = drains.next().expect("checked above")[2];
    let absorbed = of_kind(msgs, RPT_STEP)
        .find(|m| m[1] == STEP_ABSORBED)
        .expect("checked above")[2];
    assert!(
        first_drain > 0 && first_drain < second_drain && second_drain < REQUESTS,
        "the incumbent should have served more after the rollback than before it ({first_drain} \
         then {second_drain}): a rollback that left it unable to serve is not a rollback",
    );
    assert_eq!(
        absorbed, second_drain,
        "the replacement absorbed a tally of {absorbed}, and the incumbent quiesced holding \
         {second_drain}: state was lost or invented in the handoff",
    );

    // Exactly two instances ever served: the refuser never came up.
    let mut ups = of_kind(msgs, RPT_UP);
    assert_eq!(
        ups.next().map(|m| m[1]),
        Some(V1),
        "the incumbent never came up"
    );
    assert_eq!(
        ups.next().map(|m| m[1]),
        Some(V2),
        "the replacement never came up"
    );
    assert!(
        ups.next().is_none(),
        "a third instance came up: the refusing replacement served after it refused",
    );

    the_client_and_the_page_saw_the_swap_at(msgs, absorbed);

    // Four children, four corpses, none of them a fault: the refuser exited, the incumbent was
    // retired on commit, the client finished, and the replacement was retired at the end.
    assert_eq!(of_kind(msgs, RPT_DEATH).count(), 4);
    assert!(
        of_kind(msgs, RPT_DEATH).all(|m| m[2] != abi::fault::EVENT_FAULT),
        "a child of the handoff system faulted",
    );
}

/// The handoff case's two independent witnesses, the client and the page, each put the swap at
/// `absorbed`, the tally the replacement took over.
fn the_client_and_the_page_saw_the_swap_at(msgs: &[[u64; 5]], absorbed: u64) {
    // Witness one: the client. Every flag, and the swap landed exactly where the tally says.
    let client = of_kind(msgs, RPT_CLIENT)
        .next()
        .expect("the client never reported a verdict");
    for (bit, what) in [
        (CL_ALL_REPLIED, "every request was answered"),
        (
            CL_SEQ_ECHOED,
            "every reply carried the component's tally, unbroken across the swap",
        ),
        (CL_DIGEST_CORRECT, "every answer was right"),
        (
            CL_ONE_TRANSITION,
            "the version changed once, and the refused attempt did not show",
        ),
        (CL_SPANNED_SWAP, "the swap landed inside the conversation"),
    ] {
        assert!(client[1] & bit != 0, "the client could not confirm: {what}");
    }
    assert_eq!(
        client[2], absorbed,
        "the first request the replacement answered should be request {absorbed}, the tally it \
         took over; the client saw the version change at {}",
        client[2],
    );

    // Witness two: the page.
    let log = of_kind(msgs, RPT_LOG).next().expect("checked by run_swap");
    for (bit, what) in [
        (LOG_NO_GAP, "no request was lost"),
        (
            LOG_MONOTONE,
            "the incumbent never answered after the replacement",
        ),
        (LOG_BOTH_VERSIONS, "both versions served"),
    ] {
        assert!(log[1] & bit != 0, "the witness page does not show: {what}");
    }
    assert_eq!(
        log[2], absorbed,
        "the witness page puts the swap somewhere else"
    );
}

/// A dependent that is never warned costs its callers latency, not work (the non-cooperative
/// fallback, notes/non-cooperative-fallback.md).
///
/// The queued system, with the graph naming `broker` as before and the operator never warning it,
/// as when a dependent does not answer. `broker` stays in pass-through, its forwarded `CALL` parks
/// on the back endpoint's sender queue for the down window, and the replacement drains it (DECISIONS
/// §41 (the endpoint is the broker, and a device is revoked by taking it back), one hop out). The
/// queued channel's assertions hold with `WAS_BUFFERED` inverted: nothing buffered is the witness
/// that the warning was absent, not merely unobserved.
#[test_case]
fn a_dependent_that_is_never_warned_loses_nothing_and_only_waits() {
    let (msgs, n) = run_swap(ROLE_UNWARNED);
    let msgs = &msgs[..n];

    a_component_the_operator_cannot_provide_for_was_refused_first(msgs);
    // The graph's answer is unchanged: the edge is real. What is measured is ignoring it.
    the_dependency_graph_matches_what_this_channel_ran(msgs, 1, 2);

    let producer = of_kind(msgs, RPT_CLIENT)
        .next()
        .expect("the producer never reported a verdict");
    const PRODUCER_OK: u64 = CL_ALL_REPLIED | CL_SEQ_ECHOED | CL_DIGEST_CORRECT | CL_NONE_REFUSED;
    assert_eq!(
        producer[1] & PRODUCER_OK,
        PRODUCER_OK,
        "the unwarned producer's run was not clean (verdict {:#x}): skipping the warning cost \
         more than latency",
        producer[1],
    );
    assert_eq!(
        producer[1] & CL_WAS_BUFFERED,
        0,
        "the broker buffered, so it was warned after all and this run measured nothing",
    );
    assert!(
        of_kind(msgs, RPT_DRAINED).next().is_none(),
        "the broker drained a backlog it should never have had",
    );

    let log = of_kind(msgs, RPT_LOG)
        .next()
        .expect("the operator never reported its verdict");
    const LOG_CLEAN: u64 = LOG_NO_GAP | LOG_MONOTONE | LOG_BOTH_VERSIONS;
    assert_eq!(
        log[1] & LOG_CLEAN,
        LOG_CLEAN,
        "the unwarned channel lost or reordered work (verdict {:#x})",
        log[1],
    );
}

/// **A warning that arrives after the swap has finished costs nothing** (DECISIONS §231 (a swap's
/// warning to a dependent is advisory, and the supervisor never waits for it)). The operator writes
/// the broker's page down and then up, and signals, only once the replacement backend is already
/// serving. Whatever the broker makes of it, buffering a few requests or none, every request is
/// answered correctly and in order, and anything it did buffer reaches the backend.
#[test_case]
fn a_warning_that_lands_after_the_swap_loses_nothing() {
    let (msgs, n) = run_swap(ROLE_LATE_WARNING);
    let msgs = &msgs[..n];
    a_component_the_operator_cannot_provide_for_was_refused_first(msgs);
    the_dependency_graph_matches_what_this_channel_ran(msgs, 1, 2);

    let producer = of_kind(msgs, RPT_CLIENT)
        .next()
        .expect("the producer never reported a verdict");
    const PRODUCER_OK: u64 = CL_ALL_REPLIED | CL_SEQ_ECHOED | CL_DIGEST_CORRECT | CL_NONE_REFUSED;
    assert_eq!(
        producer[1] & PRODUCER_OK,
        PRODUCER_OK,
        "a late warning cost more than latency (verdict {:#x})",
        producer[1],
    );
    let drained: u64 = of_kind(msgs, RPT_DRAINED).map(|m| m[1]).sum();
    assert_eq!(
        drained, producer[2],
        "the broker buffered {} items for the producer and drained {drained}: a late warning lost work",
        producer[2],
    );
    let log = of_kind(msgs, RPT_LOG)
        .next()
        .expect("the operator never reported its verdict");
    const LOG_CLEAN: u64 = LOG_NO_GAP | LOG_MONOTONE | LOG_BOTH_VERSIONS;
    assert_eq!(
        log[1] & LOG_CLEAN,
        LOG_CLEAN,
        "the late-warning channel lost or reordered work (verdict {:#x})",
        log[1],
    );
}

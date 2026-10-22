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

/// The console UART's physical address, matching `crate::console`. This is the device the
/// operator lends, takes back, and lends again.
///
/// **Taken from `user::UART_PHYS` rather than spelled again here** (milestone 161). It was a
/// file-local pair of `cfg` arms, aarch64 and riscv64, which is a copy of a constant that already
/// existed one module up and which therefore did not compile the day a third architecture arrived.
/// The module-level one has three arms, and the third is **zero**: x86's COM1 is in the I/O port
/// space, so there is no page for a device capability to be a mapping of
/// ([DECISIONS §121](../../../design/decisions/0121-port-io-capability.md), PROPOSED). That zero is
/// what [`NO_DEVICE_PAGE`] tests for.
use crate::user::UART_PHYS;
/// **This whole file needs a device that is a page**, and on one architecture there is not one.
///
/// The operator lends the console UART, revokes it, and the test's strongest assertion is that the
/// outgoing instance then **faults inside [`DEV_VA`]**. On x86 there is no UART page to lend, and
/// the trap worth naming is that lending physical page zero instead would still produce a green
/// test: the read would return a real-mode interrupt-vector byte and the revoke would still fault.
/// That is a passing test about nothing. See `hot_swap_contract::probe_device`'s x86 arm, which refuses
/// for the same reason, and `user::machine_has_no_device_page_for_the_console`, which is the one
/// definition of the question.
use crate::user::{NO_UART_PAGE, machine_has_no_device_page_for_the_console};

/// The operator's budget: five instance regions of forty pages plus its own scratch mappings and
/// their page tables.
///
/// Kept tight on purpose, and it is not merely tidiness. `memory_region::create` takes a **contiguous**
/// run of frames and the suite runs three of these systems, on top of a dozen earlier tests that
/// each park a progenitor holding an eight-megabyte region. An over-generous budget here fragments
/// the frame allocator enough that a *later, unrelated* test cannot get the progenitor's region, which is
/// how both of this milestone's memory failures surfaced: nowhere near their cause.
const SWAPPER_BUDGET_PAGES: u64 = 224;

/// How many reports one run can make before the test gives up waiting for the operator's final
/// verdict. Generous: the loop stops at `RPT_LOG`, and this is only the tripwire for a run that
/// never gets there. Raised from 24 with milestone 23's manifest, which adds one `RPT_REFUSED` per
/// run, from 28 with the hung-component role, which adds three, and from 40 with dependency-aware
/// orchestration, which adds one `RPT_DEPENDENTS` on the direct and queued channels; a run that
/// overflows this loses the operator's verdict and fails for the wrong reason.
const MAX_REPORTS: usize = 42;

/// **Spawn the operator the way the kernel spawns the progenitor**, and return the report rendezvous every
/// process in the run holds a WRITE view of.
///
/// Deliberately the same endowment `spawn_elf_child_builder` gives (the archive read-only at
/// `INITIAL_RAMDISK_VA`, an untyped in slot 0, a report rendezvous in slot 1), **plus** the one
/// thing this milestone is about: a device capability in slot 2, `WRITE|GRANT`, exactly as the progenitor gets one at boot. So
/// what is under test is the operator's choices, not a privileged shortcut.
fn spawn_swapper(role: u64) -> (sched::RendezvousId, u64, u64, u64) {
    // Carved from a region this run owns and reclaims, not from the kernel's own pool:
    // `sched::create_rendezvous` grows that pool in 32-page chunks it never frees, so every run
    // costs the suite a page for good and the ledger moves in +32 steps (the `BUGS` on
    // `testing::SUITE_PAGE_FRAME_BUDGET`). Milestone 633 (an outside agent attacks the confinement
    // claim) added a ninth run and crossed a chunk boundary, which is how this was found. Four
    // pages: the report rendezvous, and (milestone 800 (a non-Anthropic model attacks the
    // confinement claim), fourth pass) the notification and timer that make the operator's un-park
    // of the plant bounded. The helper cannot make these, which is why the rendezvous arrives
    // made: the notification and timer are carved from the same region the report line comes from.
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

    // Let the system settle, then prove it has nothing more to say. A parked sender here would
    // mean something acted after the operator had finished, which is how "the run left nothing
    // running" is proven without a blocking receive that would hang when the code is right.
    for _ in 0..400 {
        sched::yield_now();
    }
    assert_eq!(
        sched::rendezvous_waiting_senders(report),
        0,
        "the swap system had more to say after the operator's final verdict",
    );

    // **The system reclaims itself**, and this is an assertion rather than housekeeping.
    //
    // The operator supervises every child it starts and collects every corpse through its
    // supervision rendezvous (DECISIONS §32), which returns each instance's region to its budget.
    // So a reclaim of the budget can only *succeed* if all five of those splits are gone: §16
    // refuses a region whose children are still carved out of it. Success is the statement that
    // nothing leaked; the region's page count is the statement that the whole run came back.
    //
    // It also has to work, for a reason that has nothing to do with tidiness. `memory_region::create`
    // takes a **contiguous** run of frames, these tests run three systems, and the first version
    // of this leaked all three, which fragmented the allocator badly enough that a *later* test
    // could not get the progenitor's own eight-megabyte region.
    //
    // `sched::reclaim_region` rather than `memory_region::destroy` because these regions are *pinned*:
    // the operator retyped four endpoints and a frame out of its budget. Reclaiming a region
    // with objects in it is the §16 teardown, and it is the entry point the `MemoryRegion::DESTROY`
    // syscall uses, so the test cannot succeed down a path userspace could not have taken.
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
    // **The region is gone, and its absence is the measurement.** `reclaim_region` destroys the
    // region itself, so there is nothing left to ask about usage: `memory_region::usage` returns
    // `None` once the name is stale. That `None` says more than a frame delta did. §16 (object revocation: reclaim the
    // objects a process built) refuses to
    // reclaim a region whose children are still carved out of it, so reaching a stale name at all
    // means every child was already gone and the whole run, exactly `SWAPPER_BUDGET_PAGES` of it,
    // went back to the parent. The old assertion read the machine's global free-frame count, which
    // any neighbouring test's teardown could move; this one cannot be perturbed by anything outside
    // this region.
    assert!(
        crate::memory_region::usage(budget).is_none(),
        "the operator's budget still exists after reclaim, so its {SWAPPER_BUDGET_PAGES} pages did \
         not go back to the parent",
    );

    // The operator's own address space and TCB are **not** in that budget: the kernel built the
    // operator the way it builds the progenitor. They come home through the ordinary reaper, which with
    // per-CPU run queues (DECISIONS §28) runs when the core the operator died on next schedules,
    // and the boot thread yielding here cannot force that. So this is hygiene, deliberately not
    // asserted on: what this milestone is responsible for is the swap system's own memory,
    // which is what the assertion above covers. A `debug_assert!` against a free-frame count
    // sampled at the top of the run stood here until 2026-08-03 and flaked on CI, because the
    // only thing that could trip it was an *earlier* test's teardown landing mid-run, which is
    // nothing this test is responsible for. See the BUGS section of notes/live-replacement.md.
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

/// **A component this operator cannot provide for is refused before anything is built**
/// (milestone 23's component manifest; `crates/capability_plan`, notes/component-manifest.md).
///
/// Asserted on both channels, because a mechanism that only worked for the one component it was
/// written against would not be a mechanism. On the direct channel the operator plans the queue
/// broker's declaration, which names `requests` and `backend`, and that channel routes neither; on
/// the queued channel it plans the console component's declaration, which names `uart`, and that
/// channel routes no device. Both are **real manifests against real routing tables**, not fixtures:
/// each is the other role's component, asked for by a supervisor that genuinely cannot satisfy it.
///
/// Two things are asserted, and the second is the one that matters. The refusal is the *typed* one
/// (a role went unrouted) rather than any old failure, and it arrives **ahead of every build step and
/// every instance that started**, which is what makes a manifest a request the supervisor may refuse
/// rather than an instruction it has to carry out half way. A supervisor that discovered the problem
/// after mapping a page would have already moved authority.
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

/// **The dependency graph a supervisor would compute agrees with what this channel actually ran**
/// (milestone 23's dependency-aware-orchestration residual; `crates/capability_plan`'s `dependents`).
///
/// `swapper` reports one `RPT_DEPENDENTS` per swap target it considers: `w1` = how many live
/// instances `capability_plan::dependents` returned, `w2` = the first one's id (0 if none). This is
/// not a report about what the operator *did*; it is the graph's own verdict, computed from
/// `Requirements::depends_on` before any orchestration step runs, so a mismatch here would mean the
/// graph and the hand-written sequencing had drifted apart.
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

/// **The flagship: a component is replaced under a client that is talking to it.**
///
/// The four steps all happen, in an order the operator chose, and then two independent
/// witnesses in two address spaces agree that the conversation was unbroken. The client is not
/// consulted about the swap and the operator is not consulted about the replies; each says only
/// what it saw.
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

/// **The attacker holds a real capability to the stable rendezvous and still cannot be the
/// server.**
///
/// The milestone rests on rendezvous-only naming, and the obvious worry about it is that a name
/// with no peer in it is a name anybody can answer to. It is not: `SEND` and `RECEIVE` are gated by
/// different rights on the same object, so the same rendezvous handed out two ways is a one-way
/// pipe in whichever direction each holder was trusted with. The attacker is endowed with
/// *exactly* what the honest client holds, so the refusal is about rights and not about
/// wiring.
///
/// **A plant makes this test able to fail rather than hang** (milestone 800 (a non-Anthropic model
/// attacks the confinement claim), fourth pass). Until it, a `RECEIVE_CAP` the kernel failed to
/// refuse simply blocked on an empty queue, so the break surfaced as a 60-second watchdog hang
/// (measured 2026-09-16, milestone 305 (the six kernel confinement rows get a falsification a
/// machine can replay)): milestone 202 (every confinement test is a ritual until somebody breaks
/// the confinement)'s wrong-reason red on a third claim. The operator now
/// retires the replacement first, so nothing ever receives on the endpoint again, then starts the
/// plant (a `swap_boundary_witness` role that parks a message on it), then the attacker. A let-open
/// `RECEIVE_CAP` *returns the plant's message*, and the assertion below fires: the attacker's
/// report carries the marker word rather than the refusal. The operator then drains the plant
/// itself on the honest kernel, so both programs exit and the run reclaims as before. The
/// kernel-side twin of this gate is
/// `confinement_attack_tests::a_write_only_rendezvous_holder_cannot_receive_reap_or_survey`,
/// which parks its sender by construction and needs no operator; this test carries its own record
/// so claim 26's cited evidence is not a measurement beside it.
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

/// **A confined client cannot drive the server to write past its log, into a device page it was
/// never granted** (milestone 633 (an outside agent attacks the confinement claim), third pass).
///
/// The kernel refuses the attacker's `RECEIVE_CAP` (it holds `WRITE`, not `READ`), so it cannot
/// become the server, which the test above asserts. That is not the whole of claim 26: the server's
/// `OPERATION_PUT` writes a byte at `LOG_VA + log_base + arg`, with `arg` the client's own word. Until
/// the third outsider pass found it, nothing bounded `arg`, so a `WRITE`-only client asked for an
/// offset landing in the device register page the server holds and the client does not, and the
/// server wrote there and echoed the offset. That was a confused-deputy escape of the demonstrator's
/// boundary, not a kernel gate failure, booted red on aarch64 and riscv64. `hot_swap_contract::log_put`
/// now refuses an offset outside the one log page and the server answers `PUT_REFUSED`.
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

/// **The opt-in rung: a producer keeps producing while no backend exists at all.**
///
/// The direct rung's down window costs the caller a block: its request is safe (it parks on the
/// rendezvous's sender queue and the next server drains it) but it is stopped until then. For a
/// channel that cannot afford that, `broker` takes custody. The price is one extra hop on
/// every request in the steady state, which is why it is chosen per channel and never by
/// default; `broker_rtt` in bench/baseline-aarch64.txt is that price.
///
/// What this proves that the direct test does not: there is a window here in which the backend
/// **does not exist** (it was quiesced, it died, and its corpse was collected before the
/// replacement was built), the producer kept calling through it, and every item it handed over
/// turns up in the new backend's log, in order.
#[test_case]
fn a_producer_never_blocks_on_an_absent_consumer_and_loses_nothing() {
    // No `NO_UART_PAGE` skip since 2026-09-26: this channel routes no device, so the x86 gap the
    // direct and hung channels skip on never applied to it. The skip was inherited from the file
    // rather than from anything this system does.
    let (msgs, n) = run_swap(ROLE_QUEUED);
    let msgs = &msgs[..n];

    // The manifest mechanism's other side: on this channel the console component's declaration is
    // the one that cannot be satisfied, because no device is routed here.
    a_component_the_operator_cannot_provide_for_was_refused_first(msgs);

    // **The one real edge in this milestone's residual.** `broker` declares `depends_on:
    // &["backend"]`, so the graph names it (id 2) as the sole instance that must be warned before
    // the backend is swapped, and it is what actually decides whether the broker is warned on this
    // run rather than the operator's own hard-coded memory of what it built. The warning is §231's
    // page and signal, which the operator never waits on.
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
// Milestone 23's third residual: a component that stops answering **without dying**.
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

/// **A supervisor notices a component that stops answering without dying, restores the service, and
/// reclaims the hung component** (milestone 23's third residual; notes/hung-component.md and its
/// `the-wedge-test.md`, which argues each result).
///
/// A component that merely stops answering produces no death message (DECISIONS §26 (the fault
/// endpoint)) and no corpse to reap. **Its wedge is silent**: it swallows one request and parks,
/// and the only message it sends is the ordinary swap trigger a healthy incumbent sends too.
///
/// 1. **The operator notices on its own**: its `OPERATION_QUIESCE` goes unanswered within its bound
///    (milestone 106 (a wait that ends on either the interrupt or the deadline)'s timed wait).
/// 2. **The domain does not report a hang**: every member `BLOCKED`, none `DEAD`, exactly what a
///    healthy idle system reads as.
/// 3. **The reap vocabulary is refused**: `StillAlive` for every member, since §32 (a supervisor may
///    collect a corpse) authorizes collecting a corpse and a hung component is not one.
/// 4. **The service is restored with no new authority**, and the client's stream is unbroken.
/// 5. **The hung component's region comes back while it is still hung**: `DESTROY` after `STARTED`
///    finishes it in place (milestone 133 (ending a permanently blocked thread, and deciding who
///    may)), the budget's live-children count falls by one instance, and it leaves no corpse.
/// 6. **The caller it stranded sees `Gone`** (milestone 254 (a caller stranded by a server that
///    died)) and asks the replacement again: the composition milestone 133's block argued, run.
///
/// On `x86_64` there is no device page (`NO_UART_PAGE`), so the run uses
/// `ROLE_HUNG_WITHOUT_DEVICE`, the same system with nothing to revoke, and every result but the
/// take-back holds there unchanged.
///
/// **The only clock is the operator's bound, and it is not asserted on.** Against a hung incumbent
/// any bound is eventually reached; what it decides is whether a healthy incumbent would be
/// convicted, which is measured in `components/src/swapper.rs` rather than tested here.
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

/// Result 1 of the hung case: the operator's quiesce went unanswered, and it never drained.
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

/// Result 2 of the hung case: the domain does not report a hang.
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

/// Result 3 of the hung case: every member refuses to be collected.
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

/// Result 4 of the hung case: the service comes back. Returns the client's verdict and the
/// operator's log, which result 6 reads again.
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

/// Result 5 of the hung case: the incumbent's region came back, after the service did, and the
/// incumbent never died.
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

/// Result 6 of the hung case: the caller the incumbent stranded was freed by the kernel, with
/// `Gone`, and asked the replacement again.
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

/// **A component's state survives its own replacement, and a replacement that cannot take the state
/// over does not replace anything** (DECISIONS §209 (state handoff is an opaque blob over a granted frame, and it is optional); the last residual of milestone 23 (a capability-routed component OS with live replacement);
/// notes/state-handoff.md).
///
/// The component keeps a tally of every request it has served and answers each request with it.
/// The operator tries the swap twice: once against a replacement that only understands a blob layout
/// nobody wrote, and once against one that understands the incumbent's. Three witnesses, none
/// consulted by the others:
///
/// - **The client**, from its replies. The tally rides in the reply word the other channels use to
///   echo the sequence number, so `SEQ_ECHOED` holds across all 64 requests **only if no state was
///   lost**: a replacement that started from zero would answer request 40 with 0.
/// - **The operator**, from the steps: the refused attempt rolled back and named the layout it
///   could not read, and the tally the replacement says it absorbed is exactly the tally the
///   incumbent said it had when it quiesced the second time.
/// - **The witness page**, byte by byte: one version change, no gap, never backwards.
///
/// **This is the one swap system that runs on `x86_64`.** It has no device, because the state is what
/// is under test, so the x86 gap the other three tests skip on (`NO_UART_PAGE`) does not apply. The
/// operator is still spawned holding a device capability in slot 2, because `spawn_swapper` gives
/// every role one; on x86 that capability names physical page zero, and this role never maps it.
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

/// **A dependent that is never warned costs its callers latency, not work** (the non-cooperative
/// fallback, notes/non-cooperative-fallback.md).
///
/// The queued system, with one change: the dependency graph names `broker` exactly as it does on
/// the queued channel, and the operator then never warns it, which is what a supervisor
/// is reduced to when the dependent it must warn does not answer. The backend is swapped anyway.
/// `broker` stays in pass-through, its forwarded `CALL` parks on the back endpoint's sender queue
/// for the down window, and the replacement drains it (the argument of DECISIONS §41 (the endpoint
/// is the broker, and a device is revoked by taking it back), one hop further out).
///
/// So the assertions are the queued channel's, with `WAS_BUFFERED` inverted: every request answered
/// correctly and in order, nothing refused, nothing lost, and **nothing buffered**, which is the
/// witness that the warning really was absent rather than merely unobserved.
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

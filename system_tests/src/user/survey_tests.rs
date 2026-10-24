use core::sync::atomic::{AtomicBool, Ordering};

use abi::Error;

use super::supervision_tests::{FAULT_STUB, REPORT_STUB, build_child_in};
use crate::arch::exceptions::TrapFrame;
use crate::cap::Rights;
use crate::sched;
use crate::syscall::invoke;

/// The builder's whole budget: room for three instances plus slack, so a domain can hold more than
/// one member and a second domain can exist beside it.
const BUILDER_BUDGET_PAGES: u64 = 80;

/// Pages per instance region, the same carve `build_child_in` has always made: the child's address
/// space (root and tables), its code page, its stack page, and its TCB.
pub(super) const INSTANCE_PAGES: u64 = 16;

/// Pages for the test's own rendezvous points, one per rendezvous (`RETYPE_OBJ`'s one-object-per-page rule).
/// Three is the most any test here needs; four is slack.
const RENDEZVOUS_PAGES: u64 = 4;

/// **`ps`'s row buffer must hold the kernel's whole thread table**, checked by the compiler rather
/// than by a test, which is the strongest rung available for a fact that is two constants in two
/// crates. A `ps` holding the widest grant this system can express sees every thread on the
/// machine; if the table outgrew the buffer, the listing would silently stop short, which is the
/// one failure a monitor must never have. Making the wrong state unrepresentable costs one line
/// here and nobody has to remember it.
const _: () = assert!(
    ps::MAX_ROWS >= sched::MAX_THREADS,
    "ps::MAX_ROWS is smaller than the kernel's thread table: a listing of the widest domain this \
     system can express would be silently truncated",
);

/// One test's world: a budget the builder owns, and a small region the rendezvous points come out of, so
/// `tidy` can give both back rather than spending the kernel's shared rendezvous budget on every run.
pub(super) fn arena() -> (u64, u64) {
    let budget = crate::memory_region::create(BUILDER_BUDGET_PAGES).expect("no builder budget");
    let rendezvous_region =
        crate::memory_region::create(RENDEZVOUS_PAGES).expect("no rendezvous region");
    (budget, rendezvous_region)
}

pub(super) fn rendezvous(region: u64) -> sched::RendezvousId {
    sched::create_rendezvous_from(region).expect("no rendezvous")
}

/// Hold a domain **the way a viewer holds one**: `ENUMERATE` and nothing else, which is exactly
/// what `ps` is granted. It is not that a viewer is refused `RECEIVE` and `REAP`; it cannot name them,
/// because those take `READ` and this capability does not carry it.
pub(super) fn hold_view(ep: sched::RendezvousId) -> u64 {
    sched::grant(crate::cap::rendezvous_cap(ep, Rights::ENUMERATE)).expect("grant the rendezvous")
}

/// Hold a domain **the way its supervisor holds it**: `READ` to receive and reap, `ENUMERATE` to
/// look. The two rights are separable and this is the holder that has both, which is what makes
/// [`hold_view`]'s narrowness a choice rather than the only thing available.
pub(super) fn hold_supervisor(ep: sched::RendezvousId) -> u64 {
    sched::grant(crate::cap::rendezvous_cap(
        ep,
        Rights::READ.union(Rights::ENUMERATE),
    ))
    .expect("grant the rendezvous")
}

/// Hold the same rendezvous **send-only**: a peer that may report to this supervisor and is not the
/// supervisor. The negative control's whole setup is this one line.
pub(super) fn hold_write(ep: sched::RendezvousId) -> u64 {
    sched::grant(crate::cap::rendezvous_cap(ep, Rights::WRITE)).expect("grant the rendezvous")
}

/// `invoke(cap, SURVEY, cursor, _, _)`, through the real dispatcher, returning the three words a
/// userspace caller would read out of its registers.
fn survey(slot: u64, cursor: u64) -> (i64, u64, u64) {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    match invoke(&mut frame, slot, abi::rendezvous::SURVEY, cursor, 0, 0) {
        Ok(next) => (next, frame.arg(1), frame.arg(2)),
        Err(e) => (e as i64, 0, 0),
    }
}

/// **The whole domain, walked by the real program's real loop.**
///
/// `ps::collect` is what `components/src/ps.rs` runs; driving it here rather than reimplementing the
/// cursor walk is the same discipline the `rm`, `date` and sink tests keep, and it is why a bug in
/// the cursor protocol cannot hide between the two sides.
/// Eight rows, not `ps::MAX_ROWS`. **These tests run on a kernel thread stack**, under a 4,096-byte
/// guard page, and a `[Row; 128]` local is two kilobytes; `script/stack-frame-check` is the gate
/// that made the buffer the caller's in the first place. The widest domain any test here builds is
/// three, so eight is slack with room to notice an unexpected member rather than truncate it.
pub(super) const TEST_ROWS: usize = 8;

fn walk(slot: u64, rows: &mut [ps::Row; TEST_ROWS]) -> ps::Survey<'_> {
    let s = ps::collect(rows, &mut |cursor| survey(slot, cursor));
    assert!(
        s.is_complete() || s.is_refused(),
        "the survey outgrew this test's row buffer, so what it reported is not the domain",
    );
    s
}

/// `invoke(cap, REAP, tid, _, _)`, through the real dispatcher.
fn reap(slot: u64, tid: u64) -> Result<i64, Error> {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    invoke(&mut frame, slot, abi::rendezvous::REAP, tid, 0, 0)
}

/// **Let every parked child finish.** Each `REPORT_STUB` member is blocked in a send on the shared
/// parking rendezvous, so taking `n` messages off it releases `n` of them to exit.
///
/// Deliberately not per-child: an `ipc_receive` takes whichever sender is queued, so a test with
/// members in two domains must drain the whole rendezvous before it collects any of them. Draining
/// only its own domain's count releases somebody else's child and leaves one of its own blocked
/// forever, which is the bug this comment exists to stop the next reader reintroducing.
pub(super) fn drain(parking: sched::RendezvousId, n: usize) {
    for _ in 0..n {
        sched::ipc_receive(parking);
    }
}

/// Collect every member of a domain through the rendezvous that showed it.
///
/// A region holding a live thread refuses reclamation, and the refusal arms a kill on the way past
/// (§16's amendment), so a test that keeps children alive ends them itself rather than leaving them
/// to `tidy`. It also asserts every tid a survey reported is reapable through the same rendezvous,
/// `capability::survey_includes`'s scope property from the control side. The wait is the clock's,
/// not a yield count: a released child's time to exit is the host's (notes/hvf-leg.md).
pub(super) fn collect_all(cap: u64, tids: &[u64]) {
    for &tid in tids {
        assert!(
            super::wait_for(|| reap(cap, tid) == Ok(0)),
            "a member the survey reported could not be collected through the rendezvous that \
             showed it",
        );
    }
}

/// The tids a domain reports, in the order it reported them.
fn tids<'a>(s: &'a ps::Survey<'_>) -> impl Iterator<Item = u64> + 'a {
    s.rows().iter().map(|r| r.tid)
}

/// Give everything back: the viewer's slots, then the builder's budget (which reclaims any instance
/// region still under it), then the rendezvous region. Reclaiming the rendezvous points first would revoke
/// the channels the corpses are still attached to.
pub(super) fn tidy(budget: u64, rendezvous_region: u64, slots: &[u64]) {
    for &s in slots {
        let _ = sched::delete_current_cap(s);
    }
    sched::reclaim_region(budget).expect("the builder's own budget did not come back");
    sched::reclaim_region(rendezvous_region)
        .expect("the test's rendezvous region did not come back");
}

/// Build a child into its own region carved from `budget`, so one reclaim frees the instance.
pub(super) fn child_in(
    budget: u64,
    stub: &[u32],
    report: Option<sched::RendezvousId>,
    fault_ep: sched::RendezvousId,
) -> u64 {
    let region = crate::memory_region::split(budget, INSTANCE_PAGES).expect("no instance region");
    build_child_in(region, stub, report, Some(fault_ep))
}

/// A domain is exactly the rendezvous's own children, and nothing else on the machine.
///
/// Two children under one supervision rendezvous, one under another, and the rest of the system
/// (this thread, the idle and kernel threads, other tests' leftovers) unsupervised. The survey of
/// the first reports its two and stops. Unlike Linux's `/proc`, a listing here is a fact about a
/// capability, not the machine, so it changes with the rendezvous held and nothing widens it.
#[test_case]
fn a_domain_is_exactly_the_children_of_the_rendezvous_that_was_granted() {
    let (budget, rendezvous_region) = arena();
    let mine = rendezvous(rendezvous_region);
    let theirs = rendezvous(rendezvous_region);
    // A parking rendezvous nobody ever receives on, so a REPORT_STUB child blocks in its send and
    // stays a live member of the domain for the length of the test instead of exiting under us.
    let parking = rendezvous(rendezvous_region);

    let a = child_in(budget, REPORT_STUB, Some(parking), mine);
    let b = child_in(budget, REPORT_STUB, Some(parking), mine);
    let stranger = child_in(budget, REPORT_STUB, Some(parking), theirs);

    // Both of mine have to have reached their send before the states below mean anything.
    assert!(
        super::wait_for(|| sched::rendezvous_waiting_senders(parking) == 3),
        "the three children never reached their sends",
    );

    let cap = hold_view(mine);
    let mut buf = [ps::Row::default(); TEST_ROWS];
    let seen = walk(cap, &mut buf);
    assert!(
        !seen.is_refused(),
        "a supervisor could not read its own domain"
    );

    let found = ThreadIdSet::of(&seen);
    assert_eq!(
        seen.rows().len(),
        2,
        "the domain reported {} members where two were built",
        seen.rows().len(),
    );
    assert!(found.has(a) && found.has(b), "a domain lost one of its own");
    assert!(
        !found.has(stranger),
        "another supervisor's child appeared in this domain: the scope is not the subtree",
    );

    // And the survey never reaches anything unsupervised, which is every other thread in the
    // system including the one running this assertion. That is the half a `/proc` cannot express.
    let me = sched::current();
    assert!(
        !found.has(me),
        "the surveying thread listed itself despite being supervised by nobody",
    );

    // The other rendezvous answers about its own child and only its own, from the same kernel walk.
    let other = hold_view(theirs);
    let mut buf = [ps::Row::default(); TEST_ROWS];
    let seen = walk(other, &mut buf);
    assert_eq!(seen.rows().len(), 1);
    assert_eq!(
        tids(&seen).next(),
        Some(stranger),
        "the second domain reported somebody else's child",
    );

    drain(parking, 3);
    // Cleanup goes through a *separate* supervisor capability on the same rendezvous points. The viewer
    // caps above cannot reap, which is the point: every walk in this test was made by a holder
    // that could not have collected what it saw.
    let sup_mine = hold_supervisor(mine);
    let sup_theirs = hold_supervisor(theirs);
    collect_all(sup_mine, &[a, b]);
    collect_all(sup_theirs, &[stranger]);
    tidy(
        budget,
        rendezvous_region,
        &[cap, other, sup_mine, sup_theirs],
    );
}

/// The negative control (milestone 126, the block's own requirement): a viewer not granted the
/// domain is refused loudly, not shown an empty list.
///
/// Through the real dispatcher: a send-only holder gets `NotPermitted`; a holder of nothing gets
/// `NoSuchSlot`, there being nothing to name; a `READ` holder of a genuinely empty domain gets
/// `DONE`, an answer, since its authority was never in question. A monitor that reported nothing
/// because it could not look would read like a quiet machine; `filesystem_contract` chose `EPERM`
/// over an empty listing for the same reason.
#[test_case]
fn a_viewer_without_the_domain_is_refused_rather_than_shown_an_empty_list() {
    let (budget, rendezvous_region) = arena();
    let ep = rendezvous(rendezvous_region);
    let parking = rendezvous(rendezvous_region);
    let child = child_in(budget, REPORT_STUB, Some(parking), ep);
    assert!(
        super::wait_for(|| sched::rendezvous_waiting_senders(parking) == 1),
        "the child never reached its send",
    );

    // Send-only: the rendezvous is real, the child is real, and the answer is a refusal.
    let peer = hold_write(ep);
    assert_eq!(
        survey(peer, 0),
        (Error::NotPermitted as i64, 0, 0),
        "a send-only holder was shown a domain it holds no right to look at",
    );
    let mut rbuf = [ps::Row::default(); TEST_ROWS];
    let refused = walk(peer, &mut rbuf);
    assert!(refused.is_refused());
    assert_eq!(refused.rows().len(), 0);

    // Nothing at all in the slot: a different refusal, and a louder one.
    let empty_slot = abi::CAPABILITY_TABLE_SLOTS - 1;
    assert!(
        sched::current_cap(empty_slot).is_err(),
        "pick an empty slot"
    );
    assert_eq!(
        survey(empty_slot, 0),
        (Error::NoSuchSlot as i64, 0, 0),
        "an ungranted viewer got something other than \"you hold no such capability\"",
    );

    // A domain that really is empty is an *answer*. This is the assertion that makes the two above
    // mean something: without it, "refused" and "nothing here" could be the same code path.
    let vacant = rendezvous(rendezvous_region);
    let held = hold_view(vacant);
    assert_eq!(
        survey(held, 0),
        (abi::survey::DONE as i64, 0, 0),
        "an empty domain must answer, not refuse",
    );
    let mut buf = [ps::Row::default(); TEST_ROWS];
    let seen = walk(held, &mut buf);
    assert!(
        !seen.is_refused(),
        "an empty domain was reported as a refusal, which is the confusion this method exists to \
         prevent",
    );
    assert_eq!(seen.rows().len(), 0);

    // And the holder that *is* the supervisor still sees its child, so the refusals above are
    // about the rights and not about a survey that never works.
    let sup = hold_supervisor(ep);
    assert_eq!(
        tids(&walk(sup, &mut [ps::Row::default(); TEST_ROWS])).next(),
        Some(child)
    );

    // A domain names its members and does not act on them (calef, 2026-08-17), and the two rights
    // separate both ways: a viewer holding `ENUMERATE` alone can look and cannot reap, and a
    // supervisor's plain `READ` handle can reap and cannot look, so a view is a grant somebody made.
    let viewer = hold_view(ep);
    assert_eq!(
        reap(viewer, child),
        Err(Error::NotPermitted),
        "a viewer reaped a member of the domain it was only supposed to be able to name",
    );
    let receiver = sched::grant(crate::cap::rendezvous_cap(ep, Rights::READ)).expect("grant");
    assert_eq!(
        survey(receiver, 0),
        (Error::NotPermitted as i64, 0, 0),
        "READ alone bought a domain listing, so the view is not a separate grant",
    );

    drain(parking, 1);
    collect_all(sup, &[child]);
    tidy(
        budget,
        rendezvous_region,
        &[peer, held, sup, viewer, receiver],
    );
}

/// **A corpse is still in the domain, and it says so.**
///
/// The state a supervisor most needs to see and the one Unix cannot show without a parent that
/// happened to call `wait`: the child faulted, its death message is waiting, and it persists until
/// `rendezvous::REAP` collects it (DECISIONS §26). A survey that dropped corpses would hide exactly
/// the thing a monitor is watching for.
///
/// Then the reap, through the same rendezvous, and the row is gone. The two halves are checked in one
/// test on purpose: the domain the viewer reports and the domain the supervisor may collect from
/// are one set, which is `capability::survey_includes`'s proved property observed on a real kernel.
#[test_case]
fn a_dead_child_is_still_in_the_domain_until_it_is_reaped() {
    let (budget, rendezvous_region) = arena();
    let ep = rendezvous(rendezvous_region);
    let child = child_in(budget, FAULT_STUB, None, ep);

    let cap = hold_supervisor(ep);
    // The corpse parks on its supervision rendezvous's sender queue with its death message when
    // nobody is in RECEIVE, which is how a survey can see it before anyone has collected the news.
    // The failure message carries the evidence, because a failure may come from a bench that is
    // not this machine (milestone 321): the count tells two senders from none, and
    // `thread_death_disposition` tells "never parked" from "parked, then removed".
    assert!(
        super::wait_for(|| sched::rendezvous_waiting_senders(ep) == 1),
        "the child never died onto its supervision rendezvous: {} senders parked on it, \
         and the child is {:?}",
        sched::rendezvous_waiting_senders(ep),
        sched::thread_death_disposition(child),
    );

    let mut buf = [ps::Row::default(); TEST_ROWS];
    let seen = walk(cap, &mut buf);
    assert_eq!(seen.rows().len(), 1, "the corpse fell out of its domain");
    assert_eq!(seen.rows()[0].tid, child);
    assert_eq!(
        seen.rows()[0].state,
        abi::survey::DEAD,
        "a corpse was reported as something other than dead",
    );

    // Collect it through the rendezvous the survey named it on, then the domain is empty and says so
    // rather than refusing.
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    invoke(&mut frame, cap, abi::rendezvous::RECEIVE, 0, 0, 0).expect("RECEIVE refused");
    // A refusal reaches userspace as one `NotPermitted` whatever stopped it, so the message names
    // the check that refused and the state it saw (milestone 321 (a corpse that did not park, on
    // four real cores)): a rare failure here has to be diagnosable from the log of a run nobody can
    // repeat on demand.
    let reaped = invoke(&mut frame, cap, abi::rendezvous::REAP, child, 0, 0);
    assert_eq!(
        reaped,
        Ok(0),
        "the tid a survey reported was not one the same rendezvous could reap; the reclaim \
         refused because {:?}",
        sched::last_reclaim_refusal(),
    );
    let mut buf = [ps::Row::default(); TEST_ROWS];
    let seen = walk(cap, &mut buf);
    assert!(!seen.is_refused());
    assert_eq!(
        seen.rows().len(),
        0,
        "a reaped corpse is still being listed"
    );

    tidy(budget, rendezvous_region, &[cap]);
}

/// A corpse still leaving its core is collected, not refused (milestone 321).
///
/// `depart` delivers a supervised thread's death under `IPC_TABLES`, then switches away, and
/// `handshake.on_cpu` stays set until that core's next `finish_switch`. A `REAP` in that window
/// meets `reclaim_region` refusing a stack a core stands on, and must wait rather than answer
/// `NotPermitted`, whose one ABI cause `terminal_supervisor` and `sub_server_supervisor` fail on.
/// The test holds the window open with a standing mark a helper clears after a delay.
///
/// Name: provisional, minted 2026-10-22 (UTC) by lane/321-reclaim-refused.
///
/// Falsification: replayable `system_tests/falsifications/user.survey_tests.a_corpse_still_leaving_its_core_is_reaped_once_it_has_left.patch`
#[test_case]
fn a_corpse_still_leaving_its_core_is_reaped_once_it_has_left() {
    static LEFT: AtomicBool = AtomicBool::new(false);
    LEFT.store(false, Ordering::SeqCst);

    let (budget, rendezvous_region) = arena();
    let ep = rendezvous(rendezvous_region);
    let child = child_in(budget, FAULT_STUB, None, ep);
    let supervisor = hold_supervisor(ep);
    assert!(
        super::wait_for(|| sched::rendezvous_waiting_senders(ep) == 1),
        "the child never died onto its supervision rendezvous",
    );

    // Once the real departure is over (`hold_on_its_core` refuses while the corpse genuinely
    // stands), so the real `finish_switch` cannot clear the mark this sets.
    assert!(
        super::wait_for(|| sched::hold_on_its_core(child, true)),
        "the corpse never left its core, or was not there to hold",
    );
    sched::spawn(move || {
        let until = crate::arch::timer::now() + crate::arch::timer::frequency() / 4;
        while crate::arch::timer::now() < until {
            sched::yield_now();
        }
        LEFT.store(true, Ordering::SeqCst);
        sched::hold_on_its_core(child, false);
    });

    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    invoke(&mut frame, supervisor, abi::rendezvous::RECEIVE, 0, 0, 0).expect("RECEIVE refused");
    assert!(
        !LEFT.load(Ordering::SeqCst),
        "the helper let the corpse go before the reap, so this run proves nothing",
    );
    assert_eq!(
        invoke(&mut frame, supervisor, abi::rendezvous::REAP, child, 0, 0),
        Ok(0),
        "a corpse still leaving its core was refused rather than waited for; the reclaim refused \
         because {:?}",
        sched::last_reclaim_refusal(),
    );
    assert!(
        LEFT.load(Ordering::SeqCst),
        "the reap freed a corpse a core still stood on"
    );

    tidy(budget, rendezvous_region, &[supervisor]);
}

/// **The cursor resumes, and it never repeats or skips a member.**
///
/// The walk gives `IPC_TABLES` back between entries, so the cursor is the only thing carrying the
/// position across calls (`generational_table::Table::iter_from`). A cursor that resolved to a *position* rather
/// than a slot would double-report or drop a member the moment anything changed, so what is checked
/// is the invariant that makes the shape safe: distinct tids, every one of them a real member.
///
/// It also pins the sizing claim `crates/ps` makes about its row buffer, which is otherwise one
/// number written in two crates.
#[test_case]
fn a_resumed_walk_reports_every_member_exactly_once() {
    let (budget, rendezvous_region) = arena();
    let ep = rendezvous(rendezvous_region);
    let parking = rendezvous(rendezvous_region);
    let a = child_in(budget, REPORT_STUB, Some(parking), ep);
    let b = child_in(budget, REPORT_STUB, Some(parking), ep);
    let c = child_in(budget, REPORT_STUB, Some(parking), ep);
    assert!(
        super::wait_for(|| sched::rendezvous_waiting_senders(parking) == 3),
        "the three children never reached their sends",
    );

    let cap = hold_view(ep);
    let mut buf = [ps::Row::default(); TEST_ROWS];
    let seen = walk(cap, &mut buf);
    assert_eq!(seen.rows().len(), 3, "the walk did not reach every member");

    let mut found = [0u64; 3];
    for (i, tid) in tids(&seen).enumerate() {
        found[i] = tid;
    }
    for &want in &[a, b, c] {
        assert_eq!(
            found.iter().filter(|&&t| t == want).count(),
            1,
            "tid {want} was reported {} times by one walk",
            found.iter().filter(|&&t| t == want).count(),
        );
    }

    // A cursor past the whole table is "nothing more", not a refusal: a caller that keeps feeding
    // back what it was given must never fall off the end into an answer that reads as "you may not
    // look".
    assert_eq!(survey(cap, u64::MAX), (abi::survey::DONE as i64, 0, 0));

    drain(parking, 3);
    let sup = hold_supervisor(ep);
    collect_all(sup, &[a, b, c]);
    tidy(budget, rendezvous_region, &[cap, sup]);
}

/// The cursor and the tid are machine-wide slot indices, so a viewer can count threads it cannot
/// see (the 2026-08-17 security audit's finding, stated as a test).
///
/// A viewer still cannot name a thread outside its domain (see
/// `a_domain_is_exactly_the_children_of_the_rendezvous_that_was_granted`). But `next_cursor` is the
/// machine-wide `generational_table::Table` slot + 1, and a tid's low half is that slot and its high
/// half the slot's machine-wide recycle count, so the gap between two members' cursors counts the
/// threads created between them. notes/process-view.md's `BUGS` has the disposition.
#[test_case]
fn the_survey_cursor_counts_threads_the_viewer_cannot_name() {
    let (budget, rendezvous_region) = arena();
    let mine = rendezvous(rendezvous_region);
    let theirs = rendezvous(rendezvous_region);
    let parking = rendezvous(rendezvous_region);

    // Creation order is the whole experiment: one of mine, then a stranger the viewer will never be
    // shown, then a second of mine. `generational_table::Table::insert_with` takes the lowest free slot, so the
    // three land in strictly increasing slots in this order and the stranger's is between the two
    // the viewer can see.
    let a = child_in(budget, REPORT_STUB, Some(parking), mine);
    let stranger = child_in(budget, REPORT_STUB, Some(parking), theirs);
    let b = child_in(budget, REPORT_STUB, Some(parking), mine);
    assert!(
        super::wait_for(|| sched::rendezvous_waiting_senders(parking) == 3),
        "the three children never reached their sends",
    );

    // Held the way `ps` holds it: `ENUMERATE` alone, the narrowest thing that can look at all.
    let cap = hold_view(mine);

    let (c1, t1, _) = survey(cap, 0);
    assert!(c1 > 0, "the viewer's own domain was refused or empty");
    let (c2, t2, _) = survey(cap, c1 as u64);
    assert!(
        c2 > 0,
        "the domain reported one member where two were built"
    );
    assert_eq!(
        survey(cap, c2 as u64),
        (abi::survey::DONE as i64, 0, 0),
        "a third entry appeared in a domain of two",
    );
    assert!(
        (t1 == a && t2 == b) || (t1 == b && t2 == a),
        "the domain reported a thread that is not one of its two members",
    );

    // **The tid carries the machine-wide slot index**, which is an ABI fact rather than a
    // scheduling accident: `generational_table::Table` packs a name as `(generation << 32) | slot` and the
    // cursor is that same slot plus one. Asserting the identity is what makes the leak a
    // measurement instead of an inference from reading the allocator.
    assert_eq!(
        (t1 & 0xffff_ffff) + 1,
        c1 as u64,
        "the tid's low half is no longer the slot the cursor reports",
    );
    assert_eq!(
        (t2 & 0xffff_ffff) + 1,
        c2 as u64,
        "the tid's low half is no longer the slot the cursor reports",
    );

    // **The finding.** Two members, and the cursor advances by more than one between them. The
    // extra step is the stranger: a thread this capability may not name, whose existence the
    // viewer can nonetheless count by subtracting. A cursor scoped to the domain, or an opaque
    // one, would advance by exactly one here.
    assert!(
        c2 - c1 >= 2,
        "the cursor advanced by {} between two adjacent members, so this test is no longer \
         measuring the leak it was written for (the allocator or the cursor changed shape)",
        c2 - c1,
    );

    // The stranger is still unnameable, which is the half that must not regress. The viewer knows
    // a thread is there and knows nothing else about it: not its tid, not its state, and REAP
    // through this capability is not even expressible (`hold_view` withholds `READ`).
    let mut buf = [ps::Row::default(); TEST_ROWS];
    let seen = walk(cap, &mut buf);
    let found = ThreadIdSet::of(&seen);
    assert!(
        !found.has(stranger),
        "the stranger became nameable, which is a different and much worse bug",
    );

    drain(parking, 3);
    let sup_mine = hold_supervisor(mine);
    let sup_theirs = hold_supervisor(theirs);
    collect_all(sup_mine, &[a, b]);
    collect_all(sup_theirs, &[stranger]);
    tidy(budget, rendezvous_region, &[cap, sup_mine, sup_theirs]);
}

/// `pgrep`: the filter over the same domain, and the fourth answer it adds (milestone 126).
///
/// The only place a real selector reaches the filter over a real domain: at the prompt the selector
/// arrives in a register, so a shell-spawned `pgrep` names every member (`crates/pgrep`'s `BUGS`).
/// Four distinct answers: members matched (`pgrep dead` names the corpse alone), nothing matched
/// (not the empty answer, not a refusal, which upstream's exit 1 conflates), the domain is empty
/// (`crates/ps`'s sentence), and refused (a send-only holder, milestone 108's shape). The tid
/// `pgrep` printed cannot be reaped through the capability that printed it, which is why there is
/// no `pkill`. See notes/process-view/pgrep.md.
#[test_case]
fn a_filter_names_members_and_tells_its_four_answers_apart() {
    let (budget, rendezvous_region) = arena();
    let ep = rendezvous(rendezvous_region);
    let parking = rendezvous(rendezvous_region);

    // Two live members blocked in a send, and one corpse. Three states in one domain is what makes a
    // selector's verdict mean anything: a filter over a uniform domain cannot be wrong.
    let live_a = child_in(budget, REPORT_STUB, Some(parking), ep);
    let live_b = child_in(budget, REPORT_STUB, Some(parking), ep);
    let corpse = child_in(budget, FAULT_STUB, None, ep);
    assert!(
        super::wait_for(|| sched::rendezvous_waiting_senders(parking) == 2),
        "the two live children never reached their sends",
    );
    assert!(
        super::wait_for(|| sched::rendezvous_waiting_senders(ep) == 1),
        "the third child never died onto its supervision rendezvous",
    );

    let viewer = hold_view(ep);
    let mut buf = [ps::Row::default(); TEST_ROWS];
    let seen = walk(viewer, &mut buf);
    assert_eq!(seen.rows().len(), 3, "the domain lost a member");

    // **The filter is over exactly the survey `ps` walked**, so an unfiltered selector and the
    // listing cannot disagree about how many members there are.
    let all = pgrep::select(&seen, pgrep::Selector::EVERY);
    assert_eq!(all.matched(), 3);
    assert_eq!(all.members(), seen.rows().len());

    // `pgrep dead` names the corpse and nothing else. This is the state Unix cannot show without a
    // parent that happened to call `wait`, and here it is a one-word selector.
    let dead = pgrep::select(&seen, pgrep::Selector::from_pattern(b"dead"));
    assert_eq!(dead.matched(), 1, "the corpse was not the only dead member");
    let mut printed = [0u8; 64];
    let n = render(&mut printed, |o| dead.write_report(o));
    assert!(
        is_the_one_thread_id(&printed[..n], corpse),
        "`pgrep dead` printed something other than the corpse's tid",
    );
    assert_eq!(
        render_len(|o| dead.write_diagnostics(o)),
        0,
        "a filter that matched said something on its second stream",
    );

    // **The fourth answer**: nothing is `ready`, and saying so is not the same as an empty domain
    // and not the same as a refusal.
    let missed = pgrep::select(&seen, pgrep::Selector::from_pattern(b"ready"));
    assert_eq!(missed.matched(), 0);
    let mut missed_diag = [0u8; 128];
    let missed_n = render(&mut missed_diag, |o| missed.write_diagnostics(o));
    assert!(
        contains(&missed_diag[..missed_n], b"none of the 3 processes"),
        "a selector that matched nothing did not say how many it looked at",
    );
    assert_eq!(
        render_len(|o| missed.write_report(o)),
        0,
        "nothing matched and something was printed anyway",
    );

    // The empty domain, from a second rendezvous nobody's child is under.
    let vacant = hold_view(rendezvous(rendezvous_region));
    let mut vbuf = [ps::Row::default(); TEST_ROWS];
    let vacant_walk = walk(vacant, &mut vbuf);
    let empty = pgrep::select(&vacant_walk, pgrep::Selector::EVERY);
    let mut empty_diag = [0u8; 128];
    let empty_n = render(&mut empty_diag, |o| empty.write_diagnostics(o));
    assert!(
        contains(&empty_diag[..empty_n], b"holds no processes"),
        "an empty domain was not reported as an empty domain",
    );

    // The refusal, from a send-only holder: a real relationship in this tree, a peer that reports to
    // this supervisor and may not look at it.
    let peer = hold_write(ep);
    let mut pbuf = [ps::Row::default(); TEST_ROWS];
    let denied = walk(peer, &mut pbuf);
    assert!(denied.is_refused());
    let refused = pgrep::select(&denied, pgrep::Selector::EVERY);
    let mut refused_diag = [0u8; 128];
    let refused_n = render(&mut refused_diag, |o| refused.write_diagnostics(o));
    assert!(
        contains(&refused_diag[..refused_n], b"not looked at"),
        "a refused walk was not reported as a refusal",
    );
    assert_eq!(
        render_len(|o| refused.write_report(o)),
        0,
        "a refused filter printed a list",
    );

    // **The three sentences are three sentences.** Without this, "nothing matched", "nothing is
    // here" and "you may not look" could share a code path and every assertion above would still
    // pass.
    assert!(
        missed_diag[..missed_n] != empty_diag[..empty_n]
            && empty_diag[..empty_n] != refused_diag[..refused_n]
            && missed_diag[..missed_n] != refused_diag[..refused_n],
        "two of the four answers produced the same bytes",
    );

    // **And naming the corpse bought no authority over it.** The viewer that just printed this tid
    // is refused the reap, so the answer `pgrep` gives is a name and not a handle. This is the whole
    // reason `pkill` is not a program in this system: there is nothing to point at the output.
    assert_eq!(
        reap(viewer, corpse),
        Err(Error::NotPermitted),
        "the capability that named a corpse could also collect it, so a domain confers control",
    );

    // Cleanup, through a supervisor capability rather than the viewer, which is the point restated
    // as a chore: every walk above was made by a holder that could not have collected what it saw.
    drain(parking, 2);
    let sup = hold_supervisor(ep);
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    invoke(&mut frame, sup, abi::rendezvous::RECEIVE, 0, 0, 0)
        .expect("the death message never arrived");
    collect_all(sup, &[corpse, live_a, live_b]);
    tidy(budget, rendezvous_region, &[viewer, vacant, peer, sup]);
}

/// Render a writer's bytes into `buf` and return how many landed. The kernel has no heap, so the
/// buffer is the caller's, which is `ps::collect`'s rule for the same reason.
///
/// Bytes past the end are **dropped and counted**, so a sentence that outgrew the buffer shows up as
/// a length equal to `buf.len()` rather than as a silently shorter string that a `contains` might
/// still match.
fn render(buf: &mut [u8], f: impl FnOnce(&mut dyn FnMut(&[u8]))) -> usize {
    let mut n = 0usize;
    f(&mut |bytes| {
        for &b in bytes {
            if n < buf.len() {
                buf[n] = b;
            }
            n += 1;
        }
    });
    assert!(n <= buf.len(), "a diagnostic outgrew this test's buffer");
    n
}

/// How many bytes a writer would write, for the assertions that only care whether it said anything.
fn render_len(f: impl FnOnce(&mut dyn FnMut(&[u8]))) -> usize {
    let mut n = 0usize;
    f(&mut |bytes| n += bytes.len());
    n
}

/// `haystack.contains(needle)` for bytes, because the kernel has no `str` machinery to lean on here.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Whether `printed` is exactly `tid` and a newline, which is `pgrep`'s whole output format: a name
/// per line, no header and no padding.
fn is_the_one_thread_id(printed: &[u8], tid: u64) -> bool {
    let mut want = [0u8; 21];
    let mut i = want.len() - 1;
    want[i] = b'\n';
    let mut v = tid;
    loop {
        i -= 1;
        want[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    printed == &want[i..]
}

/// A small membership check without allocating: the kernel has no heap, and these tests hold at
/// most three tids at a time.
struct ThreadIdSet {
    tids: [u64; 3],
    n: usize,
}

impl ThreadIdSet {
    fn of(s: &ps::Survey) -> ThreadIdSet {
        let mut set = ThreadIdSet { tids: [0; 3], n: 0 };
        for r in s.rows().iter().take(3) {
            set.tids[set.n] = r.tid;
            set.n += 1;
        }
        set
    }

    fn has(&self, tid: u64) -> bool {
        self.tids[..self.n].contains(&tid)
    }
}

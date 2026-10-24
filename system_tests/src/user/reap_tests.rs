use abi::Error;
use abi::fault::{EVENT_EXIT, EVENT_FAULT};

use super::supervision_tests::{FAULT_STUB, REPORT_STUB, REPORT_WORD, build_child_in};
use crate::arch::exceptions::TrapFrame;
use crate::cap::{Object, Rights};
use crate::sched;
use crate::syscall::invoke;

/// The builder's whole budget. Room for three instances plus slack, so the LIFO return-of-pages
/// (§16) can be watched happening more than once.
const BUILDER_BUDGET_PAGES: u64 = 80;

/// Pages per instance region: the child's address space (root and tables), its code page, its
/// stack page, and its TCB. Sixteen is what `build_child` has always carved.
const INSTANCE_PAGES: u64 = 16;

/// Pages for the test's own rendezvous points, one per rendezvous (`RETYPE_OBJ`'s
/// one-object-per-page rule). Two is the most any one test here needs; four is slack.
const RENDEZVOUS_PAGES: u64 = 4;

/// The slot half of a generational name (`crates/slots` packs generation in the high 32 bits,
/// slot in the low 32). The recycled-tid test needs it to assert it is genuinely replaying a
/// name that now *aliases a live thread's slot*, which is the only version of that test worth
/// running.
const SLOT_MASK: u64 = 0xffff_ffff;

/// One test's world: a budget the *builder* owns, and a small region the test's rendezvous points
/// are retyped from. The rendezvous points come out of a reclaimable region rather than
/// `create_rendezvous`'s shared kernel one so that `tidy` can give them back; six tests each
/// leaking a couple of rendezvous points exhausted the kernel's rendezvous budget for every test
/// that ran afterwards, which is the sort of failure a test should not be able to inflict on its
/// neighbours.
fn arena() -> (u64, u64) {
    let budget = crate::memory_region::create(BUILDER_BUDGET_PAGES).expect("no builder budget");
    let rendezvous_region =
        crate::memory_region::create(RENDEZVOUS_PAGES).expect("no rendezvous region");
    (budget, rendezvous_region)
}

/// An rendezvous out of the test's own region (one page each).
fn rendezvous(region: u64) -> sched::RendezvousId {
    sched::create_rendezvous_from(region).expect("no rendezvous")
}

/// Hold a supervision rendezvous the way a supervisor holds one: `READ` alone, the right to
/// receive deaths here. No `WRITE` (it is not a sender on its own children's death channel) and
/// no `GRANT`.
fn hold_rendezvous(ep: sched::RendezvousId) -> u64 {
    sched::grant(crate::cap::rendezvous_cap(ep, Rights::READ)).expect("grant the rendezvous")
}

/// `invoke(cap, REAP, tid, _, _)`, through the real dispatcher.
fn reap(slot: u64, tid: u64) -> Result<i64, Error> {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    invoke(&mut frame, slot, abi::rendezvous::REAP, tid, 0, 0)
}

/// **`reap`, retried while the corpse is still standing on its own kernel stack.**
///
/// The refusal these tests race is transient, one context switch wide, and every test here loses
/// it by design. `depart` publishes a supervised thread `Dead` and parks it on its supervision
/// rendezvous before it reaches `switch_to`, so when a test sees the death message or the parked
/// sender the corpse is still executing, and `reap_region_objects` refuses to unmap a stack a core
/// stands on (`NotPermitted`). See notes/stack/kernel-stack-freed-under-its-owner.md and milestone
/// 124 (a thread is born where it lives).
///
/// A test meaning "refused for good" calls [`reap`] and asserts on it; this is for "it succeeds".
fn reap_when_settled(slot: u64, tid: u64) -> Result<i64, Error> {
    let mut answer = reap(slot, tid);
    super::wait_for(|| {
        if answer != Err(Error::NotPermitted) {
            return true;
        }
        answer = reap(slot, tid);
        answer != Err(Error::NotPermitted)
    });
    answer
}

/// Receive one five-word death message through the ABI, so the tid a test reaps with is the tid
/// a real supervisor would have read out of its registers.
fn receive_death(slot: u64) -> [u64; 5] {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    let w0 = invoke(&mut frame, slot, abi::rendezvous::RECEIVE, 0, 0, 0).expect("RECEIVE refused");
    [
        w0 as u64,
        frame.arg(1),
        frame.arg(2),
        frame.arg(3),
        frame.arg(4),
    ]
}

/// How many capability slots the supervisor occupies. The "no richer than before" measurement:
/// a reap that handed authority back would have to put it somewhere, and every path that gives a
/// process a capability lands it in a slot.
fn occupied_slots() -> usize {
    (0..abi::CAPABILITY_TABLE_SLOTS)
        .filter(|&s| sched::current_cap(s).is_ok())
        .count()
}

/// **The supervisor cannot construct anything, audited slot by slot.**
///
/// For every empty slot, the two primitives that build a process (`RETYPE_OBJ` for a TCB and for an
/// address space) are driven through the real dispatcher and must answer `NoSuchSlot`: there is
/// nothing there, which is what no-ambient-authority feels like, and it is a different fact from
/// `NotPermitted` (something there, restricted). For every occupied slot, the capability must be an
/// `Rendezvous`, which cannot build by dispatch: the rendezvous arm of `invoke` offers send,
/// receive, delegate, call, and reap, and no constructor at all. `MemoryRegion` methods are *not*
/// invoked on those slots, because method numbers are per-object-type and `RETYPE_OBJ`'s number is
/// `SEND_CAP`'s on an rendezvous.
fn assert_can_only_supervise(expected: &[u64]) {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    for slot in 0..abi::CAPABILITY_TABLE_SLOTS {
        match sched::current_cap(slot) {
            Err(_) => {
                for objtype in [
                    abi::objtype::THREAD_CONTROL_BLOCK,
                    abi::objtype::ADDRESS_SPACE,
                ] {
                    assert_eq!(
                        invoke(
                            &mut frame,
                            slot,
                            abi::memory_region::RETYPE_OBJ,
                            objtype,
                            0,
                            0
                        ),
                        Err(Error::NoSuchSlot),
                        "slot {slot} answered something other than \"you hold no such \
                         capability\" to a construction request",
                    );
                }
            }
            Ok(cap) => {
                assert!(
                    expected.contains(&slot),
                    "the supervisor holds an unexpected capability in slot {slot}: this test's \
                     confinement claim is only about a capability table it fully accounts for",
                );
                assert!(
                    matches!(cap.object, Object::Rendezvous(..)),
                    "the supervisor holds a non-rendezvous capability in slot {slot}",
                );
            }
        }
    }
}

/// Give everything back: the supervisor's capability slots, then the builder's budget (which
/// reclaims any instance region still under it), then the rendezvous region. In that order,
/// because reclaiming the rendezvous points first would revoke the channels the corpses are still
/// attached to.
fn tidy(budget: u64, rendezvous_region: u64, slots: &[u64]) {
    for &s in slots {
        let _ = sched::delete_current_cap(s);
    }
    sched::reclaim_region(budget).expect("the builder's own budget did not come back");
    sched::reclaim_region(rendezvous_region)
        .expect("the test's rendezvous region did not come back");
}

/// **The headline: a supervisor that cannot build anything collects its dead child.**
///
/// Its entire authority is one supervision rendezvous with `READ`. It holds no untyped, no frame,
/// no TCB, and no address space, and the audit proves that from the inside, on the two
/// primitives that build a process. Then it reaps, through the rendezvous, naming the tid the
/// kernel stamped on the death message it just received. Before §32 this required `WRITE` on the
/// child's region, which is the same right that builds an arbitrary process out of it.
#[test_case]
fn a_supervisor_holding_only_its_rendezvous_reaps_its_dead_child() {
    let (budget, rendezvous_region) = arena();
    let fault_ep = rendezvous(rendezvous_region);
    let instance = crate::memory_region::split(budget, INSTANCE_PAGES).expect("no instance region");
    let child = build_child_in(instance, FAULT_STUB, None, Some(fault_ep));

    let cap = hold_rendezvous(fault_ep);
    assert_can_only_supervise(&[cap]);

    let msg = receive_death(cap);
    assert_eq!(msg[0], EVENT_FAULT, "the child should have crashed");
    assert_eq!(msg[1], child, "the death message named the wrong thread");

    assert_eq!(
        reap_when_settled(cap, msg[1]),
        Ok(0),
        "a supervisor holding only its supervision rendezvous could not collect its own corpse",
    );
    assert_eq!(
        sched::corpse_fault_msg(child),
        None,
        "the corpse survived the reap",
    );
    // The reap did not turn into authority: the same audit still holds, and the same tid is now
    // an unknown name rather than a second free reap.
    assert_can_only_supervise(&[cap]);
    assert_eq!(reap(cap, child), Err(Error::NotSupervised));

    tidy(budget, rendezvous_region, &[cap]);
}

/// **The reclaimed region returns to the builder, not to the reaper** (§32's first consequence,
/// and the property that makes the decision worth having).
///
/// The builder splits an instance region off its own budget, which bumps its watermark; the
/// supervisor reaps; the watermark comes back down (§16's LIFO return-of-pages) and the builder
/// can spend those pages again. The supervisor, meanwhile, occupies exactly the slots it did
/// before and still holds nothing but an rendezvous. A reap that had quietly credited the reaper
/// would fail the first pair of assertions; one that had merely freed the corpse without
/// returning the memory would fail the second.
#[test_case]
fn the_reaped_region_returns_to_the_builder_not_the_reaper() {
    let (budget, rendezvous_region) = arena();
    let fault_ep = rendezvous(rendezvous_region);
    let (spent_before, _) = crate::memory_region::usage(budget).expect("the budget exists");

    let instance = crate::memory_region::split(budget, INSTANCE_PAGES).expect("no instance region");
    assert_eq!(
        crate::memory_region::usage(budget).unwrap().0,
        spent_before + INSTANCE_PAGES,
        "the split did not come out of the builder's budget",
    );
    let child = build_child_in(instance, FAULT_STUB, None, Some(fault_ep));

    let cap = hold_rendezvous(fault_ep);
    let msg = receive_death(cap);
    assert_eq!(msg[1], child);
    let slots_before = occupied_slots();

    assert_eq!(reap_when_settled(cap, msg[1]), Ok(0), "the reap failed");

    assert_eq!(
        crate::memory_region::usage(budget).unwrap().0,
        spent_before,
        "the reaped pages did not go back to the builder's budget: the reaper freed the corpse \
         and stranded its memory",
    );
    assert!(
        !crate::memory_region::has_children(budget),
        "the instance region outlived its corpse, so the builder's budget is still committed",
    );
    // Genuinely spendable again, not merely un-bumped bookkeeping.
    let again = crate::memory_region::split(budget, INSTANCE_PAGES)
        .expect("the builder could not re-spend the pages the reap returned");
    sched::reclaim_region(again).expect("reclaim the re-split region");

    assert_eq!(
        occupied_slots(),
        slots_before,
        "the supervisor gained a capability by reaping",
    );
    assert_can_only_supervise(&[cap]);

    tidy(budget, rendezvous_region, &[cap]);
}

/// **A live child is refused, with an error of its own.** Collecting a corpse is not killing.
/// The child here is blocked in a `SEND` nobody has received, so it is provably alive at the
/// moment of the attempt, and the refusal is `StillAlive` rather than a generic `NotPermitted`:
/// a restart policy needs "wait, or escalate to the owner's `DESTROY`" to be distinguishable
/// from "there is no such child of mine". Then the child is let go, dies, and the *same* tid
/// through the *same* rendezvous succeeds, which is what proves the refusal was about liveness and
/// not about authority.
#[test_case]
fn reap_refuses_a_live_child_with_a_distinct_error() {
    let (budget, rendezvous_region) = arena();
    let report = rendezvous(rendezvous_region);
    let fault_ep = rendezvous(rendezvous_region);
    let instance = crate::memory_region::split(budget, INSTANCE_PAGES).expect("no instance region");
    let child = build_child_in(instance, REPORT_STUB, Some(report), Some(fault_ep));

    let cap = hold_rendezvous(fault_ep);
    assert_eq!(
        reap(cap, child),
        Err(Error::StillAlive),
        "a supervisor collected a thread that was still running: REAP authorizes collecting a \
         corpse, never killing",
    );

    // Let it finish. It SENDs, we receive, it exits, and its death arrives on the rendezvous.
    assert_eq!(
        sched::ipc_receive(report)[0],
        REPORT_WORD,
        "the child never ran"
    );
    let msg = receive_death(cap);
    assert_eq!(msg[0], EVENT_EXIT, "a clean exit must report EXIT");
    assert_eq!(msg[1], child);
    assert_eq!(
        reap(cap, child),
        Ok(0),
        "the same tid through the same rendezvous failed once dead: the earlier refusal was not \
         about liveness after all",
    );

    tidy(budget, rendezvous_region, &[cap]);
}

/// **Another supervisor's child is refused, even to a holder of both rendezvous points.**
///
/// The sharpest form of §32's authorization rule: one process holds two supervision rendezvous
/// points, and learns a tid legitimately, through the rendezvous that supervises it. Naming that
/// tid on the *other* rendezvous is refused, because authorization is the `(tid, rendezvous)`
/// relationship and not "am I a supervisor". So a tid is a name inside a relationship, never a
/// global handle, which is what lets §26's kernel-stamped tid be reused for this with no new
/// bookkeeping.
#[test_case]
fn reap_refuses_another_supervisors_child() {
    let (budget, rendezvous_region) = arena();
    let mine = rendezvous(rendezvous_region);
    let theirs = rendezvous(rendezvous_region);
    let instance = crate::memory_region::split(budget, INSTANCE_PAGES).expect("no instance region");
    let child = build_child_in(instance, FAULT_STUB, None, Some(theirs));

    let cap_mine = hold_rendezvous(mine);
    let cap_theirs = hold_rendezvous(theirs);

    let msg = receive_death(cap_theirs);
    assert_eq!(msg[1], child, "the death arrived on the wrong rendezvous");
    assert_eq!(
        reap(cap_mine, child),
        Err(Error::NotSupervised),
        "a corpse was collected through an rendezvous that does not supervise it",
    );
    assert!(
        sched::corpse_fault_msg(child).is_some(),
        "the refused reap still tore something down",
    );

    assert_eq!(
        reap(cap_theirs, child),
        Ok(0),
        "the supervising rendezvous could not collect its own corpse",
    );

    tidy(budget, rendezvous_region, &[cap_mine, cap_theirs]);
}

/// **A recycled tid is refused, not resolved to the thread now in that slot.**
///
/// Generational names (`crates/slots`) exist for exactly this, and this is the test that proves
/// the reap path actually uses them: reap a child, build a replacement that lands in the freed
/// thread-table slot with a bumped generation, then replay the old tid. The replay must be
/// refused, and the replacement must be untouched, which is asserted by letting it run to
/// completion afterwards rather than by inspecting it. The slot-aliasing assertion is part of
/// the test: without it, the replay would be of a name nothing has reused and would prove
/// nothing.
#[test_case]
fn reap_refuses_a_recycled_thread_id_rather_than_the_wrong_thread() {
    let (budget, rendezvous_region) = arena();
    let fault_ep = rendezvous(rendezvous_region);
    let report = rendezvous(rendezvous_region);
    let cap = hold_rendezvous(fault_ep);

    let first_region =
        crate::memory_region::split(budget, INSTANCE_PAGES).expect("no first region");
    let first = build_child_in(first_region, FAULT_STUB, None, Some(fault_ep));
    assert_eq!(receive_death(cap)[1], first);
    assert_eq!(
        reap_when_settled(cap, first),
        Ok(0),
        "the first reap failed"
    );

    let second_region =
        crate::memory_region::split(budget, INSTANCE_PAGES).expect("no second region");
    let second = build_child_in(second_region, REPORT_STUB, Some(report), Some(fault_ep));
    assert_eq!(
        second & SLOT_MASK,
        first & SLOT_MASK,
        "the replacement did not land in the reaped thread's table slot, so replaying the old \
         tid would not be aliasing anything: this test needs the reuse to mean something",
    );
    assert_ne!(
        second, first,
        "a reused slot handed out the same name: the generation did not bump",
    );

    assert_eq!(
        reap(cap, first),
        Err(Error::NotSupervised),
        "a stale tid resolved: the reap path is not going through the generational name",
    );
    // The replacement is untouched, proven by it running to completion.
    assert_eq!(
        sched::ipc_receive(report)[0],
        REPORT_WORD,
        "the replayed tid reaped the thread that had recycled its slot",
    );
    let msg = receive_death(cap);
    assert_eq!(msg[1], second);
    assert_eq!(reap_when_settled(cap, second), Ok(0));

    tidy(budget, rendezvous_region, &[cap]);
}

/// **Reaping a corpse whose death message was never collected leaves the rendezvous clean.**
///
/// A supervised thread that dies with nobody in `RECEIVE` parks on its supervision rendezvous's
/// sender queue holding the message (§26 (the fault endpoint) implementation note 2). A supervisor
/// need not collect it before reaping (its builder may tell it the tid), so the reap unlinks the
/// corpse from that queue before freeing its TCB, or the next `RECEIVE` follows a pointer into a
/// recycled page. `crates/inter_process_communication`'s `remove_sender` does the unlink, for
/// `MemoryRegion::DESTROY` as for `REAP`. The second half (a fresh child's death arriving normally
/// on the same rendezvous) proves the queue intact, not merely counted right.
#[test_case]
fn reaping_an_uncollected_corpse_leaves_no_ghost_on_the_rendezvous() {
    let (budget, rendezvous_region) = arena();
    let fault_ep = rendezvous(rendezvous_region);
    let instance = crate::memory_region::split(budget, INSTANCE_PAGES).expect("no instance region");
    let child = build_child_in(instance, FAULT_STUB, None, Some(fault_ep));
    let cap = hold_rendezvous(fault_ep);

    // Nobody is receiving, so the corpse must park. Wait for it **on the clock**: 4000 yields was
    // the wait, and a yield count is not a duration. On the physical core under HVF (milestone 81)
    // this core spends 4000 yields in microseconds while the child's core has not run it yet, and
    // the assertion fails with "the corpse never parked" about a corpse that was on its way.
    assert!(
        super::wait_for(|| sched::rendezvous_waiting_senders(fault_ep) == 1),
        "the corpse never parked on its supervision rendezvous",
    );

    assert_eq!(reap_when_settled(cap, child), Ok(0), "the reap failed");
    assert_eq!(
        sched::rendezvous_waiting_senders(fault_ep),
        0,
        "a freed TCB is still linked into the supervision rendezvous's sender queue: the next \
         RECEIVE would follow a dangling pointer into a recycled page",
    );

    // The rendezvous still works, which is the real assertion: a stale head or tail would show up
    // here and not in the count above.
    let next_region =
        crate::memory_region::split(budget, INSTANCE_PAGES).expect("no second region");
    let next = build_child_in(next_region, FAULT_STUB, None, Some(fault_ep));
    let msg = receive_death(cap);
    assert_eq!(
        msg[1], next,
        "the rendezvous delivered something other than the new child's death",
    );
    assert_eq!(reap_when_settled(cap, next), Ok(0));

    tidy(budget, rendezvous_region, &[cap]);
}

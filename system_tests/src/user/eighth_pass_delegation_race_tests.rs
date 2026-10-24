//! **Revocation against a delegation that is still being made** (an eighth outsider pass at the
//! confinement claim, claim 30's race window and the PortRange sweep).
//!
//! `revocation_in_flight_tests` revokes a capability already parked in the hand-off slot. This
//! module attacks the two gaps its `BUGS` names: a revoke between `SEND_CAP`'s rights gate and the
//! filing of the copy, forced at the test kernel's seam (`delegation_pause::here`), and the
//! PortRange sweep through a parked hand-off. Every wait is clock-bounded. The first test is
//! cross-ISA (DECISIONS §19 (architectural parity is a tenet)); the second is `x86_64` only,
//! because port I/O is. See notes/confinement-claims/the-eighth-pass-tests.md.

use core::sync::atomic::{AtomicBool, AtomicI64, Ordering};

use crate::arch::exceptions::TrapFrame;
// The `cap` module and these names still carry their pre-sweep spelling
// (design/naming/capability-spelling-worklist.md renames them when the tree is quiet); aliased here so
// this file spells `capability` out everywhere a reader looks (calef, 2026-10-06).
use crate::cap::{
    Object, Rights, page_frame_run_cap as page_frame_run_capability, page_frame_run_len,
    rendezvous_cap as rendezvous_capability,
};
use crate::sched;
use crate::syscall::invoke;

/// What the paused sender's `SEND_CAP` answered, as one word: the syscall's return on success, or
/// the `Error` code negated the way `abi` represents it (`Error as i64` is already negative).
const UNANSWERED: i64 = i64::MIN;

/// `invoke` through the real dispatcher, the way a program's `svc`/`ecall` reaches it.
fn call(slot: u64, method: u64, a0: u64, a1: u64, a2: u64) -> Result<i64, abi::Error> {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    invoke(&mut frame, slot, method, a0, a1, a2)
}

/// **A revocation that lands after `SEND_CAP` has started and before the copy is filed must still
/// be seen by the delegation.**
///
/// The sender parks at the seam mid-`SEND_CAP` with no lock held. The test revokes its frame,
/// asserting the sweep emptied the sender's own slot, and releases it. The delegation must answer
/// `NoSuchSlot` (it re-read a source that is gone) and park nothing.
/// [`the_same_seam_without_a_revoke_delivers`] is the control run.
///
/// Falsification: unfalsified. Written by the eighth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn a_revocation_landing_inside_the_delegation_window_is_still_seen() {
    let region = crate::memory_region::create(4).expect("no region");
    let ep = sched::create_rendezvous_from(region).expect("no rendezvous");
    let phys = crate::memory_region::retype_page(region).expect("no page to revoke");

    static ANSWER: AtomicI64 = AtomicI64::new(UNANSWERED);
    static SOURCE_SURVIVED: AtomicBool = AtomicBool::new(true);
    crate::delegation_pause::disarm();
    ANSWER.store(UNANSWERED, Ordering::SeqCst);

    sched::spawn(move || {
        let frame_slot = sched::grant(page_frame_run_capability(
            phys,
            page_frame_run_len(1),
            Rights::ALL,
        ))
        .expect("the sender could not be granted the frame");
        let door = sched::grant(rendezvous_capability(ep, Rights::WRITE))
            .expect("the sender could not be granted the endpoint");
        crate::delegation_pause::arm(sched::current());
        let answer = call(
            door,
            abi::rendezvous::SEND_CAP,
            frame_slot,
            Rights::ALL.bits() as u64,
            0,
        );
        SOURCE_SURVIVED.store(sched::current_cap(frame_slot).is_ok(), Ordering::SeqCst);
        ANSWER.store(answer.unwrap_or_else(|e| e as i64), Ordering::SeqCst);
    })
    .expect("no sender thread");

    assert!(
        super::wait_for(crate::delegation_pause::is_parked),
        "the sender never reached the seam, so the window was never forced",
    );
    assert_eq!(
        sched::rendezvous_waiting_senders(ep),
        0,
        "the sender parked on the rendezvous before the seam, so this is not the window under test",
    );

    crate::revoke::revoke_page_frame(phys);

    crate::delegation_pause::release();
    assert!(
        super::wait_for(|| ANSWER.load(Ordering::SeqCst) != UNANSWERED),
        "the sender never answered, so the delegation never finished or never restarted",
    );
    assert_eq!(
        ANSWER.load(Ordering::SeqCst),
        abi::Error::NoSuchSlot as i64,
        "the delegation answered something other than NoSuchSlot after its source was revoked \
         mid-flight: the copy was filed out of a window the sweep had already swept",
    );
    assert_eq!(
        sched::rendezvous_waiting_senders(ep),
        0,
        "a sender is parked on the rendezvous with a revoked capability in its hand-off slot",
    );
    assert!(
        super::wait_for(|| !SOURCE_SURVIVED.load(Ordering::SeqCst)),
        "the revoke never reached the sender's own table, so this test's premise is false",
    );

    the_same_seam_without_a_revoke_delivers(region, ep);
    crate::memory_region::destroy(region);
}

/// The control run for [`a_revocation_landing_inside_the_delegation_window_is_still_seen`]: the
/// same seam with no revoke. The delegation must complete, park, and deliver exactly the frame,
/// which is what makes the refused run's emptiness the revocation talking.
fn the_same_seam_without_a_revoke_delivers(region: u64, ep: sched::RendezvousId) {
    static CONTROL_ANSWER: AtomicI64 = AtomicI64::new(UNANSWERED);
    static CONTROL_DONE: AtomicBool = AtomicBool::new(false);
    crate::delegation_pause::disarm();
    CONTROL_ANSWER.store(UNANSWERED, Ordering::SeqCst);

    let phys_b = crate::memory_region::retype_page(region).expect("no control page");
    sched::spawn(move || {
        let frame_slot = sched::grant(page_frame_run_capability(
            phys_b,
            page_frame_run_len(1),
            Rights::ALL,
        ))
        .expect("the control sender could not be granted the frame");
        let door = sched::grant(rendezvous_capability(ep, Rights::WRITE))
            .expect("the control sender could not be granted the endpoint");
        crate::delegation_pause::arm(sched::current());
        let answer = call(
            door,
            abi::rendezvous::SEND_CAP,
            frame_slot,
            Rights::ALL.bits() as u64,
            0,
        );
        CONTROL_ANSWER.store(answer.unwrap_or_else(|e| e as i64), Ordering::SeqCst);
        CONTROL_DONE.store(true, Ordering::SeqCst);
    })
    .expect("no control sender thread");

    assert!(
        super::wait_for(crate::delegation_pause::is_parked),
        "the control sender never reached the seam",
    );
    crate::delegation_pause::release();
    assert!(
        super::wait_for(|| sched::rendezvous_waiting_senders(ep) == 1),
        "the control delegation never parked, so the refused run's emptiness proved nothing",
    );
    let [_word, slot, _second, _badge, ..] = sched::ipc_receive_cap(ep);
    assert_ne!(
        slot,
        abi::rendezvous::NO_CAP,
        "the control delivery was empty"
    );
    let delivered = sched::current_cap(slot).expect("the delivered slot is empty");
    assert_eq!(
        delivered.object,
        Object::PageFrame(phys_b, page_frame_run_len(1)),
        "the control delivery was not the delegated frame",
    );
    let _ = sched::delete_current_cap(slot);
    assert!(
        super::wait_for(|| CONTROL_DONE.load(Ordering::SeqCst)),
        "the control sender never returned from its send",
    );
    assert_eq!(
        CONTROL_ANSWER.load(Ordering::SeqCst),
        0,
        "the control delegation, unrevoked, did not succeed",
    );

    crate::revoke::revoke_page_frame(phys_b);
}

/// **A port capability revoked while it sits in the hand-off slot must not reach the receiver**
/// (`x86_64`).
///
/// The PageFrame sweep's twin for the third sweep named in the cited test's `BUGS`: a sender parks
/// a `PortRange` capability on a rendezvous, the whole COM1 range is revoked, and the receiver
/// must be handed nothing that names the range. A delivery here is a revived port grant the
/// revoker believed it took back, on the one architecture whose I/O space it can reach.
///
/// Falsification: unfalsified. Written by the eighth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[cfg(all(test, target_arch = "x86_64"))]
#[test_case]
fn a_port_capability_revoked_while_in_flight_does_not_reach_the_receiver() {
    use crate::cap::port_range_cap as port_range_capability;

    let region = crate::memory_region::create(4).expect("no region");
    let ep = sched::create_rendezvous_from(region).expect("no rendezvous");
    const PORT_BASE: u16 = crate::user::X86_COM1_PORT_BASE;
    const PORT_COUNT: u16 = 8;

    static PARKED: AtomicBool = AtomicBool::new(false);
    static SOURCE_SURVIVED: AtomicBool = AtomicBool::new(true);

    sched::spawn(move || {
        let slot = sched::grant(port_range_capability(PORT_BASE, PORT_COUNT, Rights::ALL))
            .expect("the sender could not be granted the range");
        let capability = sched::current_cap(slot).expect("the grant did not land");
        PARKED.store(true, Ordering::SeqCst);
        sched::ipc_send_cap(ep, 0, capability, 0);
        SOURCE_SURVIVED.store(sched::current_cap(slot).is_ok(), Ordering::SeqCst);
    })
    .expect("no sender thread");

    assert!(
        super::wait_for(
            || PARKED.load(Ordering::SeqCst) && sched::rendezvous_waiting_senders(ep) == 1
        ),
        "the sender never parked on the rendezvous, so nothing was ever in flight to revoke",
    );

    crate::revoke::revoke_port_range(PORT_BASE, PORT_COUNT);

    let [_word, slot, _second, _badge, ..] = sched::ipc_receive_cap(ep);
    let delivered = if slot == abi::rendezvous::NO_CAP {
        None
    } else {
        sched::current_cap(slot).ok()
    };

    assert!(
        super::wait_for(|| !SOURCE_SURVIVED.load(Ordering::SeqCst)),
        "the revoke did not reach the sender's own capability table, so this test's premise is \
         false and its result means nothing",
    );
    assert!(
        !matches!(delivered, Some(c) if c.object == Object::PortRange(PORT_BASE, PORT_COUNT)),
        "a revoked PortRange capability was delivered out of the hand-off slot: the sweep cleared \
         every table and not `outgoing_cap`, so the receiver now holds I/O authority the revoker \
         took back",
    );

    crate::memory_region::destroy(region);
}

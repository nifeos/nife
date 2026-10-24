//! Revocation against a capability that is in flight, parked in a rendezvous hand-off slot.
//!
//! Every revocation sweep walks capability tables, and a capability handed to a rendezvous whose
//! receiver has not arrived is in none: it waits in `Thread::outgoing_cap`, which
//! `sched::ipc_send_cap` writes and `sched::ipc_receive_cap` takes. The `PageFrame`, `DeviceFrame`
//! and `PortRange` sweeps clear that slot, as `sched::delete_reply_caps_naming` does. A slot left
//! unswept reopens the use-after-free DECISIONS §13 (capability revocation and untyped reclamation)
//! exists to prevent. See notes/confinement-claims/adversarial-pass-2026-09-21.md.

//! # BUGS
//!
//! - Only the `PageFrame` sweep is driven; the `DeviceFrame` and `PortRange` sweeps are reasoned
//!   from the code.
//! - A delivered `PortRange` capability never sets `Thread::port_range_grant`; it fails closed and
//!   is untested (see the note).
//! - The test proves delivery, not exploitation: it stops before mapping the revoked page.

use core::sync::atomic::{AtomicBool, Ordering};

use crate::cap::{Object, Rights, page_frame_run_cap, page_frame_run_len};
use crate::sched;

/// A capability revoked while it sits in the hand-off slot must not reach the receiver.
///
/// A sender holds `PageFrame(phys, 1)` and hands a copy to a rendezvous nobody receives on, so
/// `ipc_send_cap` parks it in `outgoing_cap`. `PageFrame::REVOKE` then runs over the frame,
/// whole-machine, and the receiver arrives and must be handed nothing. The headline is the last
/// assertion and every path reaches it, since `ipc_receive_cap` returns `NO_CAP` or a slot and both
/// are inspected. Above it sit a vacuity guard (the sender really parked) and a premise check (the
/// revoke reached the sender's own table); either failing means the test proved nothing.
///
/// Falsification: replayable `system_tests/falsifications/user.revocation_in_flight_tests.a_capability_revoked_while_it_is_in_flight_does_not_reach_the_receiver.patch`
#[test_case]
fn a_capability_revoked_while_it_is_in_flight_does_not_reach_the_receiver() {
    static PARKED: AtomicBool = AtomicBool::new(false);
    static RETURNED: AtomicBool = AtomicBool::new(false);
    /// Did the sender still hold the frame in its **own table** once its send completed? The
    /// premise check: a `false` here is the revoke having reached the one holder a sweep can see.
    static SENDER_STILL_HELD: AtomicBool = AtomicBool::new(true);

    let region = crate::memory_region::create(4).expect("no region");
    let ep = sched::create_rendezvous_from(region).expect("no rendezvous");
    let phys = crate::memory_region::retype_page(region).expect("no page to revoke");

    // The sender holds the capability in its table, the way a real delegator does, and hands a copy
    // to the rendezvous. Nobody is receiving, so `ipc_send_cap` parks the copy in `outgoing_cap`
    // and blocks. Holding it in the table too is what the premise check reads.
    sched::spawn(move || {
        let slot = sched::grant(page_frame_run_cap(phys, page_frame_run_len(1), Rights::ALL))
            .expect("the sender could not be granted the frame it is about to delegate");
        let cap = sched::current_cap(slot).expect("the grant did not land");
        PARKED.store(true, Ordering::SeqCst);
        sched::ipc_send_cap(ep, 0, cap, 0);
        SENDER_STILL_HELD.store(sched::current_cap(slot).is_ok(), Ordering::SeqCst);
        RETURNED.store(true, Ordering::SeqCst);
    })
    .expect("no sender thread");

    // Clock-bounded rather than yield-counted: under DECISIONS §28 (SMP placement at spawn) the
    // sender is on some other core, so this core's yields say nothing about whether it has run.
    assert!(
        super::wait_for(
            || PARKED.load(Ordering::SeqCst) && sched::rendezvous_waiting_senders(ep) == 1
        ),
        "the sender never parked on the rendezvous, so nothing was ever in flight to revoke",
    );

    crate::revoke::revoke_page_frame(phys);

    let [_word, slot, _second, _badge, ..] = sched::ipc_receive_cap(ep);
    let delivered = if slot == abi::rendezvous::NO_CAP {
        None
    } else {
        sched::current_cap(slot).ok()
    };

    assert!(
        super::wait_for(|| RETURNED.load(Ordering::SeqCst)),
        "the sender never returned from its send, so its table could not be re-read",
    );
    assert!(
        !SENDER_STILL_HELD.load(Ordering::SeqCst),
        "the revoke did not reach the sender's own capability table, so this test's premise is \
         false and its result means nothing",
    );

    assert!(
        !matches!(delivered, Some(c) if c.object == Object::PageFrame(phys, page_frame_run_len(1))),
        "a revoked PageFrame capability was delivered out of the hand-off slot: revocation swept \
         every capability table and not `outgoing_cap`, so the receiver now names a run the \
         revoker took back",
    );

    crate::memory_region::destroy(region);
}

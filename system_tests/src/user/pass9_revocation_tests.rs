//! **Pass 9 (an outside security reviewer attacks the confinement claim): a revocation reaches a
//! capability in flight, for the two sweeps no test drove through a parked hand-off** (fatal risk
//! 7's confinement claim, claim 30).
//!
//! `revocation_in_flight_tests` drives the `PageFrame` sweep through a hand-off slot and holds. Its
//! own `# BUGS` section says the other two sweeps carry the same two lines and *no test drives
//! either through a parked hand-off*, so their correctness was reasoned from the code, not measured.
//! This module measures them: a capability parked in `Thread::outgoing_cap` (where `ipc_send_cap`
//! writes a handed-off capability before any receiver has arrived) is in no capability table, so a
//! sweep that reads tables alone would leave one alive and hand it to the receiver. These are the
//! `DeviceFrame` and (x86_64) `PortRange` objects through that same window.
//!
//! The shape is `revocation_in_flight_tests`' shape, with a different object: a sender holds the
//! capability in its own table, hands a copy to a rendezvous nobody is receiving on (so the copy
//! parks in `outgoing_cap` and the sender blocks), the whole-machine revoke runs, and then the
//! receiver arrives and must be handed nothing. The last assertion is reached on every path, because
//! `ipc_receive_cap` returns either `NO_CAP` or a slot and both are inspected.
//!
//! Name: provisional (pass 9's lane, 2026-10-13 UTC), for the module, the file and each function.
//! Cross-ISA: the `DeviceFrame` sweep is portable kernel code, so its test runs on every
//! architecture (DECISIONS §19 (architectural parity is a tenet)); the `PortRange` object exists
//! only on x86_64, so its test is gated there.

use core::sync::atomic::{AtomicBool, Ordering};

// The `cap` module and its `*_cap` constructors still carry their pre-sweep spelling; aliased here
// so this file spells `capability` out everywhere a reader looks (calef, 2026-10-06).
use crate::cap::{Object, Rights, device_frame_cap as device_frame_capability};
use crate::sched::{
    self, current_cap as current_capability, ipc_receive_cap as ipc_receive_capability,
    ipc_send_cap as ipc_send_capability,
};

/// **A `DeviceFrame` revoked while it sits in the hand-off slot must not reach the receiver** (claim
/// 30, §13 (capability revocation and untyped reclamation)).
///
/// A device's MMIO page is the sharper confinement case than a RAM frame: `revoke_device_from_others`
/// exists precisely so that after a live driver swap the device has exactly one owner, and a copy
/// parked in a hand-off slot would give it a second owner the revoke never saw. The object is a
/// freshly retyped RAM page used as the `DeviceFrame`'s physical address, so nothing else in the
/// suite aliases it and the sweep is the only thing that can clear the parked copy.
///
/// The sender holds the capability in its table (so the revoke has a table copy to take, which the
/// premise check reads) and parks a copy on the rendezvous. The main test thread is the revoke's
/// caller, so it is the spared keeper; the sender is the only other holder and is swept. The
/// receiver (the main thread) then arrives at `ipc_receive_cap` and must get `NO_CAP`.
#[test_case]
fn a_device_frame_revoked_while_in_flight_does_not_reach_the_receiver() {
    static PARKED: AtomicBool = AtomicBool::new(false);
    static RETURNED: AtomicBool = AtomicBool::new(false);
    /// Did the sender still hold the frame in its **own table** once its send completed? A `false`
    /// here is the revoke having reached the one holder a table sweep can see, which is the premise.
    static SENDER_STILL_HELD: AtomicBool = AtomicBool::new(true);

    let region = crate::memory_region::create(4).expect("no region");
    let ep = sched::create_rendezvous_from(region).expect("no rendezvous");
    let phys = crate::memory_region::retype_page(region).expect("no page to name as a device");

    sched::spawn(move || {
        let slot = sched::grant(device_frame_capability(phys, Rights::ALL))
            .expect("the sender could not be granted the device frame it is about to delegate");
        let capability = sched::current_cap(slot).expect("the grant did not land");
        PARKED.store(true, Ordering::SeqCst);
        ipc_send_capability(ep, 0, capability, 0);
        SENDER_STILL_HELD.store(current_capability(slot).is_ok(), Ordering::SeqCst);
        RETURNED.store(true, Ordering::SeqCst);
    })
    .expect("no sender thread");

    assert!(
        super::wait_for(
            || PARKED.load(Ordering::SeqCst) && sched::rendezvous_waiting_senders(ep) == 1
        ),
        "the sender never parked on the rendezvous, so nothing was ever in flight to revoke",
    );

    crate::revoke::revoke_device_from_others(phys);

    let [_word, slot, _second, _badge, ..] = ipc_receive_capability(ep);
    let delivered = if slot == abi::rendezvous::NO_CAP {
        None
    } else {
        current_capability(slot).ok()
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
        !matches!(delivered, Some(c) if c.object == Object::DeviceFrame(phys)),
        "a revoked DeviceFrame capability was delivered out of the hand-off slot: the device-frame \
         sweep read every capability table and not `outgoing_cap`, so the receiver now names an \
         MMIO page the revoker took back and the device has two owners",
    );

    crate::memory_region::destroy(region);
}

/// **A `PortRange` revoked while it sits in the hand-off slot must not reach the receiver** (claim
/// 30; the x86_64 port-range object, DECISIONS §121).
///
/// The twin of the `DeviceFrame` case for the one object that exists only on x86_64. A `PortRange`
/// is enforced by the TSS I/O bitmap installed at the context switch from the holder's cached
/// grant, and `delete_port_range_caps` sweeps tables and the hand-off slot and forgets the cached
/// grant. The parked-copy limb is what this drives: the whole-machine sweep must clear the copy in
/// `outgoing_cap`, or the receiver files a `PortRange` the revoker believed it took back. The range
/// named is a plausible but harmless one (`0x1000`, past the legacy device ports), never COM1, so a
/// stray permitted `out` could not scribble on the console the transcript prints to.
#[cfg(target_arch = "x86_64")]
#[test_case]
fn a_port_range_revoked_while_in_flight_does_not_reach_the_receiver() {
    use crate::cap::port_range_cap as port_range_capability;

    const BASE: u16 = 0x1000;
    const COUNT: u16 = 8;

    static PARKED: AtomicBool = AtomicBool::new(false);
    static RETURNED: AtomicBool = AtomicBool::new(false);
    static SENDER_STILL_HELD: AtomicBool = AtomicBool::new(true);

    let region = crate::memory_region::create(4).expect("no region");
    let ep = sched::create_rendezvous_from(region).expect("no rendezvous");

    sched::spawn(move || {
        let slot = sched::grant(port_range_capability(BASE, COUNT, Rights::WRITE))
            .expect("the sender could not be granted the port range it is about to delegate");
        let capability = sched::current_cap(slot).expect("the grant did not land");
        PARKED.store(true, Ordering::SeqCst);
        ipc_send_capability(ep, 0, capability, 0);
        SENDER_STILL_HELD.store(current_capability(slot).is_ok(), Ordering::SeqCst);
        RETURNED.store(true, Ordering::SeqCst);
    })
    .expect("no sender thread");

    assert!(
        super::wait_for(
            || PARKED.load(Ordering::SeqCst) && sched::rendezvous_waiting_senders(ep) == 1
        ),
        "the sender never parked on the rendezvous, so nothing was ever in flight to revoke",
    );

    // The whole-machine sweep, the invoker included, so no keeper is spared and the parked copy is
    // the only thing the receiver could get.
    sched::delete_port_range_caps(BASE, COUNT);

    let [_word, slot, _second, _badge, ..] = ipc_receive_capability(ep);
    let delivered = if slot == abi::rendezvous::NO_CAP {
        None
    } else {
        current_capability(slot).ok()
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
        !matches!(delivered, Some(c) if c.object == Object::PortRange(BASE, COUNT)),
        "a revoked PortRange capability was delivered out of the hand-off slot: the port-range \
         sweep read every capability table and not `outgoing_cap`, so the receiver now names a \
         port range the revoker took back",
    );

    sched::reclaim_region(region).expect("the region did not come back");
}

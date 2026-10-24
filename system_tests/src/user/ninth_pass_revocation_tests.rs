//! **A revocation reaches a capability in flight, for the two sweeps no test drove through a parked
//! hand-off** (a ninth outsider pass at the confinement claim, claim 30).
//!
//! A capability parked in `Thread::outgoing_cap` (where `ipc_send_cap` writes a hand-off before a
//! receiver arrives) is in no table, so a table-only sweep would hand it on. These drive that
//! window for `DeviceFrame` and `PortRange`, which `revocation_in_flight_tests`' `# BUGS` says were
//! reasoned from the code, not measured. As there, a sender parks a copy on a rendezvous nobody
//! receives on, the revoke runs, and the receiver must get nothing; `ipc_receive_cap` returns
//! `NO_CAP` or a slot, and both are inspected.
//!
//! Name: provisional (lane/pass-9-tests, 2026-10-14 UTC), for the module, the file and each
//! function. Cross-ISA: the `DeviceFrame` sweep is portable kernel code, so its test runs on every
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

/// **A `DeviceFrame` revoked while it sits in the hand-off slot must not reach the receiver**
/// (claim 30, §13 (capability revocation and untyped reclamation)).
///
/// An MMIO page is sharper than a RAM frame: `revoke_device_from_others` gives a swapped device one
/// owner, and a parked copy would be a second the revoke never saw. The frame names a fresh RAM
/// page nothing else aliases. The sender holds it (the premise check reads that the revoke took it)
/// and parks a copy; the main thread, the spared revoker, must then receive `NO_CAP`.
///
/// Falsification: unfalsified. Written by the ninth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn a_device_frame_revoked_while_in_flight_does_not_reach_the_receiver() {
    static PARKED: AtomicBool = AtomicBool::new(false);
    static RETURNED: AtomicBool = AtomicBool::new(false);
    /// Did the sender still hold the frame in its **own table** once its send completed? A `false`
    /// here is the revoke having reached the one holder a table sweep can see, which is the
    /// premise.
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
/// 30; the x86_64 port-range object, DECISIONS §121 (x86 port I/O)).
///
/// The `DeviceFrame` twin for the x86_64-only object, enforced by the TSS I/O bitmap the context
/// switch installs from the holder's cached grant. `delete_port_range_caps` must clear the tables,
/// the parked copy in `outgoing_cap` and the cached grant. The range (`0x1000`, past the legacy
/// device ports, never COM1) is harmless: a stray permitted `out` cannot scribble on the console.
///
/// Falsification: unfalsified. Written by the ninth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
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

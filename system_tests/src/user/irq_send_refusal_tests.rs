//! **An endpoint that carries an interrupt refuses every send** (milestone 603 (provisional),
//! DECISIONS §101 (notification objects) ruling B, calef 2026-09-26). See `mod irq_send_refusal_tests` in `system_tests/src/user.rs` for why
//! this is cross-ISA and what it drives.

use core::sync::atomic::{AtomicU64, Ordering};

use abi::Error;

use crate::arch::exceptions::TrapFrame;
use crate::cap::Rights;
use crate::sched;
use crate::syscall::invoke;

/// An interrupt number no device on any of the three machines raises, so binding it steals no real
/// route. aarch64 and riscv64 deliver a routed interrupt only if something enabled it at the
/// controller, and nothing enables this one; on `x86_64` it is near the top of the MSI band, which
/// is allocated upward from `MSI_VECTOR_BASE` (0xc0) and would need fifty-eight devices to reach
/// it. Not 251 (the quiet interrupt of milestone 151 (notification objects)) and not the soak's band, which counts down from
/// 255 and exists only in soak builds.
const QUIET_INTID: u32 = 250;

/// `invoke` through the real dispatcher, the way a program's `svc`/`ecall` reaches it.
fn call(slot: u64, method: u64, a0: u64, a1: u64, a2: u64) -> (Result<i64, Error>, TrapFrame) {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    let answer = invoke(&mut frame, slot, method, a0, a1, a2);
    (answer, frame)
}

/// A program holding `WRITE` on an interrupt's endpoint cannot put anything in front of the driver.
///
/// `SEND`, `SEND_CAP` and `CALL` are refused with `NotPermitted`, and the driver parked in
/// `Irq::WAIT` stays parked with nothing queued. The test grants itself `Rights::ALL`, §101's named
/// mis-wiring, so only the kernel's rule stands between send and driver. The positive control comes
/// last: the real interrupt reaches the driver as `w0 = 1`, proving it was really parked.
///
/// Falsification: replayable `system_tests/falsifications/user.irq_send_refusal_tests.a_send_to_an_interrupts_endpoint_is_refused_and_the_driver_never_sees_it.patch`
#[test_case]
fn a_send_to_an_interrupts_endpoint_is_refused_and_the_driver_never_sees_it() {
    // What the driver's `Irq::WAIT` returned: x0, x1, x4. `u64::MAX` until it returns.
    static WOKE_WITH: [AtomicU64; 3] = [const { AtomicU64::new(u64::MAX) }; 3];

    let region = crate::memory_region::create(2).expect("no region for the endpoint");
    let ep = sched::create_rendezvous_from(region).expect("no rendezvous");
    sched::bind_irq(QUIET_INTID, ep);

    let driver = sched::spawn(|| {
        let slot = sched::grant(crate::cap::irq_cap(QUIET_INTID)).expect("grant the interrupt");
        let (answer, frame) = call(slot, abi::interrupt::WAIT, 0, 0, 0);
        WOKE_WITH[1].store(frame.arg(1), Ordering::SeqCst);
        WOKE_WITH[2].store(frame.arg(4), Ordering::SeqCst);
        WOKE_WITH[0].store(answer.map_or(u64::MAX - 1, |v| v as u64), Ordering::SeqCst);
        let _ = sched::delete_current_cap(slot);
    })
    .expect("spawn the driver");
    assert!(
        super::wait_for(|| sched::rendezvous_waiting_receivers(ep) == 1),
        "the driver never parked in Irq::WAIT"
    );

    // The mis-wiring: every right there is, GRANT included, so SEND_CAP has something to delegate.
    let forger = sched::grant(crate::cap::rendezvous_cap(ep, Rights::ALL)).expect("grant the ep");

    assert_eq!(
        call(forger, abi::rendezvous::SEND, 1, 0, 0).0,
        Err(Error::NotPermitted),
        "SEND to an interrupt's endpoint was not refused"
    );
    assert_eq!(
        call(
            forger,
            abi::rendezvous::SEND_CAP,
            forger,
            Rights::ALL.bits() as u64,
            1
        )
        .0,
        Err(Error::NotPermitted),
        "SEND_CAP to an interrupt's endpoint was not refused"
    );
    // Would block for ever if it were parked rather than refused: no reply is coming.
    assert_eq!(
        call(forger, abi::rendezvous::CALL, 1, 0, 0).0,
        Err(Error::NotPermitted),
        "CALL to an interrupt's endpoint was not refused"
    );

    // Give a wrongly delivered message every chance to land before looking for it.
    for _ in 0..50 {
        sched::yield_now();
    }
    assert_eq!(
        WOKE_WITH[0].load(Ordering::SeqCst),
        u64::MAX,
        "the driver woke before its interrupt: a send reached it"
    );
    assert_eq!(
        sched::rendezvous_waiting_receivers(ep),
        1,
        "the driver left its wait"
    );
    assert_eq!(
        sched::rendezvous_waiting_senders(ep),
        0,
        "a refused sender was queued"
    );
    assert!(sched::is_thread_present(driver));

    // The positive control: the interrupt itself still gets through, and is all the driver saw.
    sched::irq_notify(ep);
    assert!(
        super::wait_for(|| WOKE_WITH[0].load(Ordering::SeqCst) != u64::MAX),
        "the real interrupt never reached the driver"
    );
    assert_eq!(WOKE_WITH[0].load(Ordering::SeqCst), 1, "Irq::WAIT's x0");
    assert_eq!(WOKE_WITH[1].load(Ordering::SeqCst), 0, "Irq::WAIT's x1");
    assert_eq!(WOKE_WITH[2].load(Ordering::SeqCst), 0, "Irq::WAIT's x4");

    let _ = sched::delete_current_cap(forger);
    assert!(super::wait_for(|| !sched::is_thread_present(driver)));
    // The route to QUIET_INTID outlives this: `bind_irq` has no unbind. Reclaiming the region makes
    // it a stale name, which `irq_notify` drops, and nothing raises this interrupt anyway.
    sched::reclaim_region(region).expect("the endpoint's region did not come back");
}

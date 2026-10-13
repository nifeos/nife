//! **Pass 9 (an outside security reviewer attacks the confinement claim): holding a port *range*
//! does not reach a port outside it** (fatal risk 7's confinement claim, claim 27, and the shape of
//! claim 33; DECISIONS §121 (the x86 port-range capability), milestone 768).
//!
//! `x86_port_tests` proves a non-holder faults and a holder transmits, but every `out` it executes
//! is to `SCRATCH_PORT`, which is *inside* the granted `(0x3F8, 8)` range. A TSS I/O permission
//! bitmap that the kernel filled too wide (the whole 65,536-port space, or a page of it, instead of
//! exactly the eight bytes the range names) would pass every one of those tests and still let a
//! serial driver reach the keyboard controller, the PCI config ports, or a neighbor device. This
//! module is the test that sees that: a holder of exactly the COM1 range executes `out` to a port
//! one past the end of its range, and the CPU must refuse it with a #GP even though the thread holds
//! a real port capability.
//!
//! The child is built ring-3 with its whole world in one region, the way `x86_port_tests` and
//! `supervision_tests` build theirs, and holds `PortRange(0x3F8, 8)` with `WRITE` (never `GRANT`).
//! The stub is `x86_programs::port_out_then_exit`: it executes one `out` and, if the CPU permits it,
//! exits cleanly, so a wrongly permitted write arrives as `EVENT_EXIT` where the test wants
//! `EVENT_FAULT`, and a reporting child that would hang the run on a wrongly permitted write is
//! avoided.
//!
//! Name: provisional (pass 9's lane, 2026-10-13 UTC), for the module, the file and each function.
//! x86_64 only: there is no port space and no TSS I/O bitmap on the other two architectures.
#![cfg(target_arch = "x86_64")]

use abi::fault::{EVENT_EXIT, EVENT_FAULT, FAULT_EP_SLOT};

use super::*;
use crate::sched;

/// COM1's port range, the object the holder's capability names. The child may write any of these
/// eight ports and no others.
const COM1_BASE: u16 = super::X86_COM1_PORT_BASE;
const COM1_COUNT: u16 = super::X86_COM1_PORT_COUNT;

/// The port one past the end of the granted range (`0x3F8 + 8 == 0x400`). Immediately outside the
/// capability, so a holder whose range was enforced exactly must be refused it, and it drives no
/// device, so a (wrongly) permitted write scribbles on nothing. The sharpest choice: a bitmap
/// rounded up by even one byte would permit it.
const OUTSIDE_PORT: u16 = COM1_BASE + COM1_COUNT;
const OUTSIDE_VAL: u8 = 0x5A;

/// Where the child's code and stack live: the image and stack bands, offset three 2 MiB windows in
/// from `x86_port_tests` and `supervision_tests` so no stray global makes one test's leftovers look
/// like another's.
const CODE_VA: u64 = address_space_map::IMAGE_BASE + 0x60_0000;
const STACK_VA: u64 = address_space_map::STACK_TOP_PAGE - 0x60_0000;

/// Build a ring-3 child from `stub`, holding `PortRange(COM1)` with `WRITE`, supervised through
/// `fault_ep` (in the reserved fault slot, so a #GP on the `out` arrives as a message rather than
/// panicking the kernel). One region holds the address space, code, stack and TCB, so one reclaim
/// frees it. Modeled on `x86_port_tests::build_child`, trimmed to the one child this module needs.
fn build_port_holder(stub: &[u32], fault_ep: sched::RendezvousId) -> (u64, u64) {
    let region = crate::memory_region::create(16).expect("no region for the child");
    let aspace = user_address_space_create(region).expect("no aspace");

    let code_phys = code_page(region, stub);
    user_address_space_map(
        aspace,
        CODE_VA,
        code_phys,
        Flags::user_code(),
        crate::revoke::PageMapSource::NoCapability,
    )
    .expect("map code");

    let stack_phys = crate::memory_region::retype_page(region).expect("no stack frame");
    user_address_space_map(
        aspace,
        STACK_VA,
        stack_phys,
        Flags::user_data(),
        crate::revoke::PageMapSource::NoCapability,
    )
    .expect("map stack");

    let tid = sched::create_thread_control_block(region).expect("no tcb");

    // The port range, held so the TSS bitmap grants it: WRITE, never GRANT. It is not invoked by
    // slot (the child executes `out` directly), so its slot does not matter here.
    sched::thread_control_block_insert_cap(
        tid,
        crate::cap::port_range_cap(COM1_BASE, COM1_COUNT, crate::cap::Rights::WRITE),
        None,
    )
    .expect("insert the port range");

    // The reserved fault slot: born supervised, so the #GP arrives as a message.
    sched::thread_control_block_insert_cap(
        tid,
        crate::cap::rendezvous_cap(fault_ep, crate::cap::Rights::READ),
        Some(FAULT_EP_SLOT),
    )
    .expect("insert fault ep");

    sched::configure_thread_control_block(tid, CODE_VA, STACK_VA + page_frames::FRAME_SIZE, aspace)
        .expect("configure");
    sched::start_thread_control_block(tid, [0; 3]).expect("start");
    (tid, region)
}

/// Reclaim the child's region once it is a corpse, retried for the window a just-delivered death
/// message opens. Same shape and reason as `x86_port_tests::reap`.
fn reap(region: u64) {
    assert!(
        super::wait_for(|| sched::reclaim_region(region).is_ok()),
        "reaping the child's region failed",
    );
}

/// **A holder of the COM1 range faults when it writes a port one past the range.**
///
/// The child holds `PortRange(0x3F8, 8)` and executes `out` to `0x400`, the port immediately above
/// its grant. If the kernel's TSS bitmap were filled wider than the eight ports the capability
/// names, this `out` would be permitted and the child would exit cleanly; the claim is that the
/// range is enforced exactly, so the CPU raises a #GP and the supervisor reads `EVENT_FAULT` at the
/// `out` instruction's pc. A holder reaching a port it does not hold is a confinement break whatever
/// capability it holds for some *other* range.
#[test_case]
fn a_port_range_holder_cannot_reach_a_port_above_its_range() {
    let endpoints = crate::memory_region::create(2).expect("no endpoint region");
    let fault_ep = sched::create_rendezvous_from(endpoints).expect("no fault rendezvous");

    let (child, region) = build_port_holder(
        &super::x86_programs::port_out_then_exit(OUTSIDE_PORT, OUTSIDE_VAL),
        fault_ep,
    );

    let msg = sched::ipc_receive(fault_ep);
    reap(region);

    assert_ne!(
        msg[0], EVENT_EXIT,
        "a holder of the COM1 range wrote port {OUTSIDE_PORT:#x}, one past its range, and the CPU \
         PERMITTED it: the TSS bitmap was filled wider than the capability names",
    );
    assert_eq!(
        msg[0], EVENT_FAULT,
        "writing a port outside the granted range should raise a #GP",
    );
    assert_eq!(msg[1], child, "the fault named the wrong thread");
    assert_eq!(
        msg[2],
        CODE_VA + super::x86_programs::PORT_OUT_PC_OFFSET,
        "the faulting pc was not the `out` instruction",
    );

    sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

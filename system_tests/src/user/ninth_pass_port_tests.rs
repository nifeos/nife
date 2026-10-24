//! **The x86 port-range capability, attacked at the edges the existing suite does not reach** (a
//! ninth outsider pass at the confinement claim, claims 27, 28, 29 and 33; DECISIONS §121 (x86 port
//! I/O), milestones 299, 313, 768).
//!
//! `x86_port_tests` proves a non-holder faults, a holder transmits, a revoked holder faults, a
//! self-deleter faults, and a `READ`-only capability grants no output. Each test here comes at one
//! of those claims from an angle that suite lacks (notes/confinement-outsider-pass-9.md, "The
//! kernel tests"). A wrongly permitted stub exits cleanly, so a broken gate arrives as `EVENT_EXIT`
//! where the test wants `EVENT_FAULT`, never as a hang on a `SEND` nobody receives.
//!
//! Name: provisional (lane/pass-9-tests, 2026-10-14 UTC), for the module, the file and each
//! function. x86_64 only: there is no port space and no TSS I/O bitmap on the other two
//! architectures.
#![cfg(target_arch = "x86_64")]

use abi::fault::{EVENT_EXIT, EVENT_FAULT, FAULT_EP_SLOT};

use super::*;
use crate::cap::Rights;
use crate::sched::{self, RendezvousId};

/// COM1's port range, the object a holder's capability names. A holder may write any of these eight
/// ports and no others.
const COM1_BASE: u16 = super::X86_COM1_PORT_BASE;
const COM1_COUNT: u16 = super::X86_COM1_PORT_COUNT;

/// COM1's scratch register (`0x3FF`, the eighth port of the range): inside the grant, so a holder
/// with `WRITE` may write it, and it drives no device, so a permitted write scribbles on nothing.
const SCRATCH_PORT: u16 = COM1_BASE + COM1_COUNT - 1;
/// The port one past the end of the range (`0x400`): immediately outside the grant, so an exactly
/// enforced range refuses it, and it too drives no device.
const OUTSIDE_PORT: u16 = COM1_BASE + COM1_COUNT;
const VAL: u8 = 0x5A;

/// The slot a holder's port capability lands in when the child has no wake endpoint: slot 0 is the
/// report endpoint, so the port is slot 1. The self-deleter names this in its own machine code.
const PORT_SLOT_NO_WAKE: u64 = 1;

/// Where the child's code and stack live: the image and stack bands, offset four 2 MiB windows in
/// from `x86_port_tests` and `supervision_tests` so no stray global makes one test's leftovers look
/// like another's.
const CODE_VA: u64 = address_space_map::IMAGE_BASE + 0x80_0000;
const STACK_VA: u64 = address_space_map::STACK_TOP_PAGE - 0x80_0000;

/// Build a ring-3 child from `stub`, one of `x86_programs`' architect-ratified stub builders (this
/// module mints no ring-3 machine code), holding `PortRange(COM1)` with `rights`, supervised
/// through `fault_ep` (in the reserved fault slot, so a #GP arrives as a message rather than
/// panicking the kernel). Slot 0 is a report endpoint (no child here sends on it), slot 1 a wake
/// endpoint when `wake` is given (a `receive_then_port_out` child parks on it), and the port
/// capability the next slot. One region holds everything, so one reclaim frees it. Modeled on
/// `x86_port_tests::build_child`. Returns the child's tid and its region.
fn build_port_holder(
    stub: &[u32],
    rights: Rights,
    report: RendezvousId,
    wake: Option<RendezvousId>,
    fault_ep: RendezvousId,
) -> (u64, u64) {
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

    // Slot 0: the report endpoint, so slot 0 is the same for every child.
    let slot = sched::thread_control_block_insert_cap(
        tid,
        crate::cap::rendezvous_cap(report, Rights::WRITE),
        None,
    )
    .expect("insert report");
    assert_eq!(slot, 0, "the report cap must land in slot 0");

    // Slot 1: the wake endpoint a parked child calls `RECEIVE` on, when this build has one.
    if let Some(w) = wake {
        let slot = sched::thread_control_block_insert_cap(
            tid,
            crate::cap::rendezvous_cap(w, Rights::READ),
            None,
        )
        .expect("insert wake");
        assert_eq!(slot, 1, "the wake cap must land in slot 1");
    }

    // The port range, held so the TSS bitmap grants it (when `rights` carries WRITE); never GRANT.
    let port_slot = sched::thread_control_block_insert_cap(
        tid,
        crate::cap::port_range_cap(COM1_BASE, COM1_COUNT, rights),
        None,
    )
    .expect("insert the port range");
    if wake.is_none() {
        assert_eq!(
            port_slot, PORT_SLOT_NO_WAKE,
            "the port cap must land where the self-deleter looks"
        );
    }

    // The reserved fault slot: born supervised, so a #GP arrives as a message.
    sched::thread_control_block_insert_cap(
        tid,
        crate::cap::rendezvous_cap(fault_ep, Rights::READ),
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

/// Create two or three endpoints in one region and return them with the region. Report and fault
/// are always made; a wake is made when `with_wake`.
fn endpoints(with_wake: bool) -> (RendezvousId, Option<RendezvousId>, RendezvousId, u64) {
    let region = crate::memory_region::create(4).expect("no endpoint region");
    let report = sched::create_rendezvous_from(region).expect("no report rendezvous");
    let wake =
        with_wake.then(|| sched::create_rendezvous_from(region).expect("no wake rendezvous"));
    let fault_ep = sched::create_rendezvous_from(region).expect("no fault rendezvous");
    (report, wake, fault_ep, region)
}

/// **A holder of the COM1 range faults when it writes a port one past the range** (claim 27).
///
/// The child holds `PortRange(0x3F8, 8)` with `WRITE` and executes `out` to `0x400`. If the TSS
/// bitmap were filled wider than the eight ports the capability names, this would be permitted and
/// the child would exit cleanly; the claim is that the range is enforced exactly, so the CPU raises
/// a #GP at the `out`. A holder reaching a port it does not hold is a break whatever capability it
/// holds for some *other* range.
///
/// Falsification: unfalsified. Written by the ninth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn a_port_range_holder_cannot_reach_a_port_above_its_range() {
    let (report, _wake, fault_ep, region) = endpoints(false);
    let (child, child_region) = build_port_holder(
        &super::x86_programs::port_out_then_exit(OUTSIDE_PORT, VAL),
        Rights::WRITE,
        report,
        None,
        fault_ep,
    );

    let msg = sched::ipc_receive(fault_ep);
    reap(child_region);

    assert_ne!(
        msg[0], EVENT_EXIT,
        "a holder of the COM1 range wrote port {OUTSIDE_PORT:#x}, one past its range, and the CPU \
         PERMITTED it: the TSS bitmap was filled wider than the capability names",
    );
    assert_eq!(
        msg[0], EVENT_FAULT,
        "writing a port outside the range should raise a #GP"
    );
    assert_eq!(msg[1], child, "the fault named the wrong thread");
    assert_eq!(
        msg[2],
        CODE_VA + super::x86_programs::PORT_OUT_PC_OFFSET,
        "the faulting pc was not the `out` instruction",
    );
    sched::reclaim_region(region).expect("the endpoint region did not come back");
}

/// **A parked holder revoked by the sparing take-back faults on its next write** (claim 28).
///
/// The child parks in `RECEIVE` holding the range; the test runs `revoke_port_range_from_others`,
/// the sparing take-back `PortRange::REVOKE` runs for a live driver swap: it deletes the range from
/// every table but the invoker's (the test thread, which holds none) and resets the TSS on every
/// core. Woken, the child's `out` faults. The existing revoke test drives the whole-machine
/// `revoke_port_range`; none drove the sparing body against a parked holder.
///
/// Falsification: unfalsified. Written by the ninth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn a_parked_holder_revoked_by_the_sparing_take_back_faults_on_wake() {
    let (report, wake, fault_ep, region) = endpoints(true);
    let wake = wake.expect("asked for a wake endpoint");
    let (holder, child_region) = build_port_holder(
        &super::x86_programs::receive_then_port_out(SCRATCH_PORT, VAL),
        Rights::WRITE,
        report,
        Some(wake),
        fault_ep,
    );

    // Take the range back from everyone but the invoker (the test thread), while the holder is
    // provably still parked: it cannot reach its `out` before the wake below.
    crate::revoke::revoke_port_range_from_others(COM1_BASE, COM1_COUNT);

    sched::ipc_send(wake, [0, 0, 0]);

    let msg = sched::ipc_receive(fault_ep);
    assert_ne!(
        msg[0], EVENT_EXIT,
        "a holder whose range was taken back by the sparing REVOKE still completed its `out`: the \
         take-back cleared the table but not the cached grant",
    );
    assert_eq!(
        msg[0], EVENT_FAULT,
        "a revoked holder's next `out` must fault"
    );
    assert_eq!(msg[1], holder, "the fault named the wrong thread");
    reap(child_region);
    sched::reclaim_region(region).expect("the endpoint region did not come back");
}

/// **A holder that deletes its own port capability faults on its next write** (claim 29, §12
/// (call/reply IPC: a one-shot reply capability)).
///
/// The child deletes its port capability's slot (`PORT_SLOT_NO_WAKE`), then executes `out`. The
/// port grant is a cached field the context switch installs; if `SYS_CAP_DELETE` cleared the table
/// but not the cache, the child would keep the ports for life, claim 29 failing for the one object
/// enforced outside the table. So the `out` must fault, at the instruction after the delete.
///
/// Falsification: unfalsified. Written by the ninth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn a_self_deleting_holder_faults_on_its_next_write() {
    let (report, _wake, fault_ep, region) = endpoints(false);
    let (holder, child_region) = build_port_holder(
        &super::x86_programs::cap_delete_then_port_out(PORT_SLOT_NO_WAKE as u32, SCRATCH_PORT, VAL),
        Rights::WRITE,
        report,
        None,
        fault_ep,
    );

    let msg = sched::ipc_receive(fault_ep);
    assert_ne!(
        msg[0], EVENT_EXIT,
        "a holder that deleted its own port capability still completed its `out`: it kept the \
         ports after dropping the capability",
    );
    assert_eq!(
        msg[0], EVENT_FAULT,
        "a self-deleting holder's next `out` must fault"
    );
    assert_eq!(msg[1], holder, "the fault named the wrong thread");
    assert_eq!(
        msg[2],
        CODE_VA + super::x86_programs::CAP_DELETE_THEN_PORT_OUT_PC_OFFSET,
        "the faulting pc was not the `out` instruction: a red for the wrong reason",
    );
    reap(child_region);
    sched::reclaim_region(region).expect("the endpoint region did not come back");
}

/// **A `READ`-only holder cannot write a port inside its range** (claim 33, milestone 768 (a
/// read-only port range grants nothing)).
///
/// The capability names the COM1 range with `READ` alone (a `Serve`-shaped grant). The TSS bitmap
/// installs nothing for a capability without `WRITE`, so the `out` faults as a non-holder's does.
/// The port is inside the range, so only the missing `WRITE` can refuse it: the grant must read the
/// capability's rights, not just its range.
///
/// Falsification: unfalsified. Written by the ninth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn a_read_only_holder_cannot_write_a_port_inside_its_range() {
    let (report, _wake, fault_ep, region) = endpoints(false);
    let (child, child_region) = build_port_holder(
        &super::x86_programs::port_out_then_exit(SCRATCH_PORT, VAL),
        Rights::READ,
        report,
        None,
        fault_ep,
    );

    let msg = sched::ipc_receive(fault_ep);
    assert_ne!(
        msg[0], EVENT_EXIT,
        "a READ-only holder wrote a port inside its range without faulting: the port grant ignores \
         the capability's rights",
    );
    assert_eq!(
        msg[0], EVENT_FAULT,
        "a READ-only capability must not grant port output"
    );
    assert_eq!(msg[1], child, "the fault named the wrong thread");
    assert_eq!(
        msg[2],
        CODE_VA + super::x86_programs::PORT_OUT_PC_OFFSET,
        "the faulting pc was not the `out` instruction",
    );
    sched::reclaim_region(region).expect("the endpoint region did not come back");
}

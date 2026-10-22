//! **The load-bearing tests of the x86 port-range capability** (milestone 299, DECISIONS §121
//! reversed 2026-09-15; a third added by milestone 313's audit). A `PortRange` capability is enforced entirely at the context switch, by the
//! TSS I/O permission bitmap, with nothing on the syscall path to assert on; the only honest test is
//! a ring-3 program that executes `out` and observes whether the CPU allowed it. So these build
//! hand-assembled `x86_64` children (`super::x86_programs`) exactly as `supervision_tests` does, and
//! read the outcome from the child's supervision endpoint: a word arrives if the `out` was allowed,
//! an `EVENT_FAULT` if it was not.
//!
//! Two properties, both named load-bearing in the milestone's own BUGS:
//!
//! 1. **A non-holder cannot touch the port, and a holder's grant does not leak across a switch.**
//!    [`port_holder_transmits_then_a_non_holder_faults`] runs a holder (which transmits) and then a
//!    child with no port capability (which faults on the same `out`). The second running *after* the
//!    first is the hand-off test: if the switch away from the holder had left the TSS permitting the
//!    port, the non-holder would inherit it and not fault.
//! 2. **A revoked holder faults on its next `in`/`out`.**
//!    [`a_revoked_holder_faults_on_its_next_port_write`] parks a holder in `RECEIVE`, revokes its port
//!    range while it is parked, wakes it, and confirms the `out` it then executes faults.
//! 3. **A holder that drops its own port capability faults on its next `in`/`out`.**
//!    [`a_holder_that_deletes_its_port_capability_faults_on_its_next_port_write`] is milestone 313's
//!    audit finding turned into a test: the first two prove the grant follows the capability across
//!    a switch and a revoke, and neither could see that `SYS_CAP_DELETE` left it behind.
//!
//! 4. **A holder that takes the range back from everyone else keeps its own bitmap.**
//!    [`a_take_back_leaves_the_invokers_own_bitmap_installed`] is the 2026-09-24 audit's finding
//!    turned into a test: `PortRange::REVOKE` spares the invoker's capability, and until that audit
//!    the arch half reset the invoker's own core anyway, so the sparing was true of the table and
//!    false of the hardware until the next switch-in.
//!
//! `x86_64` only: there is no port space, and no TSS I/O bitmap, on the other two architectures.

use abi::fault::{EVENT_EXIT, EVENT_FAULT, FAULT_EP_SLOT};

use super::*;
use crate::sched;

/// Where the child's code and stack go: in the address-space map's image and stack bands like any
/// program's, but one 2 MiB window in from `supervision_tests`' addresses, so a stray global could
/// not make one test's leftovers look like another's.
const CODE_VA: u64 = address_space_map::IMAGE_BASE + 0x20_0000;
const STACK_VA: u64 = address_space_map::STACK_TOP_PAGE - 0x20_0000;

/// The port the children write to: COM1's **scratch register** (`0x3FF`, the eighth of the eight
/// ports the range names). It is inside the granted `(0x3F8, 8)` range, so a holder may write it, and
/// it has no device effect, so a permitted write does not scribble on the serial console the kernel
/// itself is printing the test transcript to. The value is arbitrary.
const SCRATCH_PORT: u16 = 0x3FF;
const SCRATCH_VAL: u8 = 0x5A;

/// COM1's port range, the object a `PortRange` capability over the console names. The children hold a
/// capability to this exact `(base, count)`, and revocation names the same pair.
const COM1_BASE: u16 = super::X86_COM1_PORT_BASE;
const COM1_COUNT: u16 = super::X86_COM1_PORT_COUNT;

/// The word a holder SENDs once its `out` is allowed, so a test can tell "it transmitted" from "it
/// faulted". Distinctive.
const REPORTED: u64 = 0xC0DE;

/// Where [`build_child`] lands the `PortRange` capability when the child has no wake endpoint: slot
/// 0 is the report endpoint, so the next free slot is 1. The self-deleting child names this slot in
/// its own machine code, and `build_child` asserts it.
const PORT_SLOT_WITHOUT_WAKE: u64 = 1;

/// Build a ring-3 child from `stub` with its whole world in one region (address space, code, stack,
/// TCB), so a single reclaim frees it. `report` lands in slot 0 (what the stub SENDs on), `wake` in
/// slot 1 if given (what a `receive_then_port_out` child parks on), the `PortRange` capability next if
/// `port` is given (held, never invoked by slot), and `fault_ep` in the reserved fault slot. Returns
/// `(child_tid, region)`.
fn build_child(
    stub: &[u32],
    report: sched::RendezvousId,
    wake: Option<sched::RendezvousId>,
    port: Option<crate::cap::Rights>,
    fault_ep: sched::RendezvousId,
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

    // Slot 0: the report endpoint the stub SENDs on.
    let slot = sched::thread_control_block_insert_cap(
        tid,
        crate::cap::rendezvous_cap(report, crate::cap::Rights::WRITE),
        None,
    )
    .expect("insert report");
    assert_eq!(slot, 0, "the report cap must land in slot 0");

    // Slot 1: the wake endpoint a `receive_then_port_out` child parks on, when this build has one.
    if let Some(w) = wake {
        let slot = sched::thread_control_block_insert_cap(
            tid,
            crate::cap::rendezvous_cap(w, crate::cap::Rights::READ),
            None,
        )
        .expect("insert wake");
        assert_eq!(slot, 1, "the wake cap must land in slot 1");
    }

    // The port range, held so the TSS bitmap grants it: the whole point of the test. `WRITE`, the
    // rights a driver gets; never `GRANT`, so the child cannot re-delegate or revoke it. It is not
    // invoked by slot number (the child executes `out` directly), so its slot matters to exactly one
    // child, the one that deletes it: [`PORT_SLOT_WITHOUT_WAKE`] is asserted here so that program
    // cannot delete the wrong thing and pass for the wrong reason.
    if let Some(port_rights) = port {
        let slot = sched::thread_control_block_insert_cap(
            tid,
            crate::cap::port_range_cap(COM1_BASE, COM1_COUNT, port_rights),
            None,
        )
        .expect("insert the port range");
        if wake.is_none() {
            assert_eq!(
                slot, PORT_SLOT_WITHOUT_WAKE,
                "the port cap must land where the deleter looks"
            );
        }
    }

    // The reserved fault slot: born supervised, so a #GP on the `out` arrives as a message rather
    // than panicking the kernel (there would otherwise be no thread to kill).
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

/// Reclaim a child's region once it is a corpse, retried for the window a just-delivered death
/// message opens (the corpse is still on its own kernel stack for a few hundred instructions; a
/// stack must not be unmapped under a core standing on it). Same shape and same reason as
/// `supervision_tests`' reap.
fn reap(region: u64) {
    assert!(
        super::wait_for(|| sched::reclaim_region(region).is_ok()),
        "reaping the child's region failed",
    );
}

/// How many non-holders [`port_holder_transmits_then_a_non_holder_faults`] may start before one
/// lands on the holder's core. Placement is §28 (SMP placement: two random choices at spawn) and offers no
/// lever, so the test retries rather than steers. On one core the first lands; on the two-core
/// direct boot, two runs on 2026-10-03 landed on tries 1 and 4. Sixty-four makes a miss at a one-in-four landing rate a
/// `(3/4)^64`, about one in a hundred million, and a miss fails the test loudly rather than passing.
const NON_HOLDER_ATTEMPTS: u32 = 64;

/// **A holder transmits; a non-holder that runs next on the same core faults.** The non-holder
/// test and the hand-off test in one, because the order is the hand-off: the non-holder is
/// scheduled after the holder installed and then vacated the TSS bitmap, so its fault is proof the
/// switch left the port denied rather than inheriting the holder's grant.
///
/// **The same core is the precondition, and until 2026-10-03 nothing asserted it.** The bitmap is
/// per core. On one core the order alone makes the hand-off; on two, the non-holder can start on
/// the core the holder never touched and fault there whatever the switch did. That is what the
/// direct boot did from the day it went to `NIFE_SMP=2` (2026-09-23): the falsification below left
/// the holder's grant on cpu 0, the non-holder was placed on cpu 1, faulted, and the test passed.
/// Only the real-firmware leg, which boots the suite on one core, went red. So the holder now
/// reports the core it wrote on ([`port_out_reporting_cpu`]), and the non-holder executes its `out`
/// only when it is on that core and otherwise faults at a different pc
/// ([`port_out_on_cpu_then_exit`]), which this test counts as a miss and retries.
///
/// What the core check does not cover: a thread preempted and moved between its core read and its
/// `out` (three instructions, so two preemptions and a steal for the holder's bracket to lie).
///
/// [`port_out_reporting_cpu`]: super::x86_programs::port_out_reporting_cpu
/// [`port_out_on_cpu_then_exit`]: super::x86_programs::port_out_on_cpu_then_exit
///
/// Falsification: replayable `system_tests/falsifications/user.x86_port_tests.port_holder_transmits_then_a_non_holder_faults.patch`
#[test_case]
fn port_holder_transmits_then_a_non_holder_faults() {
    // The holder: it executes `out` to a port its capability names, so the CPU permits it, and the
    // word arrives with the core it was written on.
    let endpoints = crate::memory_region::create(4).expect("no endpoint region");
    let report = sched::create_rendezvous_from(endpoints).expect("no report rendezvous");
    let sup = sched::create_rendezvous_from(endpoints).expect("no sup rendezvous");
    let (_holder, holder_region) = build_child(
        &super::x86_programs::port_out_reporting_cpu(SCRATCH_PORT, SCRATCH_VAL, REPORTED as u32),
        report,
        None,
        Some(crate::cap::Rights::WRITE),
        sup,
    );
    let msg = sched::ipc_receive(report);
    let (word, cpu_after, cpu_before) = (msg[0], msg[1], msg[2]);
    assert_eq!(
        word, REPORTED,
        "the port holder's `out` should have been permitted, and its report should have arrived",
    );
    assert!(
        cpu_before < current_cpu_contract::CPU_ID_BOUND as u64,
        "the holder read core {cpu_before:#x}: its current-cpu page was never written, so which \
         core's bitmap it set is unknown",
    );
    assert_eq!(
        cpu_before, cpu_after,
        "the holder moved cores across its `out`; which core's bitmap it set is unknown",
    );
    let holder_cpu = cpu_before;
    assert_eq!(
        sched::ipc_receive(sup)[0],
        EVENT_EXIT,
        "the holder should have exited cleanly after transmitting",
    );
    reap(holder_region);

    // The non-holder: the same `out`, granted no port capability, run after the holder on the
    // holder's core. Its `out` faults, which is both "a non-holder cannot touch the port" and "the
    // hand-off away from the holder left the TSS denying". It exits rather than reporting
    // (milestone 313 (the security audit that was due since August)): if the hand-off ever leaked
    // the grant, a reporting child would park on a `SEND` nobody receives and hang the run, and
    // this test could not go red for the one defect it exists for.
    let report2 = sched::create_rendezvous_from(endpoints).expect("no report2 rendezvous");
    let sup2 = sched::create_rendezvous_from(endpoints).expect("no sup2 rendezvous");
    for attempt in 1..=NON_HOLDER_ATTEMPTS {
        let (non_holder, nh_region) = build_child(
            &super::x86_programs::port_out_on_cpu_then_exit(
                SCRATCH_PORT,
                SCRATCH_VAL,
                holder_cpu as u8,
            ),
            report2,
            None,
            None,
            sup2,
        );
        let msg = sched::ipc_receive(sup2);
        reap(nh_region);
        if msg[0] == EVENT_FAULT
            && msg[2] == CODE_VA + super::x86_programs::PORT_OUT_ON_CPU_WRONG_CPU_PC_OFFSET
        {
            continue; // started on another core and never reached its `out`: not a test, retry
        }
        crate::println!(
            "    the non-holder ran on the holder's core {holder_cpu} on try {attempt}"
        );
        assert_eq!(
            msg[0], EVENT_FAULT,
            "a non-holder's `out` must fault, not be permitted",
        );
        assert_eq!(msg[1], non_holder, "the fault named the wrong thread");
        assert_eq!(
            msg[2],
            CODE_VA + super::x86_programs::PORT_OUT_ON_CPU_PC_OFFSET,
            "the faulting pc was not the `out` instruction",
        );
        sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
        return;
    }
    panic!(
        "no non-holder ran on the holder's core {holder_cpu} in {NON_HOLDER_ATTEMPTS} tries, so \
         this run did not test the hand-off at all",
    );
}

/// **A revoked holder faults on its next `out`.** The child holds the port range and parks in
/// `RECEIVE`; while it is parked the test revokes the range (deleting its capability and clearing the
/// cached grant the switch installs), then wakes it. The `out` it executes on waking faults, which
/// is the whole claim: a capability that was real became unusable the instant it was revoked.
///
/// BUGS: the test reads `EVENT_FAULT` and not the faulting pc, so a fault anywhere in the child after
/// the wake would pass it, not only one at the `out`. Found by milestone 633 (an outside agent attacks the confinement claim)'s third outsider pass
/// (2026-10-06 UTC), by read; the property itself held under that pass's source review.
///
/// Falsification: replayable `system_tests/falsifications/user.x86_port_tests.a_revoked_holder_faults_on_its_next_port_write.patch`
#[test_case]
fn a_revoked_holder_faults_on_its_next_port_write() {
    let endpoints = crate::memory_region::create(3).expect("no endpoint region");
    let report = sched::create_rendezvous_from(endpoints).expect("no report rendezvous");
    let wake = sched::create_rendezvous_from(endpoints).expect("no wake rendezvous");
    let sup = sched::create_rendezvous_from(endpoints).expect("no sup rendezvous");
    let (holder, region) = build_child(
        &super::x86_programs::receive_then_port_out(SCRATCH_PORT, SCRATCH_VAL),
        report,
        Some(wake),
        Some(crate::cap::Rights::WRITE),
        sup,
    );

    // Revoke the port range while the child is still parked in RECEIVE (it cannot reach its `out`
    // before the wake below, so the revoke provably precedes the port access). This deletes the
    // child's `PortRange` capability and clears the grant the context switch would install.
    crate::revoke::revoke_port_range(COM1_BASE, COM1_COUNT);

    // Wake it. It leaves RECEIVE, executes `out`, and faults, because the port it once held is gone.
    sched::ipc_send(wake, [0, 0, 0]);

    let msg = sched::ipc_receive(sup);
    assert_eq!(
        msg[0], EVENT_FAULT,
        "a revoked holder's next `out` must fault; the word must never arrive",
    );
    assert_eq!(msg[1], holder, "the fault named the wrong thread");
    reap(region);
    sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

/// **A holder that deletes its own port capability faults on its next `out`** (milestone 313's
/// audit, 2026-09-17). The third property, and the one the first two could not see: they establish
/// that the grant follows the capability across a switch and across a revoke, and this establishes
/// that it follows the capability out of the thread's own table. Before the audit it did not. The
/// grant is a cached field the switch installs, `SYS_CAP_DELETE` cleared the table and not the
/// cache, and a thread that dropped its port capability kept the ports for life, which is §12's
/// "a consumed capability cannot be used again" failing for the one object enforced outside the
/// table. The progenitor does exactly this drop on every x86 boot (`cap_delete(g.uart_dev)`).
///
/// The child deletes [`PORT_SLOT_WITHOUT_WAKE`] and then executes `out`, and **exits rather than
/// reporting** if the `out` is permitted; the first draft reported, and on the unfixed kernel that
/// `SEND` parked on a rendezvous nobody was receiving and the run hung instead of going red (row
/// 26's shape, and the program's doc carries the account). Which assertion fires is stated here so
/// nobody has to run milestone 307's sweep on it: a permitted `out` means the child exits cleanly,
/// so the supervisor's message is `EVENT_EXIT` and the **first** `assert_eq!` is the one that goes
/// red, holding `EVENT_EXIT` where it wanted `EVENT_FAULT`. The pc equality below it is the
/// wrong-reason guard for the other direction: a `syscall` that faulted, or a bad slot, would
/// report a different pc.
///
/// Falsification: replayable `system_tests/falsifications/user.x86_port_tests.a_holder_that_deletes_its_port_capability_faults_on_its_next_port_write.patch`
#[test_case]
fn a_holder_that_deletes_its_port_capability_faults_on_its_next_port_write() {
    // The report endpoint is granted so slot 0 is what it is for every other child; this child
    // never sends on it (see the program's own doc for why it exits instead).
    let endpoints = crate::memory_region::create(2).expect("no endpoint region");
    let report = sched::create_rendezvous_from(endpoints).expect("no report rendezvous");
    let sup = sched::create_rendezvous_from(endpoints).expect("no sup rendezvous");
    let (holder, region) = build_child(
        &super::x86_programs::cap_delete_then_port_out(
            PORT_SLOT_WITHOUT_WAKE as u32,
            SCRATCH_PORT,
            SCRATCH_VAL,
        ),
        report,
        None,
        Some(crate::cap::Rights::WRITE),
        sup,
    );

    let msg = sched::ipc_receive(sup);
    assert_eq!(
        msg[0], EVENT_FAULT,
        "a holder that deleted its own port capability must fault on its next `out`; it kept the \
         ports after dropping the capability",
    );
    assert_eq!(msg[1], holder, "the fault named the wrong thread");
    assert_eq!(
        msg[2],
        CODE_VA + super::x86_programs::CAP_DELETE_THEN_PORT_OUT_PC_OFFSET,
        "the faulting pc was not the `out` instruction: a red for the wrong reason",
    );
    reap(region);
    sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

/// **A take-back leaves the invoker's own bitmap installed.** `PortRange::REVOKE` deletes the range
/// from every table but the invoker's; the invoker is the thread running on this core, so the
/// bitmap this core has installed is the invoker's own grant. Before the 2026-09-24 audit's fix the
/// arch half reset it regardless, and the invoker's next `out` faulted until its next switch-in.
///
/// The test thread is a kernel thread, so it cannot execute a ring-3 `out`; it installs the grant
/// the switch path would have installed and asks the TSS afterwards. Interrupts are masked across
/// the four steps because a switch away and back would re-install the test thread's own (absent)
/// grant and the assertion would read the switch rather than the take-back. The broadcast inside
/// the take-back needs nothing from this core's interrupts: the other cores answer an NMI.
#[test_case]
fn a_take_back_leaves_the_invokers_own_bitmap_installed() {
    use crate::arch::{interrupts, segments};

    let was_enabled = interrupts::disable();
    segments::set_port_range_grant(Some((COM1_BASE, COM1_COUNT)));
    sched::delete_port_range_caps_from_others(COM1_BASE, COM1_COUNT);
    let after = segments::installed_port_grant();
    segments::set_port_range_grant(None);
    interrupts::restore(was_enabled);

    assert_eq!(
        after,
        Some((COM1_BASE, COM1_COUNT)),
        "the take-back reset the invoker's own core; the sparing held in the table and not in the TSS",
    );
}

/// **A `PortRange` capability without `WRITE` must not grant port I/O** (lane/633-outsider-2, an
/// outsider pass over `notes/confinement-claims.md`, 2026-10-05 UTC).
///
/// Rows 27 to 29 claim things about *holding* a port capability, revoking one and deleting one.
/// None of them claims that the capability's **rights** gate the `in`/`out`, yet the rest of the
/// tree treats `WRITE` as the right that drives a port: `build_child` grants the driver `WRITE`
/// with the comment "the rights a driver gets", and `capability_plan` maps a `Use` component to
/// `WRITE` and a `Serve` component to `READ`. So a thread handed a `PortRange` narrowed to `READ`
/// alone (a `Serve`-shaped grant) should fault on `out`, exactly as a non-holder does.
///
/// **Fixed by milestone 768 (provisional)** (calef's ruling, 2026-10-05 UTC): the TSS bitmap cannot
/// grant `in` without `out`, so `sched::thread_control_block_insert_from` installs
/// `Thread::port_range_grant` only for a `PortRange` carrying `WRITE`, and this `READ`-only
/// capability grants nothing. Before the fix the grant ignored rights and the `out` was permitted
/// (supervision message `[EVENT_EXIT, ..]`); the escape is written up in
/// `notes/confinement-outsider-pass-2.md`. The claim is row 33 of `notes/confinement-claims.md`. The
/// stub is [`super::x86_programs::port_out_then_exit`], chosen over `port_out` so a wrongly-permitted
/// `out` exits rather than parking on a SEND and hanging the run (the row-26 hazard).
///
/// Falsification: replayable `system_tests/falsifications/user.x86_port_tests.a_read_only_port_capability_must_not_grant_port_output.patch`
#[test_case]
fn a_read_only_port_capability_must_not_grant_port_output() {
    let endpoints = crate::memory_region::create(2).expect("no endpoint region");
    let sup = sched::create_rendezvous_from(endpoints).expect("no sup rendezvous");
    // No wake and no report: the child only executes `out` and then exits or faults, so there is
    // nothing to receive on and no way for either outcome to park it on a rendezvous.
    let report = sched::create_rendezvous_from(endpoints).expect("no report rendezvous");
    let (child, region) = build_child(
        &super::x86_programs::port_out_then_exit(SCRATCH_PORT, SCRATCH_VAL),
        report,
        None,
        Some(crate::cap::Rights::READ),
        sup,
    );

    let msg = sched::ipc_receive(sup);
    assert_eq!(
        msg[0], EVENT_FAULT,
        "a thread whose only PortRange capability lacks WRITE executed `out` without faulting: \
         the port grant ignores the capability's rights (supervision message {msg:?})",
    );
    assert_eq!(msg[1], child, "the fault named the wrong thread");
    assert_eq!(
        msg[2],
        CODE_VA + super::x86_programs::PORT_OUT_PC_OFFSET,
        "the faulting pc was not the `out` instruction: a red for the wrong reason",
    );
    reap(region);
    sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

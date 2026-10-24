use abi::fault::{EVENT_EXIT, EVENT_FAULT, FAULT_EP_SLOT};

use super::*;
use crate::sched;

/// Where a hand-built child's code and stack go: the address-space map's image base and top stack
/// page, the same places a loaded program's would be (milestone 206 (a program image has under 896 KiB)). A child built from parts is
/// still a program, and putting it where programs go keeps its page tables the shape a real
/// program's are, which is what the current-CPU page's placement beside the stack assumes.
pub(super) const CODE_VA: u64 = address_space_map::IMAGE_BASE;
pub(super) const STACK_VA: u64 = address_space_map::STACK_TOP_PAGE;
/// The unmapped address the fault stub loads from. Distinctive, so the delivered fault address
/// proves the message carries real fault-time state and not a zero placeholder.
const BAD_ADDR: u64 = 0x00A5_0000;
/// The word the report stub SENDs, so a test can tell "the child ran" from "the child faulted."
pub(super) const REPORT_WORD: u64 = 0x42;

/// **How far past the entry [`FAULT_STUB`]'s faulting instruction begins**, which is the length of
/// the one instruction that precedes it. Four on both fixed-width ISAs and five on `x86_64`, where a
/// `mov r32, imm32` is five bytes; the constant exists because a test asserting on the faulting pc
/// used to add a literal 4 and would have been asserting aarch64's instruction width on a machine
/// that has none.
#[cfg(not(target_arch = "x86_64"))]
pub(super) const FAULT_PC_OFFSET: u64 = 4;
#[cfg(target_arch = "x86_64")]
pub(super) const FAULT_PC_OFFSET: u64 = super::x86_programs::FAULT_PC_OFFSET;

/// A child that faults on its very first memory access: load from [`BAD_ADDR`], which nothing
/// maps. Two instructions; the faulting one is the second, so the reported pc is `CODE_VA + 4`.
#[cfg(target_arch = "aarch64")]
pub(super) const FAULT_STUB: &[u32] = &[
    0xD2A0_14A0, // movz x0, #0xA5, lsl #16   (x0 = 0x00A5_0000)
    0xF940_0001, // ldr  x1, [x0]             (data abort: nothing maps BAD_ADDR)
];
#[cfg(target_arch = "riscv64")]
pub(super) const FAULT_STUB: &[u32] = &[
    0x00A5_0537, // lui a0, 0xA50             (a0 = 0x00A5_0000)
    0x0005_3583, // ld  a1, 0(a0)             (load page fault: nothing maps BAD_ADDR)
];
/// `x86_64`'s is in `user::x86_programs`, not here, because the boot tour needs the same program and
/// this module is `#[cfg(test)]`. See that module's header.
#[cfg(target_arch = "x86_64")]
pub(super) const FAULT_STUB: &[u32] = &super::x86_programs::fault(BAD_ADDR as u32);

/// A child that SENDs [`REPORT_WORD`] on the endpoint in slot 0, then exits cleanly. The same
/// nine-instruction shape the region-reclaim tests use, so "it ran" is the SEND arriving.
#[cfg(target_arch = "aarch64")]
pub(super) const REPORT_STUB: &[u32] = &[
    0xD280_0000,                                       // movz x0, #0            (slot 0)
    0xD280_0001,                                       // movz x1, #0            (rendezvous::SEND)
    0xD280_0000 | ((REPORT_WORD as u32) << 5) | 2,     // movz x2, #REPORT_WORD
    0xD280_0003,                                       // movz x3, #0
    0xD280_0004,                                       // movz x4, #0
    0xD280_0000 | ((abi::SYS_INVOKE as u32) << 5) | 8, // movz x8, #SYS_INVOKE
    0xD400_0001,                                       // svc #0                 (SEND)
    0xD280_0008,                                       // movz x8, #0            (SYS_EXIT)
    0xD400_0001,                                       // svc #0                 (exit)
];
#[cfg(target_arch = "riscv64")]
pub(super) const REPORT_STUB: &[u32] = &[
    0x0000_0513,                                    // li a0, 0            (slot 0)
    0x0000_0593,                                    // li a1, 0            (rendezvous::SEND)
    0x0000_0613 | ((REPORT_WORD as u32) << 20),     // li a2, REPORT_WORD
    0x0000_0693,                                    // li a3, 0
    0x0000_0713,                                    // li a4, 0
    0x0000_0893 | ((abi::SYS_INVOKE as u32) << 20), // li a7, SYS_INVOKE
    0x0000_0073,                                    // ecall               (SEND)
    0x0000_0893 | ((abi::SYS_EXIT as u32) << 20),   // li a7, SYS_EXIT
    0x0000_0073,                                    // ecall               (exit)
];
/// `x86_64`'s is in `user::x86_programs`; see [`FAULT_STUB`].
#[cfg(target_arch = "x86_64")]
pub(super) const REPORT_STUB: &[u32] = &super::x86_programs::report(REPORT_WORD as u32);

/// Build a child from `stub` with its whole world in one region (address space, code, stack, TCB), so a
/// single `DESTROY` reclaims it. `report` goes in slot 0 (what the report stub SENDs on);
/// `fault_ep`, if given, goes in the reserved fault slot, so `START` records it as the child's
/// supervision endpoint. Returns `(child_tid, region)`.
fn build_child(
    stub: &[u32],
    report: Option<sched::RendezvousId>,
    fault_ep: Option<sched::RendezvousId>,
) -> (u64, u64) {
    let region = crate::memory_region::create(16).expect("no region for the child");
    (build_child_in(region, stub, report, fault_ep), region)
}

/// [`build_child`], but into a region the caller already owns. The reap tests need this: §32's
/// property is that the reclaimed pages go back to **the builder's** budget, which can only be
/// observed if the builder's region is one the test still holds and can measure.
pub(super) fn build_child_in(
    region: u64,
    stub: &[u32],
    report: Option<sched::RendezvousId>,
    fault_ep: Option<sched::RendezvousId>,
) -> u64 {
    let report = report.map(|rep| {
        crate::cap::rendezvous_cap(
            rep,
            crate::cap::Rights::WRITE.union(crate::cap::Rights::GRANT),
        )
    });
    // Rights do not matter for the fault capability (the kernel reads only the endpoint name and
    // its badge, and consumes the slot at START, so the child cannot forge fault messages on it);
    // READ is the minimum.
    let fault = fault_ep.map(|fe| crate::cap::rendezvous_cap(fe, crate::cap::Rights::READ));
    build_child_with(region, stub, report.as_slice(), fault)
}

/// [`build_child_in`] with the capabilities spelled out: `caps` land in slots 0, 1, ... in order,
/// and `fault`, if given, in the reserved fault slot, so `START` records it as the child's
/// supervision endpoint and keeps its badge as the child's label (milestone 105 (the two forks)).
pub(super) fn build_child_with(
    region: u64,
    stub: &[u32],
    caps: &[crate::cap::Cap],
    fault: Option<crate::cap::Cap>,
) -> u64 {
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
    for (want, &cap) in caps.iter().enumerate() {
        let slot = sched::thread_control_block_insert_cap(tid, cap, None).expect("insert cap");
        assert_eq!(
            slot, want as u64,
            "a child's capability landed out of order (its stub names slots by number)"
        );
    }
    if let Some(cap) = fault {
        // The spawn-slot convention: the supervision endpoint goes in the reserved fault slot.
        sched::thread_control_block_insert_cap(tid, cap, Some(FAULT_EP_SLOT))
            .expect("insert fault ep");
    }
    sched::configure_thread_control_block(tid, CODE_VA, STACK_VA + page_frames::FRAME_SIZE, aspace)
        .expect("configure");
    sched::start_thread_control_block(tid, [0; 3]).expect("start");
    tid
}

/// **A crash becomes a message; the corpse survives until reaped; a fresh child runs.** The whole
/// supervision cycle in one test: spawn a child holding a fault endpoint, let it crash, receive
/// the fault message with the right tid and fault address, confirm the corpse still holds its
/// fault-time state (dead until reaped), reap it with revocation, and respawn a child that runs.
#[test_case]
fn a_faulting_child_reports_to_its_supervisor_and_is_reaped_then_respawned() {
    let endpoints = crate::memory_region::create(2).expect("no endpoint region");
    let fault_ep = sched::create_rendezvous_from(endpoints).expect("no fault rendezvous");
    let (child, region) = build_child(FAULT_STUB, None, Some(fault_ep));

    // The child faults on its first load. Its death arrives here, kernel-stamped.
    let msg = sched::ipc_receive(fault_ep);
    assert_eq!(msg[0], EVENT_FAULT, "a crash must report as a FAULT event");
    assert_eq!(msg[1], child, "the fault message named the wrong thread");
    assert_eq!(
        msg[2],
        CODE_VA + FAULT_PC_OFFSET,
        "the faulting pc was not the load instruction"
    );
    assert_eq!(
        msg[3], BAD_ADDR,
        "the faulting address was not carried in the message"
    );

    // Dead until reaped: the corpse is still in the table, still holding its fault-time state,
    // and it never runs again. This is what makes postmortem (and a future resume) possible.
    assert_eq!(
        sched::corpse_fault_msg(child),
        Some(msg),
        "the corpse did not retain its fault message: it was reaped too early, or lost its state",
    );

    // Reap it with §16 revocation, the supervisor's explicit act. The corpse is Dead, not live, so
    // the region reclaims without a force-kill. Retried, not asserted once: `depart` delivers the
    // death message before the corpse leaves its own kernel stack, and until its `switch_to` the
    // reap is refused, since a stack must not be unmapped under a core standing on it. See
    // notes/stack/kernel-stack-freed-under-its-owner.md and the `on_cpu` refusal in
    // `reap_region_objects`.
    assert!(
        super::wait_for(|| sched::reclaim_region(region).is_ok()),
        "reaping the corpse's region failed",
    );
    assert_eq!(
        sched::corpse_fault_msg(child),
        None,
        "the corpse outlived its region: revocation did not reap it",
    );

    // Respawn: a fresh child, in a fresh region, runs to completion where the crashed one died.
    let report = sched::create_rendezvous_from(endpoints).expect("no report rendezvous");
    let (_c2, region2) = build_child(REPORT_STUB, Some(report), None);
    assert_eq!(
        sched::ipc_receive(report)[0],
        REPORT_WORD,
        "the respawned child never ran: the supervision cycle did not recover",
    );
    // The respawn exits unsupervised, so it is reaped by the scheduler; reclaim once it is gone.
    // Clock-bounded (milestone 81): 2000 yields elapse in microseconds on the physical core, which
    // would leave the region unreclaimed and this test's litter for a neighbour to trip over.
    assert!(
        super::wait_for(|| sched::reclaim_region(region2).is_ok()),
        "the respawned child was never reaped, so its region could not be reclaimed",
    );
    sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

/// **A clean exit flows too, distinguished by the event code.** The other half of §26's "both
/// faults and exits": a supervised child that SENDs its word and exits normally reports an EXIT
/// event (not FAULT), with no fault pc or address, so a restart policy can tell "finished" from
/// "crashed."
#[test_case]
fn a_clean_exit_reports_the_exit_event_not_a_fault() {
    let endpoints = crate::memory_region::create(2).expect("no endpoint region");
    let report = sched::create_rendezvous_from(endpoints).expect("no report rendezvous");
    let fault_ep = sched::create_rendezvous_from(endpoints).expect("no fault rendezvous");
    let (child, region) = build_child(REPORT_STUB, Some(report), Some(fault_ep));

    // It runs (the SEND proves it reached EL0), then exits cleanly.
    assert_eq!(
        sched::ipc_receive(report)[0],
        REPORT_WORD,
        "the child never ran before exiting",
    );
    let msg = sched::ipc_receive(fault_ep);
    assert_eq!(
        msg[0], EVENT_EXIT,
        "a clean exit must report EXIT, not FAULT"
    );
    assert_eq!(msg[1], child, "the exit message named the wrong thread");
    assert_eq!(msg[2], 0, "a clean exit has no faulting pc");
    assert_eq!(msg[3], 0, "a clean exit has no faulting address");

    // A cleanly-exited supervised child is dead until reaped, exactly like a crashed one, and it
    // is retried for exactly the reason its crashing sibling above is: the death message arrives
    // before the corpse leaves its kernel stack. This one has never been seen to lose the race and
    // is written the same way anyway, because "has not happened yet" is not a property of the code.
    assert!(
        super::wait_for(|| sched::reclaim_region(region).is_ok()),
        "reaping the exited corpse's region failed",
    );
    sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

// ===========================================================================================
// Milestone 105: the death message carries the builder's label (DECISIONS §148 (resolves by
// asking the kernel) as amended 2026-10-04, ruling R3).
// ===========================================================================================

/// **A supervisor that reports what it hears, label included, twice.** Slot 0 is a report endpoint
/// (WRITE), slot 1 the supervision endpoint (READ). Each round zeroes argument register 5, does a
/// plain `RECEIVE` on slot 1, and `SEND`s `[tid, label, event]` on slot 0, which is `[w1, r5, w0]`
/// of what it received. Two rounds, then `SYS_EXIT`.
///
/// It is a hand-assembled stub rather than a program because what is under test is one register on
/// three architectures, and a stub names it outright: `x5`, `a5`, `r9`.
#[cfg(target_arch = "aarch64")]
const LABEL_REPORTER_STUB: &[u32] = &label_reporter_aarch64();
#[cfg(target_arch = "riscv64")]
const LABEL_REPORTER_STUB: &[u32] = &label_reporter_riscv64();
#[cfg(target_arch = "x86_64")]
const LABEL_REPORTER_STUB: &[u32] = &label_reporter_x86_64();

/// `movz xd, #imm` (aarch64).
#[cfg(target_arch = "aarch64")]
const fn movz(rd: u32, imm: u64) -> u32 {
    0xD280_0000 | ((imm as u32) << 5) | rd
}

#[cfg(target_arch = "aarch64")]
const fn label_reporter_aarch64() -> [u32; 32] {
    const SVC: u32 = 0xD400_0001;
    let round = [
        movz(0, 1),                        // slot 1: the supervision endpoint
        movz(1, abi::rendezvous::RECEIVE), //
        movz(2, 0),                        //
        movz(3, 0),                        //
        movz(4, 0),                        //
        movz(5, 0),                        // the label register, zero unless the kernel writes it
        movz(8, abi::SYS_INVOKE),          //
        SVC,                               // RECEIVE: x0 = event, x1 = tid, x5 = label
        0xAA01_03E2,                       // mov x2, x1   (tid)
        0xAA05_03E3,                       // mov x3, x5   (label)
        0xAA00_03E4,                       // mov x4, x0   (event)
        movz(0, 0),                        // slot 0: the report endpoint
        movz(1, abi::rendezvous::SEND),    //
        movz(8, abi::SYS_INVOKE),          //
        SVC,                               // SEND [tid, label, event]
    ];
    let mut out = [0u32; 32];
    let mut i = 0;
    while i < 15 {
        out[i] = round[i];
        out[15 + i] = round[i];
        i += 1;
    }
    out[30] = movz(8, abi::SYS_EXIT);
    out[31] = SVC;
    out
}

/// `addi rd, x0, imm`, which is `li` for a small immediate (riscv64).
#[cfg(target_arch = "riscv64")]
const fn li(rd: u32, imm: u64) -> u32 {
    0x13 | (rd << 7) | ((imm as u32) << 20)
}

#[cfg(target_arch = "riscv64")]
const fn label_reporter_riscv64() -> [u32; 32] {
    const ECALL: u32 = 0x0000_0073;
    let round = [
        li(10, 1),                        // a0: slot 1, the supervision endpoint
        li(11, abi::rendezvous::RECEIVE), // a1
        li(12, 0),                        // a2
        li(13, 0),                        // a3
        li(14, 0),                        // a4
        li(15, 0),                        // a5: the label register
        li(17, abi::SYS_INVOKE),          // a7
        ECALL,                            // RECEIVE: a0 = event, a1 = tid, a5 = label
        0x0005_8613,                      // mv a2, a1   (tid)
        0x0007_8693,                      // mv a3, a5   (label)
        0x0005_0713,                      // mv a4, a0   (event)
        li(10, 0),                        // a0: slot 0, the report endpoint
        li(11, abi::rendezvous::SEND),    // a1
        li(17, abi::SYS_INVOKE),          // a7
        ECALL,                            // SEND [tid, label, event]
    ];
    let mut out = [0u32; 32];
    let mut i = 0;
    while i < 15 {
        out[i] = round[i];
        out[15 + i] = round[i];
        i += 1;
    }
    out[30] = li(17, abi::SYS_EXIT);
    out[31] = ECALL;
    out
}

/// The `x86_64` twin: one 48-byte round, its register roles beside its bytes. Two rounds, then
/// `mov eax, SYS_EXIT; syscall` and one `nop` to fill the last word.
#[cfg(target_arch = "x86_64")]
const fn label_reporter_x86_64() -> [u32; 26] {
    const {
        assert!(
            abi::rendezvous::SEND == 0,
            "`xor esi, esi` encodes SEND as zero"
        );
    }
    let rcv = (abi::rendezvous::RECEIVE as u32).to_le_bytes();
    let inv = (abi::SYS_INVOKE as u32).to_le_bytes();
    let ext = (abi::SYS_EXIT as u32).to_le_bytes();
    let round: [u8; 48] = [
        0xBF, 0x01, 0x00, 0x00, 0x00, // mov edi, 1 (slot 1: the supervision endpoint)
        0xBE, rcv[0], rcv[1], rcv[2], rcv[3], // mov esi, RECEIVE
        0x31, 0xD2, // xor edx, edx
        0x45, 0x31, 0xD2, // xor r10d, r10d
        0x45, 0x31, 0xC0, // xor r8d, r8d
        0x45, 0x31, 0xC9, // xor r9d, r9d (the label register)
        0xB8, inv[0], inv[1], inv[2], inv[3], // mov eax, SYS_INVOKE
        0x0F, 0x05, // syscall (RECEIVE: rdi = event, rsi = tid, r9 = label)
        0x48, 0x89, 0xF2, // mov rdx, rsi (tid)
        0x4D, 0x89, 0xCA, // mov r10, r9 (label)
        0x49, 0x89, 0xF8, // mov r8, rdi (event)
        0x31, 0xFF, // xor edi, edi (slot 0: the report endpoint)
        0x31, 0xF6, // xor esi, esi (SEND, which is 0)
        0xB8, inv[0], inv[1], inv[2], inv[3], // mov eax, SYS_INVOKE
        0x0F, 0x05, // syscall (SEND [tid, label, event])
    ];
    let mut b = [0x90u8; 104];
    let mut i = 0;
    while i < 48 {
        b[i] = round[i];
        b[48 + i] = round[i];
        i += 1;
    }
    let tail = [0xB8, ext[0], ext[1], ext[2], ext[3], 0x0F, 0x05];
    let mut k = 0;
    while k < 7 {
        b[96 + k] = tail[k];
        k += 1;
    }
    let mut out = [0u32; 26];
    let mut w = 0;
    while w < 26 {
        out[w] = u32::from_le_bytes([b[4 * w], b[4 * w + 1], b[4 * w + 2], b[4 * w + 3]]);
        w += 1;
    }
    out
}

/// Two labels a builder might choose. Distinctive, and nothing like a tid or a slot number.
const LABEL_ONE: u64 = 0x1abe_0001;
const LABEL_TWO: u64 = 0x1abe_0002;

/// Build [`LABEL_REPORTER_STUB`] reporting on `report` and receiving on `fault_ep`, unsupervised,
/// in its own region. Returns `(tid, region)`.
fn label_reporter(report: sched::RendezvousId, fault_ep: sched::RendezvousId) -> (u64, u64) {
    let region = crate::memory_region::create(16).expect("no region for the supervisor stub");
    let caps = [
        crate::cap::rendezvous_cap(report, crate::cap::Rights::WRITE),
        crate::cap::rendezvous_cap(fault_ep, crate::cap::Rights::READ),
    ];
    (
        build_child_with(region, LABEL_REPORTER_STUB, &caps, None),
        region,
    )
}

/// A child that faults at once, supervised on `fault_ep` with `label` stamped on its capability,
/// the way a builder stamps it with `rendezvous::BADGE`. Returns `(tid, region)`.
fn labelled_child(fault_ep: sched::RendezvousId, label: u64) -> (u64, u64) {
    let region = crate::memory_region::create(16).expect("no region for the child");
    let fault = crate::cap::rendezvous_cap_badged(fault_ep, crate::cap::Rights::READ, label);
    (
        build_child_with(region, FAULT_STUB, &[], Some(fault)),
        region,
    )
}

/// A supervision endpoint and a report endpoint, carved from a two-page region the test owns, so
/// that reclaiming it gives both pages back. `sched::create_rendezvous` would carve them from the
/// kernel's own chunk supply instead, which is never freed: six of those across this file's label
/// tests crossed one more 32-page chunk and put the suite over its frame ledger. Reclaim the region
/// last, after the regions of every thread that held a capability to either endpoint. Returns
/// `(fault_ep, report, region)`.
fn label_endpoints() -> (sched::RendezvousId, sched::RendezvousId, u64) {
    let region = crate::memory_region::create(2).expect("no region for the test's endpoints");
    let fault_ep = sched::create_rendezvous_from(region).expect("no supervision rendezvous");
    let report = sched::create_rendezvous_from(region).expect("no report rendezvous");
    (fault_ep, report, region)
}

/// Reclaim every region a test built, once its threads are gone. Clock-bounded retries for the
/// reason [`a_faulting_child_reports_to_its_supervisor_and_is_reaped_then_respawned`] gives.
fn reclaim_all(regions: &[u64]) {
    for &region in regions {
        assert!(
            super::wait_for(|| sched::reclaim_region(region).is_ok()),
            "a region this test built could not be reclaimed",
        );
    }
}

/// A supervisor tells two dead children apart by the label the kernel delivers (milestone 105,
/// DECISIONS §148 as amended 2026-10-04, ruling R3).
///
/// Two children die on one supervision endpoint, each labeled by its builder, and a user-mode
/// supervisor reports each death's tid and label. Both routes are driven: corpses parked first are
/// collected from the sender queue (`collected_without_serving`), and a supervisor already blocked
/// in `RECEIVE` is handed the label at the rendezvous (`deliver_death`).
///
/// Falsification: replayable `system_tests/falsifications/user.supervision_tests.a_supervisor_tells_two_dead_children_apart_by_label.patch`
///
/// Name: provisional, milestone 105 (the two forks)'s lane, 2026-10-05 (UTC). A test name states its claim as a sentence.
#[test_case]
fn a_supervisor_tells_two_dead_children_apart_by_label() {
    // Corpses first: both are parked with their messages before anyone receives.
    let (fault_ep, report, re) = label_endpoints();
    let (one, r1) = labelled_child(fault_ep, LABEL_ONE);
    let (two, r2) = labelled_child(fault_ep, LABEL_TWO);
    assert!(
        super::wait_for(|| sched::rendezvous_waiting_senders(fault_ep) == 2),
        "the two children never parked their deaths on the supervision endpoint",
    );
    let (_sup, rs) = label_reporter(report, fault_ep);
    let mut heard = [sched::ipc_receive(report), sched::ipc_receive(report)];
    heard.sort_unstable_by_key(|m| m[1]);
    assert_eq!(
        [(heard[0][0], heard[0][1]), (heard[1][0], heard[1][1])],
        [(one, LABEL_ONE), (two, LABEL_TWO)],
        "a parked corpse's death arrived without its own label (each pair is tid, label)",
    );
    assert_eq!(
        heard[0][2], EVENT_FAULT,
        "the first child did not die of its fault"
    );
    reclaim_all(&[r1, r2, rs, re]);

    // Supervisor first: it is blocked in RECEIVE when each child dies.
    let (fault_ep, report, re) = label_endpoints();
    let (_sup, rs) = label_reporter(report, fault_ep);
    let mut regions = [rs, 0, 0, re];
    for (i, label) in [LABEL_TWO, LABEL_ONE].into_iter().enumerate() {
        assert!(
            super::wait_for(|| sched::rendezvous_waiting_receivers(fault_ep) == 1),
            "the supervisor never blocked in RECEIVE",
        );
        let (child, region) = labelled_child(fault_ep, label);
        regions[i + 1] = region;
        let msg = sched::ipc_receive(report);
        assert_eq!(
            (msg[0], msg[1]),
            (child, label),
            "a death delivered to a waiting supervisor arrived without its own label \
             (each pair is tid, label)",
        );
    }
    reclaim_all(&regions);
}

/// A child can neither learn its label nor forge one (milestone 105, DECISIONS §148 as amended).
///
/// Learn: after `START` no capability the child holds carries its label (the kernel consumed the
/// fault slot and kept the badge), and it starts with zeroed argument registers. Forge: a second
/// child `SEND`s on a `WRITE` capability to the same endpoint badged with that very label, and its
/// message must reach the supervisor with label `0`: a sender's badge arrives in word 3, and only
/// the kernel's death path writes the label register.
///
/// Falsification: replayable `system_tests/falsifications/user.supervision_tests.a_child_can_neither_learn_nor_forge_its_label.patch`
///
/// Name: provisional, milestone 105 (the two forks)'s lane, 2026-10-05 (UTC). A test name states its claim as a sentence.
#[test_case]
fn a_child_can_neither_learn_nor_forge_its_label() {
    let (fault_ep, report, re) = label_endpoints();

    let (child, r1) = labelled_child(fault_ep, LABEL_ONE);
    let (slot_empty, carries_label) = sched::with_capability_table(child, |table| {
        let carries = (0..crate::cap::CAPABILITY_TABLE_SLOTS as u64).any(|slot| {
            table.get(slot).is_ok_and(|c| {
                matches!(c.object, crate::cap::Object::Rendezvous(_, badge) if badge == LABEL_ONE)
            })
        });
        (table.get(FAULT_EP_SLOT).is_err(), carries)
    })
    .expect("the child is gone already");
    assert!(
        slot_empty,
        "the fault slot still holds a capability after START: the child can read its supervision \
         capability",
    );
    assert!(
        !carries_label,
        "a capability the child holds carries its label"
    );

    let forger_region = crate::memory_region::create(16).expect("no region for the forger");
    let forged = crate::cap::rendezvous_cap_badged(fault_ep, crate::cap::Rights::WRITE, LABEL_ONE);
    let forger = build_child_with(forger_region, REPORT_STUB, &[forged], None);

    let (_sup, rs) = label_reporter(report, fault_ep);
    let heard = [sched::ipc_receive(report), sched::ipc_receive(report)];
    let real = heard
        .iter()
        .find(|m| m[2] == EVENT_FAULT)
        .expect("the labelled child's death never reached the supervisor");
    assert_eq!(
        (real[0], real[1]),
        (child, LABEL_ONE),
        "the real death did not carry its label, so the forgery check below proves nothing",
    );
    let fake = heard
        .iter()
        .find(|m| m[2] == REPORT_WORD)
        .expect("the forger's message never reached the supervisor");
    assert_eq!(
        fake[1], 0,
        "a message a child sent arrived carrying a label: a label can be forged \
         (the forger {forger} sent through a capability badged with it)",
    );
    reclaim_all(&[r1, forger_region, rs, re]);
}

//! **A supervised child built from parts**: a hand-assembled program, its whole world carved from
//! one untyped region, and its supervision endpoint in the reserved fault slot, on all three ports.
//!
//! The boot self-test's `supervised` and `teardown` checks spawn through [`spawn`], and so does
//! every system test that needs a child it can watch die (`system_tests`' `supervision_tests` and
//! the files that borrow its stubs). One copy, so the check on every boot and the suite prove the
//! same path. It is the TCB path (`create_thread_control_block`, `CAP_INSERT`, `CONFIGURE`,
//! `START`), not [`super::run`]: supervision is fixed at `START` from the fault slot and never
//! afterward (DECISIONS §26 (the fault endpoint)), and a thread `run` enters never passes `START`.
//! See notes/self-test-checks.md.
//!
//! # BUGS
//!
//! - The stubs are machine code written by hand, one listing per port. A wrong encoding shows up
//!   only as a child that faults somewhere unexpected; the self-test and the suite both assert the
//!   faulting pc for that reason.
//!
//! Name: provisional, minted 2026-10-24 (UTC) by milestone/944-self-test-supervision-and-teardown.

use super::*;
use crate::cap::Cap as Capability;
use crate::sched::thread_control_block_insert_cap as insert_capability;

/// Where a child's code goes: the address-space map's image base, where a loaded program's would
/// be (milestone 206 (a program image has under 896 KiB)). A child built from parts is still a
/// program, and keeping its page tables the shape a real program's are is what the current-CPU
/// page's placement beside the stack assumes.
///
/// Name: provisional, minted 2026-10-24 (UTC) by milestone/944-self-test-supervision-and-teardown.
pub const CODE_ADDRESS: u64 = address_space_map::IMAGE_BASE;
/// Where its one stack page goes: the map's top stack page; `sp` starts at the page's top.
///
/// Name: provisional, minted 2026-10-24 (UTC) by milestone/944-self-test-supervision-and-teardown.
pub const STACK_ADDRESS: u64 = address_space_map::STACK_TOP_PAGE;

/// The unmapped address [`FAULT_STUB`] loads from. Distinctive, so the delivered fault address
/// proves the message carries real fault-time state and not a zero placeholder. It is below
/// [`CODE_ADDRESS`] on every port and nothing maps it.
///
/// Name: provisional, minted 2026-10-24 (UTC) by milestone/944-self-test-supervision-and-teardown.
pub const FAULT_ADDRESS: u64 = 0x00A5_0000;

/// The word [`REPORT_STUB`] SENDs, so a caller can tell "the child ran" from "the child faulted."
///
/// Name: provisional, minted 2026-10-24 (UTC) by milestone/944-self-test-supervision-and-teardown.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub const REPORT_WORD: u64 = 0x42;

/// **How far past the entry [`FAULT_STUB`]'s faulting instruction begins**: the length of the one
/// instruction before it. Four on both fixed-width ports and five on `x86_64`, where a
/// `mov r32, imm32` is five bytes.
///
/// Name: provisional, minted 2026-10-24 (UTC) by milestone/944-self-test-supervision-and-teardown.
#[cfg(not(target_arch = "x86_64"))]
pub const FAULT_PC_OFFSET: u64 = 4;
/// [`FAULT_PC_OFFSET`] on `x86_64`.
///
/// Name: provisional, minted 2026-10-24 (UTC) by milestone/944-self-test-supervision-and-teardown.
#[cfg(target_arch = "x86_64")]
pub const FAULT_PC_OFFSET: u64 = x86_programs::FAULT_PC_OFFSET;

/// **A child that faults on its very first memory access**: a load from [`FAULT_ADDRESS`]. Two
/// instructions; the second faults, so the reported pc is `CODE_ADDRESS + FAULT_PC_OFFSET`.
///
/// Name: provisional, minted 2026-10-24 (UTC) by milestone/944-self-test-supervision-and-teardown.
#[cfg(target_arch = "aarch64")]
pub const FAULT_STUB: &[u32] = &[
    0xD2A0_14A0, // movz x0, #0xA5, lsl #16   (x0 = 0x00A5_0000)
    0xF940_0001, // ldr  x1, [x0]             (data abort: nothing maps FAULT_ADDRESS)
];
/// [`FAULT_STUB`] on riscv64.
///
/// Name: provisional, minted 2026-10-24 (UTC) by milestone/944-self-test-supervision-and-teardown.
#[cfg(target_arch = "riscv64")]
pub const FAULT_STUB: &[u32] = &[
    0x00A5_0537, // lui a0, 0xA50             (a0 = 0x00A5_0000)
    0x0005_3583, // ld  a1, 0(a0)             (load page fault: nothing maps FAULT_ADDRESS)
];
/// [`FAULT_STUB`] on `x86_64`.
///
/// Name: provisional, minted 2026-10-24 (UTC) by milestone/944-self-test-supervision-and-teardown.
#[cfg(target_arch = "x86_64")]
pub const FAULT_STUB: &[u32] = &x86_programs::fault(FAULT_ADDRESS as u32);

/// **A child that SENDs [`REPORT_WORD`] on the endpoint in slot 0, then exits cleanly.** Nine
/// instructions on the fixed-width ports, so "it ran" is the SEND arriving and "it finished" is
/// the exit.
///
/// Name: provisional, minted 2026-10-24 (UTC) by milestone/944-self-test-supervision-and-teardown.
#[cfg(target_arch = "aarch64")]
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub const REPORT_STUB: &[u32] = &[
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
/// [`REPORT_STUB`] on riscv64.
///
/// Name: provisional, minted 2026-10-24 (UTC) by milestone/944-self-test-supervision-and-teardown.
#[cfg(target_arch = "riscv64")]
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub const REPORT_STUB: &[u32] = &[
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
/// [`REPORT_STUB`] on `x86_64`.
///
/// Name: provisional, minted 2026-10-24 (UTC) by milestone/944-self-test-supervision-and-teardown.
#[cfg(target_arch = "x86_64")]
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub const REPORT_STUB: &[u32] = &x86_programs::report(REPORT_WORD as u32);

/// **A fresh page from `region` holding `code`, ready for the instruction fetcher**: retyped,
/// written through the direct map, and made coherent with `sync_icache`. `None` when the region is
/// spent or `code` does not fit one page.
///
/// Name: provisional, minted 2026-10-24 (UTC) by milestone/944-self-test-supervision-and-teardown.
pub fn code_page(region: u64, code: &[u32]) -> Option<u64> {
    if size_of_val(code) > FRAME_SIZE as usize {
        return None;
    }
    let phys = crate::memory_region::retype_page(region)?;
    let va = mmu::phys_to_virt(phys);
    // SAFETY: `retype_page` hands back a whole frame carved from `region` for this caller alone,
    // and the direct map names every RAM page, so the slice is the frame and nothing aliases it.
    let page = unsafe {
        core::slice::from_raw_parts_mut(va as *mut u32, FRAME_SIZE as usize / size_of::<u32>())
    };
    page[..code.len()].copy_from_slice(code);
    sync_icache(va, size_of_val(code));
    Some(phys)
}

/// **Build a child from `stub` with its whole world in `region`, and start it.**
///
/// The address space, the code and stack pages and the TCB all come out of `region`, so one reap
/// of it (`sched::reap_supervised`, or `sched::reclaim_region`) gives every frame back. `capabilities` land
/// in slots 0, 1, ... in order, because a stub names its slots by number. `fault`, if given, goes in
/// the reserved fault slot (`abi::fault::FAULT_EP_SLOT`), so `START` records it as the child's
/// supervision endpoint and keeps its badge as the child's label (milestone 105 (the two forks)).
/// Its rights do not matter: the kernel reads only the endpoint and the badge, and consumes the
/// slot. Returns the child's thread id, or which step refused.
///
/// Name: provisional, minted 2026-10-24 (UTC) by milestone/944-self-test-supervision-and-teardown.
pub fn spawn(
    region: u64,
    stub: &[u32],
    capabilities: &[Capability],
    fault: Option<Capability>,
) -> Result<crate::thread::ThreadId, &'static str> {
    let aspace = user_address_space_create(region).ok_or("no address space for the child")?;
    let code_phys = code_page(region, stub).ok_or("no code page for the child")?;
    user_address_space_map(
        aspace,
        CODE_ADDRESS,
        code_phys,
        Flags::user_code(),
        crate::revoke::PageMapSource::NoCapability,
    )
    .map_err(|_| "could not map the child's code")?;
    let stack_phys =
        crate::memory_region::retype_page(region).ok_or("no stack page for the child")?;
    user_address_space_map(
        aspace,
        STACK_ADDRESS,
        stack_phys,
        Flags::user_data(),
        crate::revoke::PageMapSource::NoCapability,
    )
    .map_err(|_| "could not map the child's stack")?;

    let tid = crate::sched::create_thread_control_block(region).ok_or("no TCB for the child")?;
    for (want, &capability) in capabilities.iter().enumerate() {
        let slot = insert_capability(tid, capability, None)
            .map_err(|_| "no room for one of the child's capabilities")?;
        if slot != want as u64 {
            return Err(
                "a child's capability landed out of order, and its stub names slots by number",
            );
        }
    }
    if let Some(capability) = fault {
        insert_capability(tid, capability, Some(abi::fault::FAULT_EP_SLOT))
            .map_err(|_| "no room for the child's supervision endpoint")?;
    }
    crate::sched::configure_thread_control_block(
        tid,
        CODE_ADDRESS,
        STACK_ADDRESS + FRAME_SIZE,
        aspace,
    )
    .map_err(|_| "could not configure the child")?;
    crate::sched::start_thread_control_block(tid, [0; 3])
        .map_err(|_| "could not start the child")?;
    Ok(tid)
}

/// **The capability [`spawn`] takes as `fault`, for a supervision endpoint `ep` with no label.**
/// READ is the minimum; the kernel reads only the endpoint and the badge.
///
/// Name: provisional, minted 2026-10-24 (UTC) by milestone/944-self-test-supervision-and-teardown.
pub fn supervision_endpoint(ep: crate::sched::RendezvousId) -> Capability {
    crate::cap::rendezvous_cap(ep, crate::cap::Rights::READ)
}

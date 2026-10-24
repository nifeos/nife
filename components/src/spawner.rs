//! **The construction sub-server: process building, and nothing else** (milestone 22 phase B.2).
//!
//! It holds one untyped budget (WRITE only, so it may spend memory but never lend it), a request
//! channel, and one program image `root_supervisor` copied into its address space, but no initial
//! ramdisk, so the only program it can build is the one it was handed. Each instance gets its own
//! region split off the budget, so one corpse cannot pin another's memory, and the region
//! capability is deleted once the child starts (§32). The supervisor's reap returns the pages LIFO,
//! so a restart loop is not a leak. It reads no fault message, holds no policy and answers requests
//! in arrival order. See notes/trusted-init/supervision-tree.md.
//!
//! Name: ratified 2026-07-30 (calef, DECISIONS §39), among the names recorded there as always
//! right.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

// Each binary in the tree compiles the shared module but uses a different slice of it (the sub-server
// builds nothing, the supervisor holds no memory), so the unused halves are expected, not dead. This
// is the one shape where a blanket allow is the honest one: no single binary uses all of it (§38).
use supervision_protocol::{ChildEndowment, REP_BUILT, REP_FAILED, REQ_BUILD, Retention};
use user_mode_runtime::{badge, cap_delete, receive, send};

/// The capabilities `root_supervisor` endowed us with, in order.
const REQ: u64 = 0; // READ: build/reap requests arrive here
const REP: u64 = 1; // WRITE: answers go back here
const BUDGET: u64 = 2; // WRITE: the memory every instance is made of
const REPORT: u64 = 3; // WRITE|GRANT: so each instance can report for itself
const CHILDFAULT: u64 = 4; // READ|GRANT: so each instance is born supervised

/// Where `root_supervisor` copied the one program image we may build. Must match `root_supervisor.rs`.
const IMAGE_VA: u64 = address_space_map::runtime_window(0x3000_0000);

/// Pages per instance region: enough for the sub-server's segments, its stack, its address-space
/// tables, and its TCB. A debug build of a tiny program is a handful of pages.
const INSTANCE_PAGES: u64 = 48;

#[unsafe(no_mangle)]
pub extern "C" fn _start(_a0: u64, image_len: u64, _a2: u64) -> ! {
    // SAFETY: root_supervisor mapped `image_len` bytes of program image read-only at IMAGE_VA.
    let image = unsafe { core::slice::from_raw_parts(IMAGE_VA as *const u8, image_len as usize) };
    let Ok(elf) = elf::Elf::parse(image) else {
        supervision_protocol::fail()
    };

    loop {
        let (operation, attempt, label) = receive(REQ);
        match operation {
            REQ_BUILD => {
                let ok = build(&elf, attempt, label);
                send(REP, if ok { REP_BUILT } else { REP_FAILED }, 0, 0);
            }
            _ => {
                send(REP, REP_FAILED, 0, 0);
            }
        }
    }
}

/// Build one instance in its own region, endowed with a report endpoint and its supervision endpoint,
/// started with `attempt` in its second argument register; `false` on any refusal.
///
/// `label` is stamped on a badged copy of our supervision endpoint that goes in the child's fault
/// slot, so the kernel hands it back with the child's death message, and the child, started with
/// nothing but `attempt`, can neither learn it nor lie about it (DECISIONS §148 (resolves by asking
/// the kernel) as amended 2026-10-04). We keep nothing afterwards: the TCB capability is not the
/// thread, and the region capability is not the reap (§32). See notes/trusted-init/supervision-tree.md.
fn build(elf: &elf::Elf, attempt: u64, label: u64) -> bool {
    let labelled = badge(CHILDFAULT, label);
    if labelled < 0 {
        return false; // a zero label, which BADGE refuses, or a full capability table
    }
    let labelled = labelled as u64;
    let built = build_labelled(elf, attempt, labelled);
    cap_delete(labelled);
    built
}

/// [`build`] with the labelled supervision capability already minted in slot `fault`.
fn build_labelled(elf: &elf::Elf, attempt: u64, fault: u64) -> bool {
    let Ok(region) = supervision_protocol::memory_region_split(BUDGET, INSTANCE_PAGES) else {
        return false;
    };
    let Ok(child) = supervision_protocol::build_child(
        BUDGET,
        region,
        elf,
        &ChildEndowment {
            caps: &[(REPORT, abi::rights::WRITE)],
            maps: &[],
            blobs: &[],
            fault: Some(fault),
            ..ChildEndowment::new(Retention::Nothing)
        },
    ) else {
        return false;
    };
    if !supervision_protocol::start_child(child, 0, attempt, 0) {
        return false;
    }
    cap_delete(region);
    true
}

user_mode_runtime::panic_handler!();

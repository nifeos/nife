//! **A builder that gives its scratch window back, and then reaches for it** (milestone 95 (an
//! unmap primitive, and the mappings init never lets go), §249 (a running address space stays
//! nameable)).
//!
//! Holds a memory region the child is built straight out of (slot 0), a report endpoint (slot 1), a
//! small region that pays for its own page tables (slot 2), and, in one of the kernel test's two
//! runs, a `WRITE` capability to its own address space (slot 3), as the kernel endows the
//! progenitor at slot 28. It builds one child with `supervision_protocol::build_child`, the path
//! the progenitor builds every boot server with, then writes to the first page of its own scratch
//! window, where the loader filled the child's first page.

//! # What the two runs prove together
//!
//! With slot 3, the loader gave that page up with `UNMAP` as soon as it was in the child, so the
//! write faults and the second report never comes: milestone 95's negative control, a builder
//! faulting on a page it built for a child. Without slot 3 the same write lands and the second
//! report arrives, the window a builder that cannot unmap leaves open. The second run keeps the
//! first from passing for the wrong reason (a write that faulted because nothing was ever mapped
//! there).

//! # What it reports
//!
//! `(1, va, 0)` once the child is built, `va` being the scratch page it is about to write, and
//! `(2, va, 0)` only if the write landed; on a failed build, `(9, 0, 0)`. The kernel's test
//! (`system_tests`' `running_space_tests`) mirrors these three words, as `authority_tests` mirrors
//! `root_supervisor`'s.
//!
//! Name: provisional (milestone 95's §249 lane, 2026-10-05 UTC).

#![no_std]
// Program entry points, not the crates/ library surface the ratchet of milestone 68 (code-quality
// gates) tracks (DECISIONS §107 (`missing_docs` moves to `workspace.lints.rust`)): each `[[bin]]` is
// its own crate root with one `_start`.
#![allow(missing_docs)]
#![no_main]

use supervision_protocol::{
    ChildEndowment, Retention, SCRATCH_WINDOW, build_child, give_up_scratch_through,
};
use user_mode_runtime::{cap_delete, exit, is_granted, send};

const MEMORY_REGION: u64 = 0;
const REPORT: u64 = 1;
/// The builder's own page-table budget: a region of its own rather than a split of slot 0, for
/// `scratch_window_exerciser`'s reason (no table of ours sits inside the child's region).
const OWN: u64 = 2;
/// A capability to this process's own address space, or nothing (the control run).
const OWN_SPACE: u64 = 3;

const REPORT_BUILT: u64 = 1;
const REPORT_WRITE_LANDED: u64 = 2;
const REPORT_FAILED: u64 = 9;

const PAGE: u64 = 4096;
const EHDR: usize = 64;
const PHDR: usize = 56;

/// Stack pages to add below the one the kernel maps: the loader's call chain overflows one page
/// (`scratch_window_exerciser` measured it), and eight in all is the progenitor's own number.
const EXTRA_STACK_PAGES: u64 = 7;

/// An ELF header and one `PT_LOAD`: a single executable page at the image base, holding the entry
/// point. Enough for `elf::Elf::parse`, which is all the loader needs; the child is never started.
fn forge(out: &mut [u8; EHDR + PHDR]) {
    let base = address_space_map::IMAGE_BASE;
    out[0..4].copy_from_slice(b"\x7fELF");
    out[4] = 2; // 64-bit
    out[5] = 1; // little-endian
    out[6] = 1; // version
    out[16..18].copy_from_slice(&2u16.to_le_bytes()); // ET_EXEC
    out[18..20].copy_from_slice(&elf::NATIVE_MACHINE.to_le_bytes());
    out[20..24].copy_from_slice(&1u32.to_le_bytes());
    out[24..32].copy_from_slice(&base.to_le_bytes()); // entry
    out[32..40].copy_from_slice(&(EHDR as u64).to_le_bytes()); // phoff
    out[52..54].copy_from_slice(&(EHDR as u16).to_le_bytes());
    out[54..56].copy_from_slice(&(PHDR as u16).to_le_bytes());
    out[56..58].copy_from_slice(&1u16.to_le_bytes()); // phnum
    let ph = &mut out[EHDR..EHDR + PHDR];
    ph[0..4].copy_from_slice(&1u32.to_le_bytes()); // PT_LOAD
    ph[4..8].copy_from_slice(&(elf::PF_R | elf::PF_X).to_le_bytes());
    ph[16..24].copy_from_slice(&base.to_le_bytes());
    ph[24..32].copy_from_slice(&base.to_le_bytes());
    ph[40..48].copy_from_slice(&PAGE.to_le_bytes());
    ph[48..56].copy_from_slice(&PAGE.to_le_bytes());
}

#[unsafe(no_mangle)]
pub extern "C" fn _start(_arg0: u64, _arg1: u64, _arg2: u64) -> ! {
    // Before anything deep runs, and from the budget ahead of the child's carve, so they never sit
    // above the child's region.
    for k in 1..=EXTRA_STACK_PAGES {
        let frame = user_mode_runtime::retype_into_page_frame(MEMORY_REGION);
        if frame < 0
            || !user_mode_runtime::map_page_frame(
                frame as u64,
                address_space_map::STACK_TOP_PAGE - k * PAGE,
                true,
                MEMORY_REGION,
            )
        {
            user_mode_runtime::trap()
        }
        cap_delete(frame as u64);
    }
    build_then_reach()
}

#[inline(never)]
fn build_then_reach() -> ! {
    // Exactly what `system_initializer::boot` does with slot 28, one slot number over.
    if is_granted(OWN_SPACE) {
        give_up_scratch_through(OWN_SPACE);
    }

    let mut bytes = [0u8; EHDR + PHDR];
    forge(&mut bytes);
    let Ok(image) = elf::Elf::parse(&bytes) else {
        send(REPORT, REPORT_FAILED, 0, 0);
        exit()
    };
    // Straight out of the budget rather than a split of it: the kernel's test reclaims the budget
    // once this process is gone, and a child region left inside it (whose capability died with us)
    // would make that reclaim refuse for good.
    let Ok(child) = build_child(
        OWN,
        MEMORY_REGION,
        &image,
        &ChildEndowment::new(Retention::Nothing),
    ) else {
        send(REPORT, REPORT_FAILED, 0, 0);
        exit()
    };
    // Never started: the kernel's test reclaims the budget, and the child with it.
    cap_delete(child.tcb);

    // The first page this process ever mapped in its scratch window, which is where the loader
    // filled the child's code page: the cursor starts at the window's base, and nothing else here
    // maps scratch before the build.
    let va = SCRATCH_WINDOW.start;
    send(REPORT, REPORT_BUILT, va, 0);
    // SAFETY: deliberately not. With slot 3 this page was given up and the store faults, which is
    // the point; without it the page is the child's code frame, mapped read/write in our window,
    // and one byte of a never-started child is ours to spoil.
    unsafe { core::ptr::write_volatile(va as *mut u8, 0xA5) };
    send(REPORT, REPORT_WRITE_LANDED, va, 0);
    exit()
}

user_mode_runtime::panic_handler!();

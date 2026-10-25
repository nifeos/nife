//! **A builder that builds more than its scratch window holds, and keeps building** (milestone 604
//! (provisional), the builder's scratch cursor is bounded).
//!
//! Holds a memory region to build children from (slot 0), a report endpoint (slot 1), and a second,
//! 48-page region that pays for its own page tables (slot 2). It forges a `ripgrep`-sized ELF, 668
//! pages of image, the span milestone 121 (ripgrep on nife) measured, and builds a child from it
//! with `supervision_protocol::build_child_space` [`BUILDS`] times, each out of a region of its own
//! that it destroys before the next. It never starts a child: what is under test is the builder's
//! own address space, not the child's.

//! # What it proves
//!
//! Every page the loader fills is mapped once in the builder's own scratch window, and there is no
//! unmap. The window is taken back only because destroying the child's region revokes every mapping
//! of its pages, the builder's included (DECISIONS §13 (frame revocation)). [`BUILDS`] builds put
//! 26,720 pages through the window, 1.6 laps of it, so they pass only if the loader reuses the
//! pages the kernel gave back.

//! # The negative control
//!
//! The builder's own page tables come out of slot 2, 48 pages, which covers the window's 32
//! last-level tables and the one or two above them. A cursor that never came back needs a new table
//! every 512 pages, 53 for this run. Measured on aarch64 on 2026-09-26 with the loader's wrap taken
//! out: 36 builds, 24,048 scratch pages, and the 37th refused. So this is not a test that passes by
//! having room to spare.

//! # What it reports
//!
//! `(builds, pages, window_pages)` once, then it exits: the builds that succeeded, the scratch
//! pages they used between them, and the window's own size, so the kernel's test can check the run
//! went round the window more than once without knowing this crate's numbers.
//!
//! Name: provisional (milestone 604).

#![no_std]
// Program entry points, not the crates/ library surface the ratchet of milestone 68 (code-quality
// gates) tracks (DECISIONS §107 (`missing_docs` moves to `workspace.lints.rust`)): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use supervision_protocol::{
    ChildEndowment, Retention, SCRATCH_WINDOW_PAGES, build_child_space, memory_region_destroy,
    memory_region_split,
};
use user_mode_runtime::{cap_delete, exit, send};

const MEMORY_REGION: u64 = 0;
const REPORT: u64 = 1;
/// The builder's own budget, for the page tables behind its scratch window: 48 pages, set by the
/// kernel's test. A region of its own rather than a split of slot 0, so no table retyped mid-build
/// sits above a child's region and stops it going back (the carve order `memory_regions` needs),
/// and so the kernel can reclaim it once this address space is gone.
const OWN: u64 = 2;

/// How many children to build: past the 37th, where a climbing cursor runs out of tables on this
/// budget, and past one lap of the window. It was 110 (more than the progenitor's old 65,536 pages
/// to its initial ramdisk window) until CI's aarch64 leg failed to finish that inside the test's 60
/// s, on 2026-09-26. The progenitor's own count is computed in milestone 604's block.
const BUILDS: u64 = 40;

/// Pages of image per child: `ripgrep`'s span on aarch64, `0x40_0000..0x69C_000` in
/// notes/ripgrep-on-nife.md. One executable page and the rest writable, zero-filled.
const IMAGE_PAGES: u64 = 668;

/// One child's region: the image, a four-page stack, its tables, address space and thread, with
/// room over.
const CHILD_PAGES: u64 = 720;

const PAGE: u64 = 4096;
const EHDR: usize = 64;
const PHDR: usize = 56;

/// Scratch pages one build uses: every image page, and on `x86_64` and `riscv64` the timebase
/// page `build_child_space` fills for every child.
const fn scratch_pages_per_build() -> u64 {
    if cfg!(any(target_arch = "x86_64", target_arch = "riscv64")) {
        IMAGE_PAGES + 1
    } else {
        IMAGE_PAGES
    }
}

const _: () = assert!(BUILDS * IMAGE_PAGES > 24_048);
const _: () = assert!(BUILDS * IMAGE_PAGES > SCRATCH_WINDOW_PAGES * 3 / 2);

/// An ELF header and two `PT_LOAD` headers, no file bytes: a one-page code segment holding the entry
/// point, and `IMAGE_PAGES - 1` pages of writable memory after it. Just enough for `elf::Elf::parse`,
/// which is all the loader needs.
fn forge(out: &mut [u8; EHDR + 2 * PHDR]) {
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
    out[56..58].copy_from_slice(&2u16.to_le_bytes()); // phnum
    let segments = [
        (elf::PF_R | elf::PF_X, base, PAGE),
        (elf::PF_R | elf::PF_W, base + PAGE, (IMAGE_PAGES - 1) * PAGE),
    ];
    for (i, (flags, vaddr, memsz)) in segments.into_iter().enumerate() {
        let ph = &mut out[EHDR + i * PHDR..EHDR + (i + 1) * PHDR];
        ph[0..4].copy_from_slice(&1u32.to_le_bytes()); // PT_LOAD
        ph[4..8].copy_from_slice(&flags.to_le_bytes());
        ph[16..24].copy_from_slice(&vaddr.to_le_bytes());
        ph[24..32].copy_from_slice(&vaddr.to_le_bytes());
        ph[40..48].copy_from_slice(&memsz.to_le_bytes());
        ph[48..56].copy_from_slice(&PAGE.to_le_bytes());
    }
}

/// Stack pages to add below the one the kernel maps. `kernel::user::run` gives a process one page,
/// and the loader's call chain overflowed it on the first build (measured, aarch64, 2026-09-26: a
/// data abort at `sp`, 48 bytes below the page). Eight in all, the progenitor's
/// `PROGENITOR_STACK_PAGES`, for the same reason: it builds whole ELFs.
const EXTRA_STACK_PAGES: u64 = 7;

user_mode_runtime::program_entry!(start);

fn start(_arg0: u64, _arg1: u64, _arg2: u64) -> ! {
    // Before anything deep runs. Retyped from the budget ahead of every carve, so they sit under
    // the regions this program splits and destroys and never block a region's pages going back.
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
    build_until_done()
}

#[inline(never)]
fn build_until_done() -> ! {
    let mut bytes = [0u8; EHDR + 2 * PHDR];
    forge(&mut bytes);
    let Ok(image) = elf::Elf::parse(&bytes) else {
        user_mode_runtime::trap()
    };

    let mut built = 0;
    while built < BUILDS {
        let Ok(region) = memory_region_split(MEMORY_REGION, CHILD_PAGES) else {
            break;
        };
        let ok = match build_child_space(
            OWN,
            region,
            &image,
            &ChildEndowment::new(Retention::Nothing),
        ) {
            Ok((child, aspace)) => {
                cap_delete(child.tcb);
                cap_delete(aspace);
                true
            }
            Err(()) => false,
        };
        // The destroy is what gives the scratch pages back: it revokes every mapping of the
        // region's frames, ours included.
        let destroyed = memory_region_destroy(region);
        cap_delete(region);
        if !ok || !destroyed {
            break;
        }
        built += 1;
    }

    send(
        REPORT,
        built,
        built * scratch_pages_per_build(),
        SCRATCH_WINDOW_PAGES,
    );
    exit()
}

user_mode_runtime::panic_handler!();

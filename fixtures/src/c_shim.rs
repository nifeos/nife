//! The Rust shell around a C component (milestone 36 (a foreign-language component, seam first),
//! DECISIONS §31 (the foreign-language seam)): it holds every capability, makes every syscall, and
//! hands the C nothing but a pointer and a length.
//!
//! The C has never seen a capability slot, so it cannot widen the kernel's syscall surface (rule 3,
//! DECISIONS §4 (kernel shape)), and it can corrupt memory inside its grant and nothing else. Its
//! `malloc` is the [`MemoryRegionHeap`] of milestone 27 (Rust `std` on the native ABI) on slot 1,
//! the region this instance was built in, so a C leak exhausts only this instance and the
//! `MemoryRegion::DESTROY` that reaps the corpse reclaims the heap too. See notes/c-seam.md.

//! # What this shell is endowed with
//!
//! | slot | capability | why |
//! |---|---|---|
//! | 0 | report endpoint, WRITE | so the run is legible from the outside |
//! | 1 | this instance's untyped region, WRITE | pays for `malloc`; a `DESTROY` of it is the reap |
//! | 15 | the supervision endpoint | consumed by the kernel at `START` (§26) |
//!
//! plus two mappings the confiner made: the grant at [`c_seam::GRANT_VA`], read/write, and the
//! read-only witness page immediately after it. The C is told about the first and not the second.

//! Name: ratified 2026-08-01 (calef, milestone 61), replacing `cshim`. The `c_` prefix means
//! "written in C" (DECISIONS §31), spelled with the separator the tenet requires; the run-together
//! form was the same defect `cseam` had.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

// What this binary shares with `c_confiner` comes from the `c_seam` crate (rule 7), so this file
// needs no dead-code allow under §38 (a suppression is scoped to an item and carries a reason).

use core::alloc::{GlobalAlloc, Layout};

use memory_region_heap::MemoryRegionHeap;
use user_mode_runtime::send;

/// What the confiner endowed us with, in order.
const REPORT: u64 = 0; // WRITE: say what happened
const HEAP_UT: u64 = 1; // WRITE: the region we were built in, which also pays for malloc

/// The heap's ceiling. Eight pages is one growth step (`memory_region_heap::MIN_GROW_PAGES`) and far more
/// than the component's single scratch allocation needs; small on purpose, so a runaway C `malloc`
/// loop hits a bounded wall inside its own budget.
const HEAP_MAX: u64 = 8 * c_seam::PAGE;

/// The heap behind `malloc` and `free`, which the two C entry points below call. Nothing in this
/// program uses Rust's `alloc`, but `talc`, inside the heap, brings `alloc` into the crate graph,
/// and a graph with `alloc` in it must name a global allocator.
#[global_allocator]
static HEAP: MemoryRegionHeap = MemoryRegionHeap::new();

// The C component. Three functions, one ABI: a pointer, a length, and an integer result. No
// structs, no callbacks, no ownership transfer, nothing that needs a header file to agree on.
unsafe extern "C" {
    fn c_seam_transform(grant: *mut u8, len: usize) -> u32;
    fn c_seam_overrun(grant: *mut u8, len: usize);
    fn c_seam_wild(grant: *mut u8, len: usize);
    fn c_seam_underrun(grant: *mut u8, len: usize);
}

#[unsafe(no_mangle)]
pub extern "C" fn _start(_a0: u64, attempt: u64, _a2: u64) -> ! {
    HEAP.init(HEAP_UT, memory_region_heap::DEFAULT_BASE, HEAP_MAX);

    // Report *before* the call, because two of the three attempts never return from it. This is how
    // the test knows a restarted instance reached its work at all.
    send(REPORT, c_seam::RPT_RAN, attempt, 0);

    let grant = c_seam::GRANT_VA as *mut u8;
    let len = c_seam::PAGE as usize;

    match attempt {
        // SAFETY: none of this is safe, and that is the milestone. `grant` is a page the confiner
        // mapped read/write for us, and `len` is its true length; the C is handed the truth and
        // misbehaves anyway. The kernel is what makes that survivable.
        c_seam::ATTEMPT_OVERRUN => unsafe { c_seam_overrun(grant, len) },
        // SAFETY: as above: the same true grant and true length, handed to the C that deliberately runs wild. The kernel is what makes that survivable.
        c_seam::ATTEMPT_WILD => unsafe { c_seam_wild(grant, len) },
        // SAFETY: as above again, aimed one page below the grant this time. The kernel is what
        // makes that survivable too.
        c_seam::ATTEMPT_UNDERRUN => unsafe { c_seam_underrun(grant, len) },
        // The honest path. The result also lands in the grant, so the checker reads it out of shared
        // memory rather than trusting a number we forwarded.
        _ => {
            // SAFETY: same grant, same length; this call is the well-behaved one.
            unsafe { c_seam_transform(grant, len) };
        }
    }

    // Only the honest attempt gets here. A clean exit is what the supervisor must read as "finished"
    // rather than "crashed" (DECISIONS §26.3), and it is what ends the run.
    user_mode_runtime::exit()
}

// The libc, in full: `malloc` and `free`. The C object also references `memcpy`, `memset` and
// `strlen`, which `compiler_builtins` already defines weakly, and a Rust shim of them would lower to
// a call to itself. notes/c-seam.md ("The libc question, answered by tier") has the measurement.

/// `malloc`'s header size. **This is the one place the C ABI costs us something real.** `free(p)`
/// carries no size, but `GlobalAlloc::dealloc` requires the original `Layout`, so every allocation
/// stores its size in front of the pointer it returns. Sixteen bytes because that is also the
/// alignment `malloc` must guarantee for any C type, so the payload stays aligned for free.
const MALLOC_HDR: usize = 16;

/// The layout `malloc(n)` really allocates: `n` bytes plus the header, 16-byte aligned.
fn malloc_layout(n: usize) -> Option<Layout> {
    Layout::from_size_align(n.checked_add(MALLOC_HDR)?, MALLOC_HDR).ok()
}

/// # Safety
/// The C ABI's contract: the caller must eventually pass the returned pointer to [`free`] exactly
/// once, or never.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malloc(n: usize) -> *mut u8 {
    let Some(layout) = malloc_layout(n) else {
        return core::ptr::null_mut();
    };
    // SAFETY: a non-zero-size layout, and the heap is wired (see `_start`).
    let base = unsafe { HEAP.alloc(layout) };
    if base.is_null() {
        return core::ptr::null_mut();
    }
    #[allow(
        clippy::cast_ptr_alignment,
        reason = "malloc_layout requests MALLOC_HDR (16) alignment, which exceeds align_of::<usize>()"
    )]
    // SAFETY: `base` owns `MALLOC_HDR + n` bytes, 16-aligned, so the header write is in bounds and
    // aligned for a `usize`.
    unsafe {
        base.cast::<usize>().write(n);
        base.add(MALLOC_HDR)
    }
}

/// # Safety
/// `p` must be null or a pointer [`malloc`] returned and that has not been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn free(p: *mut u8) {
    if p.is_null() {
        return;
    }
    #[allow(
        clippy::cast_ptr_alignment,
        reason = "base is the 16-aligned pointer malloc returned, so it is aligned for a usize"
    )]
    // SAFETY: `p` came from `malloc`, so `p - MALLOC_HDR` is the base and holds the size.
    unsafe {
        let base = p.sub(MALLOC_HDR);
        let n = base.cast::<usize>().read();
        if let Some(layout) = malloc_layout(n) {
            HEAP.dealloc(base, layout);
        }
    }
}

// A panic here is a bug in the shell, not in the C. Trap, so the kernel prints the pc and this
// process's supervisor sees a fault rather than a silent hang.
user_mode_runtime::panic_handler!();

//! The progenitor's stack gauge: how deep the first process's stack has ever been.
//!
//! [`paint_page`] paints each stack page as `boot_progenitor` maps it, and [`report_peak`], from
//! the idle loop, prints the settled high-water, which `script/swish-check` fails below
//! [`HEADROOM_FLOOR`]. See notes/stack/progenitor-stack.md.
//!
//! # BUGS
//!
//! - A watermark sees exercised paths only; a spawn `swish-check` never types goes unmeasured.
//! - Name provisional: the module and its two functions.

use core::sync::atomic::{AtomicU64, Ordering};

use crate::user::PROGENITOR_STACK_PAGES;

const PAGE: u64 = page_frames::FRAME_SIZE;

/// The whole stack, in bytes: what the gauge's numbers are out of.
pub(crate) const STACK_BYTES: u64 = PROGENITOR_STACK_PAGES * PAGE;

/// The least headroom a boot may leave, in bytes; below it the gauge says `BELOW` and
/// `script/swish-check` fails.
///
/// Two pages: one is the step this stack has grown by (a page buffer in the install path's biggest
/// frames), the other covers paths the gauge cannot see. notes/stack/progenitor-stack.md, "The
/// gate", has the measurements.
pub(crate) const HEADROOM_FLOOR: u64 = 2 * PAGE;

/// The kernel view (direct-map address) of each stack page, by `k`, the page's distance below
/// the top: `k = 0` is the page at `USER_STACK_VA`, and the highest `k` is the stack's bottom.
/// Zero until [`paint_page`] records it.
static PAGES: [AtomicU64; PROGENITOR_STACK_PAGES as usize] =
    [const { AtomicU64::new(0) }; PROGENITOR_STACK_PAGES as usize];

/// Paint one of the progenitor's fresh stack pages and remember where it is.
///
/// `k` is the page's distance below the top and `page` its kernel view, unused yet. Called by
/// `boot_progenitor` for each page `map_new` returns, before the progenitor exists.
pub(crate) fn paint_page(k: u64, page: &mut [u8]) {
    let bottom = page.as_mut_ptr() as u64;
    // SAFETY: `page` is a whole, page-aligned, writable frame the caller just mapped and nothing
    // has run on yet, which is `paint`'s contract. The `&mut` is what proves nobody else holds it.
    unsafe { crate::stack::paint(bottom, bottom + PAGE) };
    PAGES[k as usize].store(bottom, Ordering::Release);
}

/// Return the most bytes the progenitor's stack has ever used, or `None` before it is painted.
///
/// Scans from the lowest page and stops at the first used one, so it costs the headroom, not the
/// stack.
pub(crate) fn high_water() -> Option<u64> {
    for k in (0..PROGENITOR_STACK_PAGES).rev() {
        let bottom = PAGES[k as usize].load(Ordering::Acquire);
        if bottom == 0 {
            return None;
        }
        // SAFETY: `bottom` is a page `paint_page` painted, recorded after the paint. The progenitor
        // never exits, so its address space, and this frame, live for the whole boot; a racing
        // write from the progenitor on another core can only make the answer deeper.
        let used = unsafe { crate::stack::high_water(bottom, bottom + PAGE) };
        if used != 0 {
            return Some(k * PAGE + used);
        }
    }
    Some(0)
}

static LAST_PASS: AtomicU64 = AtomicU64::new(0);
static STABLE_PASSES: AtomicU64 = AtomicU64::new(0);
static REPORTED: AtomicU64 = AtomicU64::new(0);

/// Idle passes the mark must hold still before [`report_peak`] prints it: the capability-slot
/// gauge's window, because the progenitor blocks on IPC mid-build and each pause is an idle pass.
const STABLE_PASSES_NEEDED: u64 = 16;

/// Print the progenitor's stack high-water once it has stopped moving, once per new peak.
///
/// Called from the scheduler's idle loop.
pub(crate) fn report_peak() {
    let Some(used) = high_water() else {
        return;
    };
    if used == 0 {
        return;
    }
    if LAST_PASS.swap(used, Ordering::Relaxed) != used {
        STABLE_PASSES.store(0, Ordering::Relaxed);
        return;
    }
    if STABLE_PASSES.fetch_add(1, Ordering::Relaxed) < STABLE_PASSES_NEEDED {
        return;
    }
    if REPORTED.fetch_max(used, Ordering::Relaxed) >= used {
        return;
    }
    announce(used);
}

/// The sentence `script/swish-check` reads. Its prefix is `xtask`'s `PROGENITOR_STACK_GAUGE`.
fn announce(used: u64) {
    let spare = STACK_BYTES.saturating_sub(used);
    if spare < HEADROOM_FLOOR {
        crate::println!(
            "  progenitor stack: {used} of {STACK_BYTES} bytes at peak, {spare} spare, BELOW the \
             {HEADROOM_FLOOR}-byte floor in kernel/src/progenitor_stack.rs"
        );
        return;
    }
    crate::println!("  progenitor stack: {used} of {STACK_BYTES} bytes at peak, {spare} spare");
}

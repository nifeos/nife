//! The client windows: the pool of file
//! channels the service keeps side by side, one per live client, and the one `login` keeps for its
//! durable sessions.
//!
//! See notes/a-frame-per-filesystem-client-channel.md.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// How many client channels this service keeps side by side (provisional),
/// straight from the contract so this wiring and the server agree. Window 0 is [`FILE_SHARED`], the
/// unbadged default every legacy single-client path maps; windows 1.. are the pool [`claim_window`]
/// hands out, one per additional live client, each its own frame and its own badge.
pub(super) const CLIENT_WINDOWS: usize = filesystem_contract::fs::CLIENT_WINDOWS;

/// The physical base of each window's [`FILE_PAGES`]-frame run. `WINDOWS[0]` is [`FILE_SHARED`];
/// the rest are allocated once when the service is wired and mapped into the FS server. Plain
/// atomics for [`WIRED`]'s reason: the only writer is the boot/test thread.
pub(super) static WINDOWS: [core::sync::atomic::AtomicU64; CLIENT_WINDOWS] =
    [const { core::sync::atomic::AtomicU64::new(0) }; CLIENT_WINDOWS];

/// Which windows are handed out. Bit `w` set means window `w` is claimed; bit 0 is set from wiring,
/// because window 0 is the default rather than a claimable channel. A `u64` bitmap because
/// [`CLIENT_WINDOWS`] is far under 64; [`claim_window`] and [`release_window`] are the only writers.
pub(super) static WINDOW_TAKEN: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(0);

/// Claim a free client-channel window: returns its index (always `>= 1`, since
/// window 0 is the default) and the physical base of its frame run, or `None` when all
/// [`CLIENT_WINDOWS`] are in use, which is this service's live-client ceiling. The badge a client is
/// granted IS its window index, so the FS server reads the right frame for its request.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub(super) fn claim_window() -> Option<(u64, u64)> {
    use core::sync::atomic::Ordering;
    // Never the last: that one is `login`'s, for its durable sessions ([`durable_window`]).
    let pool = login_contract::DURABLE_WINDOW as usize;
    for (w, window) in WINDOWS.iter().enumerate().take(pool).skip(1) {
        let bit = 1u64 << w;
        if WINDOW_TAKEN.fetch_or(bit, Ordering::Relaxed) & bit == 0 {
            return Some((w as u64, window.load(Ordering::Relaxed)));
        }
    }
    None
}

/// The window `login` keeps for its durable sessions:
/// `login_contract::DURABLE_WINDOW`, claimed once and handed to every `login` this boot starts, the
/// way the progenitor reserves it. The physical base of its frame run, or `None` when the service is
/// not wired.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn durable_window() -> Option<u64> {
    use core::sync::atomic::Ordering;
    // [`claim_window`] never hands this one out, so it is always free for `login`.
    let w = login_contract::DURABLE_WINDOW as usize;
    WINDOW_TAKEN.fetch_or(1u64 << w, Ordering::Relaxed);
    let phys = WINDOWS[w].load(Ordering::Relaxed);
    (phys != 0).then_some(phys)
}

/// [`release_window`] for a test that bound a grant and holds no program.
#[cfg(any(test, feature = "system_tests"))]
pub fn release_window_after_test(w: u64) {
    release_window(w);
}

/// Return a window to the pool, the take-back half of [`claim_window`]. The
/// frame is not freed (it stays mapped in the FS server for the life of the service); it is zeroed
/// so no client's staging survives into the next client that claims the slot, which is the same
/// "no stale RAM across a share" rule [`file_channel`] applies at allocation.
///
/// `allow(dead_code)` unconditionally, a marked exception: its production caller, the
/// progenitor's reap, is not built yet, and the take-back stays beside its claim half for it.
#[allow(dead_code)]
pub(super) fn release_window(w: u64) {
    use core::sync::atomic::Ordering;
    let phys = WINDOWS[w as usize].load(Ordering::Relaxed);
    if phys != 0 {
        // SAFETY: a frame this module allocated and still owns, via the direct map; FILE_PAGES
        // frames of FRAME_SIZE bytes, reachable and mapped only into the FS server and whatever
        // client last held this window (now gone).
        unsafe {
            core::ptr::write_bytes(
                mmu::phys_to_virt(phys) as *mut u8,
                0,
                FILE_PAGES * FRAME_SIZE as usize,
            );
        }
    }
    WINDOW_TAKEN.fetch_and(!(1u64 << w), Ordering::Relaxed);
}

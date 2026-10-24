//! The file and block channels, and where every party maps them: the virtual addresses
//! each program expects, the channel sizes the contract fixes, and the frames behind them.
//!
//! See notes/fs-server/kernel-wiring.md.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

// The VAs each process expects its mappings at. Each MUST match that program's source.
pub(super) const DMA_VA: u64 = address_space_map::pair_page(0x0000_0000_0090_0000); // block server DMA region, 1 + BLK_PAGES pages (crates/virtio)
pub(super) const BLK_PAGE_FS: u64 = 0x5000_0000; // FS server's block region (redoxfs_server.rs BLK_PAGE)
// FS server's file region (redoxfs_server.rs FILE_PAGE): BLK_PAGES pages above BLK_PAGE_FS, so the
// block channel cannot walk into it.
pub(super) const FILE_PAGE_FS: u64 = BLK_PAGE_FS + (BLK_PAGES as u64) * FRAME_SIZE;
pub(super) const FILE_VA_CLIENT: u64 = 0x0000_0000_0060_0000; // client's file page (fs_test_client/channel.rs FILE_VA)

/// A std program's half of the same agreement (notes/abi.md section 4, notes/std.md): the slot the PAL
/// looks for the FS-service endpoint in, and the VA it expects the shared file page at. Read from
/// `std_runtime_contract`, which the std PAL has generated into it, so the two cannot drift. A std
/// program's slot layout differs from the hand-written client's because std already owes slots 0
/// and 1 to its heap and its stdout, and 2 and 3 to `std::net`.
pub(super) const FS_DIR_SLOT: u64 = std_runtime_contract::FS_DIR_SLOT;
pub(super) const FS_PAGE_STD: u64 = std_runtime_contract::FS_PAGE;

/// A fresh, zeroed frame, returned by physical address. Zeroed so no stale RAM is ever visible
/// across a share, and (for the DMA frame) so the device never reads a stale descriptor.
pub(super) fn page_frame() -> u64 {
    crate::memory::alloc_zeroed()
        .expect("no frame for the fs service")
        .addr()
}

/// How many pages the file channel spans, straight from the contract, so this wiring cannot
/// disagree with the two programs that speak it.
pub(super) const FILE_PAGES: usize = filesystem_contract::fs::TRANSFER_PAGES;

/// How many pages the blk channel spans, straight from the contract, as [`FILE_PAGES`] is.
pub(super) const BLK_PAGES: usize = filesystem_contract::blk::TRANSFER_BLOCKS;

/// Every client window at once: [`CLIENT_WINDOWS`] channels of [`FILE_PAGES`] frames, one
/// contiguous zeroed run, returned by its base. Window 0 is the first [`FILE_PAGES`] of it, so a
/// caller that takes the base as "the channel" finds window 0 there.
pub(super) fn file_pool() -> u64 {
    crate::memory::alloc_contiguous_zeroed(CLIENT_WINDOWS * FILE_PAGES)
        .expect("no contiguous run for the fs service's client windows")
        .addr()
}

/// The pool's length in pages: what the progenitor's run capability over it names.
pub const FILE_POOL_PAGES: u64 = (CLIENT_WINDOWS * FILE_PAGES) as u64;

/// The file channel: [`FILE_PAGES`] fresh, zeroed, physically contiguous frames, returned by the
/// base physical address. A contiguous run keeps the channel one `u64` on both sides of the
/// agreement, and zeroing keeps stale RAM from crossing a share.
pub(super) fn file_channel() -> u64 {
    crate::memory::alloc_contiguous_zeroed(FILE_PAGES)
        .expect("no contiguous run for the fs service's file channel")
        .addr()
}

/// Write the file channel's mappings into `maps`, `va` upward against `phys` upward, and answer how
/// many entries that took. One call per party that shares the channel, so the "map every page of
/// it" rule lives in one place rather than in each of them.
///
/// `pages` is how much of the channel this party maps: a client may map less than [`FILE_PAGES`]
/// (`filesystem_contract::fs::TRANSFER_PAGES` says so), and the FS server maps all of it. Visible
/// to the rest of `user`, so every driver's spawn shares this one spelling of the rule.
pub(in crate::user) fn map_channel(
    maps: &mut [Mapping],
    va: u64,
    phys: u64,
    pages: usize,
) -> usize {
    for (i, m) in maps.iter_mut().take(pages).enumerate() {
        *m = Mapping {
            va: va + i as u64 * FRAME_SIZE,
            phys: phys + i as u64 * FRAME_SIZE,
            flags: Flags::user_data(),
        };
    }
    pages
}

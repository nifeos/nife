//! Wiring for the disk tree server (`components/src/disk_tree.rs`): the whole NVMe namespace as one
//! read-only file, the installed system's second mounted tree.
//!
//! It holds the NVMe request endpoint badged with
//! [`::non_volatile_memory_express_contract::DISK_TREE_WINDOW`], its own request and report endpoints, that
//! window read-only, and the client windows. Window 0 is the file service's own window 0 frame. See
//! notes/two-trees.md, "The disk tree's server".
//!
//! Name: provisional (lane `lane/ioping-direct-io`, 2026-10-18 UTC), with the program's.

use super::*;
use crate::cap::{Rights, rendezvous_cap};
use crate::sched::RendezvousId;

/// The block channel's address on every `blk` client. Must match `components/src/disk_tree.rs`.
const BLK_PAGE: u64 = address_space_map::service_window(0x5000_0000);
/// The client windows, directly above the block channel. Must match `components/src/disk_tree.rs`.
const FILE_PAGE: u64 = BLK_PAGE + filesystem_contract::blk::TRANSFER_MAX as u64;

const BLK_PAGES: usize = filesystem_contract::blk::TRANSFER_BLOCKS;
/// Every client window, one run, as the filesystem server's pool is.
pub(crate) const POOL_PAGES: usize =
    filesystem_contract::fs::CLIENT_WINDOWS * filesystem_contract::fs::TRANSFER_PAGES;

/// A running disk tree, from the outside.
#[derive(Clone, Copy)]
pub(crate) struct DiskTree {
    /// The endpoint its clients `CALL`. Window 0 is the unbadged one.
    pub request: RendezvousId,
    /// The physical base of the client-window pool, [`POOL_PAGES`] pages.
    pub pool: u64,
}

/// Start the disk tree, when this machine has an NVMe disk. `fs_window_zero` is the physical
/// base of the file service's window 0. `None` without a disk, or when the archive lacks either
/// program, or when the server could not size the disk: each costs the second tree, not the boot.
pub(crate) fn start(fs_window_zero: u64) -> Option<DiskTree> {
    let image = program("disk_tree")?;
    let nvme = program("non_volatile_memory_express")?;
    let disk = non_volatile_memory_express_service::ensure(nvme)?;
    // Taken only if this call is the one that wired the server; the filesystem server's wiring
    // otherwise took it already.
    disk.wait_for_ready();

    let window = ::non_volatile_memory_express_contract::DISK_TREE_WINDOW;
    let blk = disk.client_capability(window);
    let staged = disk.window_phys(window);
    let pool = crate::memory::alloc_contiguous_zeroed(POOL_PAGES)?.addr();
    let request = crate::sched::create_rendezvous();
    let report = crate::sched::create_rendezvous();

    crate::sched::spawn(move || {
        let mut maps = [Mapping {
            va: 0,
            phys: 0,
            flags: Flags::user_data(),
        }; BLK_PAGES + POOL_PAGES];
        // Read-only: the controller fills these by DMA, and this server only copies out of them.
        for (i, m) in maps.iter_mut().take(BLK_PAGES).enumerate() {
            *m = Mapping {
                va: BLK_PAGE + i as u64 * page_frames::FRAME_SIZE,
                phys: staged + i as u64 * page_frames::FRAME_SIZE,
                flags: Flags::user_rodata(),
            };
        }
        super::fs_service::map_channel(&mut maps[BLK_PAGES..], FILE_PAGE, pool, POOL_PAGES);
        let window = filesystem_contract::fs::TRANSFER_PAGES;
        super::fs_service::map_channel(
            &mut maps[BLK_PAGES..BLK_PAGES + window],
            FILE_PAGE,
            fs_window_zero,
            window,
        );
        run(
            image,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[
                    blk,
                    rendezvous_cap(request, Rights::READ),
                    rendezvous_cap(report, Rights::WRITE),
                ],
                maps: &maps,
            },
        )
    })
    .expect("could not spawn the disk tree");

    let answer = crate::sched::ipc_receive(report);
    if answer[0] != filesystem_contract::READY {
        crate::println!(
            "  disk tree   : not started: the NVMe disk did not answer SIZE ({:#x}).",
            answer[1]
        );
        return None;
    }
    Some(DiskTree { request, pool })
}

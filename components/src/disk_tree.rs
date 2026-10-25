//! **The disk tree server: a whole NVMe namespace as one read-only file, with no cache**
//! (milestone 834 (ioping on nife and Linux)). The namespace, the handle table and the block
//! arithmetic are `crates/disk_tree`'s; this is the two endpoints and the pages around them, as
//! `kernel/src/user/disk_tree_service.rs` places them (the constants below).
//!
//! ```text
//!   shell, caretakers ──fs (badge = window)──► disk_tree ──blk (badge DISK_TREE_WINDOW)──► NVMe server
//! ```
//!
//! Name: provisional (lane `lane/ioping-direct-io`, 2026-10-18 UTC), with `crates/disk_tree`.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68 (code-quality gates)'s ratchet tracks
// (DECISIONS §107 (`missing_docs` moves to `workspace.lints.rust`)): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use disk_tree::{Answer, Tree};
use filesystem_contract::{blk, fs, reply_err};
use user_mode_runtime::{call, receive_request, reply, send};

/// Slot 0: the NVMe server's request endpoint, `WRITE`, badged
/// `non_volatile_memory_express::DISK_TREE_WINDOW`, so this server stages in a window of its own
/// beside the filesystem server and the NVMe server refuses it a write.
const BLK: u64 = 0;
/// Slot 1: the request endpoint, `READ`. Clients `CALL` it with `filesystem_contract::fs`.
const CLIENTS: u64 = 1;
/// Slot 2: a report endpoint, `WRITE`: [`filesystem_contract::READY`] and the disk's size once a
/// `SIZE` has come back.
const REPORT: u64 = 2;

/// The NVMe window, the block channel's address on every program that speaks `blk`. Read-only
/// here; the controller writes it by DMA.
const BLK_PAGE: u64 = address_space_map::service_window(0x5000_0000);
/// The client windows, directly above the block channel, as `redoxfs_server` lays them out:
/// `fs::CLIENT_WINDOWS` windows of `fs::TRANSFER_MAX` bytes in one run, window `badge` per request
/// (milestone 599 (a frame per filesystem client channel)'s pattern).
const FILE_PAGE: u64 = BLK_PAGE + blk::TRANSFER_MAX as u64;

const EIO: i32 = 5;
const EINVAL: i32 = 22;

user_mode_runtime::program_entry!(start);

/// Name: provisional, minted 2026-10-18 (UTC) by lane/ioping-direct-io.
fn start(_: u64, _: u64, _: u64) -> ! {
    let size = call(BLK, filesystem_contract::req(blk::SIZE), 0).0 as i64;
    if size <= 0 {
        send(REPORT, 0xDEAD_0001, size as u64, 0);
        user_mode_runtime::exit();
    }
    send(REPORT, filesystem_contract::READY, size as u64, 0);

    let mut tree = Tree::new(size as u64);
    loop {
        let req = receive_request(CLIENTS);
        let Some(to) = req.delivered.into_reply() else {
            continue;
        };
        if req.badge as usize >= fs::CLIENT_WINDOWS {
            reply(to, reply_err(EINVAL) as u64, 0);
            continue;
        }
        // SAFETY: the wiring maps `fs::CLIENT_WINDOWS` windows of `fs::TRANSFER_MAX` bytes at
        // FILE_PAGE, and the client whose badge names this one is parked inside its CALL until
        // the reply below.
        let page = unsafe {
            core::slice::from_raw_parts_mut(
                (FILE_PAGE + req.badge * fs::TRANSFER_MAX as u64) as *mut u8,
                fs::TRANSFER_MAX,
            )
        };
        let (r0, r1) = match tree.handle(req.w0, req.w1, page) {
            Answer::Reply(r0, r1) => (r0, r1),
            Answer::Read(read) => {
                let got = call(BLK, blk::req(blk::READ, read.blocks), read.block).0 as i64;
                if got < 0 {
                    (reply_err(EIO), 0)
                } else {
                    // SAFETY: the NVMe window is `blk::TRANSFER_MAX` bytes at BLK_PAGE and
                    // `skip + len` never passes `read.blocks` blocks (`crates/disk_tree`'s tests).
                    let staged = unsafe {
                        core::slice::from_raw_parts(BLK_PAGE as *const u8, blk::TRANSFER_MAX)
                    };
                    page[..read.len].copy_from_slice(&staged[read.skip..read.skip + read.len]);
                    (read.len as i64, 0)
                }
            }
        };
        reply(to, r0 as u64, r1);
    }
}

user_mode_runtime::panic_handler!();

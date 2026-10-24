//! radon's SD/MMC block server: `filesystem_contract::blk` over the JH7110's `DesignWare` Mobile
//! Storage Host Controller, from EL0 (milestone 53 (the board's own peripherals: network and
//! storage on real silicon); notes/designware-mobile-storage.md). The driver is the crate of the
//! same name; this file is its register window and serve loop. It holds only what
//! `kernel/src/user/designware_mobile_storage_service.rs` gives it: the controller's register page
//! at `serve::REGISTER_VA` (the kernel ungates the shared clock generator, §86 (whether an NVMe
//! driver can leave the kernel)), the transfer region at `serve::TRANSFER_VA`, the request (slot
//! 0) and readiness (slot 1) endpoints, and the card's window, clock, width and FIFO depth. No DMA,
//! but that page holds `BMOD` and `DBADDR` and the JH7110 has no IOMMU, so it is as confined as its
//! own code, the position of milestone 261 (the NVMe driver leaves the kernel).

//! Name: ratified 2026-10-17 (calef, a program-naming session). Kept, paired with its crate as
//! `non_volatile_memory_express` is. He considered `designware_sd_storage` and `sd_and_emmc_storage`,
//! then answered "`designware_mobile_storage`". Refused those two (against the crate's name).

//! # BUGS
//!
//! - **Never run on silicon.** `PROVEN_ON_SILICON` in the kernel service keeps the booted system
//!   from starting it until a bench boot has.
//! - **One request at a time**, polled, one `CMD18` or `CMD25` per request.
//! - **`FLUSH` asks the card for its status** and counts; an SD card at default speed has no cache
//!   this driver turned on, so there is nothing else to flush.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68 (code-quality gates: one
// lint policy)'s ratchet tracks (DECISIONS §107 (`missing_docs` moves to `workspace.lints.rust`, opt-out rather than opt-in)):
// each `[[bin]]` is its own crate root with one `_start`.
#![allow(missing_docs)]
#![no_main]

use designware_mobile_storage::host::{Host, Registers};
use designware_mobile_storage::sd::{self, Card};
use designware_mobile_storage::serve::{Handoff, REGISTER_VA, TRANSFER_VA};
use designware_mobile_storage::{command as c, regs};
use filesystem_contract::blk;
use user_mode_runtime::{exit, receive_request, reply, send};

/// The request endpoint.
const REQ: u64 = 0;
/// The readiness endpoint, sent on once.
const READY: u64 = 1;

/// Readiness words that are not [`filesystem_contract::READY`]: which step failed.
const STEP_BAD_HANDOFF: u64 = 1;
const STEP_CONTROLLER: u64 = 2;
const STEP_IDENTIFY: u64 = 3;
const STEP_WINDOW: u64 = 4;
const STEP_FIRST_READ: u64 = 5;

/// `EIO`, `EINVAL`, `EROFS`.
const EIO: i32 = 5;
const EINVAL: i32 = 22;
const EROFS: i32 = 30;

/// The controller's first page, as the kernel mapped it.
struct Page;

impl Registers for Page {
    fn read(&mut self, offset: u32) -> u32 {
        debug_assert!(offset < regs::WINDOW_USED);
        // SAFETY: the spawner maps one device-typed page of the controller at REGISTER_VA before
        // `_start` runs, and every offset the crate passes is a word-aligned register in it.
        unsafe { core::ptr::read_volatile((REGISTER_VA + u64::from(offset)) as *const u32) }
    }

    fn write(&mut self, offset: u32, value: u32) {
        debug_assert!(offset < regs::WINDOW_USED);
        // SAFETY: as `read`.
        unsafe {
            core::ptr::write_volatile((REGISTER_VA + u64::from(offset)) as *mut u32, value);
        }
    }

    fn now_us(&mut self) -> u64 {
        user_mode_runtime::monotonic_nanos() / 1000
    }
}

fn transfer() -> &'static mut [u8] {
    // SAFETY: the spawner maps `blk::TRANSFER_BLOCKS` pages read/write at TRANSFER_VA before
    // `_start` runs; this process is single-threaded, so this is the only reference at a time.
    unsafe { core::slice::from_raw_parts_mut(TRANSFER_VA as *mut u8, blk::TRANSFER_MAX) }
}

user_mode_runtime::program_entry!(start);

fn start(arg0: u64, arg1: u64, arg2: u64) -> ! {
    let Some(h) = Handoff::unpack([arg0, arg1, arg2]) else {
        send(READY, STEP_BAD_HANDOFF, arg0, 0);
        exit();
    };
    let Ok((mut host, _)) = Host::new(Page, h.ciu_hz, Some(u32::from(h.fifo_depth))) else {
        send(READY, STEP_CONTROLLER, 0, 0);
        exit();
    };
    let Ok(card) = sd::identify(&mut host, h.bus_width) else {
        send(READY, STEP_IDENTIFY, 0, 0);
        exit();
    };
    if h.window.first.saturating_add(h.window.sectors) > card.blocks {
        send(READY, STEP_WINDOW, card.blocks, 0);
        exit();
    }
    // Read the window's first block before saying ready, `non_volatile_memory_express`'s
    // discipline: ready means a client that asks will be answered.
    let first = transfer_blocks(&mut host, &card, &h, 0, 1, false);
    let report = if first >= 0 {
        filesystem_contract::READY
    } else {
        STEP_FIRST_READ
    };
    send(
        READY,
        report,
        h.window.sectors * sd::BLOCK as u64,
        card.blocks,
    );
    serve(host, card, h)
}

/// Move `count` blocks from window block `block` through the transfer region. The `blk` answer:
/// the block count, or a negated errno.
fn transfer_blocks(
    host: &mut Host<Page>,
    card: &Card,
    h: &Handoff,
    block: u64,
    count: usize,
    write: bool,
) -> i64 {
    let Some(lba) = h.window.translate(block, count as u64) else {
        return filesystem_contract::reply_err(EINVAL);
    };
    if write && !h.window.writable {
        return filesystem_contract::reply_err(EROFS);
    }
    let buf = &mut transfer()[..count * blk::BLOCK_SIZE];
    let r = if write {
        sd::write_blocks(host, card, lba, buf)
    } else {
        sd::read_blocks(host, card, lba, buf)
    };
    match r {
        Ok(()) => count as i64,
        Err(_) => filesystem_contract::reply_err(EIO),
    }
}

/// The serve loop: one endpoint, one wait point, forever.
fn serve(mut host: Host<Page>, card: Card, h: Handoff) -> ! {
    let mut flushes = 0i64;
    loop {
        let req = receive_request(REQ);
        let (w0, block) = (req.w0, req.w1);
        let Some(reply_cap) = req.delivered.into_reply() else {
            continue;
        };
        // Clamped, as every block server here clamps: the packing could spell more.
        let count = blk::req_blocks(w0).min(blk::TRANSFER_BLOCKS);
        let r0 = match filesystem_contract::operation(w0) {
            blk::READ => transfer_blocks(&mut host, &card, &h, block, count, false),
            blk::WRITE => transfer_blocks(&mut host, &card, &h, block, count, true),
            blk::SIZE => (h.window.sectors * sd::BLOCK as u64) as i64,
            blk::FLUSH => {
                let status = host.command(&c::Command::new(
                    c::SEND_STATUS,
                    u32::from(card.rca) << 16,
                    c::Response::R1,
                ));
                match status {
                    Ok(r) if r[0] & designware_mobile_storage::card::R1_ERRORS == 0 => {
                        flushes += 1;
                        flushes
                    }
                    _ => filesystem_contract::reply_err(EIO),
                }
            }
            _ => filesystem_contract::reply_err(EINVAL),
        };
        reply(reply_cap, r0 as u64, 0);
    }
}

user_mode_runtime::panic_handler!();

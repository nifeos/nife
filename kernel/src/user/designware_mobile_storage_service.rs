//! **radon's SD/MMC block server's wiring** (milestone 53 (the board's own peripherals: network
//! and storage on real silicon)), in milestone 261 (the NVMe driver leaves the kernel)'s shape.
//!
//! The kernel keeps the admin plane: it ungates the controller's clocks and releases its reset in
//! the shared SYS clock generator, then hands a process one device-typed page of the controller,
//! the transfer region it will share with its client, two endpoints, and the window of the card it
//! may serve. Everything else, the card included, is the process's to bring up:
//! `components/src/designware_mobile_storage.rs` has what it holds and what it is refused.
//!
//! **The booted system does not start it yet**, although it may ([`PROVEN_ON_SILICON`]). Its one
//! caller is the `storage_bench` boot, which starts it over a window it chose from the card's own
//! partition table and reads and writes through it the way the FS server would. What the booted
//! system's block server serves on radon is a USB drive (§260 (radon's block server serves a USB
//! drive)), not this card.

use designware_mobile_storage::jh7110::Slot;
use designware_mobile_storage::serve::{Handoff, REGISTER_VA, TRANSFER_VA, Window};
use filesystem_contract::blk;

use super::*;
use crate::cap::{Rights, rendezvous_cap};
use crate::sched::RendezvousId;

/// **Whether the booted system may start this server.** True: a `storage_bench` boot on radon
/// read and wrote through it on 2026-10-22 (UTC), `verdict READ-OK 10960 KiB/s, WRITE-VERIFIED
/// sectors 2040..2048 (were zero), restored, EL0 block server served`
/// (`bench/radon-2026-10-22/53-storage-write.log`, notes/designware-mobile-storage.md's runbook).
pub(crate) const PROVEN_ON_SILICON: bool = true;

/// A running server: where a client sends, where readiness arrives, and the physical base of the
/// transfer region a client maps.
pub(crate) struct Wiring {
    /// Requests, `filesystem_contract::blk`.
    pub request: RendezvousId,
    /// One message when the server is up, or which step stopped it.
    pub ready: RendezvousId,
    /// The transfer region, [`blk::TRANSFER_BLOCKS`] pages.
    pub transfer_phys: u64,
}

/// Why nothing was started.
#[derive(Debug)]
pub(crate) enum NotStarted {
    /// No SYS clock window was recorded: this is not a JH7110.
    NoClockWindow,
    /// A clock did not read back enabled, or the reset did not release.
    Clocks,
    /// No contiguous pages for the transfer region.
    NoMemory,
    /// The window or the slot cannot be packed into the spawn words.
    BadHandoff,
}

/// **Start a block server on `slot`, serving `window`.**
///
/// # Errors
///
/// [`NotStarted`] says which step refused; nothing is spawned then.
pub(crate) fn start(
    image: &'static [u8],
    slot: &Slot,
    window: Window,
) -> Result<Wiring, NotStarted> {
    let handoff = Handoff {
        window,
        ciu_hz: slot.ciu_hz,
        bus_width: slot.bus_width,
        fifo_depth: u8::try_from(slot.fifo_depth.unwrap_or(32)).unwrap_or(0),
    };
    let words = handoff.pack();
    if Handoff::unpack(words) != Some(handoff) {
        return Err(NotStarted::BadHandoff);
    }
    let sys = crate::memory::jh7110_sys_window().ok_or(NotStarted::NoClockWindow)?;
    // SAFETY: `memory::jh7110_sys_window` answers only on a JH7110, and `mmu::init` step 6c mapped
    // exactly this window device-typed. The plan and the domain are the same crate's, so every
    // identifier is in range; every step is idempotent on a controller already up.
    let report = unsafe {
        crate::drivers::jh7110_clock_and_reset::bring_up(
            crate::arch::mmu::phys_to_virt(sys.base) as usize,
            &jh7110_clock_and_reset::SYS,
            jh7110_clock_and_reset::SDIO_BRING_UP[usize::from(slot.index) & 1],
        )
    };
    if !report.has_clocks_running() || !report.released {
        return Err(NotStarted::Clocks);
    }
    let transfer_phys = crate::memory::alloc_contiguous_zeroed(blk::TRANSFER_BLOCKS)
        .ok_or(NotStarted::NoMemory)?
        .addr();

    let ready = crate::sched::create_rendezvous();
    let request = crate::sched::create_rendezvous();
    let base = slot.base;
    crate::sched::spawn(move || {
        let mut maps = [Mapping {
            va: 0,
            phys: 0,
            flags: Flags::user_data(),
        }; blk::TRANSFER_BLOCKS + 1];
        super::fs_service::map_channel(
            &mut maps[..blk::TRANSFER_BLOCKS],
            TRANSFER_VA,
            transfer_phys,
            blk::TRANSFER_BLOCKS,
        );
        // One page of the controller: every register the driver touches and the data FIFO.
        maps[blk::TRANSFER_BLOCKS] = Mapping {
            va: REGISTER_VA,
            phys: base,
            flags: Flags::user_device(),
        };
        run(
            image,
            Spawn {
                arg0: words[0], // see `designware_mobile_storage::serve::Handoff`
                arg1: words[1], // the window's first sector
                arg2: words[2], // the window's length in sectors
                grants: &[
                    rendezvous_cap(request, Rights::READ), // slot 0: RECEIVE blk requests
                    rendezvous_cap(ready, Rights::WRITE),  // slot 1: signal readiness once
                ],
                maps: &maps,
            },
        )
    })
    .expect("could not spawn the SD/MMC block server");

    Ok(Wiring {
        request,
        ready,
        transfer_phys,
    })
}

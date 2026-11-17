//! radon's SD/MMC block server's wiring. The clock and reset server ungates the controller's
//! clocks and releases its reset, for this controller's badge alone, and the kernel then hands a process one device-typed page of the controller, the transfer region,
//! two endpoints and the window of the card it may serve;
//! `components/src/designware_mobile_storage.rs` has what it holds and is refused. See
//! notes/designware-mobile-storage.md.
//!
//! Its one caller is the `storage_bench` boot. On radon the booted system's block server serves a
//! USB drive (§260 (radon's block server serves a USB drive)), not this card.

use designware_mobile_storage::jh7110::Slot;
use driver_process_contract::designware_mobile_storage::{
    Handoff, REGISTER_VIRTUAL_ADDRESS, TRANSFER_VIRTUAL_ADDRESS, Window,
};
use filesystem_contract::blk;

use super::*;
use crate::capability_object::{Rights, rendezvous_capability};
use crate::sched::RendezvousId;

/// Whether the booted system may start this server. True: a `storage_bench` boot on radon
/// read and wrote through it on 2026-10-22 (UTC), `verdict READ-OK 10960 KiB/s, WRITE-VERIFIED
/// sectors 2040..2048 (were zero), restored, EL0 block server served`
/// (`bench/radon-2026-10-22/53-storage-write.log`, notes/designware-mobile-storage.md's runbook).
pub(crate) const PROVEN_ON_SILICON: bool = true;

/// Extra stack pages for the server, below the single page `run` maps. The write path's programmed
/// transfer runs deeper than one page: the 2026-11-17 (UTC) write bench's server faulted with `sp`
/// 0x20 bytes into the unmapped page below the stack's top page, while the read-only build, which
/// never runs that path, stayed inside it. Four leaves 16 KiB of headroom.
const EXTRA_STACK_PAGES: usize = 4;

/// The mappings for [`EXTRA_STACK_PAGES`] pages, the first directly below `USER_STACK_VA`.
fn stack_maps(frames: [u64; EXTRA_STACK_PAGES]) -> [Mapping; EXTRA_STACK_PAGES] {
    let mut maps = [Mapping {
        va: 0,
        phys: 0,
        flags: Flags::user_data(),
    }; EXTRA_STACK_PAGES];
    for (k, m) in maps.iter_mut().enumerate() {
        m.va = USER_STACK_VA - (k as u64 + 1) * FRAME_SIZE;
        m.phys = frames[k];
    }
    maps
}

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
    /// No clock and reset server runs: this is not a JH7110, or the archive has no server.
    NoClockServer,
    /// A clock did not read back enabled, or the reset did not release.
    Clocks,
    /// No contiguous pages for the transfer region.
    NoMemory,
    /// The window or the slot cannot be packed into the spawn words.
    BadHandoff,
}

/// Start a block server on `slot`, serving `window`.
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
        card_interface_unit_hertz: slot.card_interface_unit_hertz,
        bus_width: slot.bus_width,
        queue_depth: u8::try_from(slot.queue_depth_words.unwrap_or(32)).unwrap_or(0),
    };
    let words = handoff.pack();
    if Handoff::unpack(words) != Some(handoff) {
        return Err(NotStarted::BadHandoff);
    }
    let brought = super::jh7110_clock_and_reset_service::bring_up(slot.registers_physical_address)
        .ok_or(NotStarted::NoClockServer)?;
    if !brought.is_up() {
        return Err(NotStarted::Clocks);
    }
    let transfer_phys = crate::memory::alloc_contiguous_zeroed(blk::TRANSFER_BLOCKS)
        .ok_or(NotStarted::NoMemory)?
        .addr();

    let mut frames = [0u64; EXTRA_STACK_PAGES];
    for f in &mut frames {
        // Zeroed, as every stack page is: nothing of a previous owner sits in the server's frames.
        *f = crate::memory::alloc_zeroed()
            .ok_or(NotStarted::NoMemory)?
            .addr();
    }
    let stack = stack_maps(frames);

    let ready = crate::sched::create_rendezvous();
    let request = crate::sched::create_rendezvous();
    let base = slot.registers_physical_address;
    crate::sched::spawn(move || {
        let mut maps = [Mapping {
            va: 0,
            phys: 0,
            flags: Flags::user_data(),
        }; blk::TRANSFER_BLOCKS + 1 + EXTRA_STACK_PAGES];
        super::fs_service::map_channel(
            &mut maps[..blk::TRANSFER_BLOCKS],
            TRANSFER_VIRTUAL_ADDRESS,
            transfer_phys,
            blk::TRANSFER_BLOCKS,
        );
        // One page of the controller: every register the driver touches and the data FIFO.
        maps[blk::TRANSFER_BLOCKS] = Mapping {
            va: REGISTER_VIRTUAL_ADDRESS,
            phys: base,
            flags: Flags::user_device(),
        };
        maps[blk::TRANSFER_BLOCKS + 1..].copy_from_slice(&stack);
        run(
            image,
            Spawn {
                arg0: words[0], // see `driver_process_contract::designware_mobile_storage::Handoff`
                arg1: words[1], // the window's first sector
                arg2: words[2], // the window's length in sectors
                grants: &[
                    rendezvous_capability(request, Rights::READ), // slot 0: RECEIVE blk requests
                    rendezvous_capability(ready, Rights::WRITE),  // slot 1: signal readiness once
                ],
                maps: &maps,
                windows: &[
                    startup_page_contract::Window {
                        kind: ::driver_process_contract::designware_mobile_storage::TRANSFER_KIND,
                        virtual_address: TRANSFER_VIRTUAL_ADDRESS,
                        pages: blk::TRANSFER_BLOCKS as u32,
                    },
                    startup_page_contract::Window {
                        kind: ::driver_process_contract::designware_mobile_storage::REGISTER_KIND,
                        virtual_address: REGISTER_VIRTUAL_ADDRESS,
                        pages: 1,
                    },
                ],
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The server's extra stack pages sit contiguously below the page `run` maps, so a stack that
    /// outgrows one page lands on a mapped frame and not on the unmapped page the write bench hit.
    #[test_case]
    fn the_extra_stack_pages_are_contiguous_below_the_stack_page() {
        let maps = stack_maps([1, 2, 3, 4]);
        for (k, m) in maps.iter().enumerate() {
            assert_eq!(m.va, USER_STACK_VA - (k as u64 + 1) * FRAME_SIZE);
            assert_eq!(m.phys, k as u64 + 1);
        }
    }
}

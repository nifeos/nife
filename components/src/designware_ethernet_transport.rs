//! **The JH7110 Ethernet port's data plane inside `net_stack`** (milestone 53 (the board's own
//! peripherals: network and storage on real silicon); notes/designware-ethernet.md).
//!
//! `e1000e_transport.rs`'s shape for radon: the volatile shell around
//! `crates/designware_ethernet`'s [`DataPlane`], which is two windows and a barrier. The ring
//! logic, the descriptor formats and the judgment of every word the device writes are in the
//! crate, host-tested against a simulated controller. The kernel already brought the port up,
//! measured that its DMA is coherent, and started both channels with nothing available
//! (`kernel/src/designware_ethernet.rs`); what is mapped here, and what is not, is
//! `kernel/src/user/designware_ethernet_service.rs`'s header.

//! # BUGS
//!
//! - **Polled**, for `e1000e_transport`'s reason, with its costs: no interrupt is granted, so
//!   `net_stack`'s `poll_without_interrupt` (which serves both) keeps a core busy while frames
//!   flow and sleeps to the next scheduler tick once they stop.
//! - **A full transmit ring drops the frame** after a bounded wait; the stack above retransmits.
//!
//! Name: provisional (milestone 53's lane, 2026-10-06 UTC), `<device>_transport` after
//! `e1000e_transport`, with `designware_ethernet` the crate's own provisional name.

use alloc::vec::Vec;

use designware_ethernet::process::{DATA_PLANE_VA, DMA_PAGE_VA};
use designware_ethernet::{DataPlane, Handoff, MAX_FRAME, Region, Tails, layout, regs};
use user_mode_runtime::mapped_window::{MappedWindow, PAGE, doorbell_barrier};

// SAFETY: the spawner maps `layout::PAGES` pages of the DMA region read/write at DATA_PLANE_VA, and
// the controller's DMA page device-typed at DMA_PAGE_VA, before `_start` runs
// (`designware_ethernet_service`).
const PLANE: MappedWindow = unsafe { MappedWindow::new(DATA_PLANE_VA, layout::PAGES * PAGE) };
// SAFETY: as above.
const DMA: MappedWindow = unsafe { MappedWindow::new(DMA_PAGE_VA, PAGE) };

/// The region, through its window: every offset bounds-checked by `MappedWindow`.
struct Plane;

impl Region for Plane {
    fn read_u64(&self, off: u64) -> u64 {
        PLANE.r64(off)
    }
    fn write_u64(&mut self, off: u64, v: u64) {
        PLANE.w64(off, v);
    }
    fn read_bytes(&self, off: u64, out: &mut [u8]) {
        let (words, rest) = out.as_chunks_mut::<8>();
        let mut at = off;
        for w in words {
            *w = PLANE.r64(at).to_le_bytes();
            at += 8;
        }
        for b in rest {
            *b = PLANE.r8(at);
            at += 1;
        }
    }
    fn write_bytes(&mut self, off: u64, src: &[u8]) {
        let (words, rest) = src.as_chunks::<8>();
        let mut at = off;
        for w in words {
            PLANE.w64(at, u64::from_le_bytes(*w));
            at += 8;
        }
        for &b in rest {
            PLANE.w8(at, b);
            at += 1;
        }
    }
}

/// The two tail pointers, on the one mapped page of the controller.
struct Channel;

impl Tails for Channel {
    fn set_rx_tail(&mut self, addr: u32) {
        DMA.w32(u64::from(regs::CHAN_RX_TAIL - regs::DMA_PAGE), addr);
    }
    fn set_tx_tail(&mut self, addr: u32) {
        DMA.w32(u64::from(regs::CHAN_TX_TAIL - regs::DMA_PAGE), addr);
    }
    fn barrier(&mut self) {
        doorbell_barrier();
    }
}

/// The port as `net_stack` sees it.
pub(crate) struct DesignWareNic {
    plane: DataPlane,
    mac: [u8; 6],
}

impl DesignWareNic {
    /// Post the receive buffers and clear the transmit ring; the channels are already running.
    pub fn bring_up(handoff: Handoff) -> DesignWareNic {
        let mut plane = DataPlane::new(handoff.data_plane_phys);
        plane.start(&mut Plane, &mut Channel);
        DesignWareNic {
            plane,
            mac: handoff.mac,
        }
    }

    /// The station address the kernel programmed.
    pub fn mac(&self) -> [u8; 6] {
        self.mac
    }

    /// The next received frame, if the device has written one.
    pub(crate) fn rx_take(&mut self) -> Option<Vec<u8>> {
        let mut buf = [0u8; MAX_FRAME];
        let n = self.plane.receive(&mut Plane, &mut Channel, &mut buf)?;
        Some(buf[..n].to_vec())
    }

    /// Queue `frame`, waiting a bounded while for a descriptor if the ring is full.
    pub(crate) fn tx_send(&mut self, frame: &[u8]) {
        for _ in 0..100_000 {
            if self.plane.can_transmit(&mut Plane) {
                break;
            }
            core::hint::spin_loop();
        }
        // Full after the wait, or a frame smoltcp should never have built: dropped, and the stack
        // above retransmits what mattered (this module's BUGS).
        let _ = self.plane.transmit(&mut Plane, &mut Channel, frame);
    }

    /// Whether a frame is waiting, without taking it, so the poll loop can skip a sleep.
    pub(crate) fn frame_waiting(&self) -> bool {
        let des3 = (PLANE.r64(layout::rx_descriptor(self.plane.rx_cursor()) + 8) >> 32) as u32;
        designware_ethernet::rx_verdict(des3, MAX_FRAME) != designware_ethernet::RxVerdict::NotYet
    }
}

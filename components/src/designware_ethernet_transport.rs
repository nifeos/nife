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

use designware_ethernet::{
    DataPlane, DirectMemoryAccessRegion, MAXIMUM_FRAME_BYTES, Tails, layout, registers,
};
use driver_process_contract::designware_ethernet::Handoff;
use user_mode_runtime::mapped_window::{PAGE, doorbell_barrier};
use user_mode_runtime::startup_window::StartupWindow;

// SAFETY: the spawner (`designware_ethernet_service`) lists both windows on the startup page and
// maps them before `_start` runs: `layout::DIRECT_MEMORY_ACCESS_REGION_PAGES` pages of the DMA
// region read/write, and the controller's DMA register page device-typed.
static PLANE: StartupWindow = unsafe {
    StartupWindow::new(
        ::driver_process_contract::designware_ethernet::DATA_PLANE_KIND,
        layout::DIRECT_MEMORY_ACCESS_REGION_PAGES * PAGE,
    )
};
// SAFETY: as above.
static DMA: StartupWindow = unsafe {
    StartupWindow::new(
        ::driver_process_contract::designware_ethernet::DIRECT_MEMORY_ACCESS_REGISTER_PAGE_KIND,
        PAGE,
    )
};

/// The region, through its window: every offset bounds-checked by `MappedWindow`.
struct Plane;

impl DirectMemoryAccessRegion for Plane {
    fn read_u64(&self, off: u64) -> u64 {
        PLANE.get().r64(off)
    }
    fn write_u64(&mut self, off: u64, v: u64) {
        PLANE.get().w64(off, v);
    }
    fn read_bytes(&self, off: u64, out: &mut [u8]) {
        PLANE.get().copy_to_slice(off, out);
    }
    fn write_bytes(&mut self, off: u64, src: &[u8]) {
        PLANE.get().copy_from_slice(off, src);
    }
}

/// The two tail pointers, on the one mapped page of the controller.
struct Channel;

impl Tails for Channel {
    fn set_receive_tail(&mut self, tail_pointer_low_bits: u32) {
        DMA.get().w32(
            u64::from(
                registers::CHANNEL_RECEIVE_DESCRIPTOR_TAIL_POINTER_REGISTER
                    - registers::DIRECT_MEMORY_ACCESS_REGISTER_PAGE,
            ),
            tail_pointer_low_bits,
        );
    }
    fn set_transmit_tail(&mut self, tail_pointer_low_bits: u32) {
        DMA.get().w32(
            u64::from(
                registers::CHANNEL_TRANSMIT_DESCRIPTOR_TAIL_POINTER_REGISTER
                    - registers::DIRECT_MEMORY_ACCESS_REGISTER_PAGE,
            ),
            tail_pointer_low_bits,
        );
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
        let mut plane = DataPlane::new(handoff.data_plane_physical_address);
        plane.start(&mut Plane, &mut Channel);
        DesignWareNic {
            plane,
            mac: handoff.station_address,
        }
    }

    /// The station address the kernel programmed.
    pub fn mac(&self) -> [u8; 6] {
        self.mac
    }

    /// The next received frame, if the device has written one.
    pub(crate) fn receive_frame(&mut self) -> Option<Vec<u8>> {
        let mut buf = [0u8; MAXIMUM_FRAME_BYTES];
        let n = self.plane.receive(&mut Plane, &mut Channel, &mut buf)?;
        Some(buf[..n].to_vec())
    }

    /// Queue `frame`, waiting a bounded while for a descriptor if the ring is full.
    pub(crate) fn transmit_frame(&mut self, frame: &[u8]) {
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
        let des3 = (PLANE
            .get()
            .r64(layout::receive_descriptor(self.plane.receive_cursor()) + 8)
            >> 32) as u32;
        designware_ethernet::receive_verdict(des3, MAXIMUM_FRAME_BYTES)
            != designware_ethernet::ReceiveVerdict::NotYet
    }
}

//! **The `e1000e` data plane inside `net_stack`** (milestone 494 (a driver for the network card a PC
//! actually has); notes/e1000e.md).
//!
//! The volatile shell around `crates/e1000e`'s [`DataPlane`]: two windows and a barrier. The ring
//! logic, descriptor formats and judgement of every device-written word are the crate's,
//! host-tested against a simulated device. The kernel already reset the device, read its MAC and
//! set both ring bases (`kernel/src/e1000e.rs`); `kernel/src/user/e1000e_service.rs`'s header says
//! what is mapped here. It is a `#[path]` module of `net_stack`, as virtio-net is: a separate
//! driver process would be preferred at equal cost, and is not built because it needs a frame
//! protocol two programs agree on.

//! # BUGS
//!
//! - **Polled.** No interrupt is granted (the service's header says why). While frames flow the
//!   stack looks at the ring continuously, yielding between looks, which keeps a core busy; once a
//!   scheduler tick passes with none it sleeps between looks, and a sleep lasts until the next tick
//!   (up to 10 ms), so the first frame after an idle spell waits that long. Nothing polls while no
//!   request is being served. `socket_contract::polled_wait` has the argument.
//! - **A full transmit ring drops the frame** after a bounded wait, and the stack above retransmits.
//!   The virtio transport makes the same trade.
//!
//! Name: ratified 2026-10-06 (calef, "Yes", in conversation). A network card transport is
//! `<device family>_transport`, as `virtio_net_transport` beside it in the same binary; the family
//! is qualified only when its name covers more than one kind of device, as Synopsys's does. Refused
//! `intel_e1000e_transport` and `e1000e_ethernet_transport` (nothing else is called e1000e; Linux
//! names the driver `e1000e` and puts the vendor in the directory; vendors rebrand while family
//! names stay). The vendor, Intel, is recorded here and in `crates/e1000e`'s header instead.

use alloc::vec::Vec;

use driver_process_contract::e1000e::{
    DATA_PLANE_VIRTUAL_ADDRESS, RX_QUEUE_VIRTUAL_ADDRESS, TX_QUEUE_VIRTUAL_ADDRESS,
};
use e1000e::{DataPlane, Handoff, MAX_FRAME, Region, TAIL_IN_PAGE, Tails, layout};
use user_mode_runtime::mapped_window::{MappedWindow, PAGE, doorbell_barrier};

/// How long the poll loop asks to sleep between looks at an idle receive ring when smoltcp has
/// nothing sooner. The kernel rounds every timed wait up to the next scheduler tick
/// (`user_mode_runtime::tick_hz`), so the sleep lasts up to 10 ms whatever this says.
pub(crate) const POLL_MS: u64 = 1;

// SAFETY: the spawner maps `layout::PAGES` pages of the DMA region read/write at
// DATA_PLANE_VIRTUAL_ADDRESS, and one device-typed page at each queue VA, before `_start` runs
// (`e1000e_service`).
const PLANE: MappedWindow =
    unsafe { MappedWindow::new(DATA_PLANE_VIRTUAL_ADDRESS, layout::PAGES * PAGE) };
// SAFETY: as above.
const RX_QUEUE: MappedWindow = unsafe { MappedWindow::new(RX_QUEUE_VIRTUAL_ADDRESS, PAGE) };
// SAFETY: as above.
const TX_QUEUE: MappedWindow = unsafe { MappedWindow::new(TX_QUEUE_VIRTUAL_ADDRESS, PAGE) };

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
        PLANE.copy_to_slice(off, out);
    }
    fn write_bytes(&mut self, off: u64, src: &[u8]) {
        PLANE.copy_from_slice(off, src);
    }
}

/// The two tails, each on its own mapped page of BAR0.
struct Queues;

impl Tails for Queues {
    fn set_rx_tail(&mut self, v: u32) {
        RX_QUEUE.w32(TAIL_IN_PAGE, v);
    }
    fn set_tx_tail(&mut self, v: u32) {
        TX_QUEUE.w32(TAIL_IN_PAGE, v);
    }
    fn barrier(&mut self) {
        doorbell_barrier();
    }
}

/// The NIC as `net_stack` sees it.
pub(crate) struct GigabitNic {
    plane: DataPlane,
    mac: [u8; 6],
}

impl GigabitNic {
    /// Post the receive buffers and clear the transmit ring; the device is already running.
    pub fn bring_up(handoff: Handoff) -> GigabitNic {
        let mut plane = DataPlane::new(handoff.data_plane_phys);
        plane.start(&mut Plane, &mut Queues);
        GigabitNic {
            plane,
            mac: handoff.mac,
        }
    }

    /// The station address the kernel read from the device.
    pub fn mac(&self) -> [u8; 6] {
        self.mac
    }

    /// The next received frame, if the device has written one.
    pub(crate) fn rx_take(&mut self) -> Option<Vec<u8>> {
        let mut buf = [0u8; MAX_FRAME];
        let n = self.plane.receive(&mut Plane, &mut Queues, &mut buf)?;
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
        let _ = self.plane.transmit(&mut Plane, &mut Queues, frame);
    }

    /// Whether a frame is waiting, without taking it, so the poll loop can skip a sleep.
    pub(crate) fn frame_waiting(&self) -> bool {
        let word = PLANE.r64(layout::rx_descriptor(self.plane.rx_cursor()) + 8);
        e1000e::rx_verdict(word, MAX_FRAME) != e1000e::RxVerdict::NotYet
    }
}

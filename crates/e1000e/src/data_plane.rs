//! **The data plane**: the regions and tails a driver supplies, and the receive and transmit loop
//! over them.
//!
//! Name: ratified 2026-11-03 (UTC), #3145. Ruled in the group E1 ruling on e1000e's module names;
//! kept as it stands.

use super::*;

/// The DMA region, as the data plane reads and writes it. Offsets are bytes from the region's
/// base; an implementation bounds them (a mapped window, for example, that panics on an
/// out-of-window offset rather than touching memory it was not given).
///
/// Name: ratified 2026-11-03 (UTC), #3123.
pub trait DirectMemoryAccessRegion {
    /// Read the little-endian `u64` at `offset`.
    fn read_u64(&self, offset: u64) -> u64;
    /// Write `value` at `offset`, little-endian.
    fn write_u64(&mut self, offset: u64, value: u64);
    /// Copy `out.len()` bytes starting at `offset` into `out`.
    fn read_bytes(&self, offset: u64, out: &mut [u8]);
    /// Copy `source` to `offset`.
    fn write_bytes(&mut self, offset: u64, source: &[u8]);
}

/// The two tail registers, and the ordering between them and the region. The only register writes
/// the data plane makes.
///
/// Name: ratified 2026-11-03 (UTC), #3123.
pub trait Tails {
    /// Write the receive tail (`RDT`).
    ///
    /// Name: ratified 2026-11-03 (UTC), #3123.
    fn set_receive_tail(&mut self, value: u32);
    /// Write the transmit tail (`TDT`).
    ///
    /// Name: ratified 2026-11-03 (UTC), #3123.
    fn set_transmit_tail(&mut self, value: u32);
    /// Order every region access before this against every access after it, as seen by the device.
    /// Called between filling a descriptor and the tail write that announces it, and between
    /// reading a status and reading the bytes it covers.
    fn barrier(&mut self);
}

/// Why a frame was not queued.
///
/// Name: ratified 2026-10-26 (#2761). Ruled in the tree-wide ruling that spells out rx and tx,
/// which names it. Refused `TxError` (tx abbreviates transmit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TransmitError {
    /// Empty, or longer than [`MAXIMUM_FRAME_BYTES`].
    BadLength,
    /// Every descriptor is still the device's.
    Full,
}

impl core::fmt::Display for TransmitError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            TransmitError::BadLength => "the frame is empty or longer than the maximum frame size",
            TransmitError::Full => "every transmit descriptor still belongs to the device",
        })
    }
}

impl core::error::Error for TransmitError {}

/// **The data plane**: two ring cursors and the region's physical base. Everything a network stack
/// does to the device goes through this, so all of it runs on the host against a simulated
/// device.
///
/// Name: ratified 2026-11-03 (UTC), #3123.
#[derive(Debug)]
pub struct DataPlane {
    physical_address: u64,
    receive_next: u16,
    transmit_next: u16,
    transmit_clean: u16,
    /// Frames the device wrote that were not delivered, for a report line.
    pub dropped: u32,
}

impl DataPlane {
    /// A data plane over the region at `data_plane_physical_address`. Call [`DataPlane::start`]
    /// before anything else.
    ///
    /// Name: ratified 2026-11-03 (UTC), #3123.
    pub fn new(data_plane_physical_address: u64) -> DataPlane {
        DataPlane {
            physical_address: data_plane_physical_address,
            receive_next: 0,
            transmit_next: 0,
            transmit_clean: 0,
            dropped: 0,
        }
    }

    /// The receive descriptor the next [`DataPlane::receive`] looks at, for a caller that wants to
    /// peek at its status without consuming it.
    ///
    /// Name: ratified 2026-11-03 (UTC), #3123.
    pub fn receive_cursor(&self) -> u16 {
        self.receive_next
    }

    /// Post every receive buffer and clear the transmit ring. The kernel half left both heads and both
    /// tails at zero; the receive tail goes to the last slot, which gives the device every
    /// descriptor but one (a ring with head equal to tail is empty, so one slot always separates
    /// them).
    ///
    /// Name: ratified 2026-11-03 (UTC), #3123.
    pub fn start(&mut self, region: &mut impl DirectMemoryAccessRegion, tails: &mut impl Tails) {
        for index in 0..RECEIVE_ENTRIES {
            let descriptor_offset = layout::receive_descriptor(index);
            region.write_u64(
                descriptor_offset,
                self.physical_address + layout::receive_buffer(index),
            );
            region.write_u64(descriptor_offset + 8, 0);
        }
        for index in 0..TRANSMIT_ENTRIES {
            let descriptor_offset = layout::transmit_descriptor(index);
            region.write_u64(descriptor_offset, 0);
            region.write_u64(descriptor_offset + 8, 0);
        }
        tails.barrier();
        tails.set_receive_tail(u32::from(RECEIVE_ENTRIES - 1));
    }

    /// Take the next received frame into `out`, returning its length, or `None` when the device has
    /// written nothing new. A descriptor judged [`ReceiveVerdict::Drop`] is consumed and counted
    /// and the next one is tried, so one bad frame cannot stall the ring.
    ///
    /// Name: ratified 2026-11-03 (UTC), #3123.
    pub fn receive(
        &mut self,
        region: &mut impl DirectMemoryAccessRegion,
        tails: &mut impl Tails,
        out: &mut [u8],
    ) -> Option<usize> {
        loop {
            let descriptor_offset = layout::receive_descriptor(self.receive_next);
            let verdict = receive_verdict(region.read_u64(descriptor_offset + 8), out.len());
            if verdict == ReceiveVerdict::NotYet {
                return None;
            }
            // The status was read before the bytes it covers, and nothing else stops a weakly
            // ordered CPU reading the buffer first.
            tails.barrier();
            let got = match verdict {
                ReceiveVerdict::Frame(n) => {
                    region.read_bytes(layout::receive_buffer(self.receive_next), &mut out[..n]);
                    Some(n)
                }
                _ => {
                    self.dropped = self.dropped.wrapping_add(1);
                    None
                }
            };
            // Repost: the address again (a device may write back over it), the status cleared,
            // then hand the slot back by making it the tail.
            region.write_u64(
                descriptor_offset,
                self.physical_address + layout::receive_buffer(self.receive_next),
            );
            region.write_u64(descriptor_offset + 8, 0);
            tails.barrier();
            tails.set_receive_tail(u32::from(self.receive_next));
            self.receive_next = (self.receive_next + 1) % RECEIVE_ENTRIES;
            if got.is_some() {
                return got;
            }
        }
    }

    /// Queue `frame` for transmission.
    ///
    /// Name: ratified 2026-11-03 (UTC), #3123.
    pub fn transmit(
        &mut self,
        region: &mut impl DirectMemoryAccessRegion,
        tails: &mut impl Tails,
        frame: &[u8],
    ) -> Result<(), TransmitError> {
        if frame.is_empty() || frame.len() > MAXIMUM_FRAME_BYTES {
            return Err(TransmitError::BadLength);
        }
        self.reclaim(region);
        let next = (self.transmit_next + 1) % TRANSMIT_ENTRIES;
        if next == self.transmit_clean {
            return Err(TransmitError::Full);
        }
        region.write_bytes(layout::transmit_buffer(self.transmit_next), frame);
        let descriptor_offset = layout::transmit_descriptor(self.transmit_next);
        region.write_u64(
            descriptor_offset,
            self.physical_address + layout::transmit_buffer(self.transmit_next),
        );
        region.write_u64(
            descriptor_offset + 8,
            transmit_descriptor_command_word(frame.len() as u16),
        );
        tails.barrier();
        self.transmit_next = next;
        tails.set_transmit_tail(u32::from(next));
        Ok(())
    }

    /// Is there a free transmit descriptor right now? Reclaims first.
    ///
    /// Name: ratified 2026-11-03 (UTC), #3123.
    pub fn can_transmit(&mut self, region: &mut impl DirectMemoryAccessRegion) -> bool {
        self.reclaim(region);
        (self.transmit_next + 1) % TRANSMIT_ENTRIES != self.transmit_clean
    }

    /// Move the clean cursor past every descriptor the device has reported done. `RS` is set on
    /// every frame, so the device writes `DD` on each in order.
    fn reclaim(&mut self, region: &impl DirectMemoryAccessRegion) {
        while self.transmit_clean != self.transmit_next {
            let word = region.read_u64(layout::transmit_descriptor(self.transmit_clean) + 8);
            if (word >> 32) as u8 & TRANSMIT_STATUS_DESCRIPTOR_DONE == 0 {
                break;
            }
            self.transmit_clean = (self.transmit_clean + 1) % TRANSMIT_ENTRIES;
        }
    }
}

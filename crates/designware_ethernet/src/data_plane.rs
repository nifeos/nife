//! **The data plane**: the regions and tails a driver supplies, and the receive and transmit loop
//! over them.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// The DMA region, as the data plane reads and writes it. Offsets are bytes from the region's
/// base; an implementation bounds them. `e1000e::Region`'s shape.
pub trait Region {
    /// Read the little-endian `u64` at `off`.
    fn read_u64(&self, off: u64) -> u64;
    /// Write `v` at `off`, little-endian.
    fn write_u64(&mut self, off: u64, v: u64);
    /// Copy `out.len()` bytes starting at `off` into `out`.
    fn read_bytes(&self, off: u64, out: &mut [u8]);
    /// Copy `src` to `off`.
    fn write_bytes(&mut self, off: u64, src: &[u8]);
}

/// The two tail pointers, and the ordering between them and the region: the only register writes
/// the data plane makes.
pub trait Tails {
    /// Write the receive tail pointer's low 32 bits ([`regs::CHAN_RX_TAIL`]).
    fn set_receive_tail(&mut self, addr: u32);
    /// Write the transmit tail pointer's low 32 bits ([`regs::CHAN_TX_TAIL`]).
    fn set_transmit_tail(&mut self, addr: u32);
    /// Order every region access before this against every access after it, as seen by the
    /// device. Called between filling a descriptor and the tail write that announces it, and
    /// between reading an ownership bit and reading the bytes it covers.
    fn barrier(&mut self);
}

/// Why a frame was not queued.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): `rx` and `tx` spelled out, replacing `TxError`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransmitError {
    /// Empty, or longer than [`MAXIMUM_FRAME`].
    BadLength,
    /// Every descriptor is still the device's.
    Full,
}

/// **The data plane**: ring cursors and the region's physical base. Everything `net_stack` does
/// to the device goes through this, so all of it runs on the host against a simulated device.
///
/// A tail pointer is one past the last descriptor the device may use. On receive one descriptor,
/// the one just behind [`DataPlane::receive_cursor`], is always held back, so that "the device has
/// every descriptor" and "the device has none" are never the same tail address; each received frame
/// reposts it and holds back the slot just consumed.
#[derive(Debug)]
pub struct DataPlane {
    phys: u64,
    receive_next: u16,
    transmit_next: u16,
    transmit_clean: u16,
    /// Frames the device wrote that were not delivered, for a report line.
    pub dropped: u32,
}

impl DataPlane {
    /// A data plane over the region at `data_plane_phys`. Call [`DataPlane::start`] first.
    pub fn new(data_plane_phys: u64) -> DataPlane {
        DataPlane {
            phys: data_plane_phys,
            receive_next: 0,
            transmit_next: 0,
            transmit_clean: 0,
            dropped: 0,
        }
    }

    /// The receive descriptor the next [`DataPlane::receive`] looks at.
    ///
    /// Name: provisional. Ruled 2026-10-26 (UTC): `rx` and `tx` spelled out, replacing `rx_cursor`.
    pub fn receive_cursor(&self) -> u16 {
        self.receive_next
    }

    /// The receive tail address for a ring whose held-back slot is `slot`.
    fn receive_tail(&self, slot: u16) -> u32 {
        (self.phys + layout::receive_descriptor(slot)) as u32
    }

    /// The transmit tail address when `next` is the next free slot.
    fn transmit_tail(&self, next: u16) -> u32 {
        (self.phys + layout::transmit_descriptor(next)) as u32
    }

    /// **Post every receive descriptor but the last and clear the transmit ring.** The kernel
    /// programmed both ring bases and lengths and started the DMA with both tails at the ring
    /// bases (nothing available); the receive tail write here is what hands the device its
    /// buffers.
    pub fn start(&mut self, region: &mut impl Region, tails: &mut impl Tails) {
        for i in 0..RECEIVE_ENTRIES {
            let d = layout::receive_descriptor(i);
            let [w01, w23] = if i == RECEIVE_ENTRIES - 1 {
                [0, 0]
            } else {
                receive_descriptor_words(self.phys + layout::receive_buffer(i))
            };
            region.write_u64(d, w01);
            region.write_u64(d + 8, w23);
        }
        for i in 0..TRANSMIT_ENTRIES {
            let d = layout::transmit_descriptor(i);
            region.write_u64(d, 0);
            region.write_u64(d + 8, 0);
        }
        tails.barrier();
        tails.set_receive_tail(self.receive_tail(RECEIVE_ENTRIES - 1));
    }

    /// Take the next received frame into `out`, returning its length, or `None` when the device has
    /// written nothing new. A descriptor judged [`ReceiveVerdict::Drop`] is consumed and counted
    /// and the next tried, so one bad frame cannot stall the ring.
    pub fn receive(
        &mut self,
        region: &mut impl Region,
        tails: &mut impl Tails,
        out: &mut [u8],
    ) -> Option<usize> {
        loop {
            let d = layout::receive_descriptor(self.receive_next);
            let des3 = (region.read_u64(d + 8) >> 32) as u32;
            let verdict = receive_verdict(des3, out.len());
            if verdict == ReceiveVerdict::NotYet {
                return None;
            }
            // The ownership bit was read before the bytes it covers, and nothing else stops a
            // weakly ordered CPU reading the buffer first.
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
            // Repost the held-back slot behind this one, then hold this one back: its descriptor
            // is the device's write-back and not owned, so leaving it as it is keeps it out of the
            // device's reach until it is reposted in turn.
            let behind = (self.receive_next + RECEIVE_ENTRIES - 1) % RECEIVE_ENTRIES;
            let [w01, w23] = receive_descriptor_words(self.phys + layout::receive_buffer(behind));
            let b = layout::receive_descriptor(behind);
            region.write_u64(b, w01);
            region.write_u64(b + 8, w23);
            tails.barrier();
            tails.set_receive_tail(self.receive_tail(self.receive_next));
            self.receive_next = (self.receive_next + 1) % RECEIVE_ENTRIES;
            if got.is_some() {
                return got;
            }
        }
    }

    /// Queue `frame` for transmission.
    pub fn transmit(
        &mut self,
        region: &mut impl Region,
        tails: &mut impl Tails,
        frame: &[u8],
    ) -> Result<(), TransmitError> {
        if frame.is_empty() || frame.len() > MAXIMUM_FRAME {
            return Err(TransmitError::BadLength);
        }
        self.reclaim(region);
        let next = (self.transmit_next + 1) % TRANSMIT_ENTRIES;
        if next == self.transmit_clean {
            return Err(TransmitError::Full);
        }
        region.write_bytes(layout::transmit_buffer(self.transmit_next), frame);
        let d = layout::transmit_descriptor(self.transmit_next);
        let [w01, w23] = transmit_descriptor_words(
            self.phys + layout::transmit_buffer(self.transmit_next),
            frame.len() as u16,
        );
        region.write_u64(d, w01);
        // Word 3, with the ownership bit, goes last and in one store with word 2.
        region.write_u64(d + 8, w23);
        tails.barrier();
        self.transmit_next = next;
        tails.set_transmit_tail(self.transmit_tail(next));
        Ok(())
    }

    /// Is there a free transmit descriptor right now? Reclaims first.
    pub fn can_transmit(&mut self, region: &mut impl Region) -> bool {
        self.reclaim(region);
        (self.transmit_next + 1) % TRANSMIT_ENTRIES != self.transmit_clean
    }

    /// Move the clean cursor past every descriptor the device has handed back.
    fn reclaim(&mut self, region: &impl Region) {
        while self.transmit_clean != self.transmit_next {
            let des3 = (region.read_u64(layout::transmit_descriptor(self.transmit_clean) + 8) >> 32)
                as u32;
            if des3 & TDES3_OWN != 0 {
                break;
            }
            self.transmit_clean = (self.transmit_clean + 1) % TRANSMIT_ENTRIES;
        }
    }
}

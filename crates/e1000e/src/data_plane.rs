//! **The data plane**: the regions and tails a driver supplies, and the receive and transmit loop
//! over them.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// The DMA region, as the data plane reads and writes it. Offsets are bytes from the region's
/// base; an implementation bounds them (the EL0 one is a `MappedWindow`, which panics on an
/// out-of-window offset rather than touching memory it was not given).
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

/// The two tail registers, and the ordering between them and the region. The only register writes
/// the data plane makes.
pub trait Tails {
    /// Write the receive tail (`RDT`).
    fn set_rx_tail(&mut self, v: u32);
    /// Write the transmit tail (`TDT`).
    fn set_tx_tail(&mut self, v: u32);
    /// Order every region access before this against every access after it, as seen by the device.
    /// Called between filling a descriptor and the tail write that announces it, and between
    /// reading a status and reading the bytes it covers.
    fn barrier(&mut self);
}

/// Why a frame was not queued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxError {
    /// Empty, or longer than [`MAX_FRAME`].
    BadLength,
    /// Every descriptor is still the device's.
    Full,
}

/// **The data plane**: two ring cursors and the region's physical base. Everything `net_stack`
/// does to the device goes through this, so all of it runs on the host against a simulated
/// device.
#[derive(Debug)]
pub struct DataPlane {
    phys: u64,
    rx_next: u16,
    tx_next: u16,
    tx_clean: u16,
    /// Frames the device wrote that were not delivered, for a report line.
    pub dropped: u32,
}

impl DataPlane {
    /// A data plane over the region at `data_plane_phys`. Call [`DataPlane::start`] before
    /// anything else.
    pub fn new(data_plane_phys: u64) -> DataPlane {
        DataPlane {
            phys: data_plane_phys,
            rx_next: 0,
            tx_next: 0,
            tx_clean: 0,
            dropped: 0,
        }
    }

    /// The receive descriptor the next [`DataPlane::receive`] looks at, for a caller that wants to
    /// peek at its status without consuming it.
    pub fn rx_cursor(&self) -> u16 {
        self.rx_next
    }

    /// Post every receive buffer and clear the transmit ring. The kernel left both heads and both
    /// tails at zero; the receive tail goes to the last slot, which gives the device every
    /// descriptor but one (a ring with head equal to tail is empty, so one slot always separates
    /// them).
    pub fn start(&mut self, region: &mut impl Region, tails: &mut impl Tails) {
        for i in 0..RX_ENTRIES {
            let d = layout::rx_descriptor(i);
            region.write_u64(d, self.phys + layout::rx_buffer(i));
            region.write_u64(d + 8, 0);
        }
        for i in 0..TX_ENTRIES {
            let d = layout::tx_descriptor(i);
            region.write_u64(d, 0);
            region.write_u64(d + 8, 0);
        }
        tails.barrier();
        tails.set_rx_tail(u32::from(RX_ENTRIES - 1));
    }

    /// Take the next received frame into `out`, returning its length, or `None` when the device
    /// has written nothing new. A descriptor judged [`RxVerdict::Drop`] is consumed and counted and
    /// the next one is tried, so one bad frame cannot stall the ring.
    pub fn receive(
        &mut self,
        region: &mut impl Region,
        tails: &mut impl Tails,
        out: &mut [u8],
    ) -> Option<usize> {
        loop {
            let d = layout::rx_descriptor(self.rx_next);
            let verdict = rx_verdict(region.read_u64(d + 8), out.len());
            if verdict == RxVerdict::NotYet {
                return None;
            }
            // The status was read before the bytes it covers, and nothing else stops a weakly
            // ordered CPU reading the buffer first.
            tails.barrier();
            let got = match verdict {
                RxVerdict::Frame(n) => {
                    region.read_bytes(layout::rx_buffer(self.rx_next), &mut out[..n]);
                    Some(n)
                }
                _ => {
                    self.dropped = self.dropped.wrapping_add(1);
                    None
                }
            };
            // Repost: the address again (a device may write back over it), the status cleared,
            // then hand the slot back by making it the tail.
            region.write_u64(d, self.phys + layout::rx_buffer(self.rx_next));
            region.write_u64(d + 8, 0);
            tails.barrier();
            tails.set_rx_tail(u32::from(self.rx_next));
            self.rx_next = (self.rx_next + 1) % RX_ENTRIES;
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
    ) -> Result<(), TxError> {
        if frame.is_empty() || frame.len() > MAX_FRAME {
            return Err(TxError::BadLength);
        }
        self.reclaim(region);
        let next = (self.tx_next + 1) % TX_ENTRIES;
        if next == self.tx_clean {
            return Err(TxError::Full);
        }
        region.write_bytes(layout::tx_buffer(self.tx_next), frame);
        let d = layout::tx_descriptor(self.tx_next);
        region.write_u64(d, self.phys + layout::tx_buffer(self.tx_next));
        region.write_u64(d + 8, tx_word(frame.len() as u16));
        tails.barrier();
        self.tx_next = next;
        tails.set_tx_tail(u32::from(next));
        Ok(())
    }

    /// Is there a free transmit descriptor right now? Reclaims first.
    pub fn can_transmit(&mut self, region: &mut impl Region) -> bool {
        self.reclaim(region);
        (self.tx_next + 1) % TX_ENTRIES != self.tx_clean
    }

    /// Move the clean cursor past every descriptor the device has reported done. `RS` is set on
    /// every frame, so the device writes `DD` on each in order.
    fn reclaim(&mut self, region: &impl Region) {
        while self.tx_clean != self.tx_next {
            let word = region.read_u64(layout::tx_descriptor(self.tx_clean) + 8);
            if (word >> 32) as u8 & TX_STATUS_DD == 0 {
                break;
            }
            self.tx_clean = (self.tx_clean + 1) % TX_ENTRIES;
        }
    }
}

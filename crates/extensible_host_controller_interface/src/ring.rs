//! **The rings**: the producer rings the driver fills and the event ring it drains.

use super::*;

/// **A ring the driver produces into**: the command ring and every transfer ring. Its last entry is
/// always a link TRB back to the first.
///
/// # Examples
///
/// Four entries are three usable slots and a link TRB; the third enqueue also writes the link, and
/// the cycle flips for the next lap:
///
/// ```
/// use extensible_host_controller_interface::{ProducerRing, Trb};
///
/// let mut ring = ProducerRing::new(0x1000, 4);
/// let a = ring.enqueue(Trb::enable_slot());
/// assert_eq!((a.index, a.trb.cycle(), a.link), (0, true, None));
/// ring.enqueue(Trb::enable_slot());
/// let c = ring.enqueue(Trb::enable_slot());
/// assert_eq!(c.index, 2);
/// let (at, link) = c.link.unwrap(); // the third TRB fills the lap, so the link is written too
/// assert_eq!(at, 3);
/// assert!(link.cycle()); // stamped with the lap it ends, so the controller follows it
/// let d = ring.enqueue(Trb::enable_slot());
/// assert_eq!((d.index, d.trb.cycle()), (0, false)); // second lap, cycle flipped
/// assert_eq!(ring.address_of(d.index), 0x1000);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProducerRing {
    pub(crate) base: u64,
    pub(crate) entries: u16,
    pub(crate) enqueue: u16,
    pub(crate) cycle: bool,
}

/// **What [`ProducerRing::enqueue`] says to write**: the TRB at `index`, stamped with this lap's
/// cycle, and when the ring has just filled its lap, the link TRB at the last index too.
///
/// The driver writes `trb` and then `link`, each with the cycle dword last: until a TRB's cycle bit
/// matches, the controller treats it as not yet produced, so the dword that carries it goes in
/// after the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    /// Where the TRB goes.
    pub index: u16,
    /// The TRB, cycle stamped.
    pub trb: Trb,
    /// The link TRB and where it goes, when this enqueue filled the lap.
    pub link: Option<(u16, Trb)>,
}

impl ProducerRing {
    /// A ring of `entries` TRBs (the last of them the link) at physical `base`, in the state a
    /// zeroed ring starts in: producer cycle 1, so the zeros read as not produced.
    ///
    /// # Panics
    /// When `entries` is under 2, which is a ring with no room for anything but its link; every
    /// caller passes a constant.
    pub const fn new(base: u64, entries: u16) -> ProducerRing {
        assert!(
            entries >= 2,
            "a producer ring needs a slot besides its link"
        );
        ProducerRing {
            base,
            entries,
            enqueue: 0,
            cycle: true,
        }
    }

    /// The physical address of entry `index`.
    pub const fn address_of(&self, index: u16) -> u64 {
        self.base + index as u64 * 16
    }

    /// The physical address and cycle of the next TRB to be written: what a TR Dequeue Pointer or
    /// `CRCR` is set to (the cycle in bit 0, as both registers take it).
    pub const fn dequeue_pointer(&self) -> u64 {
        self.address_of(self.enqueue) | self.cycle as u64
    }

    /// **Place one TRB**, and the link if this fills the lap.
    pub fn enqueue(&mut self, trb: Trb) -> Placement {
        let index = self.enqueue;
        let placed = trb.with_cycle(self.cycle);
        self.enqueue += 1;
        let mut link = None;
        if self.enqueue == self.entries - 1 {
            link = Some((self.enqueue, Trb::link(self.base).with_cycle(self.cycle)));
            self.enqueue = 0;
            self.cycle = !self.cycle;
        }
        Placement {
            index,
            trb: placed,
            link,
        }
    }
}

/// **The event ring, from the consumer's side**: one segment of `entries` TRBs at a physical base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventRing {
    base: u64,
    entries: u16,
    dequeue: u16,
    cycle: bool,
}

impl EventRing {
    /// A zeroed ring: consumer cycle 1, so the zeros read as not yet written.
    ///
    /// # Panics
    /// When `entries` is zero; every caller passes a constant.
    pub const fn new(base: u64, entries: u16) -> EventRing {
        assert!(entries > 0, "an event ring needs an entry");
        EventRing {
            base,
            entries,
            dequeue: 0,
            cycle: true,
        }
    }

    /// The entry the next event will be at.
    pub const fn index(&self) -> u16 {
        self.dequeue
    }

    /// Whether `trb`, read at [`index`](Self::index), is an event the controller has written
    /// this lap.
    pub const fn is_ready(&self, trb: &Trb) -> bool {
        trb.cycle() == self.cycle
    }

    /// Consume the entry at [`index`](Self::index).
    pub fn advance(&mut self) {
        self.dequeue += 1;
        if self.dequeue == self.entries {
            self.dequeue = 0;
            self.cycle = !self.cycle;
        }
    }

    /// The physical address to write to `ERDP`: where the next event will be.
    pub const fn dequeue_pointer(&self) -> u64 {
        self.base + self.dequeue as u64 * 16
    }

    /// The one event ring segment table entry describing this ring.
    pub const fn segment_table_entry(&self) -> [u32; 4] {
        [
            self.base as u32,
            (self.base >> 32) as u32,
            self.entries as u32,
            0,
        ]
    }
}

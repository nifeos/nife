//! What the kernel and the EL0 block server agree on: where the server's two windows sit, the part
//! of the card it may serve, and the three spawn words that say so. The server
//! (`components/src/designware_mobile_storage.rs`) answers `filesystem_contract::blk`, the four
//! verbs the virtio and NVMe block servers answer, so the FS server cannot tell which disk it has.
//! Its blocks are 4096 bytes; the card's are 512. A request names blocks inside a window the kernel
//! chose at spawn, never a card address, and [`Window::translate`] is the one place a block becomes
//! a sector; a Kani harness proves no request reaches outside it. Which window radon's filesystem
//! gets is a fact two programs must agree on, so an architect's call (the crate root's BUGS and
//! milestone 53's block).

use filesystem_contract::blk;

/// Where the server finds the controller's first page, mapped device-typed.
pub const REGISTER_VA: u64 = address_space_map::pair_page(0x0060_0000);
/// Where the server finds the transfer region it shares with its client, [`blk::TRANSFER_BLOCKS`]
/// pages.
pub const TRANSFER_VA: u64 = address_space_map::pair_page(0x0070_0000);

/// Card sectors in one `blk` block.
pub const SECTORS_PER_BLOCK: u64 = (blk::BLOCK_SIZE / crate::sd::BLOCK) as u64;

/// **The part of the card a server may touch**, in card sectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    /// The first sector.
    pub first: u64,
    /// How many sectors. A whole number of `blk` blocks; [`Handoff::unpack`] refuses anything else.
    pub sectors: u64,
    /// Whether `WRITE` is served. A read-only window answers it `EROFS`.
    pub writable: bool,
}

impl Window {
    /// How many `blk` blocks the window holds.
    #[must_use]
    pub const fn blocks(&self) -> u64 {
        self.sectors / SECTORS_PER_BLOCK
    }

    /// **The first card sector of `count` blocks from `block`**, or `None` if any of them is
    /// outside the window (or `count` is 0).
    ///
    /// ```
    /// use designware_mobile_storage::serve::Window;
    ///
    /// let w = Window { first: 2048, sectors: 64, writable: false };
    /// assert_eq!(w.translate(1, 2), Some(2056));
    /// assert_eq!(w.translate(7, 1), Some(2104));
    /// assert_eq!(w.translate(7, 2), None);
    /// ```
    #[must_use]
    pub const fn translate(&self, block: u64, count: u64) -> Option<u64> {
        let Some(end) = block.checked_add(count) else {
            return None;
        };
        if count == 0 || end > self.blocks() {
            return None;
        }
        // `block * SECTORS_PER_BLOCK` is at most `sectors` here; the sum is checked so a window
        // that was never through `Handoff::unpack` still cannot wrap.
        self.first.checked_add(block * SECTORS_PER_BLOCK)
    }
}

/// The spawn words' tag, so a server handed some other program's words refuses them.
pub const ROLE: u64 = 0x5d17;

/// **Everything the server is told at spawn**: the window, and the three facts about the slot the
/// controller needs that the server cannot read for itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handoff {
    /// The part of the card to serve.
    pub window: Window,
    /// The `ciu` clock's rate.
    pub ciu_hz: u32,
    /// The slot's bus width: 1, 4 or 8.
    pub bus_width: u8,
    /// The FIFO's depth in words, 1 to 255.
    pub fifo_depth: u8,
}

impl Handoff {
    /// The three spawn words: `[ROLE << 48 | fifo_depth << 40 | writable << 39 | bus_width << 32
    /// | ciu_hz, first, sectors]`.
    #[must_use]
    pub const fn pack(&self) -> [u64; 3] {
        [
            ROLE << 48
                | (self.fifo_depth as u64) << 40
                | (self.window.writable as u64) << 39
                | ((self.bus_width & 0xf) as u64) << 32
                | self.ciu_hz as u64,
            self.window.first,
            self.window.sectors,
        ]
    }

    /// **The handoff `pack` made, or `None`**: a wrong tag, a bus width that is not 1, 4 or 8, a
    /// zero clock or FIFO, an empty window, a window that is not whole blocks, or one whose end
    /// does not fit in 64 bits.
    #[must_use]
    pub const fn unpack(words: [u64; 3]) -> Option<Handoff> {
        let [w0, first, sectors] = words;
        let bus_width = ((w0 >> 32) & 0x7f) as u8;
        let h = Handoff {
            window: Window {
                first,
                sectors,
                writable: (w0 >> 39) & 1 == 1,
            },
            ciu_hz: w0 as u32,
            bus_width,
            fifo_depth: (w0 >> 40) as u8,
        };
        let ok = w0 >> 48 == ROLE
            && matches!(bus_width, 1 | 4 | 8)
            && h.ciu_hz != 0
            && h.fifo_depth != 0
            && sectors != 0
            && sectors % SECTORS_PER_BLOCK == 0
            && first.checked_add(sectors).is_some();
        if ok { Some(h) } else { None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_handoff_round_trips_and_nothing_else_unpacks() {
        let h = Handoff {
            window: Window {
                first: 2048,
                sectors: 1 << 20,
                writable: true,
            },
            ciu_hz: 50_000_000,
            bus_width: 4,
            fifo_depth: 32,
        };
        assert_eq!(Handoff::unpack(h.pack()), Some(h));
        let ro = Handoff {
            window: Window {
                writable: false,
                ..h.window
            },
            ..h
        };
        assert_eq!(Handoff::unpack(ro.pack()), Some(ro));
        let [w0, a, b] = h.pack();
        assert_eq!(Handoff::unpack([w0 ^ 1 << 48, a, b]), None);
        assert_eq!(Handoff::unpack([w0, a, b - 1]), None);
        assert_eq!(Handoff::unpack([w0, a, 0]), None);
        assert_eq!(Handoff::unpack([w0, u64::MAX, b]), None);
        let three = Handoff { bus_width: 3, ..h };
        assert_eq!(Handoff::unpack(three.pack()), None);
    }

    #[test]
    fn a_blk_block_is_eight_card_sectors() {
        assert_eq!(SECTORS_PER_BLOCK, 8);
        let w = Window {
            first: 0,
            sectors: 16,
            writable: true,
        };
        assert_eq!(w.blocks(), 2);
        assert_eq!(w.translate(0, 0), None);
        assert_eq!(w.translate(u64::MAX, 2), None);
    }
}

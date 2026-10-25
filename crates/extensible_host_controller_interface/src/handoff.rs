//! **What the kernel hands the driver** at spawn.

use super::*;

/// **Everything the driver is told at spawn**, and the whole of what it could not discover for
/// itself: where its DMA region is physically (a process knows only virtual addresses, and every
/// pointer the controller reads is physical), how many pages it is, and which register pages it
/// was mapped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handoff {
    /// The DMA region's physical base. Page aligned.
    pub dma_phys: u64,
    /// The region's length in pages: [`dma::WORK_PAGES`] plus the controller's scratchpads.
    pub dma_pages: u32,
    /// The register pages mapped, [`register_window`]'s mask.
    pub registers: u16,
}

impl Handoff {
    /// The three spawn words.
    pub const fn pack(&self) -> [u64; 3] {
        [
            self.dma_phys,
            self.dma_pages as u64 | (self.registers as u64) << 32,
            0,
        ]
    }

    /// The three spawn words back, or `None` for words no kernel would have sent: an unaligned or
    /// zero base, a region too small for the layout, or a register mask without page 0 (which
    /// holds the capability registers the driver reads first).
    pub fn unpack(w: [u64; 3]) -> Option<Handoff> {
        let dma_pages = u32::try_from(w[1] & 0xffff_ffff).ok()?;
        let registers = u16::try_from(w[1] >> 32).ok()?;
        let ok = w[0] != 0
            && w[0].is_multiple_of(PAGE)
            && u64::from(dma_pages) >= dma::WORK_PAGES
            && registers & 1 != 0
            && w[2] == 0;
        ok.then_some(Handoff {
            dma_phys: w[0],
            dma_pages,
            registers,
        })
    }
}

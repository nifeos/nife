#![no_std]
//! **xHCI, as pure logic**: everything a USB host controller driver computes, with nothing it
//! touches. A kernel finds and confines the register file and DMA memory and a
//! user-space driver drives them; this is the arithmetic between.
//!
//! [`regs`] and [`Capabilities`] name the registers; [`register_window`] decides which register
//! pages a confined driver is mapped; [`Handoff`] packs the spawn words; [`dma`] is the DMA
//! region's layout; [`Trb`], [`Event`], [`ProducerRing`] and [`EventRing`] are the ring discipline
//! (xHCI 1.2, chapters 4 and 6); [`context`] holds slot and endpoint contexts; [`port`] is the port
//! status register.

//! # BUGS
//!
//! - One page size: [`dma`] assumes 4 KiB (`PAGESIZE` bit 0), and the driver refuses otherwise.
//! - No streams, isochronous or bulk: the TRBs a keyboard needs and nothing more.
//! - Hubs are not modelled (route string 0), so a device behind a hub cannot be addressed.

pub mod context;
pub mod dma;
pub mod port;
pub mod regs;
pub mod report;

pub use regs::Capabilities;

/// **How many pages of the register file a driver can be mapped**: the bits of [`Handoff`]'s
/// register mask. 64 KiB, which is the BAR of every Intel xHCI this project has read about; a
/// larger BAR is mapped this far and no further, and [`register_window`] refuses a controller
/// whose needed registers lie beyond it.
pub const MAX_REGISTER_PAGES: u64 = 16;

/// The page size every layout here is in.
pub const PAGE: u64 = 4096;

/// TRB type codes (xHCI 1.2 Table 6-91).
pub mod trb_type;

/// Completion codes (xHCI 1.2 Table 6-90), the ones a keyboard driver tells apart.
pub mod completion;

#[cfg(test)]
mod tests;

mod event;
mod handoff;
mod ring;
mod trb;
mod window;

pub use context::interrupt_interval;
pub use event::Event;
pub use handoff::Handoff;
pub use ring::{EventRing, Placement, ProducerRing};
pub use trb::Trb;
pub use window::{WindowRefusal, register_window};

#[cfg(kani)]
mod verification;

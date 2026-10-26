#![no_std]
//! **xHCI, as pure logic**: everything a USB host controller driver computes, with nothing it
//! touches. A kernel finds and confines the register file and DMA memory and a
//! user-space driver drives them; this is the arithmetic between.
//!
//! [`regs`] and [`Capabilities`] name the registers; [`dma`] is the DMA region's layout; a
//! Transfer Request Block (TRB, [`TransferRequestBlock`]), [`Event`], [`ProducerRing`] and
//! [`EventRing`] are the ring discipline (xHCI 1.2, chapters 4 and 6); [`context`] holds slot and
//! endpoint contexts; [`port`] is the port status register.

//! # BUGS
//!
//! - One page size: [`dma`] assumes 4 KiB (`PAGESIZE` bit 0), and the driver refuses otherwise.
//! - No streams, isochronous or bulk: the TRBs a keyboard needs and nothing more.
//! - Hubs are not modelled (route string 0), so a device behind a hub cannot be addressed.

pub mod context;
pub mod dma;
pub mod port;
pub mod regs;

pub use regs::Capabilities;

/// The page size every layout here is in.
pub const PAGE_BYTES: u64 = 4096;

/// Transfer Request Block (TRB) type codes (xHCI 1.2 Table 6-91).
pub mod transfer_request_block_type;

/// Completion codes (xHCI 1.2 Table 6-90), the ones a keyboard driver tells apart.
pub mod completion;

#[cfg(test)]
mod tests;

mod event;
mod ring;
mod transfer_request_block;

pub use context::interrupt_interval;
pub use event::Event;
pub use ring::{EventRing, ProducerRing, TransferRequestBlockPlacement};
pub use transfer_request_block::TransferRequestBlock;

#[cfg(kani)]
mod verification;

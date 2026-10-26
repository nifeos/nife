//! The `e1000e` server's windows, which `net_stack`'s transport reads and the kernel maps.
//!
//! Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts, after the crate it
//! serves.

use address_space_map::pair_page;

/// The DMA region, `e1000e::layout::PAGES` pages, normal memory. The virtio server's one DMA
/// page lives at the same address; a `net_stack` drives one NIC or the other, never both.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts. It was the
/// driver crate's `DATA_PLANE_VA`, renamed to spell out the address as design/naming.md asks.
pub const DATA_PLANE_VIRTUAL_ADDRESS: u64 = pair_page(0x0000_0000_0090_0000);
/// BAR0's receive-queue page, device memory.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts. It was the
/// driver crate's `RX_QUEUE_VA`, renamed to spell out the address as design/naming.md asks.
pub const RX_QUEUE_VIRTUAL_ADDRESS: u64 = pair_page(0x0000_0000_009c_0000);
/// BAR0's transmit-queue page, device memory.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts. It was the
/// driver crate's `TX_QUEUE_VA`, renamed to spell out the address as design/naming.md asks.
pub const TX_QUEUE_VIRTUAL_ADDRESS: u64 = pair_page(0x0000_0000_009c_1000);

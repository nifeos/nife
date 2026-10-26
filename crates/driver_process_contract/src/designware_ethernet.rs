//! The JH7110 Ethernet server's windows, which `net_stack`'s transport reads and the kernel maps.
//!
//! Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts, after the crate it
//! serves.

use address_space_map::pair_page;

/// The DMA region, `designware_ethernet::layout::PAGES` pages, normal memory. The same address as
/// the virtio and `e1000e` servers' DMA windows; a `net_stack` drives one NIC, never two.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts. It was the
/// driver crate's `DATA_PLANE_VA`, renamed to spell out the address as design/naming.md asks.
pub const DATA_PLANE_VIRTUAL_ADDRESS: u64 = pair_page(0x0000_0000_0090_0000);
/// The controller's DMA page (`designware_ethernet::regs::DMA_PAGE`), device memory.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts. It was the
/// driver crate's `DMA_PAGE_VA`, renamed to spell out the address as design/naming.md asks.
pub const DIRECT_MEMORY_ACCESS_PAGE_VIRTUAL_ADDRESS: u64 = pair_page(0x0000_0000_009c_0000);

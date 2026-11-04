//! Name: ratified 2026-11-03 (UTC), #3145. Ruled in the group E1 ruling on e1000e's module names;
//! kept as it stands.

use super::{BUFFER_BYTES, PAGE, RECEIVE_ENTRIES, TRANSMIT_ENTRIES};
/// The receive descriptor ring's offset in the region, in bytes.
///
/// Name: ratified 2026-11-03 (UTC), #3148. Renamed from `RECEIVE_RING`: the unit goes in the name, and `rx` stays spelled out
/// (refused `RX_RING`, since rx abbreviates receive).
pub const RECEIVE_RING_OFFSET_BYTES: u64 = 0;
/// The transmit descriptor ring's offset in the region, in bytes.
///
/// Name: ratified 2026-11-03 (UTC), #3148. Renamed from `TRANSMIT_RING`: the unit goes in the name, and `tx` stays spelled out
/// (refused `TX_RING`, since tx abbreviates transmit).
pub const TRANSMIT_RING_OFFSET_BYTES: u64 = PAGE;
/// The first receive buffer's offset in the region, in bytes.
///
/// Name: ratified 2026-11-03 (UTC), #3148. Renamed from `RECEIVE_BUFFERS`: the unit goes in the name, and `rx` stays spelled out
/// (refused `RX_BUFFERS`, since rx abbreviates receive).
pub const RECEIVE_BUFFERS_OFFSET_BYTES: u64 = 2 * PAGE;
/// The first transmit buffer's offset in the region, in bytes.
///
/// Name: ratified 2026-11-03 (UTC), #3148. Renamed from `TRANSMIT_BUFFERS`: the unit goes in the name, and `tx` stays spelled out
/// (refused `TX_BUFFERS`, since tx abbreviates transmit).
pub const TRANSMIT_BUFFERS_OFFSET_BYTES: u64 =
    RECEIVE_BUFFERS_OFFSET_BYTES + RECEIVE_ENTRIES as u64 * BUFFER_BYTES;
/// The direct memory access region's size in bytes.
///
/// Name: ratified 2026-11-03 (UTC), #3148. Renamed from `BYTES`, to agree with `designware_ethernet`'s region size.
pub const DIRECT_MEMORY_ACCESS_REGION_BYTES: u64 =
    TRANSMIT_BUFFERS_OFFSET_BYTES + TRANSMIT_ENTRIES as u64 * BUFFER_BYTES;
/// The direct memory access region's size in pages.
///
/// Name: ratified 2026-11-03 (UTC), #3148. Renamed from `PAGES`, to agree with `designware_ethernet`'s region size.
pub const DIRECT_MEMORY_ACCESS_REGION_PAGES: u64 = DIRECT_MEMORY_ACCESS_REGION_BYTES.div_ceil(PAGE);

/// Receive buffer `index`'s offset. `index` is reduced modulo the ring so no input leaves the
/// region.
///
/// Name: ratified 2026-11-03 (UTC), #3148. Kept; `rx` stays spelled out (refused `rx_buffer`, since rx abbreviates receive).
pub const fn receive_buffer(index: u16) -> u64 {
    RECEIVE_BUFFERS_OFFSET_BYTES + (index % RECEIVE_ENTRIES) as u64 * BUFFER_BYTES
}
/// Transmit buffer `index`'s offset, reduced the same way.
///
/// Name: ratified 2026-11-03 (UTC), #3148. Kept; `tx` stays spelled out (refused `tx_buffer`, since tx abbreviates transmit).
pub const fn transmit_buffer(index: u16) -> u64 {
    TRANSMIT_BUFFERS_OFFSET_BYTES + (index % TRANSMIT_ENTRIES) as u64 * BUFFER_BYTES
}
/// Receive descriptor `index`'s offset.
///
/// Name: ratified 2026-11-03 (UTC), #3148. Kept; `rx` stays spelled out (refused `rx_descriptor`, since rx abbreviates receive).
pub const fn receive_descriptor(index: u16) -> u64 {
    RECEIVE_RING_OFFSET_BYTES + (index % RECEIVE_ENTRIES) as u64 * 16
}
/// Transmit descriptor `index`'s offset.
///
/// Name: ratified 2026-11-03 (UTC), #3148. Kept; `tx` stays spelled out (refused `tx_descriptor`, since tx abbreviates transmit).
pub const fn transmit_descriptor(index: u16) -> u64 {
    TRANSMIT_RING_OFFSET_BYTES + (index % TRANSMIT_ENTRIES) as u64 * 16
}

use super::{BUFFER_SIZE, PAGE, RECEIVE_ENTRIES, TRANSMIT_ENTRIES};
/// The receive descriptor ring.
///
/// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling
/// covers the rest of the name. Refused `RX_RING` (rx abbreviates receive).
pub const RECEIVE_RING: u64 = 0;
/// The transmit descriptor ring.
///
/// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling
/// covers the rest of the name. Refused `TX_RING` (tx abbreviates transmit).
pub const TRANSMIT_RING: u64 = PAGE;
/// The first receive buffer.
///
/// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling
/// covers the rest of the name. Refused `RX_BUFFERS` (rx abbreviates receive).
pub const RECEIVE_BUFFERS: u64 = 2 * PAGE;
/// The first transmit buffer.
///
/// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling
/// covers the rest of the name. Refused `TX_BUFFERS` (tx abbreviates transmit).
pub const TRANSMIT_BUFFERS: u64 = RECEIVE_BUFFERS + RECEIVE_ENTRIES as u64 * BUFFER_SIZE;
/// The region's size in bytes.
pub const BYTES: u64 = TRANSMIT_BUFFERS + TRANSMIT_ENTRIES as u64 * BUFFER_SIZE;
/// The region's size in pages.
pub const PAGES: u64 = BYTES.div_ceil(PAGE);

/// Receive buffer `i`'s offset. `i` is reduced modulo the ring so no input leaves the region.
///
/// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling
/// covers the rest of the name. Refused `rx_buffer` (rx abbreviates receive).
pub const fn receive_buffer(i: u16) -> u64 {
    RECEIVE_BUFFERS + (i % RECEIVE_ENTRIES) as u64 * BUFFER_SIZE
}
/// Transmit buffer `i`'s offset, reduced the same way.
///
/// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling
/// covers the rest of the name. Refused `tx_buffer` (tx abbreviates transmit).
pub const fn transmit_buffer(i: u16) -> u64 {
    TRANSMIT_BUFFERS + (i % TRANSMIT_ENTRIES) as u64 * BUFFER_SIZE
}
/// Receive descriptor `i`'s offset.
///
/// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling
/// covers the rest of the name. Refused `rx_descriptor` (rx abbreviates receive).
pub const fn receive_descriptor(i: u16) -> u64 {
    RECEIVE_RING + (i % RECEIVE_ENTRIES) as u64 * 16
}
/// Transmit descriptor `i`'s offset.
///
/// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling
/// covers the rest of the name. Refused `tx_descriptor` (tx abbreviates transmit).
pub const fn transmit_descriptor(i: u16) -> u64 {
    TRANSMIT_RING + (i % TRANSMIT_ENTRIES) as u64 * 16
}

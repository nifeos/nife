use super::{BUFFER_SIZE, DESCRIPTOR_BYTES, PAGE, RECEIVE_ENTRIES, TRANSMIT_ENTRIES};
/// The receive descriptor ring.
pub(crate) const RECEIVE_RING: u64 = 0;
/// The transmit descriptor ring.
pub(crate) const TRANSMIT_RING: u64 = PAGE;
/// The first receive buffer.
pub(crate) const RECEIVE_BUFFERS: u64 = 2 * PAGE;
/// The first transmit buffer.
pub(crate) const TRANSMIT_BUFFERS: u64 = RECEIVE_BUFFERS + RECEIVE_ENTRIES as u64 * BUFFER_SIZE;
/// The region's size in bytes.
pub const BYTES: u64 = TRANSMIT_BUFFERS + TRANSMIT_ENTRIES as u64 * BUFFER_SIZE;
/// The region's size in pages.
pub const PAGES: u64 = BYTES.div_ceil(PAGE);

/// Receive buffer `i`'s offset. `i` is reduced modulo the ring so no input leaves the region.
pub(crate) const fn receive_buffer(i: u16) -> u64 {
    RECEIVE_BUFFERS + (i % RECEIVE_ENTRIES) as u64 * BUFFER_SIZE
}
/// Transmit buffer `i`'s offset, reduced the same way.
pub(crate) const fn transmit_buffer(i: u16) -> u64 {
    TRANSMIT_BUFFERS + (i % TRANSMIT_ENTRIES) as u64 * BUFFER_SIZE
}
/// Receive descriptor `i`'s offset.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): `rx` and `tx` spelled out; was `rx_descriptor`.
pub const fn receive_descriptor(i: u16) -> u64 {
    RECEIVE_RING + (i % RECEIVE_ENTRIES) as u64 * DESCRIPTOR_BYTES
}
/// Transmit descriptor `i`'s offset.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): `rx` and `tx` spelled out; was `tx_descriptor`.
pub const fn transmit_descriptor(i: u16) -> u64 {
    TRANSMIT_RING + (i % TRANSMIT_ENTRIES) as u64 * DESCRIPTOR_BYTES
}

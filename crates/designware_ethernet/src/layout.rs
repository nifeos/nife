//! **Where each region of the DMA block sits**: the rings and buffers, as offsets from its start.
//!
//! Name: ratified 2026-11-03 (UTC), #3122.

use super::{BUFFER_BYTES, DESCRIPTOR_BYTES, PAGE, RECEIVE_ENTRIES, TRANSMIT_ENTRIES};
/// The receive descriptor ring.
pub(crate) const RECEIVE_RING: u64 = 0;
// Zero is load-bearing: `configure` writes the receive ring's base as `region + RECEIVE_RING`,
// and only while this is zero does that word read the same with the plus swapped for a minus.
const _: () = assert!(RECEIVE_RING == 0);
/// The transmit descriptor ring.
pub(crate) const TRANSMIT_RING: u64 = PAGE;
/// The first receive buffer.
pub(crate) const RECEIVE_BUFFERS: u64 = 2 * PAGE;
/// The first transmit buffer.
pub(crate) const TRANSMIT_BUFFERS: u64 = RECEIVE_BUFFERS + RECEIVE_ENTRIES as u64 * BUFFER_BYTES;
/// The size in bytes of the `DirectMemoryAccessRegion` the controller reads and writes.
///
/// Name: ratified 2026-11-03 (UTC), renamed from `BYTES`.
pub const DIRECT_MEMORY_ACCESS_REGION_BYTES: u64 =
    TRANSMIT_BUFFERS + TRANSMIT_ENTRIES as u64 * BUFFER_BYTES;
/// The size in pages of the `DirectMemoryAccessRegion`.
///
/// Name: ratified 2026-11-03 (UTC), renamed from `PAGES`.
pub const DIRECT_MEMORY_ACCESS_REGION_PAGES: u64 = DIRECT_MEMORY_ACCESS_REGION_BYTES.div_ceil(PAGE);

/// Receive buffer `index`'s offset. `index` is reduced modulo the ring so no input leaves the
/// region.
pub(crate) const fn receive_buffer(index: u16) -> u64 {
    RECEIVE_BUFFERS + (index % RECEIVE_ENTRIES) as u64 * BUFFER_BYTES
}
/// Transmit buffer `index`'s offset, reduced the same way.
pub(crate) const fn transmit_buffer(index: u16) -> u64 {
    TRANSMIT_BUFFERS + (index % TRANSMIT_ENTRIES) as u64 * BUFFER_BYTES
}
/// Receive descriptor `index`'s offset.
///
/// Name: ratified 2026-11-02 (UTC), #3094.
pub const fn receive_descriptor(index: u16) -> u64 {
    RECEIVE_RING + (index % RECEIVE_ENTRIES) as u64 * DESCRIPTOR_BYTES
}
/// Transmit descriptor `index`'s offset.
///
/// Name: ratified 2026-11-02 (UTC), #3094.
pub const fn transmit_descriptor(index: u16) -> u64 {
    TRANSMIT_RING + (index % TRANSMIT_ENTRIES) as u64 * DESCRIPTOR_BYTES
}

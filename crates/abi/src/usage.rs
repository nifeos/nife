//! Name: provisional.

/// Pages the region holds in total.
///
/// Name: provisional.
pub const SIZE: u64 = 0;
/// Pages spent so far: the watermark.
///
/// Name: provisional.
pub const COMMITTED: u64 = 1;
/// Plain pages over the subtree: mapped memory, page tables, image pages and revocation records.
///
/// Name: provisional.
pub const FRAMES: u64 = 2;
/// Pages retyped into rendezvous objects, over the subtree.
///
/// Name: provisional.
pub const RENDEZVOUS: u64 = 3;
/// Pages retyped into address-space roots, over the subtree.
///
/// Name: provisional.
pub const ADDRESS_SPACES: u64 = 4;
/// Pages retyped into thread control blocks, over the subtree.
///
/// Name: provisional.
pub const THREADS: u64 = 5;
/// Pages this region carved into child regions that are still live.
///
/// Name: provisional.
pub const CHILDREN: u64 = 6;

/// Whether this kernel answers a record, `survey::record::is_known`'s twin.
///
/// Name: provisional.
#[must_use]
pub const fn is_known(record: u64) -> bool {
    record <= CHILDREN
}

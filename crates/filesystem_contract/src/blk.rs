use super::PAGE;

/// Read [`req_blocks`] filesystem blocks starting at `w1` from the disk into the shared
/// region. Reply `r0` = 0 on success, or [`super::reply_err`] on failure; the bytes land at the
/// start of the shared region, block `w1` first.
pub const READ: u64 = 1;
/// Write [`req_blocks`] filesystem blocks, now in the shared region, to the disk starting at
/// `w1`. Reply `r0` = 0 on success, or [`super::reply_err`] on failure.
pub const WRITE: u64 = 2;
/// Ask the disk's size in bytes. `w1` ignored. Reply `r0` = the size (always non-negative here).
pub const SIZE: u64 = 3;
/// **Make everything the device has already acknowledged durable** (milestone 55 (Time Machine)):
/// a `VIRTIO_BLK_T_FLUSH` the block server does not reply to until the device completes it. `w1`
/// is 0 and ignored.
///
/// Reply `r0` = how many flushes this block server has completed since it started, this one
/// included, so a success is `>= 1` and strictly increases across calls. `EOPNOTSUPP` (95) when the
/// device did not offer `VIRTIO_BLK_F_FLUSH`; `EIO` (5) for any status but `VIRTIO_BLK_S_OK`,
/// `VIRTIO_BLK_S_UNSUPP` included. Both reach a file client unchanged through
/// [`super::fs::SYNC`]. See notes/fs-server/file-protocol.md.
pub const FLUSH: u64 = 4;

/// One filesystem block, in bytes: the transfer unit [`SIZE`] and [`FLUSH`] see and the unit
/// [`req_blocks`] counts in. Equal to `redoxfs::BLOCK_SIZE`; asserted against it in the block
/// server so a future RedoxFS bump that changed the block size would fail to build rather than
/// silently corrupt.
pub const BLOCK_SIZE: usize = PAGE;

/// The logical block size a partition table counts in: the 512-byte sector every disk this
/// project has met presents and the unit the virtio blk wire counts in. Nothing on the wire
/// reports it (`disk_surveyor`'s BUGS), so it is stated here once and every consumer takes it
/// from here, the shipping installer included, which depends on no test data.
/// Name: provisional (milestone 886); calef names public items.
pub const SECTOR_BYTES: u64 = 512;

/// The number of sectors in one filesystem block (`BLOCK_SIZE / SECTOR_BYTES = 8`).
pub const SECTORS_PER_BLOCK: u64 = (BLOCK_SIZE as u64) / SECTOR_BYTES;

/// **How many blocks the shared region spans, and the most one [`READ`] or [`WRITE`] may carry.**
/// Equal to `fs::TRANSFER_PAGES` on purpose, so a full file request fans out to one blk request
/// per RedoxFS `Disk::read_at`/`write_at` call.
///
/// The device DMAs straight into the region, so a larger request would overrun it. A caller that
/// sends [`super::req`]`(operation)`, whose low bits are zero, gets one block ([`req_blocks`]), so
/// the single-block callers (`disk_surveyor`, `disk_partitioner`, `mkfs`) need no change. See
/// notes/fs-server/file-protocol.md.
pub const TRANSFER_BLOCKS: usize = 16;

/// The largest payload one [`READ`] or [`WRITE`] may carry, in bytes: the whole shared region
/// ([`TRANSFER_BLOCKS`] blocks). The block server clamps every request's count to this.
pub const TRANSFER_MAX: usize = TRANSFER_BLOCKS * BLOCK_SIZE;

/// Pack a blk request's first word: opcode (bits 63:56, [`super::OPERATION_SHIFT`]) and a block count
/// (bits 7:0, stored as `blocks - 1` so the all-zero word a single-block caller sends decodes as
/// one block). `blocks` must be in `1..=TRANSFER_BLOCKS`.
pub const fn req(operation: u64, blocks: usize) -> u64 {
    assert!(blocks >= 1 && blocks <= TRANSFER_BLOCKS);
    (operation << super::OPERATION_SHIFT) | (blocks as u64 - 1)
}

/// The block count of a blk request word: [`req`]'s inverse. A word built by
/// [`super::req`]`(operation)`, as a single-block caller sends, decodes as 1.
pub const fn req_blocks(w0: u64) -> usize {
    ((w0 & 0xff) + 1) as usize
}

/// **The channel's arithmetic, checked by the build rather than by a test**, for
/// `fs::TRANSFER_MAX`'s reason: this contract has no test harness at EL0, and a build is the
/// only artifact that can be wrong about it there.
const _: () = {
    assert!(TRANSFER_BLOCKS >= 1);
    assert!(TRANSFER_BLOCKS <= 256); // the 8-bit count field's range
    assert!(TRANSFER_MAX == TRANSFER_BLOCKS * BLOCK_SIZE);
    // `req`'s all-zero-low-bits compatibility case: a caller that never heard of `req_blocks`
    // sends `super::req(operation)`, whose low bits are 0, and that must still mean one block.
    assert!(req_blocks(super::req(READ)) == 1);
};

#[cfg(test)]
mod tests;

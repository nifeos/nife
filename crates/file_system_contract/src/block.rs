use super::PAGE_BYTES;

/// The block protocol's version, one for this operation set and its reply shapes. Module-scoped,
/// not per crate: the file protocol moves on its own version. A break bumps this and owes a
/// dated ledger entry; an addition (a new opcode) owes no bump. The check lives at bind: a client
/// asks for the served versions and binds only when its own version is in the set.
///
/// Name: provisional, minted 2026-10-29 (UTC) by lane milestone/956-checkable-interface-versions.
pub const VERSION: u32 = 1;

/// This interface's identity in a program's `INTERFACES` note: 14, naming the block protocol. The
/// number is this interface's for good: no other interface takes it, and a format change is a new
/// version, never a new number. The project that links these crates keeps the table of numbers and holds it to that rule. A program declares the pair
/// of this id and [`VERSION`].
///
/// Name: provisional, minted 2026-11-01 (UTC) by lane lane/956-interface-ids-for-the-owed-formats.
pub const INTERFACE_ID: u64 = 14;

/// Read [`request_block_count`] filesystem blocks starting at `w1` from the disk into the shared
/// region. Reply `r0` = 0 on success, or [`super::error::word`] on failure; the bytes land at the
/// start of the shared region, block `w1` first.
pub const READ: u64 = 1;
/// Write [`request_block_count`] filesystem blocks, now in the shared region, to the disk starting at
/// `w1`. Reply `r0` = 0 on success, or [`super::error::word`] on failure.
pub const WRITE: u64 = 2;
/// Ask the disk's size in bytes. `w1` ignored. Reply `r0` = the size (always non-negative here).
///
/// Name: ratified 2026-11-22. A verb is a question and its name says what it asks (`GET_`) and in what unit (`_BYTES`). The
/// number, 3, is unchanged. Refused: `GET_CAPACITY_BYTES`, because the disk's capacity and its size are one number here and
/// "capacity" suggests a free count; `GET_SIZE`, because it leaves the unit to be guessed.
pub const GET_SIZE_BYTES: u64 = 3;
/// **Make everything the device has already acknowledged durable**:
/// a `VIRTUAL_IO_BLK_T_FLUSH` the block server does not reply to until the device completes it. `w1`
/// is 0 and ignored.
///
/// Reply `r0` = how many flushes this block server has completed since it started, this one
/// included, so a success is `>= 1` and strictly increases across calls. `EOPNOTSUPP` (95) when the
/// device did not offer `VIRTIO_BLK_F_FLUSH`; `EIO` (5) for any status but `VIRTIO_BLK_S_OK`,
/// `VIRTIO_BLK_S_UNSUPP` included. Both reach a file client unchanged through
/// [`super::file_system::SYNC`].
pub const FLUSH: u64 = 4;

/// One filesystem block, in bytes: the transfer unit [`GET_SIZE_BYTES`] and [`FLUSH`] see and the unit
/// [`request_block_count`] counts in. Equal to `redoxfs::BLOCK_BYTES`; asserted against it in the block
/// server so a future RedoxFS bump that changed the block size would fail to build rather than
/// silently corrupt.
///
/// Name: provisional, minted 2026-11-18 (UTC) by the lane of milestone 1090 (an outside review of `file_system_contract`).
pub const BLOCK_BYTES: usize = PAGE_BYTES;

/// The logical block size a partition table counts in, in bytes: the device's logical block size,
/// assumed to be 512 until a driver reports it. Nothing yet carries the driver's size to a
/// partitioner, so it is stated here once and every consumer takes it from here, an installer
/// included, which depends on no test data.
///
/// Name: ratified 2026-11-22.
pub const LOGICAL_BLOCK_BYTES: u64 = 512;

/// **How many blocks the shared region spans, and the most one [`READ`] or [`WRITE`] may carry.**
/// Equal to `file_system::TRANSFER_PAGES` on purpose, so a full file request fans out to one blk request
/// per RedoxFS `Disk::read_at`/`write_at` call.
///
/// The device reads and writes the region directly, by direct memory access, so a larger request would overrun it. A caller that
/// sends [`super::request`]`(operation)`, whose low bits are zero, gets one block ([`request_block_count`]), so
/// a caller that reads or writes one block at a time (a partition reader, a formatter) needs no
/// count.
pub const TRANSFER_BLOCKS: usize = 16;

/// The largest payload one [`READ`] or [`WRITE`] may carry, in bytes: the whole shared region
/// ([`TRANSFER_BLOCKS`] blocks). The block server clamps every request's count to this.
///
/// Name: ratified 2026-10-27 (#2797).
pub const MAXIMUM_TRANSFER_BYTES: usize = TRANSFER_BLOCKS * BLOCK_BYTES;

/// Pack a blk request's first word: opcode (bits 63:56, [`super::OPERATION_SHIFT`]) and a block count
/// (bits 7:0, stored as `blocks - 1` so the all-zero word a single-block caller sends decodes as
/// one block). `blocks` must be in `1..=TRANSFER_BLOCKS`.
///
/// Name: provisional, minted 2026-11-21 (UTC) by the request-names lane of milestone 1090 (an outside review of `file_system_contract`).
pub const fn request(operation: u64, blocks: usize) -> u64 {
    assert!(blocks >= 1 && blocks <= TRANSFER_BLOCKS);
    (operation << super::OPERATION_SHIFT) | (blocks as u64 - 1)
}

/// The block count of a blk request word: [`request`]'s inverse. A word built by
/// [`super::request`]`(operation)`, as a single-block caller sends, decodes as 1.
///
/// Name: provisional, minted 2026-11-21 (UTC) by the request-names lane of milestone 1090 (an outside review of `file_system_contract`).
pub const fn request_block_count(w0: u64) -> usize {
    ((w0 & 0xff) + 1) as usize
}

/// **The channel's arithmetic, checked by the build rather than by a test**, for
/// `file_system::MAXIMUM_TRANSFER_BYTES`'s reason: this contract has no test harness in an unprivileged process, and a build is the
/// only artifact that can be wrong about it there.
const _: () = {
    assert!(TRANSFER_BLOCKS >= 1);
    assert!(TRANSFER_BLOCKS <= 256); // the 8-bit count field's range
    assert!(MAXIMUM_TRANSFER_BYTES == TRANSFER_BLOCKS * BLOCK_BYTES);
    // `request`'s all-zero-low-bits compatibility case: a caller that never heard of `request_block_count`
    // sends `super::request(operation)`, whose low bits are 0, and that must still mean one block.
    assert!(request_block_count(super::request(READ)) == 1);
};

#[cfg(test)]
mod tests;

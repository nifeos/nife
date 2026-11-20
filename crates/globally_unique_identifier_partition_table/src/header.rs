//! The GPT header: 92 bytes at LBA 1, and the same 92 bytes again at the disk's last LBA.
//!
//! [`Header::decode`] answers only "are these bytes a GPT header?". It knows neither the disk's
//! size nor which copy it holds, so every judgement that needs the disk is in
//! [`crate::PartitionTable`], and the backup decodes on its own terms
//! before anything compares it with the primary.

use crate::crc::crc32_pieces;
use crate::guid::{self, Uuid};
use crate::{Error, is_block_size_ok};

/// `EFI PART`, the eight bytes at offset 0 of every GPT header.
pub const SIGNATURE: [u8; 8] = *b"EFI PART";

/// GPT revision 1.0, encoded as `0x00010000` (major in the high 16 bits). The only revision that
/// has ever existed, and the only one this crate accepts: a revision we do not know is a header
/// whose fields might not be where we think they are.
///
/// Name: recorded (the UEFI specification's revision field).
pub const REVISION: u32 = 0x0001_0000;

/// The size of the defined part of the header in bytes, and the size this crate writes. A header
/// may declare more, up to its block (the spec reserves room to grow); [`Header::decode`] accepts
/// that, covers the extra bytes with the CRC and otherwise ignores them.
///
/// Name: ratified 2026-11-20 (#3864).
pub(crate) const HEADER_BYTES: u32 = 92;

/// Field offsets within the header, in the order UEFI 2.10 Table 5.5 lists them. Written out
/// because a run of `u64::from_le_bytes` calls with literal indices is unreadable and one
/// transposed digit is a silent bug.
mod at {
    pub const SIGNATURE: usize = 0;
    pub(crate) const REVISION: usize = 8;
    pub(crate) const HEADER_BYTES: usize = 12;
    pub(crate) const HEADER_CRC: usize = 16;
    pub const RESERVED: usize = 20;
    pub(crate) const MY_LOGICAL_BLOCK_ADDRESS: usize = 24;
    /// Name: ratified 2026-11-20 (#3864).
    pub const ALTERNATE_LOGICAL_BLOCK_ADDRESS: usize = 32;
    /// Name: ratified 2026-11-20 (#3864).
    pub const FIRST_USABLE_LOGICAL_BLOCK_ADDRESS: usize = 40;
    /// Name: ratified 2026-11-20 (#3864).
    pub const LAST_USABLE_LOGICAL_BLOCK_ADDRESS: usize = 48;
    pub(crate) const DISK_GUID: usize = 56;
    pub(crate) const ENTRY_ARRAY_LOGICAL_BLOCK_ADDRESS: usize = 72;
    pub(crate) const ENTRY_COUNT: usize = 80;
    pub(crate) const ENTRY_BYTES: usize = 84;
    pub(crate) const ENTRY_ARRAY_CRC: usize = 88;
}

/// A decoded GPT header.
///
/// Field for field with the on-disk structure, minus the four that are not information: the
/// signature and revision are constants once [`Header::decode`] has accepted, the reserved word is
/// zero, and the header CRC is recomputed on write rather than stored (a struct that carried a
/// stale CRC would be a footgun with no upside).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Header {
    /// The LBA this header was read from, according to the header. 1 for a primary, the last block
    /// of the disk for a backup. Checked against where it actually came from by
    /// [`crate::PartitionTable`], which is what makes a header that was
    /// copied to the wrong place detectable.
    ///
    /// Name: ratified 2026-11-20 (#3864).
    #[doc(alias = "MyLBA")]
    pub my_logical_block_address: u64,
    /// Where the *other* copy of this header lives.
    ///
    /// Name: ratified 2026-11-20 (#3864).
    #[doc(alias = "AlternateLBA")]
    pub alternate_logical_block_address: u64,
    /// The first block a partition may occupy. Everything below it is table.
    ///
    /// Name: ratified 2026-11-20 (#3864).
    #[doc(alias = "FirstUsableLBA")]
    pub first_usable_logical_block_address: u64,
    /// The last block a partition may occupy, inclusive. Everything above it is backup table.
    ///
    /// Name: ratified 2026-11-20 (#3864).
    #[doc(alias = "LastUsableLBA")]
    pub last_usable_logical_block_address: u64,
    /// Names the disk, not any partition on it.
    #[doc(alias = "DiskGUID")]
    pub disk_guid: Uuid,
    /// Where this header's copy of the partition entry array starts.
    ///
    /// Name: ratified 2026-11-20 (#3864).
    #[doc(alias = "PartitionEntryLBA")]
    pub entry_array_logical_block_address: u64,
    /// How many entries the array has room for, used or not. 128 in practice, always.
    #[doc(alias = "NumberOfPartitionEntries")]
    pub entry_count: u32,
    /// Bytes per entry. 128 in practice; the spec allows any multiple of 8 that is at least 128, so
    /// that entries can grow.
    ///
    /// Name: ratified 2026-11-20 (#3864).
    #[doc(alias = "SizeOfPartitionEntry")]
    pub entry_bytes: u32,
    /// CRC-32 over `entry_count * entry_bytes` bytes of the entry array.
    #[doc(alias = "PartitionEntryArrayCRC32")]
    pub entry_array_crc: u32,
}

impl Header {
    /// Decode one block into a header, checking everything that can be checked without knowing what
    /// disk it came from.
    ///
    /// `block` is one block, 512 to 4096 bytes and a power of two; its length is the block size.
    /// Refuses a bad block size, a wrong signature, a revision other than [`REVISION`], a declared
    /// header size outside `92 ..= block_bytes` (the spec's bound), a non-zero reserved word at
    /// offset 20 or non-zero byte after the header (UEFI 2.10 section 5.3.2 requires both zero; the
    /// strictest check here), and a header CRC that does not match over `header_size` bytes with
    /// the CRC field taken as zero. A size over 92 is accepted, as the spec asks of a reader that
    /// does not know a later revision.
    pub fn decode(block: &[u8]) -> Result<Header, Error> {
        let header_size = Header::decode_checks(block)?;

        let stored = le32(block, at::HEADER_CRC);
        let computed = header_crc(&block[..header_size]);
        if stored != computed {
            return Err(Error::HeaderCrc { stored, computed });
        }

        Ok(Header::decode_fields(block))
    }

    /// The checks [`Header::decode`] makes before the CRC, returning the declared header size.
    ///
    /// Split out from `decode` for the same reason as [`Header::decode_fields`]: these checks are
    /// proved total on any symbolic block, and the CRC that follows them is proved separately (its
    /// piece-wise form equals the whole), because one harness through both would reason about a
    /// polynomial over symbolic bytes. The size returned is in `92 ..= block.len()`. See
    /// `verification::the_header_checks_before_the_crc_are_total`.
    ///
    /// Name: ratified 2026-11-20 (#3864).
    pub(crate) fn decode_checks(block: &[u8]) -> Result<usize, Error> {
        if !is_block_size_ok(block.len()) {
            return Err(Error::BlockSize(block.len()));
        }
        if block[at::SIGNATURE..at::SIGNATURE + 8] != SIGNATURE {
            return Err(Error::Signature);
        }
        let revision = le32(block, at::REVISION);
        if revision != REVISION {
            return Err(Error::Revision(revision));
        }
        let header_size = le32(block, at::HEADER_BYTES);
        if header_size < HEADER_BYTES || header_size as usize > block.len() {
            return Err(Error::HeaderSize(header_size));
        }
        if le32(block, at::RESERVED) != 0 {
            return Err(Error::ReservedNotZero);
        }
        // Every byte is folded in, with the size as a mask rather than a slice start or an early
        // exit, so the model checker sees one branch-free loop of fixed length
        // (`the_header_checks_before_the_crc_are_total`).
        let tail = header_size as usize;
        let past_header = block
            .iter()
            .enumerate()
            .fold(0u8, |seen, (i, &b)| seen | if i >= tail { b } else { 0 });
        if past_header != 0 {
            return Err(Error::BlockTailNotZero);
        }
        Ok(header_size as usize)
    }

    /// The field half of [`Header::decode`]: read the nine fields out of a block, judging nothing.
    ///
    /// Total on any slice of at least [`HEADER_BYTES`] bytes, and the exact inverse of
    /// [`Header::encode_fields`]. Split out from `decode` so the **layout** can be proved
    /// separately from the **integrity**, which is what keeps the layout proof affordable: a
    /// harness that went through the CRC would have to reason about a 92-byte polynomial over
    /// symbolic bytes, which costs minutes of solver time to prove a fact about byte offsets. See
    /// `verification::a_headers_fields_survive_the_round_trip`.
    pub(crate) fn decode_fields(block: &[u8]) -> Header {
        Header {
            my_logical_block_address: le64(block, at::MY_LOGICAL_BLOCK_ADDRESS),
            alternate_logical_block_address: le64(block, at::ALTERNATE_LOGICAL_BLOCK_ADDRESS),
            first_usable_logical_block_address: le64(block, at::FIRST_USABLE_LOGICAL_BLOCK_ADDRESS),
            last_usable_logical_block_address: le64(block, at::LAST_USABLE_LOGICAL_BLOCK_ADDRESS),
            disk_guid: guid::from_disk(
                block[at::DISK_GUID..at::DISK_GUID + 16].try_into().unwrap(),
            ),
            entry_array_logical_block_address: le64(block, at::ENTRY_ARRAY_LOGICAL_BLOCK_ADDRESS),
            entry_count: le32(block, at::ENTRY_COUNT),
            entry_bytes: le32(block, at::ENTRY_BYTES),
            entry_array_crc: le32(block, at::ENTRY_ARRAY_CRC),
        }
    }

    /// Write this header into one block, zeroing the block first and computing the header CRC last.
    ///
    /// Always emits a 92-byte header, so `decode(encode(h)) == h` for every header this crate can
    /// hold. A header that declared a larger size, which the specification permits, is rewritten as
    /// 92 bytes: a rewrite normalizes it. The block is zeroed rather than overwritten in place because the reserved tail must be
    /// zero and a caller handing us a buffer with a previous header in it is the normal case.
    pub fn encode_into(&self, block: &mut [u8]) -> Result<(), Error> {
        if !is_block_size_ok(block.len()) {
            return Err(Error::BlockSize(block.len()));
        }
        self.encode_fields(block);
        let crc = header_crc(&block[..HEADER_BYTES as usize]);
        put32(block, at::HEADER_CRC, crc);
        Ok(())
    }

    /// The field half of [`Header::encode_into`]: everything except the header's own CRC, which is
    /// left as the four zero bytes the CRC is defined to be computed over.
    ///
    /// The split is what makes the round trip provable in two independent pieces, and the two
    /// pieces compose for a reason that is three lines of source rather than an assumption: the
    /// only thing `encode_into` adds is four bytes at offset 16, the only thing `decode` adds is
    /// checks, and **the CRC field is not one of the nine fields**, so the two halves cannot
    /// interfere with each other.
    pub(crate) fn encode_fields(&self, block: &mut [u8]) {
        block.fill(0);
        block[at::SIGNATURE..at::SIGNATURE + 8].copy_from_slice(&SIGNATURE);
        put32(block, at::REVISION, REVISION);
        put32(block, at::HEADER_BYTES, HEADER_BYTES);
        // at::HEADER_CRC and at::RESERVED stay zero: the first is filled in by the caller after
        // the CRC is computed, the second is reserved and must stay zero on disk.
        put64(
            block,
            at::MY_LOGICAL_BLOCK_ADDRESS,
            self.my_logical_block_address,
        );
        put64(
            block,
            at::ALTERNATE_LOGICAL_BLOCK_ADDRESS,
            self.alternate_logical_block_address,
        );
        put64(
            block,
            at::FIRST_USABLE_LOGICAL_BLOCK_ADDRESS,
            self.first_usable_logical_block_address,
        );
        put64(
            block,
            at::LAST_USABLE_LOGICAL_BLOCK_ADDRESS,
            self.last_usable_logical_block_address,
        );
        block[at::DISK_GUID..at::DISK_GUID + 16].copy_from_slice(&guid::to_disk(self.disk_guid));
        put64(
            block,
            at::ENTRY_ARRAY_LOGICAL_BLOCK_ADDRESS,
            self.entry_array_logical_block_address,
        );
        put32(block, at::ENTRY_COUNT, self.entry_count);
        put32(block, at::ENTRY_BYTES, self.entry_bytes);
        put32(block, at::ENTRY_ARRAY_CRC, self.entry_array_crc);
    }

    /// The number of bytes of entry array this header describes: `entry_count * entry_bytes`, the
    /// exact span the array CRC covers.
    ///
    /// `None` on overflow, which a hostile header can cause with two `u32`s (`0xFFFFFFFF` each
    /// multiplies out past `usize` on a 32-bit target). Returning an `Option` rather than
    /// saturating keeps the caller from silently CRC-ing a shorter buffer than the header claimed.
    pub(crate) fn entry_array_bytes(&self) -> Option<usize> {
        usize::try_from(self.entry_count as u64 * self.entry_bytes as u64).ok()
    }
}

/// The header CRC, computed the way the spec defines it: over `header_size` bytes, with the four
/// bytes of the CRC field itself treated as zero.
///
/// The self-referential field is the fiddly part of every CRC-protected header format. The obvious
/// way is to copy the header, zero four bytes and CRC the copy, which puts a block-sized buffer on
/// a kernel stack to change four bytes. Instead the CRC is taken in three pieces, which is the same
/// computation and no buffer.
fn header_crc(header: &[u8]) -> u32 {
    crc32_pieces(&[
        &header[..at::HEADER_CRC],
        &[0, 0, 0, 0],
        &header[at::HEADER_CRC + 4..],
    ])
}

fn le32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn le64(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())
}

fn put32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn put64(bytes: &mut [u8], at: usize, value: u64) {
    bytes[at..at + 8].copy_from_slice(&value.to_le_bytes());
}

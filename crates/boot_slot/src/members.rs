//! **The version 2 slot header: a slot that names its members.** Version 1 describes one image.
//! Version 2 describes three files, the kernel, the composed archive and the system manifest, each
//! by offset, length and SHA-256 digest, so an updater can write each from the package that
//! produced it. The layout was ruled on 2026-11-01 (UTC); see
//! `design/roadmap/proposals/what-an-os-update-writes-into-a-slot.md`, question 3. Pure
//! computation over byte slices, no allocation and no I/O.
//!
//! | offset | size | |
//! |---|---|---|
//! | 0 | 8 | [`SLOT_MAGIC`] |
//! | 8 | 4 | [`MEMBERS_VERSION`], little-endian |
//! | 12 | 4 | reserved, zero |
//! | 16 | 56 each | three entries: tag 8, offset 8, length 8, SHA-256 digest 32 |
//! | 184 | 4 | CRC-32 of bytes 0 to 183, little-endian |
//! | 188 | 3908 | zero, to the end of the [`SLOT_IMAGE_OFFSET`] block |
//!
//! Members start at multiples of [`SLOT_IMAGE_OFFSET`], in table order, without overlap, inside
//! the partition. Every refusal is a [`Refusal`] with a fixed phrase for the chooser to print.
//!
//! # BUGS
//!
//! - The parser checks structure, not content. Whether a member's bytes hash to its digest is the
//!   caller's check, after it reads the member; this crate links no hash function.
//! - A zero-length member is refused as a layout break. The ruling does not say; refusing is the
//!   stricter reading, and allowing one later is compatible.
//! - A member may not start inside the header block (offset below 4096). The ruling's "without
//!   overlap" covers it; it is stated here because the header is the first thing in the table.
//! - [`Refusal::MissingTag`] cannot occur for a three-entry table: with no unknown tag and no
//!   duplicate, all three tags are present. The check stays so that a fourth tag, added behind a
//!   new version, cannot silently pass a table without it.
//! - A fuzz target for [`parse`] is not added: `.github/ci-job-budgets` holds the fuzz job at
//!   eighteen targets, and a nineteenth means splitting the job into shards. The byte-flip tests
//!   in this crate cover every byte of the 188 and a sample of the padding on the host.
//!
//! Name: provisional. The module, [`parse`], [`ParsedHeader`], [`MemberHeader`], [`Member`],
//! [`MemberTag`], [`Refusal`] and the tag spellings are all unratified.

use globally_unique_identifier_partition_table::crc;

use crate::{SLOT_IMAGE_OFFSET, SLOT_MAGIC, SLOT_VERSION, SlotHeader};

/// The layout version this module writes, and the one it reads beside version 1.
pub const MEMBERS_VERSION: u32 = 2;

/// Bytes of the version 2 header that carry anything; the rest of the block is zero.
pub const MEMBERS_HEADER_BYTES: usize = 188;

/// How many members a version 2 header holds. There is no count field: the number is the layout.
pub const MEMBER_COUNT: usize = 3;

/// Bytes of one table entry: tag 8, offset 8, length 8, digest 32.
const ENTRY_BYTES: usize = 56;
/// Where the table starts.
const TABLE_START: usize = 16;
/// Where the CRC-32 sits; it covers everything before it.
const CRC_START: usize = TABLE_START + MEMBER_COUNT * ENTRY_BYTES;
/// Bytes of the reserved field (12 to 15).
const RESERVED: core::ops::Range<usize> = 12..16;
/// The block size as a `usize`, for slicing.
const BLOCK: usize = SLOT_IMAGE_OFFSET as usize;

const _: () = assert!(CRC_START == 184 && CRC_START + 4 == MEMBERS_HEADER_BYTES);

/// **Which file a table entry names.** A closed set; any other 8 bytes is [`Refusal::UnknownTag`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MemberTag {
    /// The kernel image.
    Kernel,
    /// The composed archive of base programs.
    Archive,
    /// The system manifest, which lists each package by digest.
    Manifest,
}

impl MemberTag {
    /// All three, in the order the updater writes them.
    pub const ALL: [MemberTag; MEMBER_COUNT] =
        [MemberTag::Kernel, MemberTag::Archive, MemberTag::Manifest];

    /// The 8 ASCII bytes in the table, zero-padded on the right. Provisional spellings.
    pub const fn bytes(self) -> [u8; 8] {
        match self {
            MemberTag::Kernel => *b"KERNEL\0\0",
            MemberTag::Archive => *b"ARCHIVE\0",
            MemberTag::Manifest => *b"MANIFEST",
        }
    }

    /// The tag these 8 bytes spell, or `None` for any other value.
    pub fn from_bytes(bytes: [u8; 8]) -> Option<MemberTag> {
        MemberTag::ALL.into_iter().find(|t| t.bytes() == bytes)
    }
}

/// **One file inside the slot**: where it starts, how long it is, and the SHA-256 of its bytes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Member {
    /// Which file this is.
    pub tag: MemberTag,
    /// Byte offset from the start of the partition; a multiple of 4096.
    pub offset: u64,
    /// Length in bytes.
    pub length: u64,
    /// SHA-256 of exactly those bytes.
    pub digest: [u8; 32],
}

/// **The decoded version 2 header**: the three members in table order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MemberHeader {
    /// The table, in the order the entries sit in the header (and so in the partition).
    pub members: [Member; MEMBER_COUNT],
}

/// **A header this module reads**: version 1's single image or version 2's three members.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ParsedHeader {
    /// A version 1 header, as [`SlotHeader::decode`] reads it.
    One(SlotHeader),
    /// A version 2 header.
    Two(MemberHeader),
}

/// **Why a slot header was refused.** The chooser skips the slot for that boot and prints
/// [`Refusal::phrase`]. The phrases are fixed text, so a line in a console log names its cause.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refusal {
    /// The magic is absent or the buffer is too short to hold it: an empty or foreign partition.
    NoHeader,
    /// The magic and version are present but the buffer is shorter than that version's header.
    Truncated,
    /// The magic matches and the version is neither 1 nor 2.
    UnknownVersion,
    /// A table entry's tag is not KERNEL, ARCHIVE or MANIFEST.
    UnknownTag,
    /// Two table entries carry the same tag.
    DuplicateTag,
    /// A tag of the closed set has no table entry.
    MissingTag,
    /// A member breaks the layout: an offset that is not a multiple of 4096 or that falls inside
    /// the header block, an overlap, an entry out of table order, a zero length, or a member that
    /// ends past the partition.
    LayoutBreak,
    /// A reserved or padding byte is not zero.
    NonzeroReservedBytes,
    /// The header's CRC-32 does not match its bytes.
    CrcMismatch,
}

impl Refusal {
    /// **The phrase the chooser prints** for this refusal. Fixed text; provisional wording.
    pub const fn phrase(self) -> &'static str {
        match self {
            Refusal::NoHeader => "no slot header",
            Refusal::Truncated => "slot header truncated",
            Refusal::UnknownVersion => "unknown slot header version",
            Refusal::UnknownTag => "unknown member tag",
            Refusal::DuplicateTag => "duplicate member tag",
            Refusal::MissingTag => "missing member tag",
            Refusal::LayoutBreak => "member layout break",
            Refusal::NonzeroReservedBytes => "nonzero reserved or padding bytes",
            Refusal::CrcMismatch => "slot header checksum mismatch",
        }
    }
}

fn read_u32(bytes: &[u8], at: usize) -> u32 {
    let mut word = [0u8; 4];
    word.copy_from_slice(&bytes[at..at + 4]);
    u32::from_le_bytes(word)
}

fn read_u64(bytes: &[u8], at: usize) -> u64 {
    let mut word = [0u8; 8];
    word.copy_from_slice(&bytes[at..at + 8]);
    u64::from_le_bytes(word)
}

/// The tag check on a list of raw tags: unknown first, then duplicate, then missing. Takes a
/// slice so that the missing case, unreachable for three entries, stays testable.
fn check_tags(tags: &[[u8; 8]]) -> Result<(), Refusal> {
    for tag in tags {
        if MemberTag::from_bytes(*tag).is_none() {
            return Err(Refusal::UnknownTag);
        }
    }
    for (i, tag) in tags.iter().enumerate() {
        if tags[..i].contains(tag) {
            return Err(Refusal::DuplicateTag);
        }
    }
    for wanted in MemberTag::ALL {
        if !tags.contains(&wanted.bytes()) {
            return Err(Refusal::MissingTag);
        }
    }
    Ok(())
}

/// The layout check: aligned, past the header block, in table order, no overlap, inside the
/// partition. Arithmetic is checked, so an offset near `u64::MAX` is a break and not a wrap.
fn check_layout(members: &[Member], partition_bytes: u64) -> Result<(), Refusal> {
    let mut floor = SLOT_IMAGE_OFFSET;
    for m in members {
        if m.offset % SLOT_IMAGE_OFFSET != 0 || m.offset < floor || m.length == 0 {
            return Err(Refusal::LayoutBreak);
        }
        let end = m.offset.checked_add(m.length).ok_or(Refusal::LayoutBreak)?;
        if end > partition_bytes {
            return Err(Refusal::LayoutBreak);
        }
        floor = end;
    }
    Ok(())
}

impl MemberHeader {
    /// **Write the header into the start of a slot's header block**, zeroing all 4096 bytes first
    /// so the padding is zero by construction. Refuses, writing nothing, a table that
    /// [`parse`] would refuse, and a buffer shorter than one block ([`Refusal::Truncated`]).
    ///
    /// This is the only writer: the updater writes version 2 and nothing else.
    pub fn encode(&self, out: &mut [u8], partition_bytes: u64) -> Result<(), Refusal> {
        if out.len() < BLOCK {
            return Err(Refusal::Truncated);
        }
        let tags = self.members.map(|m| m.tag.bytes());
        check_tags(&tags)?;
        check_layout(&self.members, partition_bytes)?;
        out[..BLOCK].fill(0);
        out[0..8].copy_from_slice(&SLOT_MAGIC);
        out[8..12].copy_from_slice(&MEMBERS_VERSION.to_le_bytes());
        for (i, m) in self.members.iter().enumerate() {
            let at = TABLE_START + i * ENTRY_BYTES;
            out[at..at + 8].copy_from_slice(&m.tag.bytes());
            out[at + 8..at + 16].copy_from_slice(&m.offset.to_le_bytes());
            out[at + 16..at + 24].copy_from_slice(&m.length.to_le_bytes());
            out[at + 24..at + 56].copy_from_slice(&m.digest);
        }
        let own = crc::crc32(&out[0..CRC_START]);
        out[CRC_START..CRC_START + 4].copy_from_slice(&own.to_le_bytes());
        Ok(())
    }
}

/// **Read a slot's header block**, version 1 or 2, given the partition's length in bytes.
///
/// `bytes` is the start of the partition; a version 2 header needs the whole 4096-byte block.
/// The checks run in this order, and the first failure is the refusal: magic, version, length,
/// CRC-32, reserved and padding bytes, tags, layout. A version 1 header is read exactly as
/// [`SlotHeader::decode`] reads it; its only refusals are the first four.
///
/// # Examples
///
/// ```
/// use boot_slot::members::{Member, MemberHeader, MemberTag, ParsedHeader, Refusal, parse};
///
/// let members = MemberTag::ALL.map(|tag| Member {
///     tag,
///     offset: 4096 * (1 + tag as u64),
///     length: 100,
///     digest: [7; 32],
/// });
/// let header = MemberHeader { members };
/// let mut block = [0u8; 4096];
/// header.encode(&mut block, 1 << 20).unwrap();
/// assert_eq!(parse(&block, 1 << 20), Ok(ParsedHeader::Two(header)));
///
/// // A partition too small for the last member refuses the slot with a printable reason.
/// let refusal = parse(&block, 4096 * 3).unwrap_err();
/// assert_eq!(refusal, Refusal::LayoutBreak);
/// assert_eq!(refusal.phrase(), "member layout break");
/// ```
pub fn parse(bytes: &[u8], partition_bytes: u64) -> Result<ParsedHeader, Refusal> {
    if bytes.len() < 12 || bytes[0..8] != SLOT_MAGIC {
        return Err(Refusal::NoHeader);
    }
    match read_u32(bytes, 8) {
        SLOT_VERSION => parse_one(bytes),
        MEMBERS_VERSION => parse_two(bytes, partition_bytes),
        _ => Err(Refusal::UnknownVersion),
    }
}

fn parse_one(bytes: &[u8]) -> Result<ParsedHeader, Refusal> {
    if bytes.len() < crate::SLOT_HEADER_BYTES {
        return Err(Refusal::Truncated);
    }
    match SlotHeader::decode(bytes) {
        Some(header) => Ok(ParsedHeader::One(header)),
        None => Err(Refusal::CrcMismatch),
    }
}

fn parse_two(bytes: &[u8], partition_bytes: u64) -> Result<ParsedHeader, Refusal> {
    if bytes.len() < BLOCK {
        return Err(Refusal::Truncated);
    }
    if read_u32(bytes, CRC_START) != crc::crc32(&bytes[0..CRC_START]) {
        return Err(Refusal::CrcMismatch);
    }
    if bytes[RESERVED].iter().any(|b| *b != 0)
        || bytes[MEMBERS_HEADER_BYTES..BLOCK].iter().any(|b| *b != 0)
    {
        return Err(Refusal::NonzeroReservedBytes);
    }
    let mut tags = [[0u8; 8]; MEMBER_COUNT];
    for (i, tag) in tags.iter_mut().enumerate() {
        let at = TABLE_START + i * ENTRY_BYTES;
        tag.copy_from_slice(&bytes[at..at + 8]);
    }
    check_tags(&tags)?;
    let mut members = [Member {
        tag: MemberTag::Kernel,
        offset: 0,
        length: 0,
        digest: [0; 32],
    }; MEMBER_COUNT];
    for (i, m) in members.iter_mut().enumerate() {
        let at = TABLE_START + i * ENTRY_BYTES;
        m.tag = MemberTag::from_bytes(tags[i]).ok_or(Refusal::UnknownTag)?;
        m.offset = read_u64(bytes, at + 8);
        m.length = read_u64(bytes, at + 16);
        m.digest.copy_from_slice(&bytes[at + 24..at + 56]);
    }
    check_layout(&members, partition_bytes)?;
    Ok(ParsedHeader::Two(MemberHeader { members }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PARTITION: u64 = 64 << 20;

    fn sample() -> MemberHeader {
        MemberHeader {
            members: [
                Member {
                    tag: MemberTag::Kernel,
                    offset: 4096,
                    length: 5000,
                    digest: [1; 32],
                },
                Member {
                    tag: MemberTag::Archive,
                    offset: 12288,
                    length: 4096,
                    digest: [2; 32],
                },
                Member {
                    tag: MemberTag::Manifest,
                    offset: 16384,
                    length: 1,
                    digest: [3; 32],
                },
            ],
        }
    }

    fn block_of(header: &MemberHeader) -> [u8; BLOCK] {
        let mut block = [0u8; BLOCK];
        header.encode(&mut block, PARTITION).unwrap();
        block
    }

    /// Recompute the CRC after a hand edit, so a test reaches the check after the CRC.
    fn reseal(block: &mut [u8; BLOCK]) {
        let own = crc::crc32(&block[0..CRC_START]);
        block[CRC_START..CRC_START + 4].copy_from_slice(&own.to_le_bytes());
    }

    #[test]
    fn a_version_two_header_round_trips_and_fills_exactly_188_bytes() {
        let header = sample();
        let block = block_of(&header);
        assert_eq!(parse(&block, PARTITION), Ok(ParsedHeader::Two(header)));
        assert!(block[MEMBERS_HEADER_BYTES..].iter().all(|b| *b == 0));
        assert_ne!(
            &block[MEMBERS_HEADER_BYTES - 4..MEMBERS_HEADER_BYTES],
            &[0; 4]
        );
        assert_eq!(&block[0..8], b"NIFESLOT");
        assert_eq!(&block[8..12], &2u32.to_le_bytes());
        assert_eq!(&block[16..24], b"KERNEL\0\0");
        assert_eq!(&block[72..80], b"ARCHIVE\0");
        assert_eq!(&block[128..136], b"MANIFEST");
    }

    #[test]
    fn the_table_may_list_the_tags_in_any_order() {
        let mut header = sample();
        header.members.reverse();
        for (i, m) in header.members.iter_mut().enumerate() {
            m.offset = 4096 * (1 + 2 * i as u64);
        }
        let block = block_of(&header);
        assert_eq!(parse(&block, PARTITION), Ok(ParsedHeader::Two(header)));
    }

    #[test]
    fn a_version_one_header_still_reads_as_before() {
        let image = b"an image";
        let v1 = SlotHeader::of(image);
        let mut block = [0u8; BLOCK];
        assert!(v1.encode(&mut block));
        assert_eq!(parse(&block, PARTITION), Ok(ParsedHeader::One(v1)));
        assert_eq!(SlotHeader::decode(&block), Some(v1));
        // Just the 32 bytes also reads, as it did.
        assert_eq!(parse(&block[..32], PARTITION), Ok(ParsedHeader::One(v1)));
        block[20] ^= 1;
        assert_eq!(parse(&block, PARTITION), Err(Refusal::CrcMismatch));
        assert_eq!(parse(&block[..20], PARTITION), Err(Refusal::Truncated));
    }

    #[test]
    fn an_empty_or_foreign_partition_has_no_header() {
        assert_eq!(parse(&[0u8; BLOCK], PARTITION), Err(Refusal::NoHeader));
        assert_eq!(parse(&[], PARTITION), Err(Refusal::NoHeader));
        assert_eq!(parse(b"NIFESLOT", PARTITION), Err(Refusal::NoHeader));
    }

    #[test]
    fn an_unknown_version_is_refused_once_the_magic_matches() {
        for version in [0u32, 3, 0x0200_0000, u32::MAX] {
            let mut block = block_of(&sample());
            block[8..12].copy_from_slice(&version.to_le_bytes());
            assert_eq!(parse(&block, PARTITION), Err(Refusal::UnknownVersion));
        }
    }

    #[test]
    fn a_short_version_two_buffer_is_truncated() {
        let block = block_of(&sample());
        assert_eq!(
            parse(&block[..BLOCK - 1], PARTITION),
            Err(Refusal::Truncated)
        );
        assert_eq!(parse(&block[..12], PARTITION), Err(Refusal::Truncated));
    }

    #[test]
    fn a_wrong_crc_is_refused() {
        let mut block = block_of(&sample());
        block[CRC_START] ^= 0xFF;
        assert_eq!(parse(&block, PARTITION), Err(Refusal::CrcMismatch));
    }

    #[test]
    fn nonzero_reserved_bytes_are_refused() {
        for at in RESERVED {
            let mut block = block_of(&sample());
            block[at] = 1;
            reseal(&mut block);
            assert_eq!(parse(&block, PARTITION), Err(Refusal::NonzeroReservedBytes));
        }
    }

    #[test]
    fn nonzero_padding_is_refused_wherever_it_sits() {
        for at in [MEMBERS_HEADER_BYTES, 189, 1000, BLOCK - 1] {
            let mut block = block_of(&sample());
            block[at] = 1;
            assert_eq!(
                parse(&block, PARTITION),
                Err(Refusal::NonzeroReservedBytes),
                "{at}"
            );
        }
    }

    #[test]
    fn an_unknown_tag_is_refused() {
        for bad in [*b"KERNEL\0X", *b"kernel\0\0", [0; 8], *b"INITRD\0\0"] {
            let mut block = block_of(&sample());
            block[16..24].copy_from_slice(&bad);
            reseal(&mut block);
            assert_eq!(parse(&block, PARTITION), Err(Refusal::UnknownTag));
        }
    }

    #[test]
    fn a_duplicate_tag_is_refused() {
        let mut block = block_of(&sample());
        block[72..80].copy_from_slice(b"KERNEL\0\0");
        reseal(&mut block);
        assert_eq!(parse(&block, PARTITION), Err(Refusal::DuplicateTag));
    }

    #[test]
    fn a_missing_tag_is_refused_by_the_tag_check() {
        // Three entries cannot reach this, so the check is held on its own.
        let two = [MemberTag::Kernel.bytes(), MemberTag::Archive.bytes()];
        assert_eq!(check_tags(&two), Err(Refusal::MissingTag));
        assert_eq!(check_tags(&[]), Err(Refusal::MissingTag));
        let all = MemberTag::ALL.map(MemberTag::bytes);
        assert_eq!(check_tags(&all), Ok(()));
    }

    #[test]
    fn every_layout_break_is_refused() {
        type Edit = fn(&mut MemberHeader);
        let breaks: [(&str, Edit); 9] = [
            ("unaligned offset", |h| h.members[0].offset = 4097),
            ("inside the header block", |h| h.members[0].offset = 0),
            ("overlap", |h| h.members[1].offset = 8192),
            ("out of table order", |h| h.members[1].offset = 4096 * 100),
            ("past the partition", |h| h.members[2].length = PARTITION),
            ("offset past the partition", |h| {
                h.members[2].offset = PARTITION
            }),
            ("length overflows", |h| h.members[2].length = u64::MAX),
            ("zero length", |h| h.members[1].length = 0),
            ("offset overflows", |h| {
                h.members[2].offset = u64::MAX - 4095
            }),
        ];
        for (name, edit) in breaks {
            let mut header = sample();
            if name == "out of table order" {
                // Swap the first two offsets: member 1 now starts before member 0 ends.
                header.members[1].offset = 4096;
                header.members[0].offset = 12288;
            } else {
                edit(&mut header);
            }
            // The writer refuses it ...
            let mut out = [0u8; BLOCK];
            assert_eq!(
                header.encode(&mut out, PARTITION),
                Err(Refusal::LayoutBreak),
                "{name}"
            );
            assert!(
                out.iter().all(|b| *b == 0),
                "{name}: the writer wrote nothing"
            );
            // ... and so does the parser, given the same table with a correct CRC.
            let mut block = block_of(&sample());
            for (i, m) in header.members.iter().enumerate() {
                let at = TABLE_START + i * ENTRY_BYTES;
                block[at + 8..at + 16].copy_from_slice(&m.offset.to_le_bytes());
                block[at + 16..at + 24].copy_from_slice(&m.length.to_le_bytes());
            }
            reseal(&mut block);
            assert_eq!(
                parse(&block, PARTITION),
                Err(Refusal::LayoutBreak),
                "{name}"
            );
        }
    }

    #[test]
    fn a_gap_between_members_and_a_member_ending_at_the_partition_end_are_legal() {
        let mut header = sample();
        header.members[2].offset = 4096 * 50;
        header.members[2].length = PARTITION - header.members[2].offset;
        let block = block_of(&header);
        assert_eq!(parse(&block, PARTITION), Ok(ParsedHeader::Two(header)));
    }

    #[test]
    fn the_writer_refuses_a_duplicate_tag_and_a_short_buffer() {
        let mut header = sample();
        header.members[1].tag = MemberTag::Kernel;
        assert_eq!(
            header.encode(&mut [0u8; BLOCK], PARTITION),
            Err(Refusal::DuplicateTag)
        );
        assert_eq!(
            sample().encode(&mut [0u8; BLOCK - 1], PARTITION),
            Err(Refusal::Truncated)
        );
    }

    #[test]
    fn the_writer_zeroes_stale_bytes_in_the_block() {
        let mut block = [0xAAu8; BLOCK + 10];
        sample().encode(&mut block, PARTITION).unwrap();
        assert_eq!(parse(&block, PARTITION), Ok(ParsedHeader::Two(sample())));
        assert!(block[MEMBERS_HEADER_BYTES..BLOCK].iter().all(|b| *b == 0));
        assert!(
            block[BLOCK..].iter().all(|b| *b == 0xAA),
            "bytes past the block are not touched"
        );
    }

    #[test]
    fn flipping_any_byte_of_the_188_is_refused_or_changes_the_content_legally() {
        let original = sample();
        let block = block_of(&original);
        for at in 0..MEMBERS_HEADER_BYTES {
            for mask in 1..=255u8 {
                let mut flipped = block;
                flipped[at] ^= mask;
                match parse(&flipped, PARTITION) {
                    Err(_) => {}
                    Ok(ParsedHeader::Two(header)) => {
                        assert_ne!(header, original, "byte {at} mask {mask:#x}");
                        panic!("a CRC-32 misses no single-byte flip: byte {at} mask {mask:#x}");
                    }
                    Ok(ParsedHeader::One(_)) => {
                        // Only a flip of the version byte can reach version 1, and its own CRC
                        // then has to hold over bytes it was not computed for.
                        assert_eq!(at, 8, "byte {at} mask {mask:#x}");
                    }
                }
            }
        }
    }

    #[test]
    fn flipping_any_padding_byte_is_refused() {
        let block = block_of(&sample());
        for at in MEMBERS_HEADER_BYTES..BLOCK {
            let mut flipped = block;
            flipped[at] ^= 0x01;
            assert_eq!(
                parse(&flipped, PARTITION),
                Err(Refusal::NonzeroReservedBytes),
                "{at}"
            );
        }
    }

    #[test]
    fn every_refusal_has_its_own_fixed_phrase() {
        let all = [
            Refusal::NoHeader,
            Refusal::Truncated,
            Refusal::UnknownVersion,
            Refusal::UnknownTag,
            Refusal::DuplicateTag,
            Refusal::MissingTag,
            Refusal::LayoutBreak,
            Refusal::NonzeroReservedBytes,
            Refusal::CrcMismatch,
        ];
        for (i, a) in all.iter().enumerate() {
            assert!(!a.phrase().is_empty());
            for b in &all[i + 1..] {
                assert_ne!(a.phrase(), b.phrase());
            }
        }
        assert_eq!(Refusal::LayoutBreak.phrase(), "member layout break");
    }
}

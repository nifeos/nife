use guid::types;

use super::*;

/// The buffer length the CRC harnesses quantify over. **Measured, not guessed**; the timings
/// are in notes/globally-unique-identifier-partition-table.md. CRC-32 is a shift-and-xor over
/// every bit of the input, so the formula grows with the byte count and the solver time grows
/// with the formula.
///
/// What the bound buys is not "CRC-32 is correct for short inputs". It is the *structure*: that
/// the table-driven implementation and the bitwise definition agree, and that a changed byte
/// always changes the result. Both are properties of the algorithm rather than of the length,
/// and the lengths that matter in practice (92 bytes of header, 16 KiB of array) are covered
/// exhaustively by the tests, over bytes real tools wrote.
const PROVED_BYTES: usize = 8;

/// **The fast CRC is the CRC.** For every 8-byte input, the 256-entry table and the bitwise
/// definition give the same answer.
///
/// The table is derived by `const` evaluation from the polynomial, which is a second place a
/// bug can hide and one no amount of testing against real disks would find: a wrong table that
/// happened to be self-consistent would round-trip our own output perfectly.
/// Falsification: replayable `crates/globally_unique_identifier_partition_table/falsifications/verification.crc32_matches_its_bitwise_definition.patch`
#[kani::proof]
#[kani::unwind(9)]
fn crc32_matches_its_bitwise_definition() {
    let bytes: [u8; PROVED_BYTES] = kani::any();
    assert_eq!(crc::crc32(&bytes), crc::crc32_bitwise(&bytes));
}

/// **Changing one byte always changes the CRC.** For every buffer, every position in it, and
/// every value that byte could change to.
///
/// This is what the whole format's integrity rests on, and the general form of the exhaustive
/// mutation sweeps in the tests: those enumerate every corruption of *one* table, this
/// quantifies over the table. It is true for a reason (an error burst shorter than the 32-bit
/// polynomial is always detected) and the reason is exactly the sort of thing worth checking
/// rather than citing.
/// Falsification: replayable `crates/globally_unique_identifier_partition_table/falsifications/verification.a_single_byte_change_always_changes_the_crc.patch`
#[kani::proof]
#[kani::unwind(9)]
fn a_single_byte_change_always_changes_the_crc() {
    let mut bytes: [u8; PROVED_BYTES] = kani::any();
    let index: usize = kani::any();
    kani::assume(index < PROVED_BYTES);
    let replacement: u8 = kani::any();
    kani::assume(replacement != bytes[index]);

    let before = crc::crc32(&bytes);
    bytes[index] = replacement;
    assert_ne!(crc::crc32(&bytes), before);
}

/// **A partition entry survives the round trip, on every one of the 2^1024 bit patterns.**
///
/// Not "on a well-formed entry": there is no such qualifier, because [`Entry::decode`] judges
/// nothing. That is what makes the property provable over every input at all, and it is why
/// every rule about whether an entry makes sense lives in
/// [`GloballyUniqueIdentifierPartitionTable`] instead.
/// Falsification: replayable `crates/globally_unique_identifier_partition_table/falsifications/verification.an_entry_survives_the_round_trip.patch`
#[kani::proof]
#[kani::unwind(129)]
fn an_entry_survives_the_round_trip() {
    let bytes: [u8; entry::SIZE] = kani::any();
    assert_eq!(Entry::decode(&bytes).encode(), bytes);
}

/// **A GUID survives GPT's mixed-endian layout**, for all 2^128 on-disk byte patterns: what
/// [`guid::from_disk`] reads, [`guid::to_disk`] writes back unchanged.
///
/// The swap is the one place in this crate where a transposition produces something that looks
/// entirely reasonable and matches nothing. This proves the two directions are inverses; the
/// EFI System Partition vector in `guid.rs`'s tests proves they are the right permutation.
/// Until 2026-10-06 (UTC) the swap lived in the printer and this was
/// `a_guid_survives_printing_and_parsing`; the text half moved to
/// the `uuid` crate with the type.
/// Falsification: replayable `crates/globally_unique_identifier_partition_table/falsifications/verification.a_guid_survives_the_on_disk_layout.patch`
#[kani::proof]
#[kani::unwind(17)]
fn a_guid_survives_the_on_disk_layout() {
    let bytes: [u8; 16] = kani::any();
    assert_eq!(guid::to_disk(guid::from_disk(bytes)), bytes);
}

/// **A header's fields survive the round trip**, for every value of every one of the nine.
///
/// This is the header's *layout* proved: nine fields at nine offsets in two integer widths, and
/// not a bit lost. The integrity half is proved by the two CRC harnesses above and, at the
/// lengths that actually occur, exhaustively by the tests.
///
/// **Why the split, and why the halves compose.** Going through `encode_into`/`decode` instead
/// puts a 92-byte CRC-32 over symbolic bytes into the formula twice. That was **measured at
/// over 274 seconds and still unfinished**, against a `script/verify` budget of roughly three
/// minutes per harness, because a table-driven CRC becomes a 256-way multiplexer per byte once
/// the state goes symbolic and ninety-two of those is a large formula to prove a fact about
/// byte offsets. The two halves compose for a reason visible in three lines of `header.rs`
/// rather than by assumption: everything `encode_into` adds is four bytes at offset 16,
/// everything `decode` adds is checks, and the CRC field is not one of the nine fields, so
/// neither half can disturb the other.
/// Falsification: replayable `crates/globally_unique_identifier_partition_table/falsifications/verification.a_headers_fields_survive_the_round_trip.patch`
#[kani::proof]
#[kani::unwind(513)]
fn a_headers_fields_survive_the_round_trip() {
    let header = Header {
        my_lba: kani::any(),
        alternate_lba: kani::any(),
        first_usable_lba: kani::any(),
        last_usable_lba: kani::any(),
        disk_guid: Uuid::from_bytes(kani::any()),
        entry_array_lba: kani::any(),
        entry_count: kani::any(),
        entry_size: kani::any(),
        entry_array_crc: kani::any(),
    };
    let mut block = [0u8; MIN_BLOCK_SIZE];
    header.encode_fields(&mut block);
    assert_eq!(Header::decode_fields(&block), header);
}

/// **The other direction: no two header fields share a byte, and none is unclaimed.** For every
/// possible 512-byte block, decoding the fields and re-encoding them reproduces every byte of
/// the field region (offsets 24 to 92) exactly.
///
/// The round trip above would still pass if two fields read the same offset and one of them
/// were never written back; this is the direction that catches it, because a byte no field
/// claims comes back zero and a byte two fields claim comes back holding one of them. Together
/// they say the nine fields **partition** the header's field region.
/// Falsification: replayable `crates/globally_unique_identifier_partition_table/falsifications/verification.the_header_fields_partition_the_block.patch`
#[kani::proof]
#[kani::unwind(513)]
fn the_header_fields_partition_the_block() {
    let block: [u8; MIN_BLOCK_SIZE] = kani::any();
    let mut round_tripped = [0u8; MIN_BLOCK_SIZE];
    Header::decode_fields(&block).encode_fields(&mut round_tripped);
    assert_eq!(round_tripped[24..92], block[24..92]);
}

/// **Two partitions that share a block are always caught**, for every pair of ranges.
///
/// Pure integer comparison, so this one is complete rather than bounded, and it is the property
/// most easily got wrong by hand: `last_lba` is inclusive, so ranges that merely touch do
/// overlap. Stated as the *definition* (the intersection is non-empty) rather than as the
/// implementation, so the proof is not the code compared with itself.
/// Falsification: replayable `crates/globally_unique_identifier_partition_table/falsifications/verification.overlap_is_exactly_sharing_a_block.patch`
#[kani::proof]
fn overlap_is_exactly_sharing_a_block() {
    let a = Entry::new(
        types::LINUX_FILESYSTEM,
        Uuid::nil(),
        kani::any(),
        kani::any(),
    );
    let b = Entry::new(types::NIFE_DATA, Uuid::nil(), kani::any(), kani::any());
    kani::assume(a.first_lba <= a.last_lba && b.first_lba <= b.last_lba);

    let shared = a.first_lba.max(b.first_lba) <= a.last_lba.min(b.last_lba);
    assert_eq!(a.overlaps(&b), shared);
    assert_eq!(a.overlaps(&b), b.overlaps(&a), "overlap is symmetric");

    // An unused entry overlaps nothing, whatever its bounds say.
    let mut ghost = a;
    ghost.type_guid = Uuid::nil();
    assert!(!ghost.overlaps(&b));
}

/// **The writer never lays out a table its own reader would reject**, for every disk size and
/// every partition placement.
///
/// Every geometry rule [`GloballyUniqueIdentifierPartitionTable::parse`] enforces, asserted on
/// what [`GloballyUniqueIdentifierPartitionTable::create`] produced. A writer that emitted a
/// table its own reader refuses would be the worst outcome this crate has, because it would be
/// discovered on somebody's disk rather than in this file, and the dimension where that could
/// hide is the one enumerated tests cover worst: an unusual disk size, where the arithmetic
/// that places the backup table runs out of room.
///
/// Stated over the *geometry* rather than over the bytes, deliberately. A byte-level
/// create-then-parse harness has four symbolic CRC-32 chains in it and did not finish inside
/// the budget (see `a_headers_fields_survive_the_round_trip` for the measurement). The
/// byte-level identity is proved instead where it can be proved *completely*:
/// `GloballyUniqueIdentifierPartitionTable::create` reproduces `sgdisk`'s own 512-byte header,
/// backup header and 16 KiB array exactly, in `tests/real_disks.rs`. This harness supplies what
/// that test cannot, which is every other disk size.
///
/// `create` is allowed to fail and the harness then returns; what is proved is that whenever it
/// succeeds, the result satisfies the reader's rules.
/// Falsification: replayable `crates/globally_unique_identifier_partition_table/falsifications/verification.create_never_lays_out_a_table_parse_would_reject.patch`
#[kani::proof]
fn create_never_lays_out_a_table_parse_would_reject() {
    let part = Entry::new(
        types::NIFE_DATA,
        Uuid::from_bytes([7; 16]),
        kani::any(),
        kani::any(),
    );
    let blocks: u64 = kani::any();

    let mut array = [0u8; entry::SIZE];
    let Ok(table) = GloballyUniqueIdentifierPartitionTable::create(
        Uuid::from_bytes([3; 16]),
        MIN_BLOCK_SIZE,
        blocks,
        &[part],
        &mut array,
    ) else {
        return;
    };

    let h = table.header();
    let span = table.entry_array_blocks();
    assert_eq!(table.block_count(), blocks);
    assert_eq!(h.my_lba, PRIMARY_HEADER_LBA);

    // The four inequalities GloballyUniqueIdentifierPartitionTable::parse checks, in the same
    // order.
    assert!(h.first_usable_lba <= h.last_usable_lba);
    assert!(h.entry_array_lba > PRIMARY_HEADER_LBA);
    assert!(h.entry_array_lba + span <= h.first_usable_lba);
    assert!(h.last_usable_lba + span < h.alternate_lba);

    // And the partition it was given is inside the usable range it chose, or create refused.
    assert!(part.first_lba >= h.first_usable_lba);
    assert!(part.last_lba <= h.last_usable_lba);
    assert!(part.first_lba <= part.last_lba);

    // The backup table lands exactly below the backup header, which is what check_backup
    // compares the backup's own entry_array_lba against.
    assert_eq!(table.backup_entry_lba() + span, table.backup_header_lba());
}

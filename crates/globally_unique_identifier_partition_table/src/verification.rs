use guid::types;

use super::*;

/// The buffer length the CRC harnesses quantify over. **Measured, not guessed**; CRC-32 is a shift-and-xor over
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
/// [`PartitionTable`] instead.
/// Falsification: replayable `crates/globally_unique_identifier_partition_table/falsifications/verification.an_entry_survives_the_round_trip.patch`
#[kani::proof]
#[kani::unwind(129)]
fn an_entry_survives_the_round_trip() {
    let bytes: [u8; entry::ENTRY_BYTES] = kani::any();
    assert_eq!(Entry::decode(&bytes).encode(), bytes);
}

/// **A GUID survives GPT's mixed-endian layout**, for all 2^128 on-disk byte patterns: what
/// [`guid::from_disk`] reads, [`guid::to_disk`] writes back unchanged.
///
/// This proves the two directions are inverses; the EFI System Partition vector in `guid.rs`'s
/// tests proves they are the right permutation.
/// Falsification: replayable `crates/globally_unique_identifier_partition_table/falsifications/verification.a_guid_survives_the_on_disk_layout.patch`
#[kani::proof]
#[kani::unwind(17)]
fn a_guid_survives_the_on_disk_layout() {
    let bytes: [u8; 16] = kani::any();
    assert_eq!(guid::to_disk(guid::from_disk(bytes)), bytes);
}

/// **A header's fields survive the round trip**, for every value of every one of the nine.
///
/// The header's layout: nine fields at nine offsets, not a bit lost. It goes through
/// `encode_fields`/`decode_fields` because a run through the CRC did not finish in 274 seconds;
/// the halves compose because the CRC field is not one of the nine.
/// Falsification: replayable `crates/globally_unique_identifier_partition_table/falsifications/verification.a_headers_fields_survive_the_round_trip.patch`
#[kani::proof]
#[kani::unwind(513)]
fn a_headers_fields_survive_the_round_trip() {
    let header = Header {
        my_logical_block_address: kani::any(),
        alternate_logical_block_address: kani::any(),
        first_usable_logical_block_address: kani::any(),
        last_usable_logical_block_address: kani::any(),
        disk_guid: Uuid::from_bytes(kani::any()),
        entry_array_logical_block_address: kani::any(),
        entry_count: kani::any(),
        entry_bytes: kani::any(),
        entry_array_crc: kani::any(),
    };
    let mut block = [0u8; MINIMUM_BLOCK_BYTES];
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
    let block: [u8; MINIMUM_BLOCK_BYTES] = kani::any();
    let mut round_tripped = [0u8; MINIMUM_BLOCK_BYTES];
    Header::decode_fields(&block).encode_fields(&mut round_tripped);
    assert_eq!(round_tripped[24..92], block[24..92]);
}

/// **Two partitions that share a block are always caught**, for every pair of ranges.
///
/// Pure integer comparison, so this one is complete rather than bounded, and it is the property
/// most easily got wrong by hand: `last_logical_block_address` is inclusive, so ranges that merely touch do
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
    let b = Entry::new(
        types::LINUX_FILESYSTEM,
        Uuid::nil(),
        kani::any(),
        kani::any(),
    );
    kani::assume(
        a.first_logical_block_address <= a.last_logical_block_address
            && b.first_logical_block_address <= b.last_logical_block_address,
    );

    let shared = a
        .first_logical_block_address
        .max(b.first_logical_block_address)
        <= a.last_logical_block_address
            .min(b.last_logical_block_address);
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
/// Every geometry rule [`PartitionTable::parse`] enforces, asserted on
/// whatever [`PartitionTable::create`] produces when it succeeds. Stated
/// over the geometry, since a byte-level harness carries four symbolic CRC chains and does not
/// finish; `tests/real_disks.rs` proves the bytes for `sgdisk`'s disk size, and this every other.
/// Falsification: replayable `crates/globally_unique_identifier_partition_table/falsifications/verification.create_never_lays_out_a_table_parse_would_reject.patch`
#[kani::proof]
fn create_never_lays_out_a_table_parse_would_reject() {
    let part = Entry::new(
        types::LINUX_FILESYSTEM,
        Uuid::from_bytes([7; 16]),
        kani::any(),
        kani::any(),
    );
    let blocks: u64 = kani::any();

    let mut array = [0u8; entry::ENTRY_BYTES];
    let Ok(table) = PartitionTable::create(
        Uuid::from_bytes([3; 16]),
        MINIMUM_BLOCK_BYTES,
        blocks,
        &[part],
        &mut array,
    ) else {
        return;
    };

    let h = table.header();
    let span = table.entry_array_blocks();
    assert_eq!(table.block_count(), blocks);
    assert_eq!(
        h.my_logical_block_address,
        PRIMARY_HEADER_LOGICAL_BLOCK_ADDRESS
    );

    // The four inequalities PartitionTable::parse checks, in the same
    // order.
    assert!(h.first_usable_logical_block_address <= h.last_usable_logical_block_address);
    assert!(h.entry_array_logical_block_address > PRIMARY_HEADER_LOGICAL_BLOCK_ADDRESS);
    assert!(h.entry_array_logical_block_address + span <= h.first_usable_logical_block_address);
    assert!(h.last_usable_logical_block_address + span < h.alternate_logical_block_address);

    // And the partition it was given is inside the usable range it chose, or create refused.
    assert!(part.first_logical_block_address >= h.first_usable_logical_block_address);
    assert!(part.last_logical_block_address <= h.last_usable_logical_block_address);
    assert!(part.first_logical_block_address <= part.last_logical_block_address);

    // The backup table lands exactly below the backup header, which is what check_backup
    // compares the backup's own entry_array_logical_block_address against.
    assert_eq!(
        table.backup_entry_logical_block_address() + span,
        table.backup_header_logical_block_address()
    );
}

/// **`blocks` never panics and counts exactly**, for every pair of bounds.
///
/// The count is the inclusive range's size, so `0 ..= u64::MAX` holds 2^64 blocks and is not a
/// `u64`: `None`, as for a reversed range. Stated against 128-bit arithmetic, which cannot
/// overflow, so the proof is not the code compared with itself.
/// Falsification: replayable `crates/globally_unique_identifier_partition_table/falsifications/verification.an_entries_block_count_never_panics_and_is_exact.patch`
#[kani::proof]
fn an_entries_block_count_never_panics_and_is_exact() {
    let entry = Entry::new(
        types::LINUX_FILESYSTEM,
        Uuid::nil(),
        kani::any(),
        kani::any(),
    );
    let wide = u128::from(entry.last_logical_block_address)
        .checked_sub(u128::from(entry.first_logical_block_address))
        .map(|span| span + 1)
        .and_then(|count| u64::try_from(count).ok());
    assert_eq!(entry.blocks(), wide);
}

/// **A backup header restores the table it was written from**, for every disk size `create`
/// accepts.
///
/// The recovery path's field mapping: the header [`PartitionTable::write_backup_header`] encodes
/// sits on the last block, points back at block 1 and puts its array after the last usable block
/// and before itself, which is what [`PartitionTable::parse_backup`] checks, and the primary that
/// `parse_backup` rebuilds from it is the table's own header, field for field. The bytes between
/// are `encode_into` and `decode`, whose fields are proved by
/// `a_headers_fields_survive_the_round_trip`; a harness through them as well carries two symbolic
/// CRCs and did not finish in seventeen minutes. `tests/table.rs` runs the whole round trip at
/// both block sizes over bytes.
/// Falsification: replayable `crates/globally_unique_identifier_partition_table/falsifications/verification.a_backup_header_restores_the_table_it_was_written_from.patch`
#[kani::proof]
fn a_backup_header_restores_the_table_it_was_written_from() {
    let part = Entry::new(types::LINUX_FILESYSTEM, Uuid::from_bytes([7; 16]), 3, 3);
    let blocks: u64 = kani::any();

    let mut array = [0u8; entry::ENTRY_BYTES];
    let Ok(table) = PartitionTable::create(
        Uuid::from_bytes([3; 16]),
        MINIMUM_BLOCK_BYTES,
        blocks,
        &[part],
        &mut array,
    ) else {
        return;
    };

    let backup = table.backup_header();
    let last = blocks - 1;
    assert_eq!(backup.my_logical_block_address, last);
    assert_eq!(backup.alternate_logical_block_address, 1);
    assert!(backup.entry_array_logical_block_address > backup.last_usable_logical_block_address);
    assert!(
        u128::from(backup.entry_array_logical_block_address)
            + u128::from(table.entry_array_blocks())
            <= u128::from(last)
    );
    assert_eq!(table::primary_from_backup(backup, last), *table.header());
}

/// **The checks `Header::decode` makes before its CRC are total**, on every 92-byte header in a
/// 512-byte block: no panic, and a size that the CRC step can slice by.
///
/// The first of the two stages of `decode`'s totality. The second is
/// `the_header_crc_in_pieces_is_the_crc_of_the_whole`, and `decode_fields` is total by
/// `the_header_fields_partition_the_block`. A harness through the CRC as well would reason about a
/// polynomial over 92 symbolic bytes, which is what the split avoids. The rest of the block is
/// zero: every field these checks read is in the first 92 bytes, and with the whole block symbolic
/// the harness took 158 seconds against 28. The host tests refuse a nonzero tail byte.
/// Falsification: replayable `crates/globally_unique_identifier_partition_table/falsifications/verification.the_header_checks_before_the_crc_are_total.patch`
#[kani::proof]
#[kani::unwind(513)]
fn the_header_checks_before_the_crc_are_total() {
    let mut block = [0u8; MINIMUM_BLOCK_BYTES];
    let fields: [u8; header::HEADER_BYTES as usize] = kani::any();
    block[..fields.len()].copy_from_slice(&fields);
    if let Ok(size) = Header::decode_checks(&block) {
        assert!(size >= header::HEADER_BYTES as usize);
        assert!(size <= block.len());
    }
}

/// The input length the pieces harness quantifies over, shorter than [`PROVED_BYTES`] because it
/// takes one CRC per pair of cuts. **Measured**: two bytes prove in 40 seconds, three in 139, and
/// four did not finish in eight minutes. The property is the running CRC's carry from one piece to
/// the next, which a cut between two bytes exercises.
const PIECES_BYTES: usize = 2;

/// **The CRC fed in pieces is the CRC of the pieces joined**, for every 2-byte input cut into
/// three pieces at every pair of places.
///
/// The second stage of `decode`'s totality, and what lets the header's CRC be taken with its own
/// CRC field read as zero without a scratch block: three pieces through one running CRC equal one
/// CRC over the joined bytes. The cuts are enumerated rather than symbolic, so each comparison is
/// between two fixed shapes of one computation and the solver does not search over lengths.
/// Falsification: replayable `crates/globally_unique_identifier_partition_table/falsifications/verification.the_header_crc_in_pieces_is_the_crc_of_the_whole.patch`
#[kani::proof]
#[kani::unwind(4)]
fn the_header_crc_in_pieces_is_the_crc_of_the_whole() {
    let bytes: [u8; PIECES_BYTES] = kani::any();
    let whole = crc::crc32(&bytes);
    for first in 0..=PIECES_BYTES {
        for second in first..=PIECES_BYTES {
            let pieces = [&bytes[..first], &bytes[first..second], &bytes[second..]];
            assert_eq!(crc::crc32_pieces(&pieces), whole);
        }
    }
}

/// **A span always contains what was asked for**, on every offset and length, at every transfer
/// size a device in this tree reports and at 3, which divides nothing evenly: it starts at or
/// before the range, ends at or after it, puts `offset` on the first wanted byte and reads no
/// spare block. `None` only for a zero length or arithmetic that would leave a `u64`.
///
/// Stated in 128-bit arithmetic, which cannot overflow, so the proof is not the code compared with
/// itself. The transfer size is concrete because a symbolic 64-bit divisor did not finish in
/// seventeen minutes; the test `a_span_always_contains_what_was_asked_for` covers other sizes over
/// a grid, and a zero transfer.
/// Falsification: replayable `crates/globally_unique_identifier_partition_table/falsifications/verification.a_span_always_contains_what_was_asked_for.patch`
#[kani::proof]
fn a_span_always_contains_what_was_asked_for() {
    for transfer in [1, 3, 512, 4096] {
        a_span_contains_what_was_asked_for_at(transfer);
    }
}

fn a_span_contains_what_was_asked_for_at(transfer: u64) {
    let byte_offset: u64 = kani::any();
    let len: u64 = kani::any();
    let wanted_end = u128::from(byte_offset) + u128::from(len);
    let t = u128::from(transfer);
    match span::Span::covering(byte_offset, len, transfer) {
        Some(s) => {
            let start = u128::from(s.first_block) * t;
            let end = start + u128::from(s.blocks) * t;
            assert!(start <= u128::from(byte_offset));
            assert!(end >= wanted_end);
            assert_eq!(start + s.offset as u128, u128::from(byte_offset));
            assert!(end - t < wanted_end);
            assert!(u128::from(s.first_block) + u128::from(s.blocks) <= u128::from(u64::MAX));
        }
        None => {
            if len != 0 {
                // The two pieces of arithmetic the documentation names: the last wanted byte
                // counted from the start of the first block, and the block one past the run.
                let last_byte = u128::from(byte_offset) % t + u128::from(len) - 1;
                let past_the_run = (wanted_end - 1) / t + 1;
                let most = u128::from(u64::MAX);
                assert!(last_byte > most || past_the_run > most);
            }
        }
    }
}

/// **`name_utf8` is total and its documented buffer is always enough**, on every 36 code units a
/// hostile entry can carry, unpaired surrogates included: with 144 bytes it always succeeds and stops
/// inside the buffer. What it writes is `char::encode_utf8`'s, valid by construction.
/// Falsification: replayable `crates/globally_unique_identifier_partition_table/falsifications/verification.a_name_always_decodes_into_its_documented_buffer.patch`
#[kani::proof]
#[kani::unwind(37)]
fn a_name_always_decodes_into_its_documented_buffer() {
    let mut entry = Entry::new(types::LINUX_FILESYSTEM, Uuid::nil(), 3, 3);
    entry.name = kani::any();
    let mut out = [0u8; 4 * entry::NAME_UNITS];
    let written = entry.name_utf8(&mut out);
    assert!(written.is_ok());
    assert!(written.unwrap() <= out.len());
}

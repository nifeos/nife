//! Parse partition tables written by other people's tools.
//!
//! A parser that only round-trips its own output proves almost nothing, so these run against bytes
//! from two implementations that share no code: `sgdisk` 1.0.10 (gptfdisk, C++) and macOS
//! `diskutil` / `hdiutil`. They disagree on partition names, CHS fields and type GUIDs, and each
//! disagreement is a check one tool's output alone would have made too strict. Each fixture is the
//! first 34 and last 33 blocks of a 64 MiB, 512-byte-block image: both tables, and none of the
//! space between. The commands that regenerate them are in
//! notes/globally-unique-identifier-partition-table/the-modules.md, "Regenerating the fixtures".

use globally_unique_identifier_partition_table::guid::types;
use globally_unique_identifier_partition_table::{
    BackupMismatch, Entry, Error, GloballyUniqueIdentifierPartitionTable, MbrProblem, entry,
};
use uuid::Uuid;

const SGDISK_HEAD: &[u8] = include_bytes!("fixtures/sgdisk-64m.head");
const SGDISK_TAIL: &[u8] = include_bytes!("fixtures/sgdisk-64m.tail");
const APPLE_HEAD: &[u8] = include_bytes!("fixtures/apple-64m.head");
const APPLE_TAIL: &[u8] = include_bytes!("fixtures/apple-64m.tail");

const BLOCK: usize = 512;
/// 64 MiB of 512-byte blocks. Both fixtures are this size.
const BLOCKS: u64 = 131_072;

/// The primary header block and the whole primary entry array, out of a `.head` fixture.
fn primary(head: &[u8]) -> (&[u8], &[u8]) {
    (&head[BLOCK..2 * BLOCK], &head[2 * BLOCK..])
}

/// The backup entry array and the backup header block, out of a `.tail` fixture. The tail is 33
/// blocks: 32 of array, then the header on the very last block of the disk.
fn backup(tail: &[u8]) -> (&[u8], &[u8]) {
    let split = tail.len() - BLOCK;
    (&tail[..split], &tail[split..])
}

fn name_of(part: &Entry) -> String {
    let mut buf = [0u8; 4 * entry::NAME_UNITS];
    let n = part.name_utf8(&mut buf).expect("144 bytes always fits");
    String::from_utf8(buf[..n].to_vec()).expect("name_utf8 emits UTF-8")
}

#[test]
fn sgdisk_writes_a_table_we_accept_whole() {
    let (header, array) = primary(SGDISK_HEAD);
    let table = GloballyUniqueIdentifierPartitionTable::parse(header, array)
        .expect("sgdisk writes a valid GPT");

    assert_eq!(table.block_size(), BLOCK);
    assert_eq!(table.block_count(), BLOCKS);
    assert_eq!(
        table.disk_guid(),
        Uuid::try_parse_ascii(b"1A2B3C4D-5E6F-4718-9A0B-C1D2E3F40506").unwrap(),
        "the disk GUID we told sgdisk to use, decoded through the mixed-endian rule"
    );
    assert_eq!(table.entry_count(), 128);
    assert_eq!(table.first_usable_lba(), 34, "1 MBR + 1 header + 32 array");
    assert_eq!(table.last_usable_lba(), BLOCKS - 34);
    assert_eq!(table.primary_entry_lba(), 2);
    assert_eq!(table.backup_entry_lba(), BLOCKS - 33);
    assert_eq!(table.backup_header_lba(), BLOCKS - 1);

    let parts: Vec<_> = table.partitions().collect();
    assert_eq!(parts.len(), 3);

    assert_eq!(parts[0].0, 0, "sgdisk partition 1 is array index 0");
    assert_eq!(parts[0].1.type_guid, types::EFI_SYSTEM);
    assert_eq!(parts[0].1.first_lba, 2048);
    assert_eq!(parts[0].1.last_lba, 10239);
    assert_eq!(parts[0].1.blocks(), Some(8192), "4 MiB");
    assert_eq!(name_of(&parts[0].1), "EFI system");

    assert_eq!(parts[1].1.type_guid, types::LINUX_FILESYSTEM);
    assert_eq!(name_of(&parts[1].1), "linux root");
    assert_eq!(parts[1].1.blocks(), Some(20480), "10 MiB");
    assert_eq!(
        parts[1].1.unique_guid,
        Uuid::try_parse_ascii(b"11111111-2222-4333-8444-555555555502").unwrap()
    );

    // The one that matters for this OS: our own type GUID, recognised on a disk written by a tool
    // that has never heard of it. This is the recovery story in one assertion.
    assert_eq!(parts[2].1.type_guid, types::NIFE_DATA);
    assert_eq!(types::name(parts[2].1.type_guid), Some("nife data"));
    // The fixture's label is "cricker data", from before the milestone 120 rename, on purpose: a
    // pre-rename disk is the recovery case, and the GUID, not the label, is what is recognised.
    assert_eq!(name_of(&parts[2].1), "cricker data");
    assert_eq!(parts[2].1.last_lba, table.last_usable_lba());
}

#[test]
fn sgdisks_backup_and_protective_mbr_check_out() {
    let (header, array) = primary(SGDISK_HEAD);
    let table = GloballyUniqueIdentifierPartitionTable::parse(header, array).unwrap();
    let (backup_array, backup_header) = backup(SGDISK_TAIL);

    table.check_backup(backup_header, backup_array).unwrap();
    table.check_protective_mbr(&SGDISK_HEAD[..BLOCK]).unwrap();
}

#[test]
fn macos_writes_a_table_we_accept_whole() {
    let (header, array) = primary(APPLE_HEAD);
    let table = GloballyUniqueIdentifierPartitionTable::parse(header, array)
        .expect("diskutil writes a valid GPT");
    let (backup_array, backup_header) = backup(APPLE_TAIL);
    table.check_backup(backup_header, backup_array).unwrap();
    table.check_protective_mbr(&APPLE_HEAD[..BLOCK]).unwrap();

    // Same geometry as sgdisk's, independently arrived at. 128 entries at 128 bytes is not in the
    // spec as a requirement, it is just what everybody does, and here are two everybodies.
    assert_eq!(table.entry_count(), 128);
    assert_eq!(table.first_usable_lba(), 34);
    assert_eq!(table.last_usable_lba(), BLOCKS - 34);

    let parts: Vec<_> = table.partitions().collect();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].1.type_guid, types::MICROSOFT_BASIC_DATA);
    assert_eq!(parts[1].1.type_guid, types::APPLE_HFS_PLUS);
}

/// **macOS does not write GPT partition names**, and finding that out is why the Apple fixture is
/// here at all.
///
/// Both volumes of `diskutil partitionDisk ... MS-DOS ESP 16M HFS+ MACDATA 0b` are named, and both
/// GPT entries hold 72 zero bytes where the name goes; `sgdisk` writes the name. So nothing built
/// on this crate may key off the GPT name. See notes/globally-unique-identifier-partition-table.md,
/// "What the fixtures taught us".
#[test]
fn macos_leaves_the_partition_names_empty_and_sgdisk_does_not() {
    let (header, array) = primary(APPLE_HEAD);
    let apple = GloballyUniqueIdentifierPartitionTable::parse(header, array).unwrap();
    for (index, part) in apple.partitions() {
        assert_eq!(part.name, [0u16; entry::NAME_UNITS], "entry {index}");
        assert_eq!(name_of(&part), "");
    }

    let (header, array) = primary(SGDISK_HEAD);
    let sgdisk = GloballyUniqueIdentifierPartitionTable::parse(header, array).unwrap();
    assert!(sgdisk.partitions().all(|(_, p)| !name_of(&p).is_empty()));
}

/// The two tools write different CHS bytes in the protective MBR and both are right, which is why
/// [`globally_unique_identifier_partition_table::mbr::validate`] does not look at them.
///
/// `sgdisk` computes the real cylinder/head/sector of the last block; macOS writes `FE FF FF`, the
/// "out of range, use LBA" marker. Nothing has read a CHS field in thirty years. A validator
/// written against one tool's output would have rejected the other's disk outright.
#[test]
fn the_two_tools_disagree_about_chs_and_it_does_not_matter() {
    const RECORD: usize = 446;
    assert_eq!(&SGDISK_HEAD[RECORD + 5..RECORD + 8], &[0x28, 0x20, 0x08]);
    assert_eq!(&APPLE_HEAD[RECORD + 5..RECORD + 8], &[0xFE, 0xFF, 0xFF]);
    // They agree on everything that is load-bearing: type 0xEE at LBA 1, covering the disk.
    for head in [SGDISK_HEAD, APPLE_HEAD] {
        assert_eq!(head[RECORD + 4], 0xEE);
        assert_eq!(&head[RECORD + 8..RECORD + 12], &1u32.to_le_bytes());
        assert_eq!(
            &head[RECORD + 12..RECORD + 16],
            &(BLOCKS as u32 - 1).to_le_bytes()
        );
        globally_unique_identifier_partition_table::mbr::validate(&head[..BLOCK], BLOCKS).unwrap();
    }
}

/// Re-emitting a real table produces the real table's bytes, byte for byte.
///
/// This is the claim that ties the two halves of the crate together. Round-tripping our own output
/// would prove only that we are self-consistent; round-tripping `sgdisk`'s proves that what we
/// *write* is what a tool the rest of the world uses would have written. All four regions: the
/// protective MBR, both headers, and the entry array.
#[test]
fn re_emitting_sgdisks_table_reproduces_it_exactly() {
    let (header_block, array) = primary(SGDISK_HEAD);
    let table = GloballyUniqueIdentifierPartitionTable::parse(header_block, array).unwrap();
    let (backup_array, backup_header_block) = backup(SGDISK_TAIL);

    let mut out = [0u8; BLOCK];
    table.write_primary_header(&mut out).unwrap();
    assert_eq!(out, header_block, "primary header");

    table.write_backup_header(&mut out).unwrap();
    assert_eq!(out, backup_header_block, "backup header");

    table.write_protective_mbr(&mut out).unwrap();
    // The ending CHS (record offset 5..8, absolute 451..454) is the one place we differ from
    // sgdisk on purpose: it writes the real cylinder/head/sector of the last block and we write
    // the "beyond CHS" marker. See the test above for why neither is wrong.
    let real = &SGDISK_HEAD[..BLOCK];
    assert_eq!(&out[..451], &real[..451], "MBR up to the ending CHS");
    assert_eq!(&out[454..], &real[454..], "MBR after the ending CHS");

    assert_eq!(table.entry_array(), &array[..table.entry_array().len()]);
    assert_eq!(
        table.entry_array(),
        &backup_array[..table.entry_array().len()],
        "the backup array is the same bytes, which is why one accessor serves both"
    );
}

/// Rebuilding `sgdisk`'s disk from its description, with this crate, gives `sgdisk`'s bytes.
///
/// The strongest statement available about the writer: not "our writer agrees with our reader" but
/// "our writer agrees with gptfdisk". Every field is reproduced from the parsed values, so the only
/// thing under test is the *layout*: where the arrays go, what the usable range is, how the backup
/// differs from the primary.
#[test]
fn creating_the_same_table_from_scratch_matches_sgdisk_byte_for_byte() {
    let (header_block, array) = primary(SGDISK_HEAD);
    let original = GloballyUniqueIdentifierPartitionTable::parse(header_block, array).unwrap();
    let parts: Vec<Entry> = original.partitions().map(|(_, e)| e).collect();

    let mut rebuilt_array = [0u8; globally_unique_identifier_partition_table::ENTRY_ARRAY_BYTES];
    let rebuilt = GloballyUniqueIdentifierPartitionTable::create(
        original.disk_guid(),
        BLOCK,
        original.block_count(),
        &parts,
        &mut rebuilt_array,
    )
    .unwrap();

    assert_eq!(rebuilt.first_usable_lba(), original.first_usable_lba());
    assert_eq!(rebuilt.last_usable_lba(), original.last_usable_lba());
    assert_eq!(rebuilt.backup_entry_lba(), original.backup_entry_lba());
    assert_eq!(rebuilt.entry_array(), original.entry_array());

    let mut out = [0u8; BLOCK];
    rebuilt.write_primary_header(&mut out).unwrap();
    assert_eq!(out, header_block);
    rebuilt.write_backup_header(&mut out).unwrap();
    assert_eq!(out, backup(SGDISK_TAIL).1);
}

/// **Every single-byte corruption of a real header block is rejected.** All 130,560 of them.
///
/// 512 positions times the 255 other values of each byte: complete for this table, where the Kani
/// harness `a_single_byte_change_always_changes_the_crc` is the symbolic form. A byte may be caught
/// by the signature, the revision, the reserved fields or the CRC, and none falls through all four.
/// Skipped under Miri: each parse re-CRCs the 16 KiB array, a day's work interpreted, and the
/// clean-fixture tests walk the same paths (notes/undefined-behavior.md).
#[test]
#[cfg_attr(
    miri,
    ignore = "130,560 16 KiB CRCs; the clean-fixture tests cover these paths"
)]
fn every_single_byte_corruption_of_the_header_is_caught() {
    let (header_block, array) = primary(SGDISK_HEAD);
    GloballyUniqueIdentifierPartitionTable::parse(header_block, array)
        .expect("the clean fixture parses");

    let mut corrupt = [0u8; BLOCK];
    let mut checked = 0u32;
    for position in 0..BLOCK {
        for value in 0..=255u8 {
            if value == header_block[position] {
                continue;
            }
            corrupt.copy_from_slice(header_block);
            corrupt[position] = value;
            assert!(
                GloballyUniqueIdentifierPartitionTable::parse(&corrupt, array).is_err(),
                "byte {position} changed to {value:#04x} was accepted"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 512 * 255);
}

/// **Every single-bit corruption of a real 16 KiB entry array is rejected.** All 131,072 of them,
/// plus the complement of every byte.
///
/// The array holds every partition's bounds, so a corrupt `last_lba` let through is a filesystem
/// writing over the next partition. Bits, not every value: the full sweep below is 163 seconds,
/// and the header's can afford every value because its CRC covers 92 bytes, not 16,384. Skipped
/// under Miri, as the header sweep is. See notes/globally-unique-identifier-partition-table.md,
/// "Counted, exhaustively".
#[test]
#[cfg_attr(
    miri,
    ignore = "131,072 16 KiB CRCs; the clean-fixture tests cover these paths"
)]
fn every_single_bit_corruption_of_the_entry_array_is_caught() {
    let (header_block, array) = primary(SGDISK_HEAD);
    let clean = GloballyUniqueIdentifierPartitionTable::parse(header_block, array).unwrap();
    let len = clean.entry_array().len();
    assert_eq!(len, 16384);

    let mut corrupt = array[..len].to_vec();
    for position in 0..len {
        let original = corrupt[position];
        for mask in [1u8, 2, 4, 8, 16, 32, 64, 128, 255] {
            corrupt[position] = original ^ mask;
            assert!(
                GloballyUniqueIdentifierPartitionTable::parse(header_block, &corrupt).is_err(),
                "entry-array byte {position} xor {mask:#04x} was accepted"
            );
        }
        corrupt[position] = original;
    }
}

/// The complete version of the sweep above: every byte of the real entry array, crossed with every
/// value it could have held instead. **All 4,177,920, and it passes.**
///
/// `#[ignore]` because it is 163 seconds and the bit-flip version above covers the same ground in
/// four. Run it with `cargo test -p globally_unique_identifier_partition_table -- --ignored` after
/// touching anything in the CRC or the array handling. Left in the tree rather than deleted because
/// "we ran it once and it passed" is a claim a reader should be able to check.
#[test]
#[ignore = "163 seconds: 68 GB of CRC-32. The bit-flip sweep is the gate."]
fn every_single_byte_corruption_of_the_entry_array_is_caught() {
    let (header_block, array) = primary(SGDISK_HEAD);
    let len = GloballyUniqueIdentifierPartitionTable::parse(header_block, array)
        .unwrap()
        .entry_array()
        .len();

    let mut corrupt = array[..len].to_vec();
    for position in 0..len {
        let original = corrupt[position];
        for value in 0..=255u8 {
            if value == original {
                continue;
            }
            corrupt[position] = value;
            assert!(
                GloballyUniqueIdentifierPartitionTable::parse(header_block, &corrupt).is_err(),
                "entry-array byte {position} changed to {value:#04x} was accepted"
            );
        }
        corrupt[position] = original;
    }
}

/// A backup that belongs to a different disk is refused, one field at a time.
///
/// Built by taking the real backup and changing exactly one field, which is what a half-finished
/// resize or a `dd` of one disk onto another leaves behind. Each has to be caught by name, because
/// "the backup is wrong" is not an answer a person holding their only copy of something can act on.
#[test]
fn a_backup_that_belongs_to_another_disk_is_refused_field_by_field() {
    let (header_block, array) = primary(SGDISK_HEAD);
    let table = GloballyUniqueIdentifierPartitionTable::parse(header_block, array).unwrap();
    let (backup_array, backup_header_block) = backup(SGDISK_TAIL);
    table
        .check_backup(backup_header_block, backup_array)
        .unwrap();

    let mut header =
        globally_unique_identifier_partition_table::Header::decode(backup_header_block).unwrap();
    let mut block = [0u8; BLOCK];

    /// One field of the backup header, damaged, and the mismatch it must produce.
    type Case = (
        fn(&mut globally_unique_identifier_partition_table::Header),
        BackupMismatch,
    );

    let cases: [Case; 6] = [
        (|h| h.my_lba += 1, BackupMismatch::MyLba),
        (|h| h.alternate_lba += 1, BackupMismatch::AlternateLba),
        (|h| h.disk_guid = Uuid::nil(), BackupMismatch::DiskGuid),
        (|h| h.last_usable_lba -= 1, BackupMismatch::LastUsableLba),
        (|h| h.entry_count -= 1, BackupMismatch::EntryCount),
        (|h| h.entry_array_crc ^= 1, BackupMismatch::EntryArrayCrc),
    ];
    for (mutate, expected) in cases {
        let original = header;
        mutate(&mut header);
        header.encode_into(&mut block).unwrap();
        assert_eq!(
            table.check_backup(&block, backup_array),
            Err(Error::Backup(expected)),
            "expected {expected:?}"
        );
        header = original;
    }

    // A backup header that is internally consistent but sits at the wrong LBA: the CRC is fine,
    // every field agrees, and it is still not this disk's backup.
    header.entry_array_lba += 1;
    header.encode_into(&mut block).unwrap();
    assert_eq!(
        table.check_backup(&block, backup_array),
        Err(Error::Backup(BackupMismatch::EntryArrayLba))
    );
}

/// A hybrid MBR is refused, and named.
///
/// Made by adding a second record to `sgdisk`'s protective MBR, which is what Boot Camp and
/// `gdisk`'s `h` command produce. The disk is then described twice, by two tools that will not
/// agree for long. Refusing it is a decision; the error says which one.
#[test]
fn a_hybrid_mbr_is_refused_by_name() {
    let mut block = SGDISK_HEAD[..BLOCK].to_vec();
    globally_unique_identifier_partition_table::mbr::validate(&block, BLOCKS).unwrap();

    let second = 446 + 16;
    block[second + 4] = 0x83; // Linux, in the old MBR type space
    block[second + 8..second + 12].copy_from_slice(&2048u32.to_le_bytes());
    block[second + 12..second + 16].copy_from_slice(&8192u32.to_le_bytes());
    assert_eq!(
        globally_unique_identifier_partition_table::mbr::validate(&block, BLOCKS),
        Err(Error::Mbr(MbrProblem::ExtraRecord { index: 1 }))
    );
}

/// The primary of one fixture with the backup of the other is caught immediately.
///
/// The disk GUIDs differ, which is the first thing
/// [`GloballyUniqueIdentifierPartitionTable::check_backup`] compares that can differ between two
/// disks of the same size. Two 64 MiB images with identical geometry is the case where a lazier
/// check (sizes match, so it must be fine) would say yes.
#[test]
fn one_disks_primary_with_a_foreign_backup_is_caught() {
    let (header, array) = primary(SGDISK_HEAD);
    let table = GloballyUniqueIdentifierPartitionTable::parse(header, array).unwrap();
    let (apple_array, apple_header) = backup(APPLE_TAIL);
    assert_eq!(
        table.check_backup(apple_header, apple_array),
        Err(Error::Backup(BackupMismatch::DiskGuid))
    );
}

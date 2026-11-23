//! Kani harnesses for the on-disk parsers of `rsext4`, against the crate's public API.
//!
//! Each harness states, for the version pinned in `Cargo.toml`, the claim nife makes for its own
//! on-disk parsers: decoding a superblock, a group descriptor, an extent header or entry, or a
//! directory entry reads only inside the slice it was given, for every input up to the bound the
//! harness states; a directory block walk ends and stays inside its block; and the software
//! CRC32C's one-byte step and every entry of its tables equal the bitwise definition. Milestone
//! 190 holds them here because upstream runs no Kani (see
//! nifeos/record:design/roadmap/0190-ext4-read-write/upstream/6-kani-harnesses.md).
//!
//! They live in this tool's workspace, and not in the nife workspace, so that `rsext4` stays out
//! of every shipping lockfile (the host-only ruling in `Cargo.toml`). `script/verify` runs them
//! with `cargo kani --manifest-path tools/ext4_oracle/Cargo.toml --tests`. Outside Kani the file
//! compiles to an empty test binary.
//!
//! Every falsification record below is `attested`: `script/falsifications` sweeps the nife
//! workspace only, so each record carries the by-hand replay instead, which is a local `[patch]`
//! of `rsext4` with the named change applied, never committed.
//!
//! Name: provisional. calef names files.
#![cfg(kani)]

use rsext4::blockgroup_description::Ext4GroupDesc;
use rsext4::crc32c::{crc32c_append, crc32c_finalize, crc32c_init};
use rsext4::disknode::{Ext4Extent, Ext4ExtentHeader, Ext4ExtentIdx};
use rsext4::endian::DiskFormat;
use rsext4::entries::{DirEntryIterator, Ext4DirEntryInfo};
use rsext4::extents_tree::{ExtentNode, ExtentTree};
use rsext4::superblock::Ext4Superblock;

/// The CRC32C polynomial, reflected, as RFC 3720 appendix B.4 defines it.
const CRC32C_POLYNOMIAL_REFLECTED: u32 = 0x82F6_3B78;

/// The bitwise definition of a raw CRC32C update: no table, one bit at a time.
fn crc32c_bitwise(mut crc: u32, data: &[u8]) -> u32 {
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (CRC32C_POLYNOMIAL_REFLECTED & mask);
        }
    }
    crc
}

/// A CPUID that reports no feature at all, so `crc32c_append` takes its software path. Kani does
/// not model inline assembly, and the hardware path is the instruction, not code a prover reads.
#[cfg(target_arch = "x86_64")]
fn cpuid_reporting_no_features(_leaf: u32) -> core::arch::x86_64::CpuidResult {
    core::arch::x86_64::CpuidResult {
        eax: 0,
        ebx: 0,
        ecx: 0,
        edx: 0,
    }
}

/// A slice of `N` arbitrary bytes, cut to an arbitrary length up to `N`.
fn arbitrary_prefix<const N: usize>(storage: &[u8; N]) -> &[u8] {
    let length: usize = kani::any();
    kani::assume(length <= N);
    &storage[..length]
}

/// The software CRC32C's one-byte step equals the bitwise definition for every seed and every
/// byte, and `crc32c_init` and `crc32c_finalize` are the standard initial and final values. A
/// buffer shorter than eight bytes, and the tail of every longer one, goes through this step.
///
/// Falsification: attested 2026-11-22. Changed the tail step in `crc32c_update` in
/// `crc32c/crc32c.rs` from `(crc >> 8)` to `(crc >> 7)`; the harness failed on its first
/// equality. Replay: apply that change to a copy of rsext4 0.9.2, point a local `[patch]` at it
/// and run `cargo kani --manifest-path tools/ext4_oracle/Cargo.toml --tests --harness
/// software_crc32c_tail_step_equals_bitwise_definition -Z stubbing`.
#[kani::proof]
#[cfg_attr(
    target_arch = "x86_64",
    kani::stub(core::arch::x86_64::__cpuid, cpuid_reporting_no_features)
)]
fn software_crc32c_tail_step_equals_bitwise_definition() {
    let data: [u8; 1] = kani::any();
    let seed: u32 = kani::any();
    let table = crc32c_append(seed, &data);
    assert_eq!(table, crc32c_bitwise(seed, &data));
    assert_eq!(crc32c_finalize(table), !table);
    assert_eq!(crc32c_init(), u32::MAX);
}

/// Every entry of the eight slicing tables equals the bitwise definition: an eight-byte round
/// over a zero seed and a word that is zero but for one arbitrary byte at an arbitrary position
/// reads exactly one entry of one table, and it equals the bitwise CRC32C of that word.
///
/// What this does not prove: that a whole round with an arbitrary seed and word equals the
/// bitwise definition. That follows from the entries because CRC32C without its initial and final
/// values is linear over GF(2), so a round is the exclusive or of one entry per byte; Kani does
/// not check that step, because the solver does not finish the whole round in fifteen minutes.
///
/// Falsification: attested 2026-11-22. Shifted the eighth table's index in `crc32c_update` in
/// `crc32c/crc32c.rs` (`word >> 56` to `(word >> 56) ^ 1`); the harness failed on its equality.
/// Replay as for `software_crc32c_tail_step_equals_bitwise_definition`, with `--harness
/// software_crc32c_tables_equal_bitwise_definition`.
#[kani::proof]
#[kani::unwind(9)]
#[cfg_attr(
    target_arch = "x86_64",
    kani::stub(core::arch::x86_64::__cpuid, cpuid_reporting_no_features)
)]
fn software_crc32c_tables_equal_bitwise_definition() {
    let position: usize = kani::any();
    kani::assume(position < 8);
    let mut data = [0u8; 8];
    data[position] = kani::any();
    assert_eq!(crc32c_append(0, &data), crc32c_bitwise(0, &data));
}

/// Decoding a superblock from its 1024 bytes, whatever they hold, does not panic, so no read
/// falls past the record: every field read indexes the slice, and an index past its end panics.
///
/// Falsification: attested 2026-11-22. Read one more word (`let _ = read_u32!();`) before
/// `s_checksum` in `Ext4Superblock::from_disk_bytes` in `superblock/disk.rs`, which moves the
/// checksum to bytes 1024 to 1027; the harness failed on the slice index in `read_u32_le`.
/// Replay as for `software_crc32c_tail_step_equals_bitwise_definition`, with `--harness
/// superblock_decode_reads_only_its_record`.
#[kani::proof]
fn superblock_decode_reads_only_its_record() {
    const SIZE: usize = 1024;
    let record: [u8; SIZE] = kani::any();
    assert_eq!(Ext4Superblock::disk_size(), SIZE);
    let superblock = Ext4Superblock::from_disk_bytes(&record);
    assert_eq!(
        superblock.s_magic,
        u16::from_le_bytes([record[56], record[57]])
    );
}

/// `Ext4GroupDesc::decode_checked` is total on every slice of up to 128 bytes: it never panics,
/// accepts exactly the 32-byte record and the power-of-two records from 64 bytes, and reads no
/// field of a 32-byte record from past its end (every high half decodes as zero).
///
/// Falsification: attested 2026-11-22. Made `is_supported_record_size` in
/// `blockgroup_description/disk.rs` accept any size from 32; the harness failed on its
/// `accepted` assertion and on a slice index. Replay as above, with `--harness
/// group_descriptor_decode_is_total_and_stays_in_its_record`.
#[kani::proof]
#[kani::unwind(2)]
fn group_descriptor_decode_is_total_and_stays_in_its_record() {
    let storage: [u8; 128] = kani::any();
    let bytes = arbitrary_prefix(&storage);
    let accepted = bytes.len() == 32 || bytes.len() == 64 || bytes.len() == 128;
    match Ext4GroupDesc::decode_checked(bytes) {
        Ok(desc) => {
            assert!(accepted);
            if bytes.len() == 32 {
                assert_eq!(desc.bg_block_bitmap_hi, 0);
                assert_eq!(desc.bg_inode_bitmap_hi, 0);
                assert_eq!(desc.bg_inode_table_hi, 0);
                assert_eq!(desc.bg_free_blocks_count_hi, 0);
                assert_eq!(desc.bg_free_inodes_count_hi, 0);
                assert_eq!(desc.bg_used_dirs_count_hi, 0);
                assert_eq!(desc.bg_itable_unused_hi, 0);
            }
        }
        Err(_) => assert!(!accepted),
    }
}

/// The three twelve-byte extent records decode from any slice of at least twelve bytes without
/// panicking, and read only those twelve bytes.
///
/// Falsification: attested 2026-11-22. Read `ee_start_lo` from `bytes[9..13]` in
/// `disknode/disk_format.rs`; the harness failed on the slice index. Replay as above, with
/// `--harness extent_records_decode_reads_only_twelve_bytes`.
#[kani::proof]
fn extent_records_decode_reads_only_twelve_bytes() {
    let record: [u8; 12] = kani::any();
    let mut longer = [0u8; 16];
    longer[..12].copy_from_slice(&record);
    longer[12..].copy_from_slice(&kani::any::<[u8; 4]>());

    let (a, b) = (
        Ext4ExtentHeader::from_disk_bytes(&record),
        Ext4ExtentHeader::from_disk_bytes(&longer),
    );
    assert_eq!(
        (
            a.eh_magic,
            a.eh_entries,
            a.eh_max,
            a.eh_depth,
            a.eh_generation
        ),
        (
            b.eh_magic,
            b.eh_entries,
            b.eh_max,
            b.eh_depth,
            b.eh_generation
        )
    );
    let (a, b) = (
        Ext4Extent::from_disk_bytes(&record),
        Ext4Extent::from_disk_bytes(&longer),
    );
    assert_eq!(
        (a.ee_block, a.ee_len, a.ee_start_hi, a.ee_start_lo),
        (b.ee_block, b.ee_len, b.ee_start_hi, b.ee_start_lo)
    );
    let (a, b) = (
        Ext4ExtentIdx::from_disk_bytes(&record),
        Ext4ExtentIdx::from_disk_bytes(&longer),
    );
    assert_eq!(
        (a.ei_block, a.ei_leaf_lo, a.ei_leaf_hi, a.ei_unused),
        (b.ei_block, b.ei_leaf_lo, b.ei_leaf_hi, b.ei_unused)
    );
}

/// `ExtentTree::parse_node` is total on every slice of up to 24 bytes (a header and one entry):
/// it never panics, and a node it accepts holds no more entries than fit in
/// the bytes after the header.
///
/// Falsification: attested 2026-11-22. Dropped both guards on a leaf entry in
/// `extents_tree/parse.rs`, `|| max > capacity` from the geometry check and the `entry_truncated`
/// test in the leaf loop; the harness failed on the slice index. Either guard alone holds the
/// claim, so dropping only one leaves the harness green. Replay as above, with `--harness
/// extent_node_parse_is_total`.
#[kani::proof]
#[kani::unwind(3)]
#[kani::solver(cadical)]
fn extent_node_parse_is_total() {
    let storage: [u8; 24] = kani::any();
    let bytes = arbitrary_prefix(&storage);
    let capacity = bytes.len().saturating_sub(12) / 12;
    if let Ok(node) = ExtentTree::parse_node(bytes) {
        let entries = match &node {
            ExtentNode::Leaf { entries, .. } => entries.len(),
            ExtentNode::Index { entries, .. } => entries.len(),
        };
        assert!(entries <= capacity);
        assert!(usize::from(node.header().eh_entries) == entries);
    }
}

/// `Ext4DirEntryInfo::parse_from_bytes` is total on every slice of up to 24 bytes, and the name
/// it returns lies inside the slice, after the eight-byte entry header.
///
/// Falsification: attested 2026-11-22. Dropped `|| data.len() < 8 + name_len` from
/// `entries/iterator.rs`; the harness failed on the slice index of the name. Replay as above,
/// with `--harness directory_entry_parse_is_total`.
#[kani::proof]
#[kani::unwind(2)]
fn directory_entry_parse_is_total() {
    let storage: [u8; 24] = kani::any();
    let data = arbitrary_prefix(&storage);
    if let Some(entry) = Ext4DirEntryInfo::parse_from_bytes(data) {
        let start = data.as_ptr() as usize;
        let name = entry.name.as_ptr() as usize;
        assert!(name == start + 8);
        assert!(name + entry.name.len() <= start + data.len());
        assert!(entry.inode != 0);
    }
}

/// Walking a directory block of up to 24 bytes ends, after at most one step per eight bytes, and
/// every entry it yields, name included, lies inside the record it was read from, and that record
/// inside the block.
///
/// Falsification: attested 2026-11-22. Dropped `|| rec_len as usize > remaining.len()` from
/// `DirEntryIterator::next` in `entries/iterator.rs`; the harness failed on the slice index of
/// the record. Replay as above, with `--harness directory_block_walk_ends_inside_its_block`.
#[kani::proof]
#[kani::unwind(5)]
#[kani::solver(cadical)]
fn directory_block_walk_ends_inside_its_block() {
    let storage: [u8; 24] = kani::any();
    let block = arbitrary_prefix(&storage);
    let start = block.as_ptr() as usize;
    let mut steps = 0usize;
    let mut previous_offset = None;
    for (entry, offset) in DirEntryIterator::new(block) {
        steps += 1;
        assert!(steps <= block.len() / 8);
        assert!(previous_offset.is_none_or(|previous| offset >= previous + 8));
        previous_offset = Some(offset);
        let rec_len = usize::from(u16::from_le_bytes([block[offset + 4], block[offset + 5]]));
        assert!(offset + rec_len <= block.len());
        let name = entry.name.as_ptr() as usize;
        assert!(name == start + offset + 8);
        assert!(name + entry.name.len() <= start + offset + rec_len);
    }
}

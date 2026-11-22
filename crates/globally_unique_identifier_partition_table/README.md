# globally_unique_identifier_partition_table

The globally unique identifier (GUID) partition table of the Unified Extensible Firmware
Interface (UEFI) 2.10, section 5.3, the GPT (GUID
Partition Table): reading one, writing one, and refusing a bad one. The three letters "GPT" are the
UEFI disk format and have nothing to do with language models. `no_std`, no allocation, no `unsafe`,
no I/O of any kind. Every function takes bytes the caller already read and returns bytes the caller
is about to place, so the tests run on the host against disks that real tools made. Its
dependencies are `uuid` (default features off) and `master_boot_record`, which writes and checks the
protective master boot record (MBR).

## The contract

- `PartitionTable::parse(header_block, entry_array)` judges the table against
  the disk: the header (`Header::decode`: signature, revision, size, both reserved regions, its own
  cyclic redundancy check (CRC)), the array CRC, the usable range, partitions off the end and
  partitions that overlap.
  `parse` takes the disk's size from the header and never sees the device, so a caller runs three more
  checks after it: `check_disk_size` against the device, `check_backup` against the last block, and
  `check_protective_master_boot_record` against block 0. `check_backup` compares the backup copy field by field.
  `parse_backup` opens a disk whose primary is damaged from its last block alone, and returns a
  table normalized to the primary's orientation, so `write_primary_header` and `entry_array`
  restore the primary. `create` builds a table, and
  `write_protective_master_boot_record`, `write_primary_header` and `write_backup_header` lay it out. `master_boot_record::validate`
  refuses a hybrid MBR on purpose (one disk described twice by tools that will
  not agree for long).
- `Entry::decode` judges nothing: every one of 2^1024 bit patterns is an entry, so the round trip
  is provable over all of them. An entry is unused when its type GUID is all zero, and only then.
- A GUID is mixed-endian on disk: the first three groups are little-endian integers and the last two
  are bytes in the order written. `guid::from_disk` and `guid::to_disk` are the only functions that
  see on-disk bytes, so a `Uuid` here is always in the order of Request for Comments 9562.
- `last_logical_block_address` is inclusive. A one-block partition has `first_logical_block_address == last_logical_block_address`.
- `entry_count * entry_bytes` from a hostile header is computed without overflow and returns an
  error, never a shortened CRC.
- `span::Span::covering` turns a byte range into whole transfers.
- `guid::types` names the well-known type GUIDs. A system's own private types belong to that system
  and are not listed; `guid::types::name` answers `None` for one it does not know, so a tool prints
  the raw GUID.
- What the two real fixtures taught: macOS writes no GPT partition names (72 zero bytes where the
  name goes), so nothing built on this crate may key off the name; identify a partition by type GUID
  or unique GUID. The two tools write different cylinder-head-sector (CHS) bytes in the protective
  MBR and both are right, so `master_boot_record::validate` checks only the fields that protect (start at logical
  block address (LBA) 1, cover the disk).

## EXAMPLES

```rust
use globally_unique_identifier_partition_table::PartitionTable;
use globally_unique_identifier_partition_table::testing::sample_disk;

// A 64 MiB disk with 512-byte blocks and one partition; only its first 34 blocks are real.
let disk = sample_disk();
// The array is not always at block 2: decode the header first and read `entry_array_logical_block_address`. Here
// the sample disk puts it at block 2.
let table = PartitionTable::parse(&disk[512..1024], &disk[1024..1024 + 16384])?;
for (index, partition) in table.partitions() {
    println!("{}: {} blocks", index + 1, partition.blocks().unwrap());
}
# Ok::<(), globally_unique_identifier_partition_table::Error>(())
```

## What is proven

Seventeen Kani harnesses, in `src/verification.rs`, each with a falsification patch in
`falsifications/` named `<module>.<harness>.patch`:

- `crc32_matches_its_bitwise_definition` and `a_single_byte_change_always_changes_the_crc`
- `an_entry_survives_the_round_trip` and `a_guid_survives_the_on_disk_layout`
- `a_headers_fields_survive_the_round_trip` and `the_header_fields_partition_the_block`
- `overlap_is_exactly_sharing_a_block` and `an_entries_block_count_never_panics_and_is_exact`
- `create_never_lays_out_a_table_parse_would_reject` and
  `a_backup_header_restores_the_table_it_was_written_from`
- `the_header_checks_before_the_crc_are_total` and
  `the_header_crc_in_pieces_is_the_crc_of_the_whole`, the two stages of `Header::decode`
- `a_span_always_contains_what_was_asked_for` and `a_name_always_decodes_into_its_documented_buffer`
- `the_entry_array_check_is_total`, `the_table_checks_are_total_on_one_entry` and
  `the_partition_checks_are_total_on_one_entry`, which with the two `Header::decode` stages make
  `parse` total on a 512-byte block and a one-entry array. Two of them replace a function with
  what its own harness proves of it, so the proofs need Kani's `-Z stubbing`

The proofs need Kani, which `rustup` does not install. Run `cargo install --locked
kani-verifier --version 0.68.0`, then `cargo kani setup`. The setup step downloads a model
checker, so it needs network access. CI uses version 0.68.0, and another version may verify
differently.

Where a domain is small enough to count, the tests count it: every single-byte corruption of a real
header block and of a small table. Run the proofs with `cargo kani -Z stubbing`. Applying a patch with `patch -p3`
from the crate root makes its harness fail at its own assertion.

## What is on the disk

```text
  LBA 0     protective MBR   one fake record of type 0xEE covering the whole disk
  LBA 1     primary header   92 bytes: signature, geometry, two CRC-32s, the disk GUID
  LBA 2..   entry array      128 entries x 128 bytes = 16 KiB
  ...       the partitions, between first_usable_logical_block_address and last_usable_logical_block_address
  LBA n-33  backup array     the same 16 KiB again
  LBA n-1   backup header    the same header, my_logical_block_address and alternate_logical_block_address swapped
```

Four CRC-32s (a header CRC and an array CRC, each stored twice) make a GPT a format that can tell
you it is broken, so every check here is an error and never a warning.

## Building and testing

```sh
cargo test --release     # the exhaustive sweeps take minutes in a debug build
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
cargo kani -Z stubbing
```

## Regenerating the fixtures

Each fixture in `tests/fixtures/` is the first 34 and last 33 blocks of a 64 MiB image with
512-byte blocks: the whole primary table and the whole backup table, without the space between.

```sh
# sgdisk (gptfdisk)
dd if=/dev/zero of=gpt64.img bs=1m count=64
sgdisk -n 1:2048:+4M  -t 1:EF00 -c 1:"EFI system"   gpt64.img
sgdisk -n 2:0:+10M    -t 2:8300 -c 2:"linux root"   gpt64.img
sgdisk -n 3:0:0 -t 3:EC5CC08B-D749-4434-AC38-A274C50385BA -c 3:"cricker data" gpt64.img
sgdisk -U 1A2B3C4D-5E6F-4718-9A0B-C1D2E3F40506 gpt64.img
sgdisk -u 1:11111111-2222-4333-8444-555555555501 \
       -u 2:11111111-2222-4333-8444-555555555502 \
       -u 3:11111111-2222-4333-8444-555555555503 gpt64.img
dd if=gpt64.img of=sgdisk-64m.head bs=512 count=34
dd if=gpt64.img of=sgdisk-64m.tail bs=512 skip=131039 count=33

# macOS, no root needed for a disk image
dd if=/dev/zero of=apple64.img bs=1m count=64
hdiutil attach -nomount -imagekey diskimage-class=CRawDiskImage apple64.img   # prints /dev/diskN
diskutil partitionDisk /dev/diskN GPT MS-DOS ESP 16M HFS+ MACDATA 0b
hdiutil detach /dev/diskN
dd if=apple64.img of=apple-64m.head bs=512 count=34
dd if=apple64.img of=apple-64m.tail bs=512 skip=131039 count=33
```

## BUGS

- A header that declares more than 92 bytes is re-emitted as 92 by `write_primary_header`.
- The reader checks the header's reserved tail and tolerates a non-zero entry tail and non-zero
  reserved attribute bits: the header tail protects the CRC's meaning, and nothing this crate reads
  depends on the others.
- `uuid` is re-exported as `globally_unique_identifier_partition_table::uuid`, so a caller names the
  type this crate returns without a second manifest line at a matching major.

## Minimum Rust version

Rust 1.89 (the `uuid` dependency requires it).

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.

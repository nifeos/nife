# globally_unique_identifier_partition_table

The GUID Partition Table (UEFI 2.10, section 5.3): reading one, writing one, and refusing a bad one.
The three letters "GPT" are the UEFI disk format and have nothing to do with language models.
`no_std`, no allocation, no `unsafe`, no I/O of any kind. Every function takes bytes the caller
already read and returns bytes the caller is about to place, so the tests run on the host against
disks that real tools made. The only dependency is `uuid` (default features off). It was written
for the nife kernel and its disk programs.

## What is on the disk

```text
  LBA 0     protective MBR   one fake record of type 0xEE covering the whole disk
  LBA 1     primary header   92 bytes: signature, geometry, two CRC-32s, the disk GUID
  LBA 2..   entry array      128 entries x 128 bytes = 16 KiB
  ...       the partitions, between first_usable_lba and last_usable_lba
  LBA n-33  backup array     the same 16 KiB again
  LBA n-1   backup header    the same header, my_lba and alternate_lba swapped
```

Four CRC-32s (a header CRC and an array CRC, each stored twice) make a GPT a format that can tell
you it is broken, so every check here is an error and never a warning.

## The contract

- `GloballyUniqueIdentifierPartitionTable::parse(header_block, entry_array)` judges the table against
  the disk: the header (`Header::decode`: signature, revision, size, both reserved regions, its own
  CRC), the array CRC, the usable range, partitions off the end and partitions that overlap.
  `check_backup` compares the backup copy field by field. `create` builds a table, and
  `write_protective_mbr`, `write_primary_header` and `write_backup_header` lay it out. `mbr::validate`
  refuses a hybrid MBR on purpose (one disk described twice by tools that will not agree for long).
- `Entry::decode` judges nothing: every one of 2^1024 bit patterns is an entry, so the round trip
  is provable over all of them. An entry is unused when its type GUID is all zero, and only then.
- A GUID is mixed-endian on disk: the first three groups are little-endian integers and the last two
  are bytes in the order written. `guid::from_disk` and `guid::to_disk` are the only functions that
  see on-disk bytes, so a `Uuid` here is always in RFC 9562 order.
- `last_lba` is inclusive. A one-block partition has `first_lba == last_lba`.
- `entry_count * entry_size` from a hostile header is computed without overflow and returns an
  error, never a shortened CRC.
- `span::Span::covering` turns a byte range into whole transfers, and `VERSION` (1) declares the
  generation of this crate's reading and writing, so that assembly can check what a package was
  built against. Nothing checks it at run time.
- `guid::types` names the well-known type GUIDs, and holds two this project owns, `NIFE_DATA` (a
  RedoxFS data partition) and `NIFE_BOOT` (a boot slot). Those two are fixed forever.
- What the two real fixtures taught: macOS writes no GPT partition names (72 zero bytes where the
  name goes), so nothing built on this crate may key off the name; identify a partition by type GUID
  or unique GUID. The two tools write different CHS bytes in the protective MBR and both are right,
  so `mbr::validate` checks only the fields that protect (start at LBA 1, cover the disk).

## Usage

```rust
use globally_unique_identifier_partition_table::GloballyUniqueIdentifierPartitionTable;
use globally_unique_identifier_partition_table::testing::sample_disk;

// A 64 MiB disk with 512-byte blocks and one partition; only its first 34 blocks are real.
let disk = sample_disk();
let table = GloballyUniqueIdentifierPartitionTable::parse(&disk[512..1024], &disk[1024..1024 + 16384])?;
for (index, partition) in table.partitions() {
    println!("{}: {} blocks", index + 1, partition.blocks().unwrap());
}
# Ok::<(), globally_unique_identifier_partition_table::Error>(())
```

## What is proven

Eight Kani harnesses, in `src/verification.rs`, each with a falsification patch in
`falsifications/` named `<module>.<harness>.patch`:

- `crc32_matches_its_bitwise_definition` and `a_single_byte_change_always_changes_the_crc`
- `an_entry_survives_the_round_trip` and `a_guid_survives_the_on_disk_layout`
- `a_headers_fields_survive_the_round_trip` and `the_header_fields_partition_the_block`
- `overlap_is_exactly_sharing_a_block`
- `create_never_lays_out_a_table_parse_would_reject`

Where a domain is small enough to count, the tests count it: every single-byte corruption of a real
header block and of a small table. Run the proofs with `cargo kani`. Applying a patch with `patch -p3`
from the crate root makes its harness fail at its own assertion.

## Building and testing

```sh
cargo test --release     # the exhaustive sweeps take minutes in a debug build
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
cargo kani
```

The minimum supported Rust version is 1.89 (the `uuid` dependency requires it). The crate opts out of `missing_docs` until its 23
undocumented items are documented.

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

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.

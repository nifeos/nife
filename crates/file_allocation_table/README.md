# file_allocation_table

A FAT12, FAT16 and FAT32 filesystem as pure computation, growing in stages. It does no I/O and
needs no allocator (`no_std`). Two halves exist.

The reader opens a volume through `SectorDevice`, a trait you implement over anything that reads
whole sectors. It checks the boot sector, decodes the file allocation table, walks cluster chains
and reads directories slot by slot. Every byte it reads is hostile: it never panics, never reads
outside the volume, and no table contents can make a chain walk loop. It does not yet assemble long
names or read a file's bytes.

The writer creates an EFI system partition holding one boot file. You give it a partition size, a
file length and a file name; it tells you what every metadata sector must contain, and you write
those sectors and then the file's bytes. It writes VFAT long-name entries for a name that is not
8.3, such as `BOOTRISCV64.EFI`. Extending, deleting, renaming and a general format are planned.

## What is proven

Nine Kani harnesses cover the reader. The four on the boot sector cover every 512-byte sector:
what the parse accepts lies inside the volume, the FAT type is the cluster count's, the table has
an entry for every cluster, and only a data cluster has sectors. Five more cover the rest:

- `every_entry_lies_inside_the_table` and `an_entry_decodes_by_its_markers`: for every boot
  sector and cluster number, a table entry lies inside the table, and every raw value decodes by
  the specification's markers.
- `no_chain_walk_outlasts_the_volume`: over a table whose entries change on every read, a walk
  yields only data clusters, at most as many as the volume holds, then ends. Volumes of one to
  eight clusters.
- `every_slot_parses_as_its_bytes_say` and `a_directory_reads_only_its_own_sectors`: every
  32-byte slot parses as its first byte and attributes say, and a directory read visits only its
  region's or its clusters' sectors.

Three cover the long-name writer:

- `the_checksum_is_the_specifications_loop`: the checksum that binds a long name to its short
  alias equals the specification's shift-and-add loop for every one of the 2^88 short names.
- `every_long_name_run_is_well_formed`: for every name up to 255 UCS-2 units, the run of long-name
  entries has the right count, ordinals, attribute, checksum and padding, and reads back as the
  name.
- `every_alias_is_a_legal_short_name`: every generated `~1` alias is either refused or a legal
  11-byte short name.

Rerun them with `cargo kani`. Each harness has a falsification record at
`falsifications/<module.path>.<harness>.patch`; applying it with `patch -p3` from the crate root
makes that harness fail at its own assertion, which shows the proof checks something.

Unit tests cover the volume layout (boot sector, `FSInfo`, both FATs, directory entries, cluster
alignment), and an ignored test has the host's own FAT driver check and read an image
(`cargo test -- --ignored`, on macOS).

`tests/the_oracle_agrees.rs` checks the crate against rust-fatfs (the `lamfat` dev-dependency):
rust-fatfs writes a tree on FAT12, FAT16 and FAT32 and this crate must read the same names, sizes,
bytes and free clusters, and rust-fatfs must read the writer's volume. Where `fsck.fat` or
`fsck_msdos` is installed, every image must pass it too.

## Usage

A 512 MiB EFI system partition at LBA 2048 holding a 9 MB boot file:

```rust
use file_allocation_table::{SECTOR_BYTES, VolumeLayout};

let volume = VolumeLayout::new(1024 * 1024, 2048, 9_000_000, 0x1234_5678, "BOOTX64.EFI")
    .expect("512 MiB is a FAT32 volume");

let mut sector = [0u8; SECTOR_BYTES];
for index in 0..volume.metadata_sectors() {
    assert!(volume.fill_sector(index, &mut sector));
    // write `sector` at partition-relative LBA `index`
}
// then the file's own bytes, contiguous, at this LBA:
assert_eq!(volume.file_first_sector(), volume.metadata_sectors());
```

`VolumeLayout::new` refuses a partition below 65,525 data clusters (about 256 MiB, since the cluster
count decides the FAT type), one too large for FAT32, a name no VFAT entry may carry, and a file
that does not fit.

## Limitations

- One file per volume. A long name's alias is always `~1`, which is right only because no other
  name can collide with it.
- Sectors are 512 bytes and clusters 4096 bytes; nothing else is offered.
- Every timestamp is zero.
- The `FSInfo` free-cluster count is written once.
- Not crash-atomic, and there is no journal, since FAT has none.
- The reader stops at directory slots: it reports long-name pieces as stored, without assembling
  them or checking their checksum, and reads no file's bytes.
- A loop in a hostile table stops a chain walk only at the volume's cluster count.

## Minimum Rust version

Rust 1.88.

## License

Licensed under either of the MIT license ([LICENSE-MIT](LICENSE-MIT)) or the Apache License,
Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE)), at your option.

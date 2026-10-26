# file_allocation_table

Enough of FAT32 to create an EFI system partition holding one boot file, as pure computation. You
give it a partition size, a file length and a file name; it tells you what every metadata sector
must contain, and you write those sectors and then the file's bytes. It does no I/O, needs no
allocator (`no_std`), and writes VFAT long-name entries for a name that is not 8.3, such as
`BOOTRISCV64.EFI`.

It is a `mkfs`, not a filesystem: one volume, one file at `\EFI\BOOT\<name>`, then finished. It
cannot read, open, extend, delete or rename anything yet. Read support, FAT12 and FAT16, and a
general format are planned.

## What is proven

Three Kani harnesses cover the long-name writer:

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

The volume layout itself (boot sector, `FSInfo`, both FATs, directory entries, cluster alignment)
is covered by unit tests, and an ignored test has the host's own FAT driver check and read an image
(`cargo test -- --ignored`, on macOS).

## Usage

A 512 MiB EFI system partition at LBA 2048 holding a 9 MB boot file:

```rust
use file_allocation_table::{SECTOR, Volume};

let volume = Volume::new(1024 * 1024, 2048, 9_000_000, 0x1234_5678, "BOOTX64.EFI")
    .expect("512 MiB is a FAT32 volume");

let mut sector = [0u8; SECTOR];
for index in 0..volume.metadata_sectors() {
    assert!(volume.sector(index, &mut sector));
    // write `sector` at partition-relative LBA `index`
}
// then the file's own bytes, contiguous, at this LBA:
assert_eq!(volume.file_first_sector(), volume.metadata_sectors());
```

`Volume::new` refuses a partition below 65,525 data clusters (about 256 MiB, since the cluster
count decides the FAT type), one too large for FAT32, a name no VFAT entry may carry, and a file
that does not fit.

## Limitations

- One file per volume. A long name's alias is always `~1`, which is right only because no other
  name can collide with it.
- Sectors are 512 bytes and clusters 4096 bytes; nothing else is offered.
- Every timestamp is zero.
- The `FSInfo` free-cluster count is written once.
- Not crash-atomic, and there is no journal, since FAT has none.
- Nothing in the crate reads back what it writes.

## Minimum Rust version

Rust 1.88.

## License

Licensed under either of the MIT license ([LICENSE-MIT](LICENSE-MIT)) or the Apache License,
Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE)), at your option.

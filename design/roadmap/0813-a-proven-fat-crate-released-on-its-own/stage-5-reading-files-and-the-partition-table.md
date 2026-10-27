# Stage 5: reading files and the partition table

Appendix to [813. `file_allocation_table`](../0813-a-proven-fat-crate-released-on-its-own.md). It
records what stage 5 built and the evidence. Milestone 140 (mount a drive this system did not create) can start its read-only FAT stratum from
this stage.

## What stage 5 built

- `Volume::lookup(path)` and `Volume::find(directory, name)`. A path joins components with `/` or
  `\`, and lookup skips empty components. Each component matches an entry by its long name or its
  short name, without regard to case. A long name folds each UTF-16 unit by its simple Unicode
  upper-case mapping. A short name compares as its dotted ASCII form, so a query with a non-ASCII
  character matches only a long name. `.` and `..` are names like any other: a subdirectory holds
  them and the root does not. `NamedEntry::is_called` asks the question of one entry. The errors are
  `LookupError`: `Chain`, `NotFound`, `NotADirectory` and `NoName` (the root has no entry).
- `Volume::open_file(&entry)` returns a `FileReader`, which reads forward with `read` and moves with
  `seek`. A read returns fewer bytes than the buffer holds only at the end of the file. It never
  returns a byte past the size the entry states. When the chain is longer than the size, a read
  stops at the size. A chain shorter than the size fails with `ShorterThanItsSize` after the bytes it
  covers. A looping chain fails with `TooLong` at the volume's cluster count. After an error the
  reader returns `AfterAFailure`, so a caller cannot skip bytes by ignoring one. `open_file`
  refuses a directory and the volume label.
- `MasterBootRecord`, `Partition` and `PartitionDevice`. The parse reads the four entries at byte
  446 and the signature. It refuses a bad status byte, a used entry with no sectors, and a partition
  that ends past sector 2^32; the parse ignores an unused entry (type 0) whatever else it holds.
  `Partition::device` wraps any `SectorDevice` so that sector 0 is the partition's first sector and
  the wrapper refuses a sector past its length without a read. `Volume::open` over that device opens the
  volume inside the partition. A caller tries `Volume::open` first for a bare volume, because a FAT
  boot sector carries the same signature as an MBR.

No nife type appears in a public signature.

## The evidence

| check | result |
|---|---|
| Kani, three new harnesses | `every_sector_parses_to_a_sound_table_or_an_error` covers every 512-byte sector. `a_partition_device_reads_only_its_partition` covers every partition and sector index. `a_file_read_stays_inside_its_size_and_its_clusters` covers any size, first cluster, request size and seek over volumes of one to three clusters of one, two or four sectors |
| falsification | each harness has a patch that turns it red: an end check that allows 2^33, an off-by-one partition end, and a span that may run one byte past the size |
| unit tests | files across clusters read in chunks of 1 to 700 bytes, seeks both ways, each chain failure, a failing device, entry kinds `open_file` refuses, every FAT type byte, each partition table refusal, case folding for long and short names, paths through a subdirectory and `..`, and a stale long-name run |
| #1803's images | the bare image opens as a volume. The MBR image opens through its partition table (type `0x0B`, first sector 63). Every file in both reads to its recorded hash through `open_file`, and lookup finds each in two or three spellings |
| oracle tests | on FAT12, FAT16 (512 and 4096-byte sectors) and FAT32 volumes rust-fatfs wrote, every file read through `open_file` equals rust-fatfs's bytes and a raw read of the chain. Every path is found in rust-fatfs's spelling, in upper case, in lower case and by short name. The FAT12, FAT16 and FAT32 volumes with 512-byte sectors also read through an MBR that wraps them. The volume this crate's writer lays out reads back by path and by alias |
| fuzz target `file_allocation_table_boot_sector` | reads every file to its end, holds the bytes to the size, checks a failed reader stays failed, and opens the volumes an MBR names |

Kani bounds what it unrolls. The file harness covers three clusters at most; the chain walk's own
bound is the cluster count, and the fuzz target holds whole images.

## Names minted

Every name in this stage is provisional, with a `Name: provisional` block in its source.

- `FileReader`, `FileReadError` (`Chain`, `ShorterThanItsSize`, `AfterAFailure`), `NotAFile`,
  `Volume::open_file`, and the methods `size_bytes`, `position`, `seek` and `read`.
- `LookupError`, `Volume::find`, `Volume::lookup` and `NamedEntry::is_called`.
- `MasterBootRecord` (`parse`, `read`, `partition`, `partitions`), `MasterBootRecordError`,
  `MasterBootRecordReadError`, `PARTITION_COUNT`, `Partition` (`is_bootable`, `kind`,
  `first_sector`, `sector_count`, `holds_file_allocation_table`, `device`), `PartitionDevice` and
  `PartitionError`.
- The modules `file`, `lookup` and `master_boot_record`.

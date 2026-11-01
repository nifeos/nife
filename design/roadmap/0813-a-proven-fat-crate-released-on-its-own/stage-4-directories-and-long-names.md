# Stage 4: directories and long names

Appendix to [813. `file_allocation_table`](../0813-a-proven-fat-crate-released-on-its-own.md). It
records what stage 4 built, what the macOS fixtures hold and the evidence.

## What stage 4 built

- `LongFileName` and `LongFileNameError`: the assembled name (up to 255 UTF-16 code units, with
  `characters()` to decode them) and the reason a run failed its checks. The assembler behind them
  stays inside the crate, in `src/long_name_assembly.rs`, beside the writer it mirrors.
- The assembler takes pieces in directory order and then a short entry's name. It accepts a run
  when all of these hold:
  - the first piece carries the last-piece bit with an ordinal from 1 to 20;
  - the ordinals fall by one to 1;
  - every piece carries one checksum, and it equals the checksum of the short name;
  - the name ends at one `0x0000`, and only `0xFFFF` follows;
  - the last piece holds at least one unit, and the name has at most 255 units.

  It refuses anything else whole.
- `NamedEntry` and `NamedEntries`, from `Volume::root_named_entries` and `Volume::named_entries`, yield every used
  short entry in order with its short name, attributes, first cluster, size and long name. They
  yield `.`, `..` and the volume label too, and flag them (`is_dot_entry`, `is_volume_label`).
- A refused run does not stop the listing. The entry still comes out with no long name and the
  reason in `long_name_refusal`, because a stranger's volume may hold a stale piece and its files
  stay reachable by their short names.
- A new first piece drops a run that never reached a short entry. A deleted file's pieces are free
  slots, and the directory reader skips them with it.

No nife type appears in a public signature.

## The macOS fixtures

`tests/fixtures/macos/` holds the two FAT32 images that
`notes/filesystem-crates-2026-10-07/make-images.sh` builds on a macOS host, compressed with zstd:
one bare, one inside an MBR. Its README records the provenance and the hash of every file.
`tests/the_macos_images_read.rs` decompresses them, lists every file with this crate, reads each
one through its chain, and checks `hello.txt`, `sub/data.bin` and `Long File Name.txt` against
`expected.sha256` with a SHA-256 the test carries. It always runs. It opens the MBR image at the
partition's first sector, which the test reads from the partition table itself; the crate reads a
partition table in stage 5.

macOS stores the names of 8.3-shaped files in upper case and leaves a `.fseventsd` directory on a
volume it mounted. The test compares paths without regard to case and ignores what the script did
not write.

`ruzstd` 0.9.1 (pure Rust, MIT) decompresses them. It is a `[dev-dependencies]` line of this crate
only, beside `lamfat`.

## The evidence

| check | result |
|---|---|
| Kani, one new harness | `a_run_is_accepted_only_when_it_is_well_formed`. For up to three pieces and any short name, an accepted run has the last-piece bit on its first piece and ordinals that fall by one to 1. Every piece carries the short name's checksum. The name equals the pieces' units up to the first terminator, with only padding after it |
| falsification | the harness's patch removes the checksum comparison, and the harness fails at its checksum assertion |
| unit tests | writer-to-assembler round trips, and each refusal. The refusals cover another entry's checksum, a missing nearest piece, a missing first piece, pieces out of order or repeated, disagreeing checksums, a bad first ordinal, bad padding, an empty last piece and an over-long name. A 255-unit name passes |
| oracle tests | this crate lists the trees rust-fatfs writes on FAT12, FAT16 and FAT32 through `NamedEntries`, long names included, and accepts every run; rust-fatfs reads the volume this crate's writer lays out |
| #1803's images | both read to their recorded hashes, with the long name in each |
| fuzz target `file_allocation_table_boot_sector` | walks through `NamedEntries` and holds an accepted name to 255 units with no terminator |

Kani bounds what it unrolls. The assembly harness covers runs of up to three pieces (39 units)
with a first ordinal of at most three. The unit tests hold the twentieth piece and the longest
name, and the fuzz target holds whole images.

## Names minted

Every name in this stage has a ratified `Name:` block in its source.

- `LongFileName`, with `code_units` and `characters`.
- `LongFileNameError`, with `BadStart`, `OutOfOrder`, `PiecesDisagree`, `Incomplete`,
  `ChecksumMismatch` and `BadPadding`.
- `NamedEntry`, with `short_entry`, `short_name`, `long_name`, `long_name_refusal`, `attributes`,
  `first_cluster`, `size_bytes`, `is_directory`, `is_volume_label` and `is_dot_entry`.
- `NamedEntries`, `Volume::root_named_entries` and `Volume::named_entries`.
- The modules `named_entry` and `long_name_assembly`.

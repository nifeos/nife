# Stage 2: the sector interface, the boot sector, and the oracle's pin

Appendix to [813. `file_allocation_table`](../0813-a-proven-fat-crate-released-on-its-own.md). It
records what stage 2 built, the provenance of the names it minted, and the open question on the
oracle's dev-dependency pin, which this stage did not need and stage 3 does.

## What stage 2 built

- `SectorDevice`, the sector interface. A caller states a `SectorSize` (512, 1024, 2048 or 4096)
  and implements `read_sector(index, buffer)`, where `buffer` is always one sector. It names no
  nife type, no capability and no partition: a volume inside a partition is read through an
  implementation that offsets the index. A blanket implementation covers `&mut D`.
- `BootSector::parse`, over any byte slice, reading only bytes 0 to 511. It requires the signature
  and the jump, checks every BPB field a later read depends on, and refuses with a `ParseError`
  that names the field. What it accepts has its regions in order inside its own sector count, at
  least one cluster, and a table entry for every cluster.
- The FAT type by cluster count, per Microsoft's specification, with the type string never read. A
  BPB whose layout contradicts its count is refused. The case met in the field is the FAT32 layout
  under 65525 clusters that `mkfs.fat -F 32` makes with a warning; Linux reads it as FAT32 and the
  specification as FAT16, so no answer here can agree with both.
- `BootSector::read`, which reads sector 0 through a device and refuses a volume whose sector size
  is not the device's.
- FAT32's pointers into the reserved region (`FSInfo`, the backup boot sector) are kept only when
  they name a reserved sector. Otherwise they read as `None` instead of refusing the volume, which
  is what Linux does with a bad `FSInfo` number.

The evidence:

| check | result |
|---|---|
| Kani, four harnesses over every 512-byte sector | `what_the_parse_accepts_lies_inside_the_volume`, `the_fat_type_is_the_cluster_counts`, `the_table_has_an_entry_for_every_cluster`, `only_a_data_cluster_has_sectors`; 27 seconds together |
| falsification sweep | every patch red, each at its own assertion |
| fuzz target `file_allocation_table_boot_sector` | 26 million executions in 120 seconds, no crash |
| `cargo mutants --in-diff` | 168 caught, 8 unviable, none missed |
| line coverage | `boot_sector.rs` and `sector.rs` 100% |
| fixtures | six boot sectors dosfstools' `mkfs.fat` 4.2 formatted (FAT12, FAT16 and FAT32, at 512 and 4096 bytes), rebuilt by `tests/fixtures/make-boot-sectors.sh` |

#1803's macOS images are 64 MiB each and are not in the tree, and no macOS host was at hand, so
this stage reads no macOS-made boot sector. Stage 4 reads both images to their hashes.

## Names minted, all provisional

The crate's sources carry no `Name:` blocks, because the crate publishes on its own and its
sources cite nothing of this tree. The provenance lives here instead. Each name is provisional,
minted on 2026-10-25 (UTC) by `813-stage-2-boot-sector`, and calef has named none of them.

| name | why this one |
|---|---|
| `SectorDevice` | the thing the caller supplies is a device addressed by sector. `BlockDevice`, the word `embedded-sdmmc` and `hadris-fat` use, was set aside: FAT's unit is the sector, and this tree's own block protocol is a different thing |
| `SectorSize` | the four sizes the specification permits, as an enum so a wrong size cannot be represented |
| `BootSector` | the specification's name for sector 0 |
| `FatType` | the specification's term ("FAT type"). `Fat12`, `Fat16` and `Fat32` keep Microsoft's spelling, as a name arriving across somebody else's format does |
| `RootDirectory` | where the root lives: a region on FAT12 and FAT16, a cluster on FAT32 |
| `ParseError`, `ReadError` | a refusal of the bytes, and a failure of the read that fetched them |
| `boot_sector`, `sector` | the modules, named for the on-disk structure each one parses |

## Open: the oracle's pin (needs calef, before stage 3)

calef's #2580 ruling keeps rust-fatfs as the test oracle, outside the base graph, and 813 says the
pin is a dependency ruling. Stage 2 needed no oracle: a boot sector is checked against another
tool's output, and `mkfs.fat` supplied it. Stage 3 reads FAT entries that rust-fatfs wrote, so it
needs the pin first. In every option the oracle is a `[dev-dependencies]` line of this crate
only. It never enters a binary that boots.

- **A. `lamfat = "=0.4.3"` from crates.io (recommended).** It republishes rust-fatfs's main line
  under MIT, by Lamco Development LLC. 0.4.3 shipped on 2026-10-15, and #1803 measured 0.4.2
  clean under `fsck_msdos`. `Cargo.lock` pins its checksum, it needs no repository of ours, and the
  standalone crate's own CI resolves it the same way. The risk is trusting a third party's copy of
  upstream, which a test oracle can bear: a wrong oracle fails tests, it does not ship.
- **B. A `nifeos/rust-fatfs` fork pinned by `rev`** to upstream's main-line commit `38b3a795`
  (2026-10-16), as §305 (git dependencies on the nifeos organization only) allows. Provenance runs
  straight to upstream's own commit. It costs a repository someone with the organization's rights
  must create and keep, and cargo drops a version-less dev-dependency when the crate publishes.
- **C. No linked oracle.** Read back with tools run as processes: `fsck.fat` or `fsck_msdos`, and
  mtools (`mdir`, `mcopy`) for names and bytes. No dependency at all. But mtools is not on a stock
  macOS, and the round trip in the other direction (this crate reading what rust-fatfs wrote)
  needs a second writer, which `mkfs.fat` and mtools can supply only in part.

`fatfs = "=0.3.6"` is not offered: #1803 found its subdirectories rejected by `fsck_msdos`.

Recommendation: A. It has the fewest moving parts and puts nothing of ours to maintain behind an
oracle, and I would choose it at equal effort, because B's extra provenance buys little for code
that only judges tests. B is right if calef will not trust a republisher with the oracle. If no
ruling arrives, stage 3 builds its decode and chain walk against `mkfs.fat` and mtools images and
adds the oracle tests when the pin lands.

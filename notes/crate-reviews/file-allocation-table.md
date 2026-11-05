# Review package: `file_allocation_table`

*Name: provisional, minted by lane milestone/813-review-package. The `file_allocation_table`
package of [crate-reviews.md](../crate-reviews.md), in its own file because its signature list
alone outgrows the other packages.*

The architect reads this to clear `crates/file_allocation_table` for its move to its own repository
and its first publication ([journey 5](../../design/journeys/05-crates-released-on-their-own.md),
step 2; [milestone 813 (a proven FAT crate, released on its own)](../../design/roadmap/0813-a-proven-fat-crate-released-on-its-own.md)).
It describes the crate as #3267 leaves it, with no reference to nife. Every public item and its
signature is in [the public API appendix](file-allocation-table-public-api.md). The questions close
this page, each with its background, a recommendation and, for six of the seven, the maintainer's
provisional ruling, which #3267 applied.

## In one paragraph

FAT12, FAT16 and FAT32 as pure computation, `no_std` with no allocator, no `unsafe` and no run-time
dependency: read, write and format. Kani proves the parsers and the table, allocation and write
steps; the tests check what the crate writes against rust-fatfs and `fsck.fat`; the README alone
builds, tests and proves it. It passes every gate. What it needs is the architect's clearance of
the API the provisional rulings below leave it, and of the version, tag and repository.

## Identity

| item | value |
|---|---|
| crate | `file_allocation_table`, ratified on #1806 (FAT spelled out); crates.io answers 404 for it |
| version and tag | `0.1.0` in the manifest; the tag `v0.1.0` goes on the move commit, as `nifeos/jh7110_entropy`'s did |
| repository | `nifeos/file_allocation_table`, the crate's own name, as the pilot's is; GitHub answers 404 for it |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0`, both texts at the crate root |
| `publish` | `false`; `crates/publish-allowlist.txt` lists no crate |
| size | 7,676 lines of library source, 1,516 of Kani harnesses, 5,450 of unit tests, 3,339 of integration tests |
| package | `cargo package` packs 119 files, 736 KiB (187 KiB compressed), the 31 falsification patches and the test fixtures included |

## What is surprising in the API

- No error implements `Display` or `core::error::Error`, so a caller cannot `?` one into a boxed
  error. Adding both later breaks nothing, so this need not hold the release.
- A directory is a `u32` cluster number, and 0 means the root. The crate reads any other number
  as a directory's first cluster, and the chain walk, not the type system, refuses a wrong one.
- Reads cannot see timestamps. The writer stamps creation, write and access times, but neither
  `NamedEntry` nor `ShortEntry` returns them. Adding a method later breaks nothing.
- `CivilDateTime` has public fields and stops at seconds, though FAT's creation time carries
  hundredths. A new field would break callers.
- `VolumeLayout::new` takes seven positional arguments; see the label section below.

What the rulings settled: every error enum is `#[non_exhaustive]`; `VolumeLayout` publishes five
methods, none of which can panic; its constants live in `efi_system_partition`, not at the root;
the table-editing primitives are crate-private. No shipped text reads as this tree's.

## Names

`python3 helpers/name_release.py --draft file_allocation_table` lists 365 names: the crate, 58
modules and 306 items. Every name a stranger meets has a `Name:` block, and the move commit strips
them all.

| source | names |
|---|---|
| #1806 (the crate) | 1 |
| #2720 (the reader's and the EFI writer's first names) | 94 |
| #2760 (directories and long names) | 55 |
| #2996, #2999 and #3001 (stages 6 to 9, the clock, and the class rule on `tests` and `verification`) | 214 |
| #3267 (`LayoutError::LabelRefused`, question 1) | 1 |
| provisional | none |

## Proofs

Each of the 31 Kani harnesses has a falsification patch in `falsifications/`. The stranger copy
proved them all with stock Kani 0.68.0, the version the shared template pins.

| area | harnesses | what they prove |
|---|---|---|
| boot sector and BPB | 4 | any 512-byte sector parses or meets a refusal; what parses lies inside the volume; the type is the cluster count's; the table has an entry per cluster; only data clusters have sectors |
| table | 4 | every entry lies inside the table; decoding by markers; a stored entry reads back and disturbs nothing else; encoding inverts decoding |
| chain walk | 1 | no walk outlasts the volume's cluster count (volumes of one to eight clusters) |
| directories | 3 | every slot parses as its bytes say; a directory reads only its own sectors; the run finder takes the first run of free slots |
| long names | 5 | the checksum is the specification's loop; every written run is well formed; every alias is a legal short name and carries its number; the reader accepts only a well-formed run |
| file read | 1 | a read stays inside the file's size and clusters |
| allocation | 3 | allocation takes one free cluster and nothing else; a release frees only its own chain and never a bad cluster; a cut keeps exactly the head |
| write cursor | 2 | a partial sector write keeps the head and zeroes the slack; a write extends the chain to the size and stays inside it |
| `FSInfo` | 2 | any sector parses to hints inside the volume; a store changes only the two hints |
| format | 1 | a computed geometry parses back as itself (clusters of 1 to 8 sectors) |
| partition table | 2 | any sector parses to a sound table or an error; a partition device reads only its partition |
| attributes, label, time | 3 | setting attributes changes the four user bits only; an accepted label is legal bytes then spaces; any time packs to legal fields |

What the proofs leave out, and what covers it:

- Whole images. Kani proves per sector and over model volumes of a few clusters. The oracle tests
  and two fuzz targets in nife's `fuzz/` (`file_allocation_table_boot_sector` and
  `file_allocation_table_writer`) cover whole images. Question 6.
- The operations built from the proven steps: create, rename, remove, path lookup, name
  comparison, directory growth and `VolumeLayout`. Tests and the writer fuzz target cover them.
- Crash consistency, which FAT lacks. The crate orders its writes so a crash leaks a cluster and
  never shares one; tests check that order, and no proof does.
- The caller's device. The crate assumes `read_sector` fills its buffer and the sector size holds
  still.

## Tests

On Rust 1.88 and on stable, 300 pass and one stays ignored: 244 unit tests, 47 integration tests in
12 files, and 9 doctests. `cargo clippy --all-targets` and `cargo doc` print no warning.

- The oracle. `lamfat` (rust-fatfs's main line) reads every tree this crate writes on FAT12, FAT16
  and FAT32, and this crate reads every tree `lamfat` writes, name for name and byte for byte. The
  allocation oracle checks, which drive the crate-private primitives, run as unit tests in
  `src/allocation/tests.rs`.
- Other implementations. Where the host has `fsck.fat` or `fsck_msdos`, every image must pass
  `fsck -n`; where it has `mkfs.fat`, the crate's writer works on that tool's FAT12 and FAT16
  volumes. This run had both. The crate also reads two FAT32 images a macOS host wrote to their
  recorded hashes.
- Pinned bytes. `the_zero_timestamp_images_do_not_change` pins the writer's output digests.
- `a_third_party_driver_reads_it` stays ignored: it attaches an image for the host's own driver on
  macOS.
- Mutation. nife's ledger (`notes/project-metrics/mutation-triage.csv`) holds 40 rulings on this
  crate: 21 equivalent, 16 killed, 3 gaps. The gaps are a directory's 65,536-entry ceiling, which no
  test reaches, and one `slot_location` mutant that outruns `cargo-mutants`' time bound. The ledger
  stays in nife.

## Dependencies, `unsafe`, `no_std`

- Run time: none. `[dependencies]` is empty.
- Development only, from crates.io and pinned with `=`: `lamfat 0.4.3` (MIT), the oracle, and
  `ruzstd 0.9.1` (MIT), which decompresses the macOS images. nife's `deny.toml` allows `lamfat` to
  this crate alone, and the new repository's `deny.toml` needs the same exception.
- `unsafe`: no site.
- `no_std`, with no `alloc`. The library builds on Rust 1.88 for `aarch64-unknown-none-softfloat`,
  `riscv64imac-unknown-none-elf` and `x86_64-unknown-none`. No public signature names another
  crate's type.
- Panics. Library code has no `unwrap`, `expect` or `panic!`. Its `assert!` sites run in constant
  evaluation and one `debug_assert!` bounds a loop. `VolumeLayout::cluster_first_sector` underflows
  below cluster 2, and it is crate-private; every call passes a constant from 2 up.

## License and notices

`MIT OR Apache-2.0`, nife's own pair. The crate copies no code. It follows Microsoft's "FAT:
General Overview of On-Disk Format" version 1.03, and
[milestone 1019 (the driver crates carry facts from their sources, not their text)](../../design/roadmap/1019-the-driver-crates-carry-facts-not-source-text.md)
does not list it. Its authors read rust-fatfs, hadris-fat, embedded-sdmmc and simple-fatfs for
behavior only. Microsoft's long-name patents have all expired (milestone 813's patent table).

The six fixtures `tests/fixtures/*.boot` are the first 512 bytes of volumes `mkfs.fat` made, with
the boot-code region zeroed: bytes 62 to 509 on FAT12 and FAT16, 90 to 509 on FAT32. They keep the
jump, the OEM name, the BPB and the signature, and none of the boot stub or message that dosfstools
(GPL-3.0-or-later) writes there. `make-boot-sectors.sh` writes them that way and says why, and it
reproduces all six byte for byte with `mkfs.fat` 4.2. Question 5.

## The README, from a stranger's chair

The review lane copied the crate's tracked files alone into an empty directory and ran only the
README's commands, on the crate as #3233 left it. The reader had already seen the crate, so this is
not a true stranger run; soak condition 4 records one in the new repository.

| step | result |
|---|---|
| `cargo test` on stable | 300 pass, 1 ignored |
| `cargo +1.88 test`, the stated minimum | 300 pass, 1 ignored |
| `cargo kani` on Kani 0.68.0, as the README names | 31 of 31 harnesses verified, in 21 minutes on one core |
| `patch -p3` from the crate root, for each falsification patch | all 31 apply |
| `cargo package` | packs, and warns that the manifest has no `repository`, `homepage` or `documentation` |

Nothing stopped the run. Kani prints two warnings of its own, as for every crate: an unstable
`register_tool` feature and two unsupported constructs no harness reaches. One README sentence
could say so.

## The `VolumeLayout` label

Background. `VolumeLayout` writes the one volume a bootloader installer needs: FAT32, 512-byte
sectors, 4 KiB clusters, and `\EFI\BOOT\<file>` holding one file whose bytes the caller writes
after the metadata. nife's system installer and its cloud-image builder use it. Before #3233 it
wrote the fixed label `NIFE` into the boot sector and the root directory, the only project policy
in an otherwise general writer.

`VolumeLayout::new` takes `label: &str` between `volume_id` and `file_name`, with no default. It
applies `FormatOptions::with_label`'s check (one to eleven bytes a short name allows) and refuses
a bad label with `LayoutError::LabelRefused`. nife's two callers pass `"NIFE"`. #3233 rejected the
alternative, moving the writer into nife.

The trade-off:

- For: the crate keeps one tested EFI writer that any installer can use, and the policy stays with
  the caller. The general format and this writer share one label check, so they cannot disagree.
- Against: `new` takes seven positional arguments, three `u64` and two `&str` side by side. A
  scratch program swapped the two strings, and the crate refused `"BOOTX64.EFI"` as a label because
  a label cannot hold a dot, so the likely swap fails loudly. A swap of the three sizes compiles.
- Against: the general format makes a label optional, and `VolumeLayout` requires one. It refuses
  an empty label; a caller who wants none passes `"NO NAME"`, the specification's placeholder, which
  the documentation does not mention.

Question 2 settled how much of this shape to publish: `new`, `metadata_sectors`, `fill_sector`,
`file_first_sector` and `file_sectors`, with the shape's constants in `efi_system_partition`.

## Semantic versioning

The first release has no baseline. `cargo semver-checks` builds the crate and passes its 202 checks
against a copy of itself, so the tool works on it. The tag `v0.1.0` on the move commit becomes the
baseline for every later change through `--baseline-rev`, as the pilot's tags are. Soak condition 5
then measures a period with no change the tool calls breaking; nobody has set the period yet.

## What the move still needs

| item | owner |
|---|---|
| Clear this package: confirm or change the provisional rulings on questions 1 to 6, and rule on question 7. | architect |
| `repository = "https://github.com/nifeos/file_allocation_table"` in the manifest. | move lane |
| The `lamfat` exception in the new repository's `deny.toml`. | move lane |
| nife's three consumers (`components`, `xtask`, `fuzz`) move from the path dependency to `git` and `rev` with `version = "0.1.0"`. | move lane |

## Result of `script/crate-standalone`

One failure, in names: `LayoutError::LabelRefused` is provisional. The manifest, the files, the
citations and the build (host and the three bare-metal targets on 1.88) pass. In the stranger
copy, stock Kani proved all 31 harnesses, and each of the 31 patches made its own harness fail.

## Questions and rulings

Each decides what the first release publishes. A published API is a one-way door: once a stranger
depends on it, removing anything breaks them. The maintainer ruled questions 1 to 6 provisionally,
each as recommended, and #3267 applied the rulings in nife so the move commit
carries them. They stand until the architect clears this package or changes one. Question 7 is
open.

1. The name `LayoutError::LabelRefused`.
   Background: #3233 added the variant when the label became the caller's. Its siblings
   `FormatError::LabelRefused` and `WriteError::LabelRefused` carry ratified names, and so does
   `LayoutError::NameRefused` beside it.
   Recommendation: ratify it as it stands. It says the same thing in the same words as its three
   neighbors.
   Ruling: ratified as it stands. `script/crate-standalone` no longer fails at its names step.

2. How much of the EFI-system-partition writer to publish.
   Background: `VolumeLayout` is one FAT32 shape for installers, with the caller's label. It had
   eight public methods, seven crate-root constants and the three `efi_system_partition` constants.
   nife, its only user, uses four methods and one constant. The crate-root constants describe the
   EFI shape alone but read as the crate's, and `cluster_first_sector` underflows below cluster 2.
   Options: (a) publish it as it stands; (b) publish `VolumeLayout` with `new`, `metadata_sectors`,
   `fill_sector`, `file_first_sector` and `file_sectors`, move the constants into
   `efi_system_partition`, and make the rest private; (c) leave `VolumeLayout` out of the first
   release.
   Recommendation: (b). It keeps the writer #3233 chose to keep, takes away nothing nife uses, and
   publishes nothing a stranger could misread or call into a panic. A later minor release can make
   a hidden item public without breaking anyone; the reverse breaks.
   Ruling: (b). All seven constants moved, `FAT32_MINIMUM_CLUSTERS` included, since the writer is
   the one part of the crate that refuses by it; the reader keeps its own private copy.

3. `#[non_exhaustive]` on the error enums.
   Background: an exhaustive enum lets a caller match every variant with no wildcard arm, so a new
   variant breaks that caller. `#[non_exhaustive]` makes every caller write a wildcard arm, and then
   new variants break nobody. Adding the attribute after publication breaks callers; removing it
   does not. The data enums (`TableEntry`, `Slot`, `SectorSize`, `FileAllocationTableType`) are
   complete by the specification and stay as they are.
   Recommendation: mark the error enums `#[non_exhaustive]` before 0.1.0, the stricter choice,
   which a later release can relax.
   Ruling: as recommended. Thirteen public error enums carry it; the fourteenth,
   `AllocationError`, became crate-private under question 4.

4. The allocation primitives on `WritableVolume`.
   Background: `allocate_cluster`, `deallocate_chain` and `truncate_chain` edit the table directly.
   Kani proves each correct alone, but a caller who uses one outside a file operation can leak
   clusters or free a live file's chain.
   Recommendation: make the three crate-private before 0.1.0 and move their oracle checks into the
   crate's unit tests. Keep the read-side primitives public.
   Ruling: as recommended. The three were forwarders to `Volume`'s crate-private methods, so they
   are gone, and `AllocationError`, which only they returned, is crate-private.

5. The `mkfs.fat` boot stub in six test fixtures.
   Background: `mkfs.fat` (dosfstools, GPL-3.0-or-later) wrote these 512-byte boot sectors, and the
   bytes after the BPB were dosfstools' boot code and message. The parser never reads them. They
   ship inside the `.crate`, whose license is MIT or Apache-2.0. The fixtures are the program's
   output, and that output embedded a fragment of the program, which a stranger auditing licenses
   would ask about.
   Recommendation: zero the boot-code region of the six fixtures and say so in
   `make-boot-sectors.sh`. Every test keeps its meaning, and the question goes away.
   Ruling: as recommended; the License section above states the result.

6. Where the two fuzz targets live after the move.
   Background: `file_allocation_table_boot_sector` and `file_allocation_table_writer` sit in nife's
   `fuzz/`, which the fuzz host runs; it files each finding as a private advisory on nifeos/nife.
   The shared crate template has no fuzz job. After the move the targets fuzz the crate through
   nife's `git` and `rev` dependency, and a finding lands on nife for code in another repository.
   Recommendation: keep both targets in nife for the soak, since the pinned dependency is exactly
   what nife ships, and decide their home when the template gains a fuzz job. This does not hold
   the move.
   Ruling: as recommended. Both targets stay in nife during the soak, with no change now.

7. The version, tag and repository.
   Background: the manifest says `0.1.0`. The pilot took the tag `v0.1.0` at its move, and nife
   pins it by `rev` with the version beside it. The pilot's repository carries the crate's own
   name, underscores included.
   Recommendation: `0.1.0`, the tag `v0.1.0` on the move commit once the answers above have landed,
   and `nifeos/file_allocation_table`.

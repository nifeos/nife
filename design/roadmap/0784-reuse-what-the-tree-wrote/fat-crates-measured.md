# `fatfs` and `embedded-sdmmc` measured against milestones 560, 140 and 813

Appendix to [784. Reuse what the tree wrote](../0784-reuse-what-the-tree-wrote.md). What follows
from it, the cost of a full crate of our own and the options, is in
[fat-adopt-or-build.md](fat-adopt-or-build.md).

calef, 2026-10-22 (UTC): "measure before 560, 140 or 813 start." Three NOT-STARTED milestones
would build what an existing FAT crate may already have. They are 560 (a long file name, or riscv64
cannot be installed) and 140 (mount a drive this system did not create). The third is 813
(`file_allocation_table`: a proven FAT crate, released on its own). This page is the data, for `fatfs` (rafalh/rust-fatfs) and
for `embedded-sdmmc` (rust-embedded-community).

Measured 2026-10-22 (UTC) at base `2f641c7e2`, on an Apple M3 running macOS 26.6.2, with the pinned
nightly of 2026-10-22. `fatfs` is upstream's main line at `38b3a79` (2026-10-16), and
`embedded-sdmmc` is crates.io's 0.10.0. Every `fatfs` build, image and boot below is generated
outside the tree by [fatfs-probe.sh](fatfs-probe.sh). No third-party crate entered this tree's
Cargo files. Release dates, downloads and issue text come from the crates.io and GitHub APIs,
read the same day.

## Result

- **No `fatfs` 0.4 release is scheduled.** crates.io's latest is still 0.3.6 (2023-01-17). Upstream
  resumed work: 17 commits landed on 2026-10-16, and the main line is 0.4.0, unreleased.
- **The main line builds `no_std` with no allocator and no `core_io`**, long names on, for all
  three bare-metal targets. 0.3.6 cannot build `no_std` with any current compiler.
- **It costs one crate and about 58 KiB.** Its two dependencies are already in the tree. A git pin
  fails this tree's `deny.toml`, but `lamfat` 0.4.3, a third party's crates.io republish, passes it.
- **It writes the ESP the installer needs, long name included, and a firmware boots from it.**
  macOS's driver and `fsck_msdos` accept the volume, and EDK2 on riscv64 ran a real
  `BOOTRISCV64.EFI` from it to the progenitor.
- **`embedded-sdmmc` cannot do 560's job.** It creates no long names, formats nothing, refuses
  FAT12 and opens only an MBR partition, never a bare volume or a GPT one.
- **Neither is a proven driver for 140's hostile sticks.** A Kani harness over `fatfs`'s mount
  found no verdict in 30 minutes; `embedded-sdmmc` was not tried.

## 1. `fatfs` release status

| | |
|---|---|
| crates.io | 0.3.6, 2023-01-17; 1.7M downloads, 401k in the last 90 days |
| main line | `version = "0.4.0"`, and its CHANGELOG heads 0.4.0 "not released yet" |
| commits since 0.3.6's date | 64 (51 by the maintainer, 13 by eight contributors) |
| last activity | 2026-10-16: 17 commits, 7 pull requests merged, 5 issues closed |
| release request | rafalh/rust-fatfs#81, open since 2023-03-17, last comment 2026-04-28 |

The maintainer's own words on a release, on GitHub. In March 2022 (#53) he said he had changes
to make first, and asked users to take a git reference meanwhile. On 2026-03-08 (#110) he wrote "I
want to release soon." On 2026-10-16 (#115) he named his uncertainty about the future of
`fscommon`, his companion crate, as one of the main reasons 0.4.0 waits. He gave no date. 0.3.6
was cut from a `stable-0.3` branch and is not an ancestor of the main line.

`lamfat` republishes the main line under another name, for a company that needed it on crates.io
for its own bootloader. Its provenance file pins upstream `c4b8847` (2026-03-08) and claims no change
to the format logic. 0.4.3 was published 2026-10-15. It lacks upstream's 2026-10-16 fixes, among
them a panic on a file name that starts with a multibyte character (#120). Its README says it lasts
until upstream publishes 0.4.0.

What 0.4 changes, from the CHANGELOG and read against the source:

- Its own `IoBase`, `Read`, `Write` and `Seek` traits and an `Error` enum replace `std::io`, which
  removes `core_io`. This is a breaking change to every signature.
- The time provider and code-page converter become owned type parameters, not `&'static` options.
- Fixes since 0.3.6: subdirectories `fsck_msdos` accepts (`..` cluster and no long names for `.`
  and `..`, 2024-11), the FAT32 root zeroed and FSInfo updated after it is allocated, `format_volume`
  panics removed, and long names without `alloc`.
- New: `FormatVolumeOptions::hidden_sectors` (a volume inside a partition) and `reserved_sectors`,
  file attributes, a flush without unmounting, and the `strict` mount option.
- `byteorder` dropped, `bitflags` 2, edition 2021, minimum Rust 1.71.

## 2. `no_std`, on this tree's targets

Each crate with default features off, linked into a `#![no_std]` `#![no_main]` program, release
profile with overflow checks:

| build | riscv64imac | aarch64 softfloat | x86_64 none |
|---|---|---|---|
| `fatfs` main line, `lfn` | builds and links | builds and links | builds and links |
| `fatfs` main line, `lfn` + `alloc` | needs a `#[global_allocator]` | same | same |
| `fatfs` 0.3.6 + `core_io` | `core_io`'s build script panics: "Unknown compiler version" | the same: the script runs on the host | the same |
| `embedded-sdmmc` 0.10.0 | builds and links | builds and links | builds and links |

`core_io`'s last release, 0.1.20210325, recognizes compilers up to March 2021, so 0.3.6 has no
`no_std` build on any toolchain this tree could pin.

What `fatfs`'s features ask for:

- `lfn` writes and reads long names in fixed buffers, with no allocator. Without `alloc`, a reader
  gets a long name as UTF-16 units (`long_file_name_as_ucs2_units`). Getting it as a `String`
  (`file_name`) needs `alloc`.
- `alloc` needs a heap. `system_installer` links none today, but the tree's `talc` heap is
  available to a program that wants one.
- Leaving `unicode` off folds case for ASCII only when a path is looked up.
- With `chrono` off, the clock is `NullTimeProvider`, which stamps the DOS zero date (month 0, day
  0); macOS lists it as 1969-12-31. An installer would supply its own `TimeProvider`.

`embedded-sdmmc` never allocates. Its block device's error type must implement `core::error::Error`.

## 3. Cost

### The graph `system_installer` would take

`fatfs` with `lfn` resolves to `fatfs`, `bitflags` 2.13.2 and `log` 0.4.34. This tree's `Cargo.lock`
already holds `bitflags` 2.13.2 and `log` 0.4.33, so one crate is new. `embedded-sdmmc` brings
seven: itself, `embedded-sdmmc-types`, `embedded-hal`, `embedded-io`, `arbitrary-int`,
`hex-literal` and the `bitbybit` proc macro.

| | code lines | license | RustSec advisories |
|---|---|---|---|
| `fatfs` main line, `src/` | 5,204 (its tests add 984) | MIT | none for `fatfs`, `lamfat` or `core_io` |
| `embedded-sdmmc` 0.10.0, both crates' `src/` | 6,997 (its tests add 1,296) | MIT or Apache-2.0 | none |
| `file_allocation_table`, `src/` | 479 (its tests add 33) | the tree's | not applicable |

Code lines are counted as 784 counts them, with blank and comment lines removed.

`cargo deny check`, run with this tree's `deny.toml`:

| shape | advisories | bans | licenses | sources |
|---|---|---|---|---|
| `lamfat` 0.4.3 from crates.io | ok | ok | ok | ok |
| `fatfs` git pin on rafalh/rust-fatfs | ok | ok | ok | **failed** |
| `embedded-sdmmc` 0.10.0 | ok | **failed**: a second `syn` major through `bitbybit` | ok | ok |

`[sources]` allows git only under `nifeos`, per §305 (git dependencies are allowed on `nifeos/*`
only, pinned to a landed commit). So `fatfs` can arrive today as `lamfat`, as a `nifeos` fork, or
later as a `fatfs` release. Taking a third party's republish is its own question: one publisher,
behind upstream by seven months of fixes.

### Size

The same ESP written two ways, `.text` and `.rodata` in bytes. The release profile matches the
installer's (no LTO):

| target | ours `.text` | ours `.rodata` | `fatfs` `.text` | `fatfs` `.rodata` | delta |
|---|---|---|---|---|---|
| riscv64imac | 4,148 | 670 | 50,158 | 12,635 | +57,975 |
| aarch64 softfloat | 4,536 | 650 | 55,120 | 10,099 | +60,033 |
| x86_64 none | 3,071 | 410 | 55,887 | 8,139 | +60,545 |

This is not like for like. Ours computes the sectors of one fixed layout holding one 8.3 file.
The `fatfs` program formats a general volume, makes two directories, and writes a long name through
cluster allocation. It also carries the adapter that `fatfs`'s byte addressing needs over the
installer's block capability, a one-sector read-modify-write cache of about 60 lines. So the figure
is the cost of the general driver, not of the long name: 560's long-name writer is about a hundred
lines in our crate. `embedded-sdmmc` has no size row, because it cannot write this ESP at all.

Two things in the installer would change. Ours hands back sector contents, and the installer
writes the boot file straight to the blocks the directory entry names. It refuses a data area that
does not land on transfer blocks, since a misaligned write puts the file where its directory entry
does not point. Through `fatfs`, that alignment becomes the formatter's choice and the check would
have to move. Stack use was not measured.

## 4. Fit: the installer's ESP, read by somebody else

The `image` probe writes what `system_installer` writes. It is 512 MiB, FAT32, with 4 KiB
clusters, and `\EFI\BOOT\BOOTRISCV64.EFI` holds 3,000,000 bytes in a prime-stride pattern. This is
the shape of `crates/file_allocation_table/tests/a_third_party_driver_reads_it.rs`, with the long
name that crate refuses.

- `fsck_msdos -n` exits 0 (3 files, 130,079 free clusters).
- macOS's `msdos` driver mounts it as `MS-DOS FAT32` and lists `BOOTRISCV64.EFI` under its long
  name. The bytes compare equal with `cmp`, SHA-256 `4d3870d4…b76f`.
- `fatfs` reads the entry back as `BOOTRISCV64.EFI`, short alias `BOOTRI~1.EFI`.

Then a firmware. The `firmware` probe writes the same volume holding a real `BOOTRISCV64.EFI`,
13,416,960 bytes from a `cargo xtask stick` build of 2026-10-22. It boots it with
`helpers/qemu-stick.sh riscv64`, which is QEMU's EDK2 on `virt` with the volume as USB storage.
EDK2 found the file by its long name and started it. The transcript has the loader's banner, `nife
self-test: 8 of 13 passed, 5 absent` with nothing failed, and the hand-over to the progenitor:
the three lines `cargo xtask stick-boot` requires. The volume is bare, with no partition table. The
installer's GPT, with the ESP at an offset, was not booted; `hidden_sectors` and an offset adapter
are the pieces that would need it.

## 5. What each one covers, against ours

Read from each crate's public API and source, not from its README:

| capability | `fatfs` main line | `embedded-sdmmc` 0.10.0 | `file_allocation_table` |
|---|---|---|---|
| FAT12 | yes | refused by its BPB parser | no |
| FAT16 | yes | yes | no |
| FAT32 | yes | yes | writes one fixed layout |
| bare volume, MBR, GPT | bare; the caller slices a partition | MBR only | bare; the installer places it |
| long names, read | yes, checksum checked | yes, and open by long name | no |
| long names, write | yes, with `~N` aliases | no: "you cannot create them" | refused (560) |
| read and write files, extend, truncate | yes | yes | one file, at format time |
| create and delete files | yes | yes | no |
| make and remove directories | yes | yes, empty ones only | three fixed ones |
| rename and move | yes, `..` updated | no | no |
| timestamps | a `TimeProvider`; set created, accessed, modified | a `TimeSource` | zero |
| attributes, volume label | read and write | read | fixed |
| format | FAT12/16/32, cluster size, FATs, label, hidden and reserved sectors | none | FAT32, one layout |
| free space and FSInfo | reads, counts, updates FSInfo | tracks the count and next free | writes FSInfo once |
| dirty and I/O-error bits | set on mount, cleared on unmount, reported | no volume bit | no |
| interface | byte-addressed `Read`, `Write`, `Seek`; no cache of its own | a `BlockDevice` of 512-byte blocks | sector contents out, no I/O |
| fuzzing in its tree | none; #72, an integer-overflow panic a fuzzer found, open since 2022-07-02 | none | none |
| Kani harnesses | none | none | none |

`fatfs` covers 140's read side as a driver, and it would run well as 140's one process per volume.
Neither crate offers evidence that its parser is safe on a stranger's stick. That is what 140's
confinement argument rests on and what 813 exists to prove.

Against this tree's own size gates, measured with `helpers/function_length.py`: none of `fatfs`'s
431 functions passes §283's 100 code lines (a function is held to 100 code lines by a lint ratchet).
Seven of `embedded-sdmmc`'s 251 do, the longest 219. Neither has a file at §266's 2,000 lines.

`embedded-sdmmc`'s CONTRIBUTING refuses contributions "substantially or wholly generated by AI", so
under 857's fork 8 nife could send it bug reports and nothing else. rust-fatfs publishes no
contribution policy (no `CONTRIBUTING`, nothing in its README).

## 6. 813, and what Kani reaches

813's *What is proven* table requires four properties before the crate publishes. Each needs a Kani
harness and a falsification patch, replayable with stock Kani in the crate's own repository:

1. No panic and no out-of-bounds read on any image, sector by sector.
2. Cluster chains cannot loop: every walk ends within the volume's cluster count.
3. Write then read round-trips on a small symbolic volume.
4. Every short name is legal, and every long-name run carries the right checksum and ordinals.

For `fatfs`, one harness was tried in scratch: mount must never panic over an eight-sector
in-memory image whose boot sector is symbolic, through the public API, with Kani 0.68.0 and an unwind bound of 520. CBMC was still in symbolic
execution after 30 minutes, near 4 GB of memory, and was stopped with no verdict. The reach
is the obstacle and not the tool. A harness enters `fatfs` only through `FileSystem::new` over a
byte stream, so every symbolic BPB field becomes a symbolic seek, and the solver has to follow all
of them. Proving the four properties would mean restructuring `fatfs` around per-sector parse
functions and bounded walks. That is the shape §46 (thin primitives or whole subsystems) gives as
the reason nife writes the crates Kani proves, and the restructuring would be upstream's to accept.

For ours, the four properties fit the crate's shape. It is sans-I/O, and `Volume::sector` is a pure
function of an index. `globally_unique_identifier_partition_table` proves eight harnesses in the same
shape, in 103 code lines of harness and 135 lines of falsification patches. Ours has no harnesses
and no reader today, so all four are still to write.

# A FAT crate: adopt one, or build a full one of our own

Appendix to [784. Reuse what the tree wrote](../0784-reuse-what-the-tree-wrote.md). The
measurements it rests on are in [fat-crates-measured.md](fat-crates-measured.md).

calef, 2026-10-22 (UTC), widening the question: if nife releases a FAT crate, it is full-featured,
doing what `fatfs` does while meeting this tree's standards. This page prices that, gives the order
it would be built in, ranks which crate should publish first, and ends with the options.

## What a full crate of our own costs

The scope is `fatfs`'s parity column in the measurements: FAT12, FAT16 and FAT32 read and write,
long names both ways, files and directories, rename, timestamps, attributes, format options, FSInfo
and the dirty bit. It also covers what neither upstream crate has: an MBR, a sector interface a
confined server can hold, and evidence on hostile images.

What this tree's standards add, each measured where the tree already pays it:

| standard | what it asks of a FAT crate | what it costs here |
|---|---|---|
| `no_std`, no `core_io`, no allocation in the parser | sans-I/O over a sector interface, as `globally_unique_identifier_partition_table` is | design, not lines: both upstream crates already manage `no_std` |
| Kani, where tractable | boot sector and BPB parse, FAT entry decode, directory entry and long-name checksum and assembly, chain walks bounded by the cluster count, and a small round trip | GPT carries 8 harnesses in 103 code lines and 135 lines of patches; FAT needs about twice the properties |
| fuzzing for the reader | whole images through mount, walk and read, and a second target through the writer | the tree's fuzz targets run 27 to 41 code lines each, plus a seed corpus (#1803's two images) |
| the mutation gate | every surviving mutant killed by a test or excluded with its reason | host tests sized like `fatfs`'s 984 lines or `embedded-sdmmc`'s 1,296 |
| §283 (a function is held to 100 code lines by a lint ratchet) and §266 (a Rust source file stays under 2,000 lines) | the parser split by on-disk structure | `fatfs` already passes both, so the shape is known to fit |
| §267 (a comment states the constraint as it is now) | a doc comment on every item, constraints only in bodies | prose, counted in no line figure here |
| oracle tests | rust-fatfs reads what we wrote; `fsck_msdos` exits 0; FAT12 and FAT16 images from `newfs_msdos` | a dev-dependency ruling for the oracle's pin |

The estimate, stated as one: about 5,500 code lines of crate, 1,500 of host and oracle tests, 300 of
harnesses with 400 lines of patches, and 150 of fuzz targets. That is about 7,850 lines over ten
pull requests. The crate figure is `fatfs`'s 5,204 plus an MBR reader and the sector interface. Kani
is the risk in it: a property that will not close in bounded time forces a restructure, and that
cost is not known until the first harness runs.

## The order it would be built in

Each step lands on its own and leaves the installer working.

1. 560: the long-name writer in the existing `mkfs`. A `BOOTRI~1.EFI` alias, its checksum, and the
   `LDIR` run, checked by the host mount, `fsck_msdos`, and EDK2 booting `BOOTRISCV64.EFI` on
   riscv64. The first harness proves the writer's long-name run is well formed (property 4's write
   half). About 150 lines. riscv64 installs need this step and nothing later.
2. The sector interface, boot sector and BPB parse, and FAT type by cluster count. A per-sector
   no-panic harness (property 1) and the reader's first fuzz target.
3. FAT entry decode and the bounded chain walk (property 2).
4. Directories: entries, long-name assembly with the checksum checked (property 4's read half), and
   #1803's macOS images read to their recorded hashes.
5. File read and the MBR. 140's read-only FAT stratum can start here.
6. Allocation, write, extend, truncate and delete, with FSInfo and the dirty bit, and the round-trip
   harness (property 3). Two pull requests.
7. Make and remove directories, rename, and long-name creation with alias numbering.
8. The general format for FAT12, FAT16 and FAT32, folding the `mkfs` in, with FAT12 and FAT16
   oracle images.
9. Timestamps, attributes and the label, then `script/crate-standalone` and the README for release.

## Which crate should publish first

The test is calef's of 2026-10-22: release our own crate only where no maintained upstream crate
deserves our proofs, and contribute where one does. The candidates are 857's review list, with each
one's upstream from [the-rest-of-the-tree.md](the-rest-of-the-tree.md). Read on 2026-10-22 from each
repository and crates.io. No upstream named here carries a Kani harness.

| ours | upstream that could take proofs | latest release, last commit | contribution policy | by the test |
|---|---|---|---|---|
| FAT | `fatfs` | 2023-01-17; 2026-10-16 | none published | contribute, but nothing reaches crates.io until 0.4.0 ships |
| FAT | `embedded-sdmmc` | 2026-07-24; 2026-10-07 | refuses AI-written work | bug reports only, and it lacks 560's features |
| GPT | `gpt_disk_types` | 2025-04-01; 2026-10-20 | Google's CLA | contribute |
| `elf` | `elf` (cole14) | 2025-05-14; 2025-11-29 | none published | contribute |
| `network_time_protocol` | `sntpc` | 2026-07-06; 2026-07-06 | none on AI | contribute |
| `network_time_protocol` | `ntp-proto` | 2026-06-12; 2026-10-22 | dismisses heavily AI-written work | bug reports only, and it needs `tokio` |
| `device_tree_blob` | `dtoolkit` | 2026-09-25; 2026-10-22 | Google's CLA | contribute (`fdt` is MPL-2.0); measured in [`dtoolkit` against `device_tree_blob`](dtoolkit-against-device-tree-blob.md) |
| `glob` | `wildmatch` | 2026-10-20; 2026-10-20 | none on AI | std only, and ours serves `no_std` grants |
| `domain_name_system` | `hickory-proto`, `simple-dns` | 2026-09-10, 2026-07-26; both active | none on AI | contribute (`domain`'s NLnet Labs policy refuses LLM-written code) |

So the test sends most of the list upstream. Two rows survive it, and each on a judgment:

1. FAT, if a crate that has not released in three and three-quarter years does not count as
   maintained. Proofs given to `fatfs` would sit on a branch nobody can depend on from crates.io.
2. `glob`, if a std-only crate does not deserve proofs that need `no_std`. Contributing means asking
   `wildmatch` to take a `no_std` port first.

FAT is first if it is built at all. It is the only row already ruled for release, and 140 is on the
customer path. `glob` is second and smaller, at 650 lines with six harnesses already. The CLA rows
need a person to sign Google's agreement once, which 857's fork 8 did not price.

## Decision, and the options

The question is whether to adopt a FAT crate or build a full one of our own. `embedded-sdmmc` is not
offered: it cannot create a long name or format a volume, so it fails 560 outright.

- A. Build our own full-featured crate, in the order above, 560 first. About 7,850 lines over ten
  pull requests. `fatfs` stays the test oracle, pinned as `lamfat` 0.4.3 or a `nifeos` fork; that
  pin is a dev-dependency ruling when step 2 starts.
- B. Adopt `fatfs`. Take it now as `lamfat` 0.4.3 or a `nifeos` fork, move to `fatfs` 0.4.0 when it
  publishes, and offer it our proofs under 857's fork 8. 560 is about 60 lines of adapter and 58 KiB
  in the installer. 813 becomes a contribution, and the proofs wait on restructuring upstream
  accepts.

Recommendation: A. B is less work, so the effort test decides whether this holds, and I would still
choose A at equal effort. 813's properties need a parser shaped for Kani, and `fatfs`'s shape kept the
measurements' mount harness from closing in 30 minutes. Whether it is restructured is upstream's call, and its releases
wait on a question its maintainer has not settled. Base would carry a 5,204-line parser for hostile
sticks that nothing here can prove. B is the better choice if calef ranks the time over the proofs.

Ruled: pending.

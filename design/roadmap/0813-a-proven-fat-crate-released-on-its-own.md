---
status: PARTIAL
raised: 2026-10-07
milestone_dependencies: 887
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 813. `file_allocation_table`: a proven FAT crate, released on its own

*(From calef's rulings on pull requests #1803 and #1806; the title and slug are drafts. On the
customer path: milestone 140 (mount a drive this system did not create)
needs it for its FAT32 stratum.)*

calef's rulings, quoted from the maintainer comments of 2026-10-07 and 2026-10-22 (UTC):

- #1803, fork 2: *"take no FAT dependency. nife writes its own FAT12/16/32 crate (about 5,000 lines
  by the probed crates' sizes), proven with Kani because it parses hostile input from foreign
  sticks, with rust-fatfs and this probe's round-trip images as its test oracle; and it is built,
  proven and released on its own, independently of nife."* His words: *"I'm really questioning
  taking a dependency on something that isn't updating."* and *"It seems like something we should
  build and prove and release on its own independently of nife."*
- #1806, scope: *"FAT12, FAT16 and FAT32, read and write, with long file names (VFAT), in the first
  release; exFAT later, as its own milestone or sibling crate. Proven: hostile images never panic or
  read out of bounds, cluster chains cannot loop, written data reads back; oracle rust-fatfs and
  `fsck_msdos` on #1803's images."* His word: *"Yes"*.
- #1806, name: *"`file_allocation_table` (crates.io treats it and `file-allocation-table` as one
  name; free as of today). Permanent once first published."* His word: *"Yes"*.
- #1806, milestone: *"mint a new milestone, "`file_allocation_table`: a proven FAT crate, released
  on its own" (write it, prove it, oracle tests, its own nifeos repository on the shared CI template
  (milestone 887) at first publication, trusted publishing); milestone 140's FAT32 stratum depends
  on it and adds the filesystem-contract server. On the customer path."* His word: *"Yes"*.
- #2580, build or adopt (2026-10-22 19:57 UTC): *"build our own full-featured FAT crate, not
  adopt `fatfs`. It covers FAT12/16/32 read-write, long names, rename, format, FSInfo and the dirty
  bit, MBR, and a sector interface, with Kani proofs on the parsers, fuzzing and the mutation gate,
  staged as fat-adopt-or-build.md sets out. Stage 1 is 560's long-name writer in the existing mkfs.
  `fatfs` stays the test oracle, outside the base graph."* His word: *"A"*.

## The stages

Built in this order, by the #2580 ruling. Each lands on its own and leaves the installer working;
[fat-adopt-or-build.md](0784-reuse-what-the-tree-wrote/fat-adopt-or-build.md) prices them.

1. The long-name writer in the existing `mkfs`: milestone 560, built.
2. The sector interface, boot sector and BPB parse, and FAT type by cluster count, with a
   per-sector no-panic harness and the reader's first fuzz target. The oracle's dev-dependency pin
   is ruled here. Built: [stage-2-the-reader-begins.md](0813-a-proven-fat-crate-released-on-its-own/stage-2-the-reader-begins.md)
   has what landed and the names it minted. The pin is still open, and stage 3 needs it.
3. FAT entry decode and the bounded chain walk.
4. Directories: entries, long-name assembly with the checksum checked, and #1803's macOS images
   read to their recorded hashes.
5. File read and the MBR. Milestone 140's read-only FAT stratum can start here.
6. Allocation, write, extend, truncate and delete, with FSInfo and the dirty bit, and the
   round-trip harness. Two pull requests.
7. Make and remove directories, rename, and long-name creation with alias numbering.
8. The general format for FAT12, FAT16 and FAT32, folding the `mkfs` in, with FAT12 and FAT16
   oracle images.
9. Timestamps, attributes and the label, then `script/crate-standalone` and the README for release.

## What exists, checked 2026-10-07 (UTC)

- `crates/file_allocation_table` already exists: 794 lines, no Kani harnesses, written by milestone
  198 (a package manager, and the trivial install that makes a second customer possible) rung 2a.
  It is a `mkfs` that lays out one FAT32 EFI system partition holding one 8.3-named file, as pure
  computation with no I/O. `components/src/system_installer.rs` is its one caller. It reads nothing.
  This milestone grows that crate rather than starting a second one, since the ratified name is
  already its name; the installer's call keeps working throughout. Its module doc says it "cannot
  open, read, extend, delete or rename anything, and it never will", and points the read half at
  milestone 140. This milestone retires that sentence. calef has since ruled that the crate is the
  format and 140 is the server. No scope conflict: the `mkfs` becomes the crate's format path.
- Milestone 560 (a long file name, or riscv64 cannot be installed) is stage 1, built: the `mkfs`
  writes VFAT long-name entries in `src/long_name.rs`, with three Kani harnesses in
  `src/long_name/verification.rs` on the writer's half of the fourth property below.
- `notes/filesystem-crates-2026-10-07.md` (#1803) measured five read-write FAT crates on nife's
  three targets and built two FAT32 images that macOS formatted and wrote, one bare and one inside
  an MBR, with every file's hash in `expected.sha256`. `make-images.sh` rebuilds them on macOS.
- `crates.io` answers 404 for `file_allocation_table` (checked 2026-10-07).

Reuse: written here, not taken, by calef's ruling on #1803 fork 2. Three reasons, all from that
ruling. It parses hostile input (a stranger's USB stick), and §46 (thin primitives or whole
subsystems) has nife write the crates Kani proves, because a model checker needs code we can
restructure. Proofs are the point. And the main-line candidate, rust-fatfs, has not released since
`fatfs` 0.3.6 (2023-01-17); that release writes subdirectories `fsck_msdos` rejects, the fix landed
upstream on 2024-10-23 unreleased, and a release request has been open since 2023 (rafalh/rust-fatfs
#81).

`fatfs` and `embedded-sdmmc` were measured against this block on 2026-10-22 (UTC), Kani included,
in [784's FAT appendix](0784-reuse-what-the-tree-wrote/fat-crates-measured.md). calef's #2580
ruling above settles it: this block builds the full crate, in the stages above.

## References and oracles

Read for behavior and test against; copy no code unless its license allows it and the copy is
recorded at the copied item with its source, commit and license.

| crate | license | use here |
|---|---|---|
| rust-fatfs (main line) | MIT | the oracle: host tests read what this crate wrote, and this crate reads what it wrote |
| lamfat 0.4.2 | MIT | rust-fatfs's main line republished; the way to pin it as a dev-dependency if a git pin will not do |
| hadris-fat 3.0.0-rc.1 | MIT | a second reader, and the exFAT reference for the later crate |
| embedded-sdmmc 0.10.0 | MIT or Apache-2.0 | a reference for FAT16/32 without allocation. Its project bans AI-generated contributions, so per #1806 fork 8 it gets bug reports only |
| simple-fatfs 0.1.0-alpha.2 | MIT | a `no_std` reference; alpha, last updated 2025-09-14 |

The specification is Microsoft's "FAT: General Overview of On-Disk Format" (version 1.03), which the
existing crate already cites. A dev-dependency is still a dependency under §46, so the lane names
the oracle's pin in its pull request for calef's ruling.

## Scope

- FAT12, FAT16 and FAT32, read and write: mount, walk directories, read, create, extend, truncate,
  delete, rename, make and remove directories, and format (the existing `mkfs` folds in here).
- Long file names (VFAT): read and write, with the short-name alias and checksum generated and
  checked.
- An MBR-partitioned stick and a bare volume, since #1803's images are one of each.
- `no_std`, with no allocation in the parser if that proves feasible. The crate stays sans-I/O, the
  rule `globally_unique_identifier_partition_table` keeps: the caller supplies sectors through a
  small interface the lane chooses, and the crate never names a device.

Non-goals for the first release:

- exFAT. calef ruled it later, as its own milestone or a sibling crate. hadris-fat is its reference.
  Milestone 923 (exFAT waits for its patents) holds it until Microsoft's exFAT patents expire.
- A journal or crash-consistency claim. FAT has none. Milestone 140 already says the difference
  belongs where a user meets it; this crate's `BUGS` says it too.
- The filesystem-contract server. That is milestone 140's FAT32 stratum, which consumes this crate.

## What is proven

The writer's half of the fourth property is proved; the reader's half and the other three are
this milestone's scope, not a step before it. Each row is a property this crate must prove before it publishes. The harness
cell names the Kani harness that proves it, or says `none yet`; every named harness carries a
falsification patch that makes it fail at its own assertion. Each patch sits at
`crates/file_allocation_table/falsifications/<module.path>.<harness>.patch`, as §134 (a harness
carries a machine-replayable falsification record) and `script/falsifications` require.

| property | harness |
|---|---|
| No panic and no out-of-bounds read on any image: every parse step (boot sector, BPB, FSInfo, FAT entry, directory entry, long-name run) takes a symbolic sector and returns a value or an error | boot sector and BPB: `boot_sector::verification::what_the_parse_accepts_lies_inside_the_volume`, `the_fat_type_is_the_cluster_counts`, `the_table_has_an_entry_for_every_cluster`, `only_a_data_cluster_has_sectors`; FSInfo, FAT entry, directory entry, long-name run: none yet |
| Cluster chains cannot loop: a walk from any start over any FAT contents ends within the volume's cluster count, at an end-of-chain marker or with an error | none yet |
| Write then read round-trips on a small symbolic volume, byte for byte, with the FAT and directory consistent | none yet |
| Directory entries and long names are well formed: every short name is legal, every long-name run carries the matching checksum and ordinals, and the reader refuses a run that does not | writer: `long_name::verification::the_checksum_is_the_specifications_loop`, `every_long_name_run_is_well_formed`, `every_alias_is_a_legal_short_name`; reader: none yet |

Kani bounds its inputs, so whole-image coverage comes from composing per-sector proofs, with
`script/fuzz` over whole images as the backstop. The block states that bound rather than claiming
more. Not proven, by design: crash consistency (FAT has none), and the caller's sector interface.

## Oracle tests

- Read #1803's two macOS images and check every file against `expected.sha256`.
- Write a tree (long names, nested directories, a file spanning many clusters, a delete, a rename)
  and check it two ways: rust-fatfs reads back the same names and bytes, and `fsck_msdos` exits 0.
  Where CI runs Linux, `fsck.fat` from dosfstools stands in, run as a tool and never linked.
- FAT12 and FAT16 volumes made by `newfs_msdos` or `mkfs.fat`, since #1803's images are FAT32 only.
  No test image is made by this crate.

## Parity

The crate is pure logic and its tests run on the host. It builds for
`aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and `x86_64-unknown-none` in the
tree's build, so nothing about it can work on one target and not another.

## Release, by #1806's rulings

1. The name is settled before anything leaves (857's Fork 5 as calef amended it on 2026-10-11 UTC).
   `script/names file_allocation_table` reports it ratified, crates.io answers 404 for it (or it is
   already ours), and `nifeos/file_allocation_table` does not exist yet. `script/crate-standalone`
   checks the first.
2. In the tree, `script/verify` runs the harnesses, and `Cargo.toml` says `publish = false`.
3. The package is version 0.1.0 with its own semver, states its minimum Rust version in
   `rust-version`, and ships its harnesses and falsification patches. `cargo package --list` shows
   them.
4. At first publication, and not before, calef adds it to the lint-checked publish allowlist. The
   `nifeos/file_allocation_table` repository is created when the crate moves, for the soak (step 5),
   on the shared CI template (Kani through `kani-github-action`, lint, release). Its release
   workflow refuses to publish unless the proof job is green.
5. Soak, as its own repository (calef, 2026-10-17 UTC, replying "Agrred" (sic) to the maintainer's
   proposal; §305 (git dependencies are allowed on `nifeos/*` only, pinned to a landed commit)).
   Milestone 818 (`jh7110_entropy`: proven JH7110 TRNG logic, released on its own) is the pilot and
   soaks first. Before publication the crate is moved, not copied: it leaves this tree, and nife
   depends on `nifeos/file_allocation_table` by `rev`. A copy left behind does not count. The soak
   is how the policy's "API stable" is measured. The soak ends on evidence, not a duration, when all
   five hold:
   1. nife builds and passes CI against the repository at a pinned commit, with no path dependency
      left.
   2. The shared template's weekly run (milestone 887) is green on the crate.
   3. A release dry run is green.
   4. The stranger test builds and proves the crate from its README alone.
   5. The API has gone a set period with no change `cargo-semver-checks` calls breaking. The period
      is not yet set.
6. crates.io trusted publishing from that workflow, owned by a `nifeos` team, with no stored token.
7. calef approves this publication, as he does every one.
8. nife then consumes the crates.io release, pinned with its checksum in `Cargo.lock`, and the
   soak's git dependency goes.

## Standalone, measured 2026-10-11 (UTC)

`script/crate-standalone file_allocation_table` fails today, at base `ae6b91e79`. The manifest
inherits `license` and `lints` from nife's workspace and states no `rust-version`, and there is no
README, `LICENSE-MIT` or `LICENSE-APACHE`. Those hold for all nine crates. Builds and tests on
stable for the host and the three bare-metal targets, and has no harness for Kani to run. Beyond
manifest and files, 14 lines cite this tree: 7 `milestone N`, 3 tree paths, 3 naming calef and 1
provenance block. A test reads `../../target/file-allocation-table-check.img` through
`CARGO_MANIFEST_DIR` at run time, which the script does not catch and a stranger's checkout does not
have.

Name: `file_allocation_table` is ratified today (`script/names`, checked 2026-10-11).

## Review before release

- The public API speaks the device's vocabulary, not nife's: no nife capability type, spawn
  handoff or nife error enum in a public signature. A reviewer judges this; no script can.
  The sector interface is `SectorDevice`: a sector size and `read_sector` by index, with no nife
  type in it.

## Exit

`cargo test -p file_allocation_table` fails if any piece is missing. It reads both #1803 images to
their recorded hashes, writes the tree above on FAT12, FAT16 and FAT32, and has rust-fatfs and
`fsck_msdos` (or `fsck.fat`) accept each. It formats a FAT32 EFI system partition holding
`\EFI\BOOT\BOOTRISCV64.EFI`, a name the `mkfs` refuses today. `script/verify` proves the four
properties, `script/falsifications` reports every harness `replayable`, and the crate builds for all
three targets. Publication is the last step and not the exit, since it waits on calef. The five soak
criteria of release step 5 hold before it publishes.

The name check of release step 1 passes first, so the repository, the package and every doc link
carry a ratified name. Before the move, `script/crate-standalone file_allocation_table` exits 0 in
nife, and every row of *What is proven* names a harness that stock Kani proves in the copy and whose
falsification patch makes it fail. No crate publishes without Kani harnesses covering its
hostile-input and state-machine surface, each carrying a falsification patch that makes it fail at
its own assertion, replayable in the crate's own repository with stock Kani.

## Patents

Microsoft's long-name (VFAT) patents have all expired, per Google Patents, checked 2026-10-14
(UTC). Google Patents marks its status an assumption rather than a legal conclusion, and this
record is not legal advice.

| patent | title | priority | expired |
|---|---|---|---|
| [US 5,579,517](https://patents.google.com/patent/US5579517A/en) | Common name space for long and short filenames | 1993-04-01 | 2013-11-26 |
| [US 5,758,352](https://patents.google.com/patent/US5758352A/en) | Common name space for long and short filenames | 1993-04-01 | 2013-04-01 |
| [US 6,286,013](https://patents.google.com/patent/US6286013B1/en) | A common name space for long and short file names in an operating system | 1993-04-01 | 2013-04-01 |
| [US 5,745,902](https://patents.google.com/patent/US5745902A/en) | Accessing a file using file names having different file name formats | 1992-07-06 | 2015-04-28 |

The first two were the ones asserted against TomTom in 2009 and upheld on reexamination. All four
predate 1995, so none can still be in force. FAT12, FAT16 and FAT32 themselves date from 1977 to
1996. The search was for the known Microsoft families, not a freedom-to-operate opinion.

## Open: the oracle's pin

The pin of rust-fatfs as a dev-dependency waits on calef, and stage 3 needs it.
[The stage 2 appendix](0813-a-proven-fat-crate-released-on-its-own/stage-2-the-reader-begins.md)
gives three options. Its recommendation is `lamfat = "=0.4.3"` from crates.io, with a
`nifeos/rust-fatfs` fork pinned by `rev` as the alternative.

## BUGS

- Steps 4 and 5 wait on milestone 887 (crate repositories share one CI template),
  which owns the shared CI template, the publish allowlist lint and the `nifeos` crates.io team.
- The "about 5,000 lines" figure is the probed crates' size, not an estimate of this one.

## Index row

nife's own FAT12/16/32 read-write crate with long file names, proven with Kani because a USB stick
is hostile input, checked against rust-fatfs and `fsck_msdos`, and released on crates.io from its
own `nifeos` repository. Milestone 140's FAT32 stratum is built on it. exFAT comes later.

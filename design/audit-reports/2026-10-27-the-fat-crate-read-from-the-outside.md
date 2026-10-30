# Documentation sweep, 2026-10-27: the FAT crate, read from the outside

*Documentation audit, run 2026-10-26 and 2026-10-27 (UTC) at base `01cfcf87a` by the lane
`documentation-audit-2026-10-26`. The procedure is
[notes/documentation-audit.md](../../notes/documentation-audit.md); the index is
[README.md](README.md). The file name is provisional, like every audit report's.*

**Kind:** documentation. **Lens:** a subsystem rewritten inside its existing crate:
`crates/file_allocation_table`, which gained a reader in the window, read from the notes that
describe it and its two writers from the outside, passage by passage against the code, with each
program's spawn and report words swept for contracts stated only in prose. **Findings:** fixed 6,
minted 4, accepted 4.

The finding to carry off: every note outside the crate still described a writer with no reader,
and the one program that writes an install to a disk agrees with the kernel on its roles, slots,
mapping and verdicts through "Must match" comments and no shared code.

## Why this lens

`script/audits` on this branch at `01cfcf87a` reported the documentation kind DUE on one count:

```
documentation  last 2026-10-19 (Decisions read forward: the ten sections minted since the la)
  milestones built     466 -> 488  (+22, fires at 10)   <-- FIRED
  components           235 -> 235  (+0, fires at 10)
  ABI constants        97 -> 97  (+0, any change fires)
  external packages    190 -> 201  (+11, fires at 30)
  calendar             7 days since (12 weeks)
  DUE: milestones built +22 (fires at 10)
  ? has a subsystem been rewritten inside its existing crate since the last sweep? ...
  ? has a decision superseded a plan that a note still prescribes? ...
```

A count trigger points nowhere in particular, so the lens came from the two judgment triggers and
from what the last sweep left. The 2026-10-19 sweep took the second trigger. No sweep has taken the
first, and it fits this window exactly. Between 2026-10-19 and 2026-10-26, stages 2 and 3 of
milestone 813 (a proven FAT crate, released on its own), milestone 560 (a long file name, or
riscv64 cannot be installed) and the crate's naming passes added about 5,000 lines to
`crates/file_allocation_table` (#2585, #2709, #2739, #2731, #2760). That moved no component and no
ABI constant, which is the blind spot the README's `BUGS` names. The staleness worklist, which
the 2026-10-15 sweep took and the 2026-10-19 one left, does not rank these notes; its top entries
are roadmap blocks and an audit note.

### The two uncountable triggers, answered

Has a subsystem been rewritten inside its existing crate? Yes: the FAT crate, and this report.

Has a decision superseded a plan a note still prescribes? Yes, in passing: `notes/installing.md`
still called the crate finished after the 2026-10-22 ruling on #2580 that milestone 813 records
(finding 2).

### The method

The method was ruled 2026-10-26 UTC, in two parts: notes are read against the code, and a contract
is code, not prose. A note is read against the code, and each
passage is sorted as gone (it narrates the code), moves (it belongs beside the code it describes),
stays (a reason, a measurement, a refused alternative, or orientation), or contract. A
contradiction is fixed, or filed as a `BUGS` entry when the code may be the wrong side, and a note
names the symbol and keeps the reason. A contract is code, not prose: one stated only in prose,
or only implied, is a finding, proposed as a milestone with a code sketch, and this lane writes no
contract code.

The two pilots of that method, #2778 (`notes/system-initializer`) and #2782 (`notes/e1000e`),
found two things this sweep was built around. A grep for a crate's name misses stale claims that
never name it, so every passage in scope was read, not only those that name the crate. And the
contract check needs a sweep of each program's spawn and report words, so `system_installer`'s and
the boot stick's were read on both sides. Neither pilot's area was reread.

## What was read

About 13,100 words of prose, and the code each passage rests on:

| prose | words | read against |
|---|---|---|
| `notes/file-allocation-table.md` | 572 | `crates/file_allocation_table` (all of `src/`, both tests) |
| `crates/file_allocation_table/README.md` and `Cargo.toml` | 747 | the same |
| `notes/installing.md` and `notes/installing/` | 3,390 | `components/src/system_installer.rs`, `kernel/src/user/install_service.rs`, `xtask/src/install.rs`, `uefi_loader`'s chooser and PVH handoff, `kernel/src/arch/x86_64/machine.rs` |
| `notes/boot-stick.md` and `notes/boot-stick/` | 5,374 | `crates/stick_maker`, `xtask/src/stick.rs` and `cloud_image.rs`, `uefi_loader`'s per-architecture entry, `helpers/stick-maker-proof.sh`, `.github/workflows/stick-maker-hosts.yml` |
| `design/roadmap/0813-a-proven-fat-crate-released-on-its-own.md` | 2,979 | the crate, for its status claims only |

A second reader took the boot stick in parallel. Every finding below was checked against the tree
before it was fixed or routed.

### The sort

| area | passages | gone | moves | stays | contract | contradictions |
|---|---|---|---|---|---|---|
| FAT note | 8 | 2 | 1 | 5 | 0 | 2 |
| installing, with its appendix | 21 | 0 | 0 | 17 | 4 | 6 |
| boot stick | 82 | 17 | 6 | 55 | 4 | 13 |

## What came back clean

The crate's `lib.rs` and its `BUGS` match the code, as do the README's harness counts (nine on the
reader, three on the long-name writer, twelve falsification patches). The alignment test's seven
volume sizes, the 1023-sector table and the six-sector slip in `installing.md` section 3 are
re-derivable from `VolumeLayout::new`. The install transcript's `EFI system at LBA 1046528` is
what `Layout::fit` computes for a 1 GiB disk with the slots below the EFI system partition.
`boot_slot::medium`, the chooser's "the boot slots here are on another disk" line,
`refuse_an_unsealed_pair` and `NIFE_NVME_DEVICE_OPTS` exist as named. `confirm`'s write order
(backup array, backup header, primary array, primary header) matches the appendix's account.

## Findings

### 1. FIXED: the FAT note described a crate with no reader

`notes/file-allocation-table.md` said "this crate has no reader at all". Since milestone 813's
stages 2 and 3 it has one (`Volume`, `BootSector::read`, `Volume::chain`, `Directory`), proved by
nine Kani harnesses. The note's list of outside readers lacked rust-fatfs, which
`tests/the_oracle_agrees.rs`'s `rust_fatfs_reads_what_this_crate_wrote` runs in every `cargo test`.

The sort cut the rest of the page to what the crate cannot say. The shape table restated the
constants in `volume.rs` and `efi_system_partition`, whose docs give each reason; it is now a
pointer to those symbols (gone). The table's one reason no doc held, why two tables, moved into
`TABLE_COUNT`'s doc (moves). The long-name format repeated `long_name.rs`'s module doc almost word
for word, which milestone 813 had already moved there (gone). The headings stay, since milestone
813 cites two of them by name.

### 2. FIXED: installing.md called the FAT crate finished

Section 3 said the crate closes the FAT gap "and is then finished", and that the FAT32 stratum of
milestone 140 (mount a drive this system did not create) was read-first and unbuilt. The ruling of
2026-10-22 on #2580, which milestone 813 records, made the crate a full FAT12, FAT16 and FAT32
reader and writer, with 140 the server built on it. The section now says so. The crate's manifest
said the same as the note (a `description` and a comment naming only the FAT32 writer); both now
name both halves.

### 3. FIXED: installing.md named the wrong process for the survey

"The looking is done by ... the `disk_partitioner` verify role of milestone 57." (Milestone 57
(partitioning and formatting a real drive) built that role.) The kernel's offer spawns
`system_installer` with `ROLE_SURVEY` (`install_service.rs`, `spawn_system_installer`), and the
page's own table says so two paragraphs earlier. The sentence now names the role and keeps milestone
57's role as the shape it follows.

### 4. FIXED: installing.md's proposals, floor and pointers had moved on

- The five follow-ons were listed as files in `design/roadmap/proposals/`. Every one is a numbered
  milestone (560, built; 568, 569, 570, 572), and the table now links them.
- "A disk under about 600 MiB is refused". `Layout::fit` reserves 512 MiB for the EFI system
  partition, two 64 MiB slots, a 64 MiB data floor and the alignment: about 705 MiB, as
  `system_installer`'s own `BUGS` says ("about 700"). The figure is 700.
- "Nothing schedules it" now cites milestone 712 (the install gates run nowhere), as
  `xtask/src/install.rs`'s `BUGS` does.
- `kernel/src/user/fs_service.rs` grew its NVMe arm in what is now `fs_service/wiring.rs`.
- The appendix said the chooser's `the_disk_with_slots` collects boot partitions. `read_table`
  does; `the_disk_with_slots` picks the disk.

### 5. FIXED: stale claims beside the crate, in code and in milestone 813

- Milestone 813's exit called `BOOTRISCV64.EFI` "a name the `mkfs` refuses today"; milestone 560
  writes it. Its "What is proven" paragraph said only the writer's half of one property was
  proved, against its own table, which names nine reader harnesses. It cited `Volume::new` for
  `VolumeLayout::new`.
- `system_installer`'s `BOOT_FILE_NAME` doc sent a reader to the crate's `BUGS` "for the riscv64
  name that is not" written; the crate writes it. `Layout::fit` cited a reason "this file's header
  gives"; the header points to the appendix, which now gets the pointer.
- `uefi_loader`'s aarch64 and riscv64 `hand_over` cited "`main.rs`'s `place_boot_file`"; it is in
  `placement.rs`. The kernel's `initial_ramdisk` cited `hand_over` for the module order;
  `write_start_info` holds it.
- `VolumeLayout`'s example called 9,000,000 bytes "9 MiB". The README's writer limitation read as
  the whole crate's, and the reader takes 512 to 4096-byte sectors; it now names the writer.

### 6. MINTED: the installer's spawn and report words live in two files

`install_service.rs` and `system_installer.rs` each spell the three roles, the five capability
slots, the boot file's mapping address and four report words, with a "Must match" comment on each
side and no shared code. `install_service.rs` records why: the words are the kernel's own convention
with a program it spawns. Under the method that is a contract stated in prose, and
`hot_swap_contract` is the tree's existing shape for the same convention. Proposed as [the
installer's spawn and report words are one
definition](../roadmap/0973-the-installers-spawn-and-report-words-are-one-definition.md). It is
the installer's rows within milestone 407 (one definition of the numbers a kernel test and its
program agree on), whose measurement does not list this pair, and it waits on §187 (one crate per
kernel-test pair, or one crate for all of them?) for where the definitions live.

### 7. MINTED: the loader's two install agreements are prose

The PVH module order (archive at 0, boot file at 1) is an array order in `write_start_info` and
literal indices in the kernel's `machine.rs`, each comment naming the other. Which partition is boot
slot `n` is counted twice, by the chooser's `read_table` and by `confirm`'s loop, which the appendix
calls "the whole agreement between the two". Proposed as [the loader's two install agreements are
code](../roadmap/0974-the-loaders-two-install-agreements-are-code.md), with constants in
`machine_discovery` and a `boot_slot::nth_slot`.

### 8. MINTED: the boot stick's shared names are spelled on every side

The kernel's handover line, which every boot gate waits for, is spelled in six places; the loader's
first line in three; each architecture's removable-media file name in four; the payload directory's
environment variables in two; and the downloads' names in two. Proposed as [the boot stick's shared
names are code](../roadmap/0972-the-boot-sticks-shared-names-are-code.md), in
`crates/boot_ladder` beside the lines it already holds.

### 9. MINTED: the boot stick notes want the pilots' rewrite

The sort found 17 passages in the boot stick notes that narrate a code doc and 6 that belong beside
their code. This sweep made none of those cuts: the system-initializer pilot measured a third of
first-pass cuts as wrong, and a review pass is what caught them. Proposed as [the boot stick notes
are read against their
code](../roadmap/0971-the-boot-stick-notes-are-read-against-their-code.md), with the rows the
sort produced.

### 10. FIXED: the boot stick notes contradicted their code in twelve places

- "`cargo xtask stick` defaults to debug ... which one a customer downloads is rung 4's choice."
  `stick()` has built release only since the ruling of 2026-10-03 UTC; `--release` does nothing.
  The `BUGS` entry now says so, and the usage listing is marked as a 2026-09-19 debug build.
- "Erasing uses each host's own tool, not a FAT32 writer in this tree": the tree has one
  (`VolumeLayout`), used on disks the installer and `cloud_image` own. The reason for not erasing
  with it stays.
- `the-program.md`, "No stick shares a GUID": "Nothing in the build writes a partition table."
  `cloud_image::write` writes GPT disks, for `stick-boot` and `acpi-boot` (finding 11).
- `stick-boot` boots a fourth time, `riscv64_from_our_volume`, from a FAT32 volume this tree
  wrote; the proof table, the transcript (which now names the medium, as `boot_and_check` prints)
  and the time (eight minutes, four boots at the 120-second `NIFE_STICK_TIMEOUT`) follow.
- `acpi-boot`'s GICv3 boot is from `target/cloud-image/nife-aarch64.img`, not `target/stick`.
- `--disk ID --erase` is not "no questions": a FAT disk still gets `confirm_copy`'s question
  unless `--yes` is given.
- The block number of milestone 441 (the program that makes the stick) is not provisional; the block
  is BUILT.
- `the-program.md`: `windows.rs` is compiled everywhere, not "only where it runs"; Windows uses
  five `kernel32` functions, not four (`SetErrorMode` is the fifth), fixed in the note and in five
  code docs and comments; and Linux is no longer unchecked on a live kernel, since
  `stick-maker-hosts.yml` runs both paths on a loop device. `linux.rs`'s and `windows.rs`'s `BUGS`
  ("None of this has run") are corrected the same way.
- `argon.md` quoted the loader's "offers no device tree", which only riscv64 prints; aarch64 falls
  back to ACPI and says "neither a device tree nor ACPI".
- `device-tree-handover.md` names the symbol, `r_efi::efi::DTB_TABLE_GUID`, beside EDK2's name.

### 11. ACCEPTED: copies of one cloud image share its GUIDs

`cloud_image::write` draws fresh GUIDs per run, but CI uploads the aarch64 disk as an artifact, so
every import of one artifact carries the same disk and partition GUIDs. That is the case
`the-program.md` argues the stick avoids. Accepted: a cloud import is one disk per machine, and the
fix (re-drawing GUIDs at import) is the importer's step, not the build's. Recorded in
`cloud_image.rs`'s `BUGS` and in the note.

### 12. ACCEPTED: whether Windows cross-links from a Mac is unsettled

The note says Windows targets need a MinGW runtime or the Windows SDK to link; the comment in
`host/windows.rs` said `raw-dylib` lets the program cross-link from any host with `rust-lld`.
`raw-dylib` covers the five `kernel32` calls, not std's own runtime, and no trial build was run
here. Accepted until one is: the comment now says the rest of std is untested and points at the
note.

### 13. ACCEPTED: the crate defines its FAT32 cluster floor twice

`boot_sector.rs` holds a private `FAT32_MINIMUM_CLUSTERS` for typing a volume, and `volume.rs`
the public one for the writer's floor. Both are 65,525, inside one crate, so no two programs can
disagree. Accepted: milestone 813's stage 8 folds the writer into the general format, where the
two meet, and editing `boot_sector.rs` now would re-cut its four falsification patches for no
change in behavior.

### 14. ACCEPTED: the riscv64 banner names hart 0 whichever hart booted

The radon table in `notes/boot-stick.md` read "the kernel's `hart N booted`". `riscv64_banner` in
`kernel/src/boot.rs` prints a hard-coded `hart 0 booted`, but on RISC-V the boot hart's logical id
is its physical hart id (`percpu::boot_cpu_id`), which need not be 0. The note described what the
kernel should print, so the code is the wrong side. Recorded as a `BUGS` entry on
`riscv64_banner`; the note now names the kernel's `booted:` line without a number.

## What was deliberately not examined

- Milestone 813's two stage appendices (`stage-2-the-reader-begins.md`,
  `stage-3-the-table-and-the-chain.md`, 1,646 words), which are dated records of what each stage
  landed.
- `notes/x86-uefi-boot.md` and its appendices (about 12,000 words), which describe the loader the
  installer builds on; only the PVH module order was followed into it.
- `notes/boot-slots.md`, which the installer's appendix points to for the slot states.
- `crates/boot_slot`, `crates/globally_unique_identifier_partition_table` and
  `uefi_loader`'s chooser, beyond the functions a passage named.
- Every other "Must match" in the tree beyond the installer's, counted and not read (below).
- `design/decisions/`, read only where a passage cited it.

## What converts to a gate

Nothing in this scope. The phrases "Must match", "must agree" and "the two must" mark many
contracts stated in prose: 207 lines in 117 Rust files when the search ignores case, 173 lines in
95 files when it does not. They also match test oracles, hardware specifications, invariants inside
one crate and quotations of rule 7, and a ratchet on the wording would be met by rewording. The
count is a measurement; milestone 407's exit, one definition per pair, is the gate.

## What wants a lane of its own

The four proposals of findings 6 to 9.

---
status: NOT-STARTED
raised: 2026-11-09
milestone_dependencies: 813, 1034, 1043
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1061. `file_allocation_table`: the outside review's findings land before its first release

Raised by the lane `milestone/1061-file-allocation-table-outside-review`. A reviewer with no prior
exposure to this project read `nifeos/file_allocation_table` at commit `8622b02` (`main`, after its
pull request #16), against [the review package](../../notes/crate-reviews/file-allocation-table.md)
and [its public API appendix](../../notes/crate-reviews/file-allocation-table-public-api.md). This
block cites files and symbols at that commit, which is still `main`. It proposes work and changes
nothing in that repository. Every item below is work for a lane there, except where an item names
nife's own callers.

The review package and milestone 813 (`file_allocation_table`: a proven FAT crate, released on its
own) settle the crate name, the names ratified at the package's clearance, the license and the
version. They stay closed here, except for the five names the rulings below replace. Milestone 813
owns the release path; this block owns the findings that land inside it.

Reuse: the shared crate workflows (`check`, `proofs`, `mutation`, `weekly` and `release` callers),
the tests against rust-fatfs (through `lamfat`) and `fsck.fat`, and the stranger cadence in
`docs/stranger-test.md`. No new tool.

## Goal

A stranger finds `file_allocation_table` on crates.io, reads its README, depends on it, and reads,
writes or formats a FAT12, FAT16 or FAT32 volume without asking anyone, on a volume another system
wrote and on one another system will read. Everything that would break that stranger later (a public
name, a signature, a default that changes output bytes) is settled before the first tag, and
everything that only improves the crate waits for a later minor release.

The organization's publishing hold exempts this crate. What stands in its place is an assessment of
whether the crate is complete and offers what no other FAT crate does, and this block is that
assessment's worklist. Nothing in it places a tag or publishes: each is a separate go of the
architect.

## What the review covered

The reviewer ran these firsthand at `8622b02`, on an x86_64 Linux host with Rust 1.99.1 stable:

| check | result |
|---|---|
| `cargo test` | 332 pass, 1 ignored (the macOS driver test); `fsck.fat` 4.2 and `mkfs.fat` were present, so the tests that call them ran |
| `cargo clippy --all-targets`, `cargo doc` with warnings denied | clean |
| `cargo build --lib` for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf`, `x86_64-unknown-none` | all three build |
| `cargo package` | 126 files, 797 KiB (205 KiB compressed), verifies with no warning |
| every library source file, the harness list, the integration tests, both fuzz targets, the workflows and the repository settings | read in full (7,824 lines of library source) |
| a scratch program | creates `readme.txt`, `a.b` and `hello world.txt` on a fresh FAT16 volume, renames one, marks it clean; `fsck.fat -n` reports it clean |
| crates.io figures for the nearest crates | read from the registry's API |

Rechecked for this block: `main` is still `8622b02`, and the template's `main` is four commits past
the pin, the one-shard verdict fix among them. The `proofs` run on `main` at this commit is green
(31 harnesses).

Not run: `cargo kani` by the reviewer, `cargo mutants`, `cargo fuzz`, Miri, `cargo semver-checks`
(no baseline tag exists), the macOS driver test, and any device that injects faults for the
power-loss findings.

Effort sizes: S is an afternoon for a lane, M a lane, L several lanes.

## Findings, prioritized

P0 blocks any publish. P1 lands before the first release, because changing it afterward breaks or
surprises a dependent. P2 follows it, in
[the appendix](1061-file-allocation-table-outside-review/after-the-first-release.md). A door is
one-way where something outside the repository acts on it after the release: a public name or
signature, the bytes a format writes, or the license. The maintainers decide every two-way item
provisionally.

### P0: blocks any publish

#### P0-1. No full mutation run exists, and the release gate requires one

The caller `.github/workflows/mutation.yml` runs on a schedule and by dispatch, and its run list is
empty. The template's `helpers/release-gate.sh` refuses a release without a green full run at the
released commit.

Change: after P0-2, dispatch `mutation.yml` on `main`, read the survivors, and add a test or a
reasoned line in `.github/mutants-exempts` for each. Two-way. S to M.

#### P0-2. The template pin predates the one-shard verdict fix

Every caller pins `nifeos/crate-workflows` at `57f2d528`. The template commit `ced625f4` fixes the
mutation verdict for a run of one shard, which failed every one-shard repository because
`actions/download-artifact` extracts a lone artifact one level up. This crate's census is about 150
mutants against a 200-per-shard default, so it runs as one shard, and P0-1 cannot go green at the
current pin.

Change: the next pin bump, to `e9723181` or the template's next release tag. Two-way. S.

#### P0-3. The first publish needs a token placed once

The `release` caller asks for `id-token: write`. A crate that does not yet exist on crates.io cannot
use trusted publishing for its first upload, so the first publish needs a crates.io token placed
once, at the architect's go to publish. Every later release uses trusted publishing. Two-way. S.

### P1: before the first release

#### P1-1. A directory grows by linking a cluster before zeroing it

`grow_directory` in `src/file_writer.rs` calls `append`, which writes the link into every table
copy, and only then writes zeros over the new cluster. A power loss between the two leaves a
directory whose last cluster holds whatever the device held, which a reader parses as entries.
`create_directory` in `src/directory_writer.rs` already takes the safe order: take the cluster, fill
it, then make it reachable.

Change: take and mark the cluster, zero it, then link it, and add a test over a device that fails
after a chosen write count. Two-way. S.

#### P1-2. The crash-ordering claim does not hold for `rename`

The README's `BUGS` and the crate overview say a crash leaks a cluster and never shares one.
`rename` in `src/directory_writer.rs` writes the new entry, updates `..` for a moved directory, then
frees the old slots. A crash between the first step and the last leaves two entries naming one
chain, which `fsck.fat` reports as shared clusters and truncates one of. The other order loses the
file on the same crash.

Change: keep the order, and qualify the claim in both places: a crash during a rename can leave two
entries on one chain, and every other operation leaks and never shares. Two-way. S.

#### P1-3. The `main` ruleset lacks a required check the template now expects

The weekly `settings drift` job is red: the required checks lack `check / mutation-diff`, which
`check.yml` at the pin defines. Change: an administrator adds the check to the ruleset, through the
template's apply script. Two-way. S.

#### P1-4. The default cluster size is one sector at every volume size

`FormatPlan::new` in `src/format.rs` tries cluster sizes from one sector up and keeps the first that
gives a legal count. A default format of a 100 GiB device gets 512-byte clusters and a table near
800 MiB per copy, and the README's write amplification entry measures 5.0 table writes per data
sector. `mkfs.fat`, Windows and the specification's `DskTableFAT16` and `DskTableFAT32` choose the
cluster size by volume size. Changing the default after release changes the bytes of every
default-formatted image, which a dependent who pins digests will notice.

The maintainers' provisional call: default to the specification's tables when the caller names no
cluster size, keep one sector as an explicit choice, and update the pinned-digest test and the
README's size examples. M.

#### P1-5. `VolumeLayout::new` takes seven positional parameters

`VolumeLayout::new` in `src/volume.rs` takes three `u64` and two `&str` side by side. A swap of the
two strings fails loudly; a swap of the three sizes compiles.

Ruled 2026-11-10. `VolumeLayout::new(options, clock)` takes a `VolumeLayoutOptions`, built as the
general format's `FormatOptions` is: `VolumeLayoutOptions::new(total_sectors)`,
`with_hidden_sectors`, `with_volume_id`, `with_label` and `with_file(name, size_bytes)`. The names
are provisional until the release review. nife's two callers, `xtask/src/cloud_image.rs` and
`components/src/system_installer.rs`, move to the new shape when nife takes the release. One-way
(signature). S to M.

#### P1-6. `CivilDateTime` has public fields and stops at seconds

`CivilDateTime` in `src/time.rs` has six public fields. FAT's creation time carries hundredths, and
a seventh field later breaks every struct literal.

Ruled 2026-11-10. The fields become private. A checked
`CivilDateTime::new(year, month, day, hour, minute, second)` returns a `Result` and refuses a date
no calendar holds: a month outside 1 to 12, a day past the end of its month, an hour, minute or
second out of range. Six getters (`year`, `month`, `day`, `hour`, `minute`, `second`) read it. A
year FAT cannot store stays the writer's `TimeOutOfRange` refusal, since a real date can fall
outside FAT's range. The error's name is the lane's to mint, provisional with the rest. nife's fuzz
target `fuzz/fuzz_targets/file_allocation_table_writer.rs` builds the value by literal and moves to
`new`. One-way (shape). S.

#### P1-7. `VolumeLabel::trimmed` has a standard-library spelling

`VolumeLabel::trimmed` in `src/label.rs` returns the label without its trailing spaces. The
standard library spells that `trim_ascii_end` on `[u8]`, and a label holds no other ASCII
whitespace, so the meanings coincide on every value the type holds.

Ruled 2026-11-10. It is renamed `trim_ascii_end`, with no `doc(alias)` for the old name, since the
crate has no release. A test pins that the result equals `as_bytes().trim_ascii_end()` for a label
padded with spaces. One-way (name). S.

#### P1-8. A directory is a bare `u32`

`directory`, `named_entries`, `find_in_directory`, `create_file`, `create_directory` and `rename`
take a directory as a `u32`, 0 for the root. Nothing stops a caller passing a file's first cluster,
and the chain walk, not the type, refuses it. The stranger run named it as a surprise.

Ruled 2026-11-10. A `DirectoryCluster(u32)` type with the associated constant
`DirectoryCluster::ROOT` replaces the `u32` in every method that takes a directory.
`NamedEntry::directory_cluster() -> Option<DirectoryCluster>` returns it for a directory entry and
`None` for a file. The names are provisional until the release review. One-way (signatures). M.

#### P1-9. `open_file_reader` cannot report a device error

`open_file_reader` in `src/file.rs` returns `FileOpenError`, which carries `IsADirectory` and
`IsAVolumeLabel` and no device error. So the reader cannot check that the entry's slot still holds
the entry, as `FileWriter` does on its first write. The README's `BUGS` carries the result: a reader
opened from the entry `create_file` returned reads 0 bytes with no error. Widening an error type
after release breaks every caller that matches it; doing it now breaks nobody.

Ruled 2026-11-10. One generic `OpenError<E>`, with `IsADirectory`, `IsAVolumeLabel`, `EntryChanged`
and `Chain(ChainError<E>)`, replaces `FileOpenError` and is shared by `Volume::open_file_reader` and
`WritableVolume::open_file_writer`. Both verify the directory slot at open. A test pins that
`EntryChanged` means the same in each: the slot no longer holds the entry the caller passed. The
stale-entry entry in the README's `BUGS` shrinks to looking the entry up again after a write. The
name is provisional until the release review. nife's fuzz target
`fuzz/fuzz_targets/file_allocation_table_boot_sector.rs` matches the new error. One-way (shape). S
to M.

## Decisions this block needs

Ruled 2026-11-10, and closed here:

1. `VolumeLayout::new(options, clock)` takes a `VolumeLayoutOptions` built with `new`,
   `with_hidden_sectors`, `with_volume_id`, `with_label` and `with_file` (P1-5).
2. `CivilDateTime` has private fields, a checked `new` that refuses an impossible date, and six
   getters (P1-6).
3. `VolumeLabel::trimmed` becomes `trim_ascii_end`, with no alias of the old name, and a test pins
   the standard library's meaning (P1-7).
4. A `DirectoryCluster(u32)` with `ROOT` is the directory handle, and
   `NamedEntry::directory_cluster()` returns `Option<DirectoryCluster>` (P1-8).
5. One `OpenError<E>` serves both `open_file_*` methods, both verify the slot, and a test pins that
   `EntryChanged` means the same in each (P1-9).

Every name these rulings bring is provisional until the release review ratifies them together.

For the architect, one-way, not yet ruled: the tag and the first publish, each asked separately once
the P0 and P1 items have closed.

The maintainers' provisional calls: every two-way item above and in the appendix, among them the
default cluster size (P1-4), untailed aliases (P2-2), the lenient open (P2-3) and the keywords
(P2-8), and the names a lane mints for them, which go through the naming review before any tag.

## Prior art

rust-fatfs, `lamfat`, embedded-sdmmc, hadris-fat and the small FAT32 crates, against this one, are
in [the prior art appendix](1061-file-allocation-table-outside-review/prior-art.md).

## Done when

The checks are in [the done-when appendix](1061-file-allocation-table-outside-review/done-when.md).

## BUGS

- The power-loss findings (P1-1 and P1-2) were read, not reproduced: no fault-injecting
  `WritableSectorDevice` exists in the tests. The lane that fixes P1-1 adds one.
- Every statement about another crate's behavior is a reading of its source or documentation, not
  an execution; only the download figures were fetched.
- The one-shard count of P0-2 rests on the census line in `.github/mutants-exempts`, not on a count
  from `cargo mutants --list`.
- The P2 order is the reviewer's judgment of what a user asks for first. Nobody outside has asked
  for anything yet.

## Index row

An outside reading of `file_allocation_table` before its first release: three publish blockers (no
full mutation run, a template pin behind the one-shard fix, the first-publish token) and nine items
before the release. Five carry rulings: a `VolumeLayoutOptions` builder, a checked `CivilDateTime`,
`trim_ascii_end`, a `DirectoryCluster` handle and one `OpenError<E>`. Thirteen follow the release.
The tag and the first publish are open.

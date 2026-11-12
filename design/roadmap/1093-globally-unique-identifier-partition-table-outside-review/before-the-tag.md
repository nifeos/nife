# P1: before the tag

An appendix to [the milestone](../1093-globally-unique-identifier-partition-table-outside-review.md).

## P1: before the first tag

2. `Entry::blocks` panics on an entry a caller can build.
   Finding: `blocks` returns `Some(self.last_lba - self.first_lba + 1)` once `last_lba >=
   first_lba`. For `Entry::new(t, u, 0, u64::MAX)` the addition overflows: a panic
   ("attempt to add with overflow") with overflow checks on, `Some(0)` with them off. Verified in
   the scratch probe. No parsed table reaches it: `parse` and `create` both run `check_partitions`,
   which bounds every used entry inside a usable range that ends before the backup array. But
   `Entry` is a public struct with public fields and a `const fn new`, the documentation promises
   `None` only for `last_lba < first_lba`, and the fuzz target calls `blocks` only on parsed
   entries, so nothing in the tree can find it.
   Evidence: `src/entry.rs` line 120 to `src/entry.rs` line 126; `fuzz/fuzz_targets/globally_unique_identifier_partition_table.rs`.
   Change: `self.last_lba.checked_sub(self.first_lba)?.checked_add(1)`, with the documentation
   saying `None` also names a range the count cannot hold; one test; and a two-line Kani harness
   over two symbolic `u64`s that `blocks` never panics, with its falsification patch.
   Door: two-way. Effort: XS.

3. The crate cannot open a disk whose primary table has damage, and the specification requires it to.
   Finding: `parse` refuses a header whose `my_lba` is not 1 (`Error::NotPrimary`), and
   `check_backup` needs a parsed primary. So when the primary header or its array is corrupt, which
   is the case the backup exists for, the crate offers no way to read the backup, learn the
   partitions, or restore the primary. UEFI 2.10 section 5.3.2: "If the primary GPT is corrupt,
   software must check the last LBA of the device to see if it has a valid GPT Header and point to
   a valid GPT Partition Entry Array. If it points to a valid GPT Partition Entry Array, then
   software should restore the primary GPT if allowed by platform policy." Linux, FreeBSD and
   `gdisk` all read the backup on its own (prior art). The crate's own documentation calls the
   recovery story its reason for the `NIFE_DATA` constant, and this is the half of it the crate
   does not do.
   Evidence: `src/table.rs` line 58 to `src/table.rs` line 62; `src/table.rs` line 217 to `src/table.rs` line 253;
   `src/guid.rs` line 139 to `src/guid.rs` line 145.
   Change: add `parse_backup(backup_block, backup_entries, block_count: u64)`. It decodes the
   header, requires `my_lba == block_count - 1` and `alternate_lba == PRIMARY_HEADER_LBA`, and
   applies the same array and geometry checks (`entry_array_lba > last_usable_lba`, `entry_array_lba
   + array_blocks <= my_lba`). It returns a table normalized to the primary's orientation (`my_lba`
   1, `alternate_lba` the last block, `entry_array_lba` 2, the convention every tool writes and the
   one `write_backup_header` already assumes in reverse). Then `write_primary_header` and
   `entry_array` restore the primary with no new code, and `check_backup` against the real primary
   says whether a restore is needed. Add one test that starts from the `sgdisk` fixture's tail with its
   head zeroed. Add one Kani harness, with a patch, that `parse_backup` of what
   `write_backup_header` wrote agrees with the table that wrote it.
   Door: two-way (additive). Effort: M.

4. Nothing compares the header's idea of the disk with the device.
   Finding: `parse` takes the disk's size from `alternate_lba + 1` and checks every other field
   against that. A table cloned onto a smaller or larger disk, or a disk grown under its table,
   parses clean. `check_protective_mbr` then checks the MBR against the header's figure, not the
   device's, so an MBR cloned with it passes too. Linux checks `first_usable_lba` and
   `last_usable_lba` against the device's last block and warns when `alternate_lba` is not there.
   FreeBSD marks a header invalid when `hdr_lba_alt > last`. The documentation of `parse` and of
   `block_count` does not tell a caller to compare. No caller in nife does. `uefi_loader`'s chooser
   holds `media.last_block` and never compares it with `table.block_count()`. `system_installer`
   reads the device's size for its own refusal and not for this one.
   Evidence: `src/table.rs` line 67 to `src/table.rs` line 72; `src/table.rs` line 270 to `src/table.rs` line 273;
   `src/table.rs` line 344 to `src/table.rs` line 346; `uefi_loader/src/chooser.rs` line 340 to `:361`.
   Change: add `check_disk_size(&self, device_blocks: u64) -> Result<(), Error>` with a new variant
   `Error::DiskSize { header: u64, device: u64 }`. Require equality, since a grown disk is a resize
   for a tool to perform, not a table to accept. Add one sentence in `parse`'s documentation and the
   README contract naming the three checks a caller runs after `parse` (disk size, backup,
   protective MBR). Add a `BUGS` line in each nife caller that does not yet call it, for the lanes
   that follow.
   Door: two-way. Effort: S.

5. `parse` cannot tell where the array bytes came from, and every caller guesses block 2.
   Finding: the documentation says `entry_array` "is read from the header's `PartitionEntryLBA`",
   but a caller learns that field only from `Header::decode` or from `parse` itself. All seven nife
   callers, the fuzz target and the README example read block 2 unconditionally. The specification
   requires only that the primary array lie after the header and end before the first usable block.
   A conformant table with its array elsewhere therefore fails here with `EntryArrayCrc` (the bytes
   from block 2 are not the array) or `TableOverlapsUsable`. The error documentation attributes both
   to a corrupt disk. Verified in the scratch probe with
   `entry_array_lba` set to 3 on the sample disk.
   Evidence: `src/table.rs` line 22 to `src/table.rs` line 28; `src/error.rs` line 37; `README.md` line 47;
   `uefi_loader/src/chooser.rs` line 355; `components/src/system_installer.rs` line 215.
   Change: take the cheap form first. `parse`'s documentation and the README example show the two
   steps (`Header::decode` the block from LBA 1, read `entry_array_lba` for `array_blocks`, then
   `parse`). `Error::EntryArrayCrc`'s documentation names "an array read from the wrong
   block" as a cause. The maintainer may prefer the firm form, a `read_from: u64` parameter with
   `Error::EntryArrayElsewhere { header: u64, read: u64 }`. It changes the one signature every
   caller uses, which is cheap before the tag and not after.
   Door: two-way now, one-way for the firm form after the tag. Effort: S.

6. The claim "no `unsafe`" is prose.
   Finding: the README and the crate root say the crate has no `unsafe`, and it has none, but
   nothing refuses one. The template's weekly fuzz opts a crate in only when its shipped source
   carries no `unsafe`, and its scan passes a crate whose roots carry `#![forbid(unsafe_code)]`
   without reading further.
   Evidence: `src/lib.rs` line 5; `README.md` line 5; `crate-workflows/README.md`, "The weekly fuzz run".
   Change: `#![forbid(unsafe_code)]` under `#![no_std]`.
   Door: two-way. Effort: XS.

7. Twenty-three public items have no documentation, and the lint that would say so is off.
   Finding: `Cargo.toml` turns `missing_docs` on and `src/lib.rs` line 17 turns it off again, for the
   eighteen fields of `Error`'s variants, four variants of `BackupMismatch` and the field of
   `MbrProblem::ExtraRecord`. The README's `BUGS` records it. The review package assigns it to the
   lane that ratifies the names; this block adds the shape of the fix.
   Evidence: `src/lib.rs` line 15 to `src/lib.rs` line 17; `src/error.rs` line 25 to `src/error.rs` line 69;
   `src/error.rs` line 86 to `src/error.rs` line 89.
   Change: one line per field, saying what the number is ("the CRC the header stores", "the CRC
   computed over the bytes", "the array's zero-based index"); drop the `allow` and the `BUGS` line.
   Door: two-way. Effort: S.

8. The error type is not an error, and two of its parts print as Rust.
   Finding: `Error` implements `Display` and not `core::error::Error` (stable since Rust 1.81,
   under this crate's 1.89); a caller cannot put it in a `Box<dyn Error>` or behind `?` into
   `anyhow`. `BackupMismatch` and `MbrProblem` have no `Display`, so a person reads "the backup
   header disagrees about MyLba" and "the protective MBR is wrong: ExtraRecord { index: 1 }". None
   of the three enums is `#[non_exhaustive]`, so items 3, 4 and 5 each add a variant that would be
   a breaking change after the tag, and so would any later one.
   Evidence: `src/error.rs` line 114 to `src/error.rs` line 170; `src/error.rs` line 159 and `src/error.rs` line 160.
   Change: `impl core::error::Error for Error {}`; `Display` for `BackupMismatch` ("its MyLBA
   field") and `MbrProblem` ("record 1 is a second partition: a hybrid MBR"), fed from the `Error`
   messages; `#[non_exhaustive]` on all three. The last is the stricter choice, and the one that is
   reversible: dropping it later breaks nobody, adding it later breaks every exhaustive `match`.
   Door: two-way until the tag; the `#[non_exhaustive]` choice is one-way after it. Effort: S.

9. The template's name gates are red today, on ten register names and one spelled-out word.
   Finding: `register_names` refuses `entry::SIZE`, `entry::SIZE_U32`, `header::HEADER_SIZE`,
   `header::at::HEADER_SIZE`, `header::at::ENTRY_SIZE`, `Header::entry_size`, `MINIMUM_BLOCK_SIZE`,
   `MAXIMUM_BLOCK_SIZE`, `mbr::SIZE_LBA` and the field `block_size`; `spelled_out` refuses the test
   `the_three_reads_a_gpt_reader_makes`. The package's rows 4 to 7 and 15 cover seven of these. It
   did not list `SIZE_U32`, the two offset constants in `header::at` or `mbr::SIZE_LBA`. They are
   private and `name_release.py` does not list them, but the template judges every name the crate
   defines.
   Evidence: the gate output; `src/entry.rs` line 16, `src/entry.rs` line 205, `src/header.rs` line 25,
   `src/header.rs` line 33, `src/header.rs` line 43, `src/mbr.rs` line 21.
   Change: in the one rename lane the package already plans, add the four. They are
   `entry::ENTRY_BYTES_U32` beside the package's `ENTRY_BYTES`; `at::HEADER_BYTES` and
   `at::ENTRY_BYTES` so the offset constants follow the fields they locate; and `mbr::SIZE_BLOCKS`
   with `#[doc(alias = "SizeInLBA")]` (the unit is blocks; under the package's decision 2 the `_LBA`
   names change together). All are provisional, for the architect's ratification with the package's
   table.
   Door: names are the architect's; the private four are two-way. Effort: XS on top of the
   package's lane.

10. The template's prose gate is red on nineteen acronyms.
    Finding: `check / standalone` wants every acronym expanded at its first use, at a ceiling of 0.
    `prose_acronyms` reports these. In the README (lines 3 and 24): UEFI, GPT and RFC. In the crate
    root (`lib.rs` line 1, `lib.rs` line 4): GPT and UEFI. In `crc.rs` line 3: IEEE and PNG. In `entry.rs` line 22,
    `:26` and `:30`: OEM, IO and BIOS. In `guid.rs` line 1, `:3`, `:61`, `:69`, `:114`, `:123` and
    `:131`: UUID, RFC, EFI, FAT, GRUB, LUKS, HFS and APFS. In `entry.rs` line 132: FFFD, which is the
    code point U+FFFD and a false positive the gate should learn.
    Evidence: the gate output.
    Change: expand each at its first use in the README and once per crate in the sources ("Unified
    Extensible Firmware Interface (UEFI)", "Request for Comments (RFC)", "Logical Unit Key Setup
    (LUKS)"). For FFFD, write "the replacement character U+FFFD" and report the false positive to
    the template. A hexadecimal code point after `U+` is not an acronym. The template owns that
    fix; this block names it on the blocked side.
    Door: two-way. Effort: S.

11. The repository's mutation verdict stays red until the maintainers exempt the recorded equivalents.
    Finding: the recorded baseline for the crate is 373 mutants, 3 missed, 2 timed out, 33 unviable.
    The triage finds six survivors equivalent: a shift of zero, three disjoint-bit `|` to `^`, one
    comparison the next check repeats, and one `usize` bound a 64-bit host cannot fail. The
    template's `mutation` verdict judges every shard against `.github/mutants-exempts` and fails on
    a missed mutant that is not there; the scaffold's file is empty.
    Evidence: `.cargo/mutants-baseline.txt` line 48; the crate's section of the recorded census triage
    under `notes/mutation-testing/`, which `notes/mutation-testing/README.md` line 63 links;
    `crate-workflows/README.md`, "The full mutation run".
    Change: the move lane carries the six equivalents into the new repository's
    `.github/mutants-exempts`, each with the triage's one-line reason. It dispatches `mutation.yml`
    once on `main` so the release gate has a green full run to read. The two timeouts are the
    exhaustive sweeps. The shard timeout or `--timeout-multiplier` needs checking on the first run.
    Door: two-way. Effort: S.

12. The README is not yet the template's README.
    Finding: there is no badge row under the title (`readme_badges` fails `check / standalone` on
    it). The patch recipe says `patch -p3` (`README.md` line 71), which is the path depth inside nife,
    and the regenerated patches in the new repository apply at `-p1`. The README has no `cargo kani
    --harness <module>::<name> --exact` form, no measured time and peak memory for `cargo kani`, and
    no sentence on what a passing run prints that looks alarming. It has no `rustup target add` line
    before the bare-metal build. The `uuid` sentence under "Minimum Rust version" is right (verified
    against 1.28.0 and 1.29.2), and the scaffold wants the `cargo +1.89 test` check beside it. The README's one `BUGS` line is item
    7 and goes with it.
    Evidence: `README.md` line 1, `:71`, `:88` to `:92`, `:125`, `:129`; `crate-workflows/scaffold/README-crate.md`.
    Change: add the scaffold's badge row with this repository's name. Add the "Running the proofs"
    and "Running one falsification patch" sections with this crate's figures (165 seconds total on
    the review host, the peak from `/usr/bin/time -v`). Add the target lines and the version check.
    Then record a stranger run.
    Door: two-way. Effort: S.

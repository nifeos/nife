# What was checked, and how

An appendix to [the milestone](../1093-globally-unique-identifier-partition-table-outside-review.md).

Verified firsthand, on a fresh shallow clone of `nifeos/nife` at its head, on a Linux host. The
host had the workspace's nightly toolchain (1.102) and the three bare-metal targets the README names:

- `cargo test --release`: 13 unit tests, 12 tests in `tests/real_disks.rs` (one `#[ignore]`, the
  163-second sweep, not run), 33 in `tests/table.rs` and 4 documentation tests, the README's
  included. All pass. `cargo clippy --all-targets` and `cargo doc --no-deps` print nothing.
- `cargo build --lib` for `x86_64-unknown-none` and `riscv64imac-unknown-none-elf` passes.
- Kani 0.68.0 verifies all eight harnesses in 165 seconds of solver time. The two CRC harnesses take
  56 and 19 seconds, and `create_never_lays_out_a_table_parse_would_reject` takes 66. The
  falsification patches were not replayed. The review package records all eight failing their own harness.
- `cargo package --list`: the manifest, both licenses, the README, eleven source files, eight
  patches, two test files and four fixtures of 16.5 to 17 KiB. Cargo warns that the manifest has
  no `documentation`, `homepage` or `repository`.
- The template's gates, run from a clone of `nifeos/crate-workflows` at its head over `src/*.rs`
  with the crate's manifest: `unit_symbols`, `question_shape`, `british_spelling`, `contract_names`
  and `service_model` judge 432 names and refuse none. `spelled_out` refuses one and
  `register_names` refuses ten. `prose_acronyms` reports nineteen unexpanded acronyms in the README
  and the sources. `readme_sections` reports no finding. `readme_badges` reports four, all the
  move's to fix (no badge row, no `repository`, no caller workflows).
- Three probes in a scratch crate depending on the crate by path. They cover `Entry::blocks` on a hand-built
  entry (item 2), `parse` handed an array read from the wrong block (item 5), and the length of the
  table's `Debug` output (item 15).
- The UEFI 2.10 specification's chapter 5 as published at uefi.org, for every sentence this block
  cites. Also Linux `block/partitions/efi.c` and FreeBSD `sys/geom/part/g_part_gpt.c` at their
  repositories' heads, the public API of `gpt_disk_types` and `gpt_disk_io` from their sources, and
  download figures for the comparable crates from the crates.io API.
- The `uuid` dependency: 1.28.0 declares `rust-version = "1.89.0"`. So do 1.29.0 through 1.29.2,
  which a fresh lockfile resolves to. The crate's stated minimum holds either way.
- The reviewer read every caller in nife (`components`, `boot_slot`, `redoxfs_server`,
  `tools/redoxfs_host`, `uefi_loader`, `xtask`, the `fuzz` workspace) for how it hands bytes to
  `parse`.

Inferred from reading, not run: the mutation figures (the recorded baseline, 373 mutants, 3 missed,
2 timed out, 33 unviable, with the triage that finds 6 of 7 recorded survivors equivalent). Also the
behavior on a 32-bit host (items 16 and 17), and whether any partitioning tool writes a backup array
that does not end on the block before the backup header (item 13).

Not run: a mutation census (outside its window), the 163-second exhaustive sweep, the fuzz target,
`cargo semver-checks` (no prior release), the stranger test, `script/crate-move --dry-run` (the
package ran it), and the falsification replay.

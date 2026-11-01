# Stage 9, the part that needs no clock: attributes, labels and the release checks

Appendix to [813. `file_allocation_table`](../0813-a-proven-fat-crate-released-on-its-own.md). It
records what the clock-free part of stage 9 built. Timestamps are the rest of stage 9; they wait on
the question of how a crate with no clock gets the time, which pull request #2984 carries.

## What this part built

- `Attributes`, a set of the four bits a user sets (read-only, hidden, system, archive), and
  `Volume::set_attributes(entry, attributes)`, which sets exactly those bits and keeps the
  directory bit and every other. It refuses the volume label, `.` and `..`. `NamedEntry::file_attributes`
  reads them. A file with the read-only bit refuses a write, a resize and a remove with
  `WriteError::ReadOnly` until the bit clears.
- `VolumeLabel`, `Volume::volume_label` and `Volume::set_volume_label`. A label is up to eleven
  upper-case letters, digits, spaces and the punctuation a short name admits. It lives twice, in the
  root's label entry and in the boot sector (and FAT32's backup boot sector); the read prefers the
  root and falls back to the boot sector, and treats `NO NAME` as no label. A change writes both.
- `README.md` rewritten for release, with an example that `tests/the_readme_example.rs` compiles and
  runs, and a manifest description that says what the crate does now.
- `script/crate-standalone file_allocation_table` passes on everything but one check. The fixture
  notes no longer cite the nife tree, and the copy builds on Rust 1.88 for the host and the three
  bare-metal targets. What remains is names: 191 provisional names await ratification, which the
  architect does with `python3 helpers/name_release.py --draft file_allocation_table`.

No clock and no time appear in any signature this part adds.

## The evidence

| check | result |
|---|---|
| Kani, two new harnesses | `setting_attributes_changes_the_user_bits_and_nothing_else` covers every stored attribute byte and every set: the user bits become the set, every other bit stays, and a byte that was not a label never becomes a long-name piece. `an_accepted_label_is_legal_bytes_then_spaces` covers every byte string up to twelve bytes |
| falsification | one patch ORs the new bits into the old ones, the other lets a twelve-byte label through; each turns its harness red at its own assertion |
| unit tests | attribute sets, the stored byte, files and directories, read-only refusals and release, dots and stale entries; labels set, changed, read from the boot sector, `NO NAME`, refusals, a full fixed root |
| oracle tests | on FAT12, FAT16 and FAT32 volumes rust-fatfs formatted, every combination of the four bits on a file and a directory reads the same in rust-fatfs and passes `fsck.fat -n`; a label this crate sets is the one rust-fatfs reports, in the root and the boot sector and on FAT32 the backup; a label a format wrote and a label `mkfs.fat` wrote read back here and change |

## Names minted

All provisional: `Attributes` (`NONE`, `READ_ONLY`, `HIDDEN`, `SYSTEM`, `ARCHIVE`, `union`,
`without`, `contains`, `bits`), `Volume::set_attributes`, `NamedEntry::file_attributes`,
`WriteError::ReadOnly`, `VolumeLabel` (`as_bytes`, `trimmed`), `Volume::volume_label`,
`Volume::set_volume_label`, and the modules `attributes` and `label`.

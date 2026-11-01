# Stage 9, the part that needs no clock: attributes, labels and the release checks

Appendix to [813. `file_allocation_table`](../0813-a-proven-fat-crate-released-on-its-own.md). It
records what stage 9 built: the clock-free part (attributes, labels, the release checks) and
timestamps, which follow a ruling of 2026-11-01 (UTC) that the clock is required.

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

## Timestamps and the required clock

The architect ruled on 2026-11-01 (UTC): any caller that writes to a volume supplies a clock, and "no time" is
an explicit value the caller writes. The clock's method may report no time yet, so a real clock with
no time set stamps the zero timestamp. The ruling refuses an optional clock, the time on every call, and documented
zero timestamps. A required clock can later relax to optional without breaking semver; the reverse
cannot.

- `Clock` (`now(&mut self) -> Option<Timestamp>`), `Timestamp` (year, month, day, hour, minute and
  second as plain numbers, no nife type), and `NoClock`, which always reports none.
- `WritableVolume<D, C: Clock>` holds a `Volume` and its clock and derefs to the `Volume`. Every
  write method lives there; a read-only `Volume` has none, so a write without a clock does not
  compile. The clock is a type parameter and not a trait object, so the crate keeps `no_std`
  without an allocator and a caller's clock is a plain value. `Format::new` and
  `VolumeLayout::new` take a clock too, and read it once.
- Each operation reads the clock once. A new file or directory (and its `.` and `..`, and a new
  label entry) takes that time as its creation, write and access time, with the odd second in the
  creation tenths byte. A write or a resize sets the write time, the write date and the access date, and an
  empty write stamps nothing. A rename keeps the entry's times; an attribute change, a removal and
  every read use no clock.
- The installer and the cloud image pass `NoClock`. `tests/the_zero_timestamp_images_do_not_change.rs`
  holds digests taken before the change over every metadata sector of the EFI volumes and the
  formatted volumes, and they still match.

The architect ruled on 2026-11-01 (UTC) that timestamps are UTC: the required clock supplies UTC, the crate stores the fields the clock reports, converts nothing and cannot check the zone. `times_agree_on_fat12`, `_fat16` and `_fat32` in `tests/the_timestamps_agree.rs` already have an independent reader return exactly the clock fields, with no offset.

The architect also ruled detail (b) on 2026-11-01 (UTC). The crate refuses a time FAT cannot store
(a year before 1980 or after 2107, or a field outside its range) with `TimeOutOfRange` on `WriteError`,
`FormatError` and `LayoutError`, before it writes a byte. Relaxing a refusal to a clamp later is
compatible; tightening a clamp is not. `None` is unchanged. The architect ruled detail (a) the same day: reading takes no clock and never writes. A read-only
open is a `Volume` with no clock parameter, and only `WritableVolume<D, C: Clock>` stamps. A read
leaves the last-accessed date alone, because a read must work on a read-only device or image. A
clock-stamping read, if ever wanted, arrives as a new method. One recommendation stays open. The
code follows it, and it is easy to change before release.

- Ruled (a): `tests/a_read_never_writes.rs` reads every way the crate reads, over a device with no
  write method and over one whose write method panics, on FAT12, FAT16 and FAT32. The reads are
  lookup, listing, file bytes, the label, the free count, the clean bit, `FSInfo` and a chain. The
  image stays byte for byte as it was. The test found that `Volume::is_clean` needed a writable
  device; it now needs only a readable one.
- Recommended, not yet ruled (c): the crate stores exactly what the clock reports. FAT carries no
  time zone and a host shows the stored time as local time, so a caller chooses local time or UTC.

| check | result |
|---|---|
| Kani | `any_time_packs_to_legal_fields` covers every year, month, day, hour, minute and second, valid or not: the pack refuses exactly when the time is out of range, and otherwise the packed fields are legal, the tenths byte is 0 or 100, and the time comes back exactly |
| falsification | a month check that lets 13 through turns the harness red at its range assertion |
| unit tests | packing, clamping, the two stamp shapes, and a counting clock that proves one reading per operation, none for a rename, an attribute change, a removal or a read, and every stamped field on create, directory, `.`, `..`, write, resize and label |
| refusal | for create, make directory, label, write (of bytes and of none) and resize (up and down), an out-of-range time returns `TimeOutOfRange` and leaves the image byte for byte as it was; a refused writer works again once the clock is right; tests through a volume pin the exact edges (1979-12-31 23:59:58 refuses, 1980-01-01 00:00:00 and 2107-12-31 23:59:58 pass, 2108-01-01 refuses); `Format` and `VolumeLayout` refuse the same way |
| oracle tests | on FAT12, FAT16 and FAT32 volumes rust-fatfs formatted, rust-fatfs reads the creation, write and access times back as the clock gave them, the odd second included; `fsck.fat -n` accepts every volume, the first and last moments FAT stores included; a clock with no time equals `NoClock`; the format's label entry and the EFI volume's directories carry the time |
| fuzz | the writer target now feeds the volume a clock built from its input, including invalid times, which the crate refuses without a panic; 127,000 runs in 61 seconds, no crash |

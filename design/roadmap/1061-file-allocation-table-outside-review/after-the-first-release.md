# P2: after the first release

An appendix to [the milestone](../1061-file-allocation-table-outside-review.md). Every item here is
two-way and the maintainers' provisional call. A name an item mints is provisional.

## P2-1. Lowercase flags are not read

`Slot::parse` in `src/directory.rs` never reads byte 12 (`DIR_NTRes`), whose bits 3 and 4 say the
base and the extension are lowercase. Linux's `vfat` driver writes `readme.txt` as the short entry
`README  TXT` with those bits and no long name, and this crate shows it as `README.TXT`. Lookup
ignores case, so paths still resolve; only display differs. Change: an additive method on
`ShortEntry` and `NamedEntry` that yields the cased short name. S.

## P2-2. Every long name gets a numbered alias, even when the basis fits

The scratch program stored `readme.txt` as a long-name run plus the alias `README~1TXT`, and `a.b`
as `A~1     B  `. Windows, Linux and rust-fatfs write the plain basis (`README  TXT`) when it is
exact, fits 8.3 and collides with nothing, and number it only otherwise; the specification permits
either. A DOS or firmware listing then shows `README~1.TXT`. Change: when the basis is lossless,
fits and is free in the directory, use it with no tail, and number it otherwise. It changes output
bytes and the pinned digests, and every reader accepts both. M.

## P2-3. An opt-in lenient open for FAT32 volumes under 65,525 clusters

`BootSector::parse` refuses them with `ParseError::LayoutContradictsClusterCount`, as the README's
`BUGS` says. `mkfs.fat -F 32` writes them with a warning, and Linux reads them by layout. An additive
`Volume::open_with(options)` whose default stays strict keeps the present guarantee. rust-fatfs has
an unreleased `strict` option of the same kind. M.

## P2-4. The weekly fuzz job starts from no seeds

`fuzz/.gitignore` ignores `corpus/`, the tree commits no seed, and the `fuzz` caller has never run.
Change: commit a corpus per target (the six boot fixtures and a small tree image) under
`fuzz/corpus/<target>/`, where cargo-fuzz looks by default, and confirm that the template's job
reads that directory. S.

## P2-5. Interoperability with Linux tools beyond `fsck`

The suite compares against rust-fatfs and `fsck.fat`, works on `mkfs.fat` volumes, and reads two
macOS images; the only third-party driver test runs on macOS alone and is ignored. `mtools` (`mdir`,
`mcopy`, `mattrib`) runs unprivileged and reads and writes images. Change: a test that uses it when
present, the way the `fsck` check does. S to M.

## P2-6. The memory footprint is not stated

`Volume`, `Directory` and `FileReader` each embed a 4,096-byte sector buffer, and the write paths
in `src/file_writer.rs` and `src/directory_writer.rs` put 4 KiB arrays on the stack per call. A
bare-metal caller with a small stack needs the number. Change: a README paragraph with the sizes. A
const-generic sector size that shrinks them would be a one-way door, and this block does not
recommend it now. S.

## P2-7. `slot_location` walks a directory's chain for every slot

`slot_location` in `src/file_writer.rs` walks from the directory's first cluster for every slot, as
the README's `BUGS` records. Remembering the last located cluster and its chain index removes the
repeated walk in `write_run` and `ensure_slots`. Internal. S.

## P2-8. Cargo metadata for discovery

`keywords` holds `no-std`, `fat32`, `filesystem` and `kani`, and nobody searches crates.io for a
model checker. Change: `["fat", "fat32", "vfat", "no-std", "filesystem"]`, within the five-keyword
cap, and `embedded` among the `categories`. S.

## P2-9. No `examples/` directory

The README example is a doctest. A host program that opens an image file through `std::fs::File`
as a `SectorDevice`, lists a tree and formats a file is what a stranger copies first, and docs.rs
links it. S.

## P2-10. An optional `embedded-io` feature

`Read`, `Write` and `Seek` for `FileReader` and `FileWriter`, behind a feature that is off by
default, as embedded-sdmmc offers. Additive. M.

## P2-11. A comparison paragraph in the README

Name rust-fatfs, embedded-sdmmc and hadris-fat, and what each has that this crate does not, from
[the prior art appendix](prior-art.md). S.

## P2-12. `.github/mutants-exempts` contradicts itself

Its header says no mutant is missed and no line is expected below, and eleven exemption lines
follow. The gate reads the file, so its text states the present rule only. S.

## P2-13. Text a stranger run left behind

- The `PartitionDevice` doctest calls `open` on a hidden `Empty`.
- The `Volume` doctest asserts that `chain(5)` is `[5, 6]` and does not say why 5.
- The `remove_*` summaries read awkwardly ("Remove the empty directory `entry` names").
- The tests print a line only when `fsck.fat` or `mkfs.fat` is missing, never when they ran.

S.

# P2: later

An appendix to [the milestone](../1107-sealed-pair-outside-review.md).

## P2: later

11. The table check hashes an entry the kernel and the progenitor never see, and skips one they
    never read.
    Finding: `inspect_table` walks `fs.entries()` and reads each by name with `fs.read(name)`,
    which returns the first entry of that name. `nifefs::write_image` does not refuse duplicate
    names, so an archive with two `console` entries has its first hashed twice and its second never.
    The kernel and the progenitor also read the first, so the verdict is right and the second entry
    is dead bytes nobody reports. An entry whose name fails UTF-8 gets skipped with `continue`. No
    reader can name it, so it too is dead bytes, reported by nothing. And the
    `unwrap_or_default()` on line 339 hashes an empty slice in a case that cannot occur, which
    hides the impossible case as a healthy digest.
    Evidence: `crates/sealed_pair/src/lib.rs` lines 334 to 348; `crates/nifefs/src/lib.rs` lines
    221 to 230 and 254 to 297.
    Change: hash the entry's own bytes (`nifefs` could expose a `read_entry(&Entry)`). Report a
    duplicate name and a name that is not UTF-8 as a fourth disagreement ("dead entry"). State in
    `BUGS` that duplicates are not refused by the writer. Or state the three facts in `BUGS`
    and change nothing.
    Door: two-way. Effort: S.

12. In the stick path the haystack contains the archive.
    Finding: `card_check` passes the whole boot file as the kernel and the archive found inside it
    as the archive, so `contains` searches the archive too. Today no archive entry carries another
    entry's digest as raw bytes (the table and the loader's words are hex text), so the search
    cannot find a digest the kernel lacks. A future entry that embeds raw digests (a binary catalog
    or index) would make a stick read as sealed with no digest in the kernel. The crate's
    documentation says searching the whole file is sound and does not state the condition.
    Evidence: `crates/sealed_pair/src/lib.rs` lines 362 to 377; `xtask/src/card_check.rs` lines
    166 to 190.
    Change: have `embedded_archive` return the archive's span (its start and its length, which the
    directory gives) so the caller can search the bytes outside it. Or state the condition in `BUGS`
    and add a test that an archive carrying a raw digest of its own entry is not vouched for by a
    kernel that lacks it.
    Door: two-way. Effort: S.

13. `contains` is a byte-at-a-time window scan over a release kernel.
    Finding: `haystack.windows(32).any(|w| w == needle)` compares at every offset. A release
    kernel is several megabytes and the scan runs once per measured entry, so the cost is small
    today and unmeasured. The standard library's `memmem`-style searchers and the `memchr` crate's
    `memmem::find` are the usual answer; the crate has no third-party dependency and the tree's
    rule on new dependencies applies.
    Evidence: `crates/sealed_pair/src/lib.rs` lines 385 to 390.
    Change: measure first on a real `nife-vf2.img`; if it shows, a first-byte `memchr`-style skip
    written here (ten lines, no dependency) or `memchr` behind the dependency rule.
    Door: two-way. Effort: XS to measure, S to change.

14. No proof, in a crate whose two neighbors each carry two.
    Finding: `nifefs` and `measured_boot` each have two Kani harnesses with falsification patches;
    this crate has none. Its logic is small and pure. It has `contains` (a window exists if and only
    if the needle is at some offset) and `embedded_archive` (the result starts with `MAGIC` and
    parses with a `progenitor`). It also has the table comparison: every `Changed` names an entry
    whose bytes hash to something other than the table's digest. Tests and the census cover the behavior;
    nothing covers it for all inputs.
    Evidence: `crates/sealed_pair/src/lib.rs` lines 369 to 377 and 388 to 390; the packet's
    "Kani harnesses" section.
    Change: one harness for `contains` against a bounded haystack (a `const` bound of 64 bytes)
    with its falsification patch, under the template's `proofs` job. The table comparison is a
    second, if the first is cheap enough.
    Door: two-way. Effort: S.

15. `hex_string` duplicates a function one level down, and `measured_entries` has one reader.
    Finding: `hex_string` wraps `measured_boot::hex` into a `String`; a `std` feature on
    `measured_boot` could offer it. `measured_entries` is public and read only inside this crate
    and its tests; `xtask::measure` reads `BOOT_PROGRAMS`. A public item with no outside reader is
    a promise nobody asked for.
    Evidence: `crates/sealed_pair/src/lib.rs` lines 37 to 41 and 380 to 383; `xtask/src/measure.rs`
    line 25.
    Change: keep both until the move and decide then: `measured_entries` private unless a caller
    appears, `hex_string` moved or kept with a sentence saying why it is here.
    Door: two-way until a release. Effort: XS.

16. A note in this tree describes a list the crate no longer has.
    Finding: `notes/boot-stick.md` says the loader's build hashes "the archive's `progenitor`, the
    six ELF child builders and `program_measurements`". `BOOT_PROGRAMS` holds `progenitor` alone,
    and the table measures the builders; `measured_entries` is two names. The crate's
    own documentation is right.
    Evidence: `notes/boot-stick.md` lines 150 to 153; `crates/sealed_pair/src/lib.rs` lines 22 to
    28.
    Change: the note states the two names and points at `measured_entries`, as milestone 971 (the boot stick notes are read against their code)
    intended.
    Door: two-way. Effort: XS.

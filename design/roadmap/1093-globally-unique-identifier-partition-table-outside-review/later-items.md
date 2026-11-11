# P2: later

An appendix to [the milestone](../1093-globally-unique-identifier-partition-table-outside-review.md).

## P2: later

13. `check_backup` demands a layout the specification does not.
    Finding: the backup's `entry_array_lba` must equal `alternate_lba - entry_array_blocks()`
    exactly. UEFI 2.10 section 5.3.1 says "The backup GPT Partition Entry Array must be located
    after the Last Usable LBA and end before the backup GPT Header", which allows slack. Linux reads
    the backup array from the backup header's own field; FreeBSD too. Whether any tool writes slack
    no source confirms it; `sgdisk` and `diskutil` do not (the fixtures).
    Evidence: `src/table.rs` line 245 to `src/table.rs` line 247; `src/error.rs` line 92 and `:93`.
    Change: accept `last_usable_lba < entry_array_lba` and `entry_array_lba + array_blocks <=
    alternate_lba`, keep `backup_entry_lba()` as the convention the writer uses, and reword
    `BackupMismatch::EntryArrayLba`. Or keep the strictness and say in the documentation that it is
    stricter than the specification and why.
    Door: two-way. Effort: S.

14. The proofs stop short of the hostile-input surface.
    Finding: eight harnesses prove the codecs, the CRC's structure at eight bytes, the overlap rule
    and the writer against the reader's rules. Five properties are not proved. First, that
    `Header::decode` is total on any block (the package's open question 1; the CRC chain is what
    makes it costly). Second, that `mbr::validate` is total on any 512 bytes (no CRC, so cheap).
    Third, that `Span::covering` contains what the caller asks for on all `u64`s (pure arithmetic; the
    test `a_span_always_contains_what_was_asked_for` already states the property over a grid).
    Fourth, that `Entry::name_utf8` is total over 36 symbolic code units. Fifth, item 2's `blocks`.
    The fuzz target is the only machine check of `parse` on
    hostile bytes and it runs in nife's workspace, not the crate's.
    Evidence: `src/verification.rs`; `src/span.rs` line 129 to `src/span.rs` line 148.
    Change: add four cheap harnesses (`mbr::validate`, `Span::covering`, `name_utf8`, `blocks`),
    each with a patch. For `Header::decode`, try the two-stage form the header round trip already
    uses rather than the whole function. That form proves the checks before the CRC total on a
    symbolic block, and the CRC's piece-wise form equal to the whole on short inputs. Then move the fuzz target under
    milestone 1043 (A moved crate is fuzzed in its own repository) and opt the repository in (no `unsafe`, so eligible).
    Door: two-way. Effort: M.

15. `Debug` on a table prints the entry array.
    Finding: the derived `Debug` prints the 16 KiB `entries` slice: 49,513 characters for the
    sample disk (measured). A stranger's first `{:?}` floods a console or a kernel log.
    Evidence: `src/table.rs` line 12 to `src/table.rs` line 19.
    Change: a hand-written `Debug` with the header, the block size and the count of used entries.
    Door: two-way. Effort: XS.

16. Two casts narrow on a 32-bit host without saying so.
    Finding: `Span::covering` returns `offset as usize` (`src/span.rs` line 61), which truncates when
    `transfer` exceeds `usize::MAX` on a 32-bit host, while `buffer_bytes` three lines down guards
    the same case; `create` computes `(entry_array.len() / entry::SIZE) as u32` (`src/table.rs` line 158),
    which truncates for an array over 512 GiB. Neither is reachable in practice, and the README's
    claim that every arithmetic edge has a check carries these two asterisks.
    Change: `usize::try_from(offset).ok()?` and `u32::try_from(...)` with
    `Error::EntryArrayShape`.
    Door: two-way. Effort: XS.

17. The crate checks the header's reserved tail but not the entry's, and not the attribute bits.
    Finding: `Header::decode` refuses a non-zero byte after the header (UEFI: "The rest of the
    block is reserved by UEFI and must be zero"), the strictest check in the crate. `Entry::decode`
    reads the first 128 bytes of a larger entry and ignores the rest, which the specification also
    says "must be zero"; attribute bits 3 to 47 "must be zero" and are not checked. The asymmetry is
    a choice the documentation does not state.
    Evidence: `src/header.rs` line 108 to `:110`; `src/entry.rs` line 12 to `:15`; `src/entry.rs` line 60 to `:62`.
    Change: one paragraph in the crate root saying which "must be zero" rules the reader enforces
    and which it tolerates, and why (the header's tail protects the CRC's meaning; an entry's tail
    and its reserved bits protect nothing this crate reads). Or enforce both and record the first
    real disk that trips them.
    Door: two-way. Effort: XS.

18. A header that declares more than 92 bytes re-emits as 92.
    Finding: `Header` drops `header_size`; `encode_into` always writes 92. A parsed table from a
    disk with a 96-byte header passes `parse`, and `write_primary_header` then writes a header the
    other tool would read as changed. The specification permits the larger size, and no tool
    writes one that the review knows of (unverified).
    Evidence: `src/header.rs` line 22 to `:25`; `src/header.rs` line 145 to `:158`.
    Change: say so in `encode_into`'s documentation (a rewrite normalizes the header to 92 bytes),
    which is what a tool that does not know the larger layout should do anyway.
    Door: two-way. Effort: XS.

19. A caller must add `uuid` itself to name the type the crate returns.
    Finding: `guid::Uuid` is re-exported, so a caller can write `guid::Uuid`, but
    `Uuid::from_fields`, `Builder` and the parsers need the caller's own `uuid`
    dependency at a matching major; nife's `uefi_loader/Cargo.toml` carries that second line.
    `gpt_disk_types` re-exports `uguid` and `crc` whole.
    Evidence: `src/guid.rs` line 10; `uefi_loader/Cargo.toml` line 106.
    Change: `pub use uuid;` at the crate root, and the README example writes `use globally_unique_identifier_partition_table::uuid::Uuid`.
    Door: two-way. Effort: XS.

20. The MBR validator calls a dead record a hybrid.
    Finding: `validate` refuses any record beside the protective one whose `OSType` is non-zero.
    UEFI 2.10 section 5.2.1: "A Partition Record that contains an OSType value of zero or a
    SizeInLBA value of zero may be ignored." A record with a type and a zero size is noise the
    specification lets a reader skip; here it is `MbrProblem::ExtraRecord`. No source says whether any tool leaves
    one.
    Evidence: `src/mbr.rs` line 70 to `:72`.
    Change: skip a record whose `SizeInLBA` is zero before judging it extra, with a test.
    Door: two-way. Effort: XS.

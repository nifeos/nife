# P2: later

An appendix to [the milestone](../1082-capability-manifest-note-outside-review.md).

## P2: after the move

10. Callers cannot display or box `Error`.
    Finding: `Error` derives `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq` and nothing else. An
    outside caller cannot `?` it into `Box<dyn core::error::Error>` or print it to a person
    without matching every variant.
    Evidence: `src/lib.rs` lines 178 to 191.
    Change: `impl core::fmt::Display` with one sentence per variant and the offset for `BadField`,
    and `impl core::error::Error` (stable in `core` since 1.81, inside the crate's rust-version if
    item 1 sets it there or later). Additive.
    Door: two-way. Effort: XS.

11. `Zones` is missing the std pair of `is_empty`, and the two views disagree on lifetimes.
    Finding: `Zones` has `iter`, `is_empty` and `has_root` and no `len`; `Verbs` has `len` and
    `is_empty`. The shapes rule asks for std's spelling and meaning, where the two come as a pair,
    and a reader who finds one expects the other. `Zones::iter`
    returns `impl Iterator<Item = &'a [u8]> + use<'a>` (the items outlive the view), and
    `Verbs::iter` returns `+ '_` (the iterator borrows the view), so a caller who collects one
    cannot write the other the same way. Neither implements `IntoIterator`.
    Evidence: `src/lib.rs` lines 791 to 818 and 1239 to 1285.
    Change: `Zones::len` (a count of the length-prefixed records; O(n) and documented so), the
    same `use<'a>` form on `Verbs::iter`, and `IntoIterator for &Zones` and `&Verbs`.
    Door: two-way (additive). Effort: XS.

12. The proofs cover version 1 and the first byte of version 2.
    Finding: `decode_never_panics` runs to `DESCRIPTOR_LEN + 1` bytes, which is the zone count
    byte and nothing after; `a_decoded_manifest_encodes_back_to_its_bytes` is version 1 only. The
    zone walk (`check_zones`, `check_serviced`, `zones`, `service`, `zones_bytes`),
    `decode_verbs`, `decode_subtree_grants` and the encoding direction (item 3) have no harness.
    The fuzz target's documentation says so, candidly; the README should too.
    Evidence: `src/verification.rs` lines 20 to 36; `fuzz/fuzz_targets/capability_manifest_note_decode.rs`
    lines 11 to 16.
    Change: three bounded harnesses, each with its falsification patch. `decode` is total at
    `DESCRIPTOR_LEN + 1 + 1 + 3 + 1` bytes (a version 3 descriptor with one three-byte zone, which
    exercises both walks, the service byte and the exact-length check). `decode_verbs` is total at `verbs_len(2)` bytes and `grants_for` finds each decoded word. `decode_subtree_grants` is total at 9 bytes. The unwind bounds are small because the lengths are. Add the encoding harness
    of item 3. `memchr` under `position` dominates Kani's time on the existing harness; a
    plain `while` loop in `zero` would cut it, with no change in meaning.
    Door: two-way. Effort: M.

13. The code writes the note header four times and pins the owner's length by hand.
    Finding: `Note::of`, `ZonedNote::of`, `ServicedNote::of` and `SubtreeGrantsNote::of` each
    write `namesz`, `descsz`, the type and `[OWNER[0], OWNER[1], OWNER[2], OWNER[3]]`. The
    compile-time guard is `OWNER.len() < NAME_FIELD`, so an owner of five bytes would build with
    its last byte dropped and `namesz` saying 6.
    Evidence: `src/lib.rs` lines 577 to 597, 839 to 860, 953 to 974, 1087 to 1105, 978.
    Change: one `const fn header(out: &mut [u8], descriptor_bytes: usize, kind: u32)` that writes
    the three words and copies `OWNER` in a loop, and a guard `OWNER.len() + 1 <= NAME_FIELD`
    that is the real condition. The mutation triage's six equivalent `+`/`-` mutants in these
    loops become one.
    Door: two-way. Effort: XS.

14. The dev-dependency on `interfaces_note` buys one equality.
    Finding: `interfaces_note` repeats `OWNER` so that it can depend on nothing, and this crate's
    test `the_interfaces_note_shares_the_manifest_notes_header` holds the two equal. After both
    move, that is a cross-repository dev-dependency pinned by `rev` for one assertion.
    Evidence: `Cargo.toml` lines 18 to 20; `src/tests.rs` lines 579 to 601.
    Change: keep it (the dependency is the organization's own and the test is the only thing
    that would notice drift), but cite it with a `version`, and say in the README that the two
    crates share one note header. Dropping it is also fine once the README states the header
    bytes, since the ELF format fixes them.
    Door: two-way. Effort: XS.

15. The `Program`-sized tests and the `fuzz_seed` writer use `std` from inside a `no_std` crate.
    Finding: `extern crate std` appears inside three test functions and the `verb_tests` module
    rather than once at the test root; `tests/fuzz_seed.rs` uses `std` freely, which is fine for an
    integration test. The pattern works and is only untidy.
    Evidence: `src/tests.rs` lines 276, 333; `src/lib.rs` line 1340.
    Change: one `extern crate std;` under `#[cfg(test)]` at the crate root.
    Door: two-way. Effort: XS.

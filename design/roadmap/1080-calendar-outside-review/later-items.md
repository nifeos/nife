# P2: later

An appendix to [the milestone](../1080-calendar-outside-review.md).

## P2: later

10. `-00:00` loses its meaning on the way through.
    Finding: RFC 3339 section 4.3 gives `-00:00` the meaning "the local offset is unknown", as
    distinct from `Z` and `+00:00`. The parser folds it to offset zero (`src/lib.rs` line 795 to
    `:796`) and `Format::Rfc3339` prints it back as `Z`. Every library in the table except
    possibly `jiff` does the same (unverified), and a type that carried the distinction would be
    a bigger `UtcOffset` for a case no caller here has.
    Evidence: the parser lines; the probe run.
    Change: one sentence in `parse_rfc3339`'s doc and the README's parsing section stating the
    fold. Not a type change.
    Door: two-way as a doc; the behavior is one-way after the tag and stays as it is.
    Effort: S.

11. The crate claims no `unsafe` but does not forbid it, and the parser has no fuzz target.
    Finding: the sources contain no `unsafe`, and no `#![forbid(unsafe_code)]` says so at the
    root, so a later edit could add one silently. The template's weekly fuzz job is opt-in for crates without `unsafe` (ruled). The parser over hostile bytes is the natural target. The harness proves totality to 26 bytes (item 9 to 28), and a fuzzer covers the long fractions beyond any harness. `notes/fuzzing` has no target for this crate and the dry run stages none.
    Evidence: `src/lib.rs` line 13; the dry run's `fuzz` note.
    Change: `#![forbid(unsafe_code)]`; a `fuzz/` workspace with two targets, `parse_rfc3339_bytes`
    on raw bytes and a round trip (`from_unix` of a bounded `i64` and a bounded offset, format,
    parse back, compare); `public-crash-inputs: true` in the repository's `fuzz.yml`, which the
    safe-Rust ruling permits.
    Door: two-way. Effort: S.

12. Test coverage of `Weekday::name` and the `Human` format's weekday is one date each.
    Finding: `tests::weekdays` pins seven dates and one `name()`; the `Human` format's weekday prefix has a check at one instant. The Kani harness proves the cycle, so a wrong table entry is the remaining risk, and only a table can check a table.
    Evidence: `src/tests.rs` line 181 to `:205`, `:230` to `:240`.
    Change: one test iterating all seven variants through `abbrev`, `name` and `iso_number`,
    asserting the three agree (`name` starts with `abbrev`, `iso_number` steps from Monday).
    Door: two-way. Effort: S.

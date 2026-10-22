//! **Reading captures taken before milestone 297 renamed the console markers**, for the tests that
//! replay them.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

/// **Fixtures captured before milestone 297 renamed the console markers**, made readable by the
/// recogniser that reads the current ones.
///
/// `script/soak` became `script/soak-test` on 2026-09-14 (calef's ruling), and the markers moved
/// with it: `soak:` to `soak-test:`, `soak-census:` to `soak-test-census:`. Two files in
/// `tests/fixtures/captured/` are QEMU runs taken before that, and **they are left exactly as the
/// machine printed them**, because a capture is a record and this tree has already carried one
/// rewritten transcript for twelve days (`design/naming/rename-what-moves.md`, "a quotation never
/// moves").
///
/// So the transposition happens here, at test time, on a copy, and it is deliberately the narrowest
/// edit that makes the file parse: the marker prefix and nothing else. Every number, every field
/// and every other line is the machine's own.
///
/// **Why the recogniser itself does not simply match both spellings.** It did for `init/build`
/// (milestone 295) and had to: that marker survives only in a VisionFive 2 capture off real
/// silicon, which cannot be re-taken, so the live parser is the only thing that can ever read it.
/// These two are QEMU runs. What makes them unrepeatable is time rather than hardware, which is a
/// weaker claim, and it is a claim about *these files* rather than about anything a board will
/// print tomorrow. A permanent second spelling in the parser would be paying forever for a fact
/// that belongs to two files, and `captured/qemu-2026-09-14-riscv64-soak-test.log` is a real run in
/// the current vocabulary, so nothing here rests on hand-written text either.
///
/// Name: provisional (milestone 297 (`soak` becomes `soak-test`)): calef names public items, and
/// this one is `pub(crate)` and test-only, so it is the cheap end of that rule rather than an
/// exception to it.
#[cfg(test)]
pub(crate) fn respell_pre_297_markers(capture: &str) -> String {
    capture
        .replace("soak-census:", "soak-test-census:")
        .replace("soak: ", "soak-test: ")
}

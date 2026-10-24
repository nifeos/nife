//! **Reading captures taken before milestone 297 renamed the console markers**, for the tests that
//! replay them.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

/// **Fixtures captured before milestone 297 renamed the console markers**, made readable by the
/// recogniser that reads the current ones.
///
/// Two files in `tests/fixtures/captured/` are QEMU runs from before `soak:` became `soak-test:`
/// and `soak-census:` became `soak-test-census:`. They stay as the machine printed them (a capture
/// is a record: `design/naming/rename-what-moves.md`, "a quotation never moves"), so the prefix is
/// respelled here, at test time, on a copy, and nothing else is touched. The live parser reads
/// only the current spelling: these are QEMU runs, and
/// `captured/qemu-2026-09-14-riscv64-soak-test.log` is a real run in today's vocabulary.
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

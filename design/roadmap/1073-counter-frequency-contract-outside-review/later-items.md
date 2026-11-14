# P2: later

An appendix to [the milestone](../1073-counter-frequency-contract-outside-review.md).

10. The page crates spell their offset constants `<FIELD>_OFFSET_BYTES`.
    Rule in force (ruled 2026-11-14 UTC, for the whole family): the position of a page field or
    region-layout field is named `<FIELD>_OFFSET_BYTES`, with the field name spelled out under
    `design/naming.md` (`MAGIC_OFFSET_BYTES`, `HERTZ_OFFSET_BYTES`,
    `TIME_ZONE_LENGTH_OFFSET_BYTES`). The suffix names the unit, so a constant that counts words
    says `_OFFSET_WORDS`. No old spelling is kept as an alias.
    Evidence: `crates/counter_frequency_contract/src/lib.rs`.
    Door: two-way (private).

11. The reader test in nife compares the user-space number with the kernel's and stays in nife.
    Finding: `system_tests/src/user/counter_frequency_tests.rs` drives the whole path through QEMU.
    It points from nife to the crate, which the move permits, and the crate's own tests already
    drive both halves of the page without a kernel. Nothing to move; the README's "Building and
    testing" section should say that the end-to-end test lives with the kernel that maps the page.
    Evidence: `system_tests/src/user/counter_frequency_tests.rs` lines 1 to 40.
    Change: one sentence in the README.
    Door: two-way. Effort: XS.

12. The pointer-bearing type derives `Debug`, which prints the address.
    Finding: harmless, and the address is a compile-time constant in every real use. A stranger
    may prefer a `Debug` that prints the rate or "unknown". Not worth a release on its own.
    Evidence: `crates/counter_frequency_contract/src/lib.rs` lines 150 to 153.
    Change: none now; fold into the next API change if one comes.
    Door: two-way. Effort: XS.

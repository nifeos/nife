# P2: later

An appendix to [the milestone](../1073-counter-frequency-contract-outside-review.md).

10. The private offset constants abbreviate.
    Finding: `OFF_MAGIC` and `OFF_HERTZ` shorten "offset". The word is not on the refused list
    and the sibling page crates use the same prefix, so this is a family question rather than a
    defect; the template's abbreviations check is report-only.
    Evidence: `crates/counter_frequency_contract/src/lib.rs` lines 88 to 89.
    Change: `MAGIC_OFFSET` and `HERTZ_OFFSET`, when the family decides, in every page crate at
    once.
    Door: two-way (private). Effort: XS.

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

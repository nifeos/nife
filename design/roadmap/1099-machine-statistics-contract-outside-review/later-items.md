# P2: later

An appendix to [the milestone](../1099-machine-statistics-contract-outside-review.md).

## P2: later

12. The dry run and the standalone check disagree on what the crate carries.
    Finding: the dry run's build fails on the inherited `license` and never reaches the path
    dependencies, so its report lists eight findings where the standalone check lists fourteen. A move lane reading only the dry run would miss the two dependencies. The dry run's `citations`
    step also reports the scaffold's lines ahead of the crate's own. Both are findings against
    `helpers/crate_move.py`, for a lane of its own; the open fix for the diff output (pull
    request 3521) has nothing to do with this.
    Evidence: the dry run's `build` line; `script/crate-standalone` output.
    Change: in the dry run, inline the inherited keys before the build the way the standalone
    check does, so the build reports the real blocker.
    Door: two-way. Effort: S, in nife's tooling.

13. The end-to-end test stays in nife.
    Finding: `system_tests/src/user/machine_statistics_tests.rs` drives the kernel's writer and
    this crate's reader under QEMU. It points from nife to the crate, which the move permits. The
    crate's own tests will drive both halves on the host after item 8.
    Evidence: `system_tests/src/user/machine_statistics_tests.rs` lines 95 to 147.
    Change: one sentence in the README's "Building and testing" section saying the end-to-end
    test lives with the kernel that maps the page.
    Door: two-way. Effort: S.

14. `no_std` is conditional on `cfg(test)`.
    Finding: `#![cfg_attr(not(test), no_std)]`. The tests use nothing from `std` that the review
    could see, so `#![no_std]` with the test harness may compile unchanged (unverified).
    Evidence: `crates/machine_statistics_contract/src/lib.rs` line 23.
    Change: try the unconditional attribute; keep the conditional one if the harness needs it.
    Door: two-way. Effort: S.

15. `Snapshot` is a 416-byte `Copy` type.
    Finding: `[Cpu; 8]` at 48 bytes a line plus four header fields. Every caller in the tree passes
    it by reference after one copy, so this costs nothing today; a stranger may still wonder
    why the type derives `Copy` at this size.
    Evidence: `crates/machine_statistics_contract/src/lib.rs` lines 166 to 178.
    Change: none now; note the size in the type's documentation.
    Door: two-way. Effort: S.

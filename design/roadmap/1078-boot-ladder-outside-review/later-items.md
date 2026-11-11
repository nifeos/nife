# P2: later

An appendix to [the milestone](../1078-boot-ladder-outside-review.md).

## P2: after the first tag

10. Two printed check names are abbreviations, and one ruling is outstanding.
    Finding: `blockdev` and `trng` (`src/lib.rs` line 95 and 98) appear in the verdict and in `serial_console`'s board profiles (`must_not_be_absent`). `device_interrupt` (line 96) awaits a ruling per its own comment, and older logs print `deviceirq`. A profile that forbids
    `device_interrupt` absent reads such a log's `deviceirq` absence as nothing. The kernel pads
    the name column to 10 characters (`kernel/src/self_test.rs` line 127, 149, 163) and
    `device_interrupt` is 16, so that column no longer lines up (cosmetic, kernel-side).
    Why it matters: the names are a contract that logs and profiles hold, so they are one-way the moment the crate ships.
    Change: decision 3 below. Effort: S for the rename lane.

11. The contract has no version a reader can check, and the substitute covers lengths only.
    Finding: `SELF_TEST_CHECKS_EARLIER` lets a reader accept an older kernel's total; nothing lets
    it map an older check name, and no constant says which revision of the ladder a kernel speaks.
    Milestone 1017 (the contract crates leave in this order) gives wire contracts an
    `INTERFACE_ID` and a `VERSION`; a console line cannot carry a checkable version without a
    tenth rung.
    Why it matters: after the move, a wording change is a cross-repository change, and a stranger
    needs the policy written down.
    Change: a README section stating the policy: a change to a head or a check name bumps the Cargo minor and adds a compatibility entry (the earlier-lengths list, and a renamed-checks list if a ruling ever renames a check). The Cargo version is the contract's version. No identity constant exists to add. Door: two-way. Effort: S.

12. Growth already proposed should land before the move, or the move should take it on as cross-repository work.
    Finding: milestone 972 proposes `PROGENITOR`, `LOADER` and `REMOVABLE_MEDIA` for this crate;
    `notes/bugs-without-milestones/drivers-boot-and-install-1.md` proposes hoisting the soak
    markers of `crates/serial_console/src/stop.rs` line 51 to 60 and the job-mix field names of
    `progress.rs` into it. Each is the same defect this crate exists to remove.
    Why it matters: each addition after the move is a bump pull request in nife and a release
    here; before the move each is one pull request.
    Change: land milestone 972 and the two hoists first, or record in this block that they wait.
    Door: two-way. Effort: S each, in nife.

13. The banner's tail is not the architecture name on one port.
    Finding: the doc says `nife on <architecture> (...)` (`src/lib.rs` line 29); aarch64 and `x86_64`
    print their `arch::NAME` form but riscv64 prints `RISC-V (rv64, S-mode, Sv39)`
    (`kernel/src/boot.rs` line 213). `exposure.rs` reads the architecture from the machine line
    instead, so nothing breaks today.
    Change: say in the doc that the tail is free text and a reader takes the architecture from
    `MACHINE`'s tail, or have riscv64 print `arch::NAME` like the others. Door: two-way (the doc declares the tail free). Effort: S.

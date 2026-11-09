# P2: after the first release

An appendix to [the milestone](../1050-designware-mobile-storage-outside-review.md).

**P2-1. No fuzz target.** There is no `fuzz/` directory, and `fuzz.yml` opts out
(`public-crash-inputs: false`). The crate has no `unsafe` (verified by search), so it qualifies for
the public weekly fuzz. Candidates: `MasterBootRecord::parse`,
`CardSpecificData::from_sd_register`, `CardIdentification::from_sd_register`, `Slot::from_node`,
and a byte-driven `RegisterWindowAndClock` that runs `sd::identify` and one `read_blocks`. Long
random runs complement the bounded Kani totality harnesses. Two-way. M.

**P2-2. The simulator is not available to users.** The crate doc in `src/lib.rs` tells readers that
every sequence is tested against `simulator`, which is `#[cfg(test)]`. A kernel that integrates
this crate cannot run its own tests against the same fake. Recommended: a `simulator` feature that
exports it, as `embedded-hal-mock` does for `embedded-hal`. Two-way (additive). M.

**P2-3. Card presence is read once.** `HostController::new` samples `Identity.card_detect`, and no
method reads `CDETECT` later. A removed card surfaces as `NoResponse`, which the BUGS section of the
README says. Recommended: an `is_card_present` method, provisional, and the BUGS line kept until hot
removal is tested on the board. Two-way. S. The board confirms the polarity of the detect line.

**P2-4. An eMMC runs 1-bit at 25 MHz.** The BUGS sections say so; `SWITCH` is public and unused.
Recommended, in order: `EXT_CSD` byte 183 to 4 or 8 bits, then high-speed timing. Two-way. M. Needs
the board with an eMMC module fitted.

**P2-5. `reset` leaves the registers of the IDMAC alone.** `HostController::reset` clears the DMA
enables in `CONTROL_REGISTER`. It never writes the software reset of `BUS_MODE_REGISTER` or clears
`IDSTS`, although firmware hands the controller over with the IDMAC selected. A test in
`src/internal_direct_memory_access.rs` models that handover. Linux resets the IDMAC at probe.
Recommended: the software reset and status clear in `reset`, behind the same deadline. Verified in
the source. Two-way. S. The board shows whether it matters.

**P2-6. Repository facts gone stale.** The comment beside `publish = false` in `Cargo.toml` explains
it by a path that exists in no repository a stranger can see. `CODEOWNERS` says the
`crate-maintainers` team may not exist, and it does (verified through the API). The GitHub
description is the boilerplate of the template, and the topics lack `sd`, `emmc` and `riscv`. The
manifest uses three of the five keywords crates.io allows. Recommended: fix each. Two-way. S.

**P2-7. User-facing text says "queue" for the FIFO.** `Error::QueueWidth` displays as "unsupported
queue width field". The identifier follows the ruling; the display string is prose and may say
FIFO once expanded. Two-way. S.

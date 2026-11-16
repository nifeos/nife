# P2: later

An appendix to [the milestone](../1106-program-argument-contract-outside-review.md).

## P2: for later

10. The reader truncates an address on a target narrower than 64 bits.
    Finding: `va as *const u8` turns a `u64` into a pointer through `usize`, so on a 32-bit
    target an address above 4 GiB reads a different page. No target the crate builds for breaks;
    the claim is wider than the test.
    Evidence: line 197.
    Change: `usize::try_from(virtual_address)` with a documented panic in the `# Safety` section,
    or one sentence that the address must fit the target's `usize`. The sibling
    `environment_contract` shares the shape and should move with it.
    Door: two-way. Effort: XS.

11. Nothing uses `ArgPage::EMPTY` and `parse` duplicates its literal.
    Finding: no caller in the tree names `EMPTY`; `parse` writes the same struct literal instead of
    `Self::EMPTY`. Either the constant earns a use or it goes.
    Evidence: lines 155 to 167; a tree-wide search for `ArgPage::EMPTY` finds only the definition.
    Change: `Self::check(page).unwrap_or(Self::EMPTY)`, and a test or documentation example that
    names `EMPTY`; or remove it before the first release, which is the last cheap moment.
    Door: two-way. Effort: XS.

12. The builder takes a fixed 4,096-byte array while the parser takes any slice.
    Finding: `PageBuilder::new(&mut [u8; PAGE_BYTES])` forces a 4 KiB array on every test and
    proof, and the round-trip proof over it ran out of memory. `parse(&[u8])` already bounds its
    room by the slice length, capped at the page's content length.
    Evidence: lines 109 and 162 to 175.
    Change: one of `new(page: &mut [u8]) -> Result<Self, Refused>` (refusing a buffer shorter than
    the header) or a const-generic length, with the capacity read from the buffer and capped as the
    parser caps it. The shell keeps passing its frame. Unverified: that Kani then fits the round
    trip under 12 GiB over a 64-byte buffer.
    Door: a public signature, two-way before the first release. Effort: S.

13. The layout arithmetic has a second copy in `grant_plan`.
    Finding: `grant_plan::check_words` sums `4 + w.len()` against `CAPACITY` to refuse a line
    before any page exists, repeating the record cost the builder knows. A change to the prefix
    width would have to find it.
    Evidence: `crates/grant_plan/src/parsing.rs` lines 70 to 80; line 120 here.
    Change: `pub const fn record_bytes(argument_bytes: usize) -> usize` (the four-byte prefix
    plus the bytes) and `PageBuilder::remaining_bytes()`, with `check_words` summing
    `record_bytes`. With it, add the pedantic lints a stranger runs. Put `#[must_use]` on `parse`,
    `iter`, `len`, `is_empty` and `count`. Add `impl IntoIterator for &ArgumentPage`. Add `Debug` on
    the three public types. Add `FusedIterator` on `Iter`. Add an `# Errors` section on `push`. Put
    `u32::try_from` or a `const` assertion `MAXIMUM_RECORDS_BYTES <= u32::MAX as usize` behind the two
    `as u32` casts at lines 125 and 140.
    Door: two-way. Effort: S.

14. The data bounds the record walk, and a proof reads that poorly.
    Finding: `for _ in 0..count` runs up to `count` iterations, where `count` is any `u32` from the
    page. It stops early only because each record consumes at least four bytes of at most
    4,080, so 1,021 iterations is the real bound. That is safe and finite. A model checker cannot
    see the bound without unwinding to it, which is part of why the round-trip proof is heavy.
    Evidence: lines 181 to 186.
    Change: walk `records` until empty, counting, and compare the count walked with `count`; the
    loop's bound then reads from the slice length, and the same `None` answers both disagreements.
    Door: two-way. Effort: XS.

15. The file must stay one file, and nothing says so.
    Finding: the `std` port copies `src/lib.rs` by name into `sys/pal/nife/argproto.rs` and strips
    crate-level inner attributes. A lane that splits the crate into modules, or adds a
    `#![forbid(unsafe_code)]`-style attribute the port needs, breaks the `std` build in nife, not in
    this repository. This repository's gates stay green.
    Evidence: `xtask/src/farm/copied.rs` lines 90 to 94; `xtask/src/farm/generate.rs` lines 61
    and 87.
    Change: one README `BUGS` line stating the constraint, and a test in the crate that `src/`
    holds exactly one file, so the constraint fails here first.
    Door: two-way. Effort: XS.

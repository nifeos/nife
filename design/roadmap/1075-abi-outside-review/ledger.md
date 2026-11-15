# The ledger

An appendix to [the milestone](../1075-abi-outside-review.md).

| item | state |
|---|---|
| 1 | done: the `VisionFive 2` span is in backticks; `cargo clippy --all-targets -- -D warnings` passes with no `clippy.toml` |
| 2 | left: decision A |
| 3 | done: `helpers/lint_externalized_crates.py` counts zero hits and the crate's `WAIVERS` entry is gone; the ring offsets left `src/virtio.rs` |
| 4 | done for the 28 references: each is restated as a fact about the boundary, the HTML link is gone and `address_space` cites `page_frame::MAP`; `RUSTDOCFLAGS="-D warnings" cargo doc` passes |
| 5 | done: `SETUP_QUEUE` documents `WrongObject` for all three causes |
| 6 | done: the crate root carries the six-word table for aarch64, riscv64 and `x86_64`, read off the three `trap6` bodies in `user_mode_runtime`; `CONFIGURE` says word 5 |
| 7 | (a) and (c) done; (b) left: decision B |
| 8 | done in the doc comments: no dated phrase remains outside a `Name:` block, and the four index-exception comments lost their dates |
| 9 | done: the README is unbolded, the methods bullet is a table and the six passives are rewritten, with the register sentence and the `cargo test` line. The badge row is the move's |
| 10 | done: the `publish` comment names no file of this repository |
| 11 | the counting test is done (`every_public_number_is_pinned`, falsified by deleting a pinned line); the version rule in the new repository is the move's |
| 12 | left: a rename of two public items, so the architect's |
| 13 | left: the package and the marks need the architect's rulings first |
| 14 | done: `Hash`, `Display` and `core::error::Error` for `Error` (`error_displays_and_is_an_error_and_hashes`) |
| 15, 16, 17 | left: each changes or deletes a public item, or moves one out of the crate |
| 18 | done: the five nits, including the unused `cfg(kani)` allowance |
| 19 | done: the five quotations are restated as the decision |
| 20, 21 | left: both need the new repository's first runs |


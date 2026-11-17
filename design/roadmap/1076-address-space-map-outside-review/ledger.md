# The ledger

An appendix to [the milestone](../1076-address-space-map-outside-review.md).

| item | state |
|---|---|
| 1 | done: the test is `the_user_linker_script_links_at_the_image_base` in `crates/user_mode_runtime/src/linker_script.rs`, beside the script it parses, and the `IMAGE` doc says where it lives |
| 2 | left, the move's: nife's own falsification sweep resolves each patch from the repository root, so crate-relative record lines and patch headers would stop it here. `helpers/crate_move.py` rewrites the records and regenerates every patch for `-p1` at the move. The patches were regenerated here over the new band functions and `script/falsifications --sweep address_space_layout` finds all eleven red |
| 3 | done: the manifest carries a literal `license`, `rust-version = "1.88"` (the lowest installed toolchain that builds the crate; 1.85 fails on `is_multiple_of`, and 1.86 and 1.87 were not tried, so the template's `msrv` job is the check), the template's three rust lints and the five clippy lints nife applies inline, a `description`, `repository`, `readme`, `keywords` and `categories`. `publish = false` stays, with a comment a stranger can act on |
| 4 | done: the root doc carries the band table, the placement order and the reasons; every path, section, milestone, pull request, person and `Was` sentence is gone from the docs. The `Name:` blocks stay until the move strips them |
| 5 | partly done: the crate has a `README.md` with the scaffold's `EXAMPLES` and `BUGS` sections, the bands, how to build, test and prove, and what is proven. Left, the move's: the license files, `CHANGELOG.md`, `_typos.toml`, the caller workflows, the `.github` files and the recorded stranger run |
| 6 | done: `va` is `virtual_address` at every site and `allowed` is `is_allowed` |
| 7 | waiting: decision 2 (the constants) |
| 8 | done: the crate is `address_space_layout` (decision N1) |
| 9 | done, provisional: `window(band, start_virtual_address, bytes)` checks the whole window; the three one-page functions are one-line wrappers over it; two harnesses and their falsification patches cover it; `process_builder`'s `SCRATCH_WINDOW` checks its 64 MiB with one call. The name `window` is provisional (decision 3) |
| 10 | done: the five derived-constant assertions share one block |
| 11 | left, the move's: the `.github/mutants-exempts` line |
| 12 | done: the layout-is-fixed entry is in the crate root's BUGS and the README |
| 13 | left, the template's: `register_names.py` reads `address` in a function name as an address |
| 14 | done: `BANDS` is `&[(&str, Band)]`; nothing in the tree read it by type |
| 15 | left: `Band::new` is a new public name, which an architect names |
| 16 | done: `#![forbid(unsafe_code)]` |
| 17 | left: it needs `LOW_HALF_SPLIT_SHIFTS` public, a new name, which an architect names |
| 18 | left, block 1017's: the `counter_frequency_contract` row is gone, but the row that says the `link.ld` test must move is stale, since it moved, and only the lane that owns block 1017 edits it |

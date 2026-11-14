# The ledger

An appendix to [the milestone](../1076-address-space-map-outside-review.md).

| item | state |
|---|---|
| 1 | done: the test moved to `xtask/src/link_script_base.rs` (`the_user_linker_script_links_at_the_image_base`), because `user_mode_runtime` has no host test target and no dependency on the crate, which the item assumed; the `IMAGE` doc says where the test lives |
| 2, 3, 5, 13 | left: the move's (the repository scaffold, crate-relative patch paths and the standalone manifest), or the template's |
| 4 | done: the root doc carries the band table, the placement order and the reasons; every path, section, milestone, pull request, person and `Was` sentence is gone from the docs. The `Name:` blocks stay until the move strips them |
| 6 | done: `va` is `virtual_address` at every site and `allowed` is `is_allowed` |
| 7, 8, 9 | left: decisions 2, 1 and 3 |
| 10 | done: the five derived-constant assertions share one block |
| 11 | left: the move's `.github/mutants-exempts` line |
| 12 | done: the layout-is-fixed entry is in the crate root's BUGS; the README is the move's |
| 14 | done: `BANDS` is `&[(&str, Band)]`; nothing in the tree read it by type |
| 15 | left: `Band::new` is a new public name |
| 16 | done: `#![forbid(unsafe_code)]` |
| 17 | left: it needs `LOW_HALF_SPLIT_SHIFTS` public, a new name |
| 18 | left: the edit is to block 1017, which this lane does not own |

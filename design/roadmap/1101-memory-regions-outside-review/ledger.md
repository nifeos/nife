# Ledger

An appendix to [the milestone](../1101-memory-regions-outside-review.md).

Earlier lanes built items 1, 3 and 4 before this one began; this lane checked each against
the code and the tests named.

| item | state | where |
|---|---|---|
| 1 split refused for a full table | DONE | `a_split_refused_for_a_full_table_leaves_the_parent_unchanged`; Kani `a_split_refused_for_a_full_table_moves_nothing` |
| 2 cannot stand alone | LEFT | README, licenses, `rust-version`, inline lints and the citations are the move's work, and Decision 1 says whether the crate leaves |
| 3 root at the top of the page space | DONE | `a_root_of_no_pages_or_past_the_last_page_number_is_refused`. LEFT: the Kani harness that drives any one operation from any accepted root |
| 4 second release | DONE | `a_released_object_gives_its_count_back_to_its_region_and_every_ancestor`; Kani `a_release_of_nothing_held_moves_nothing` |
| 5 loom runner after the move | LEFT | template and move work (`loom: true` input); waits on Decision 1 |
| 6 template gates | PARTIAL | (b) `is_same_snapshot`, `is_unmoved_since` and (c) the three acronyms spelled out count as done. LEFT: (a), (e) are the template's, (d) is `helpers/crate_move.py`'s and has no owner here yet |
| 7 documentation says what code does not | DONE | `NO_PARENT` is now `EntryName::NEVER_MINTED.to_bits()`; the table, `usage`, `levels_up` and crate-root docs now match the code; the crate-root doc gives the model |
| 8 manifest metadata | PARTIAL | `description`, `keywords`, `categories` set. LEFT: `documentation`, `readme` and `repository` wait for the README and the move; the dependency form waits for publish |
| 9 public names | LEFT | a naming pass, the architect's |
| 10 root ceiling under sixteen | DONE | documented in `insert_root_with_ceilings` and `for_child_of`; the share itself is Decision 4 |
| 11 cost of `spent` and the return | DONE | a sentence in each doc comment. LEFT: the per-parent page count that would avoid the scan |
| 12 `ObjectKind::ALL` | DONE | `ObjectKind::ALL` is public, `COUNT` is `ALL.len()`, the proofs' `KINDS` and `any_object_kind` read it |
| 13 `forbid(unsafe_code)` | DONE | crate root attribute |
| 14 table-level invariant harness | PARTIAL | Kani `a_return_keeps_every_run_inside_its_parents_spent_part` with its falsification patch. LEFT: recording the six subtree harness times in `falsification-times.tsv` |
| 15 direct pure-function tests | DONE | `a_retype_count_of_zero_means_one_page`, `a_retype_fits_only_inside_the_budget`, `the_watermark_falls_only_when_quiescent_and_never_rises`, and two asserts in `the_nearest_ancestor_at_its_ceiling_is_the_one_named` |
| 16 dropped claim | PARTIAL | the type's doc now says a dropped claim leaks. LEFT: the `Drop` panic and by-value `return_to_parent`, a signature that is one-way after publish |

Decisions 1 to 4 carry background, a recommendation, prior art and an example above; the
architect has ruled none.

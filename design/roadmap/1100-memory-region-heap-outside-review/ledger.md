# Ledger

An appendix to [the milestone](../1100-memory-region-heap-outside-review.md). Each item is DONE
with the test or harness that holds it, or LEFT with the reason. Pull request 3559 changes
`crates/memory_region_heap/src/lib.rs`, its manifest and this block's `before-the-tag.md`, so every
item that needs an edit there waits for it to merge, and the rows for items 5 and 6 are written
from that pull request's own text once it lands.

| item | state | where |
|---|---|---|
| 1 names and the missing `Name:` blocks | LEFT | Decision A (public names); the five missing blocks ride with that ruling |
| 2 `talc` and the path dependencies | LEFT | Decision B (a dependency) |
| 3 manifest, README, license files | LEFT | the move lane writes them |
| 4 `va` binding and `commit_all` | LEFT | `va` is in `src/lib.rs`, which pull request 3559 changes; `commit_all` is Decision A |
| 5 a second `initialize` | PENDING | done by pull request 3559 (`is_initialized`; a later call changes nothing); this row takes its test name when that pull request merges |
| 6 `grow` of zero bytes | PENDING | done by pull request 3559 (`got > 0 && got >= need_pages`); this row takes its test name when that pull request merges |
| 7 kernel refusals read as budget spent | LEFT | an edit to `src/lib.rs`, after pull request 3559 |
| 8 page-source seam, host tests, proofs | LEFT | an edit to `src/lib.rs` and the manifest, after pull request 3559; the std overlay and `second_mount` follow it |
| 9 documentation, minimum cap | LEFT | an edit to `src/lib.rs`, after pull request 3559; the README is the move lane's |
| 10 `PAGE_BYTES` | LEFT | an edit to `src/lib.rs`, after pull request 3559 |
| 11 panic inside the allocator | DONE | read: the default handler `user_mode_runtime::panic_handler!` calls `trap` and allocates nothing, so a debug-build allocator panic faults legibly and does not spin; no README or code change is owed unless a program's own handler allocates |
| 12 growth is monotonic | LEFT | one README sentence and one `BUGS` line in the crate; the crate documentation is `src/lib.rs`, after pull request 3559 |
| 13 accessors for the cap and base | LEFT | Decision A (public names) |
| 14 the `Sync` argument | LEFT | one paragraph in `src/lib.rs`, after pull request 3559 |

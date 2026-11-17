# The ledger

An appendix to [the milestone](../1077-allocation-cache-outside-review.md).

| item | state |
|---|---|
| 1 | done in the tree, as the sibling crates are: a README with both examples checked as doctests, the two license files, `license`, `rust-version`, `description`, `readme`, `keywords` and `categories`, an inline `[lints]` table, and a `publish = false` comment a stranger can act on. The move still strips the `Name:` blocks, and the repository's own `repository` and `documentation` fields are the move's |
| 2 | done: the root documentation, the manifest and the crate's tests name no consumer, no generator, no note and no tree path; the talc example is the README's |
| 3 | left: the move's, and decision 1 may close the move |
| 4 | done: `class` compares before it rounds (`a_size_near_the_top_of_usize_has_no_class`), and decision 3 is built provisionally: `class` returns `Option<Class>` and every method that takes a class takes `Class`, so no index can pass the last class and `class_bytes` cannot overflow |
| 5 | done: the sentence on `Cache` and `an_all_zero_cache_is_empty` |
| 6 | done: `Cache`, `push`, `spill` and `give_back_all` state the owner's rule |
| 7 | done provisionally, as decision 2 rules: `GRID_BYTES`, `MAXIMUM_CLASS_BYTES`, `BLOCKS_PER_CLASS`, `BATCH_BLOCKS`, `MAXIMUM_CACHED_BYTES`, `push`, `pop` and `give_back_all`; the consumer's `use` sites follow, and the glibc spellings are `doc(alias)` search terms |
| 8 | done: five harnesses with a falsification patch each. Two prove the size-class arithmetic and the heap layout. Three are the bounded model of `push` and `pop` over a fixed arena: a pop returns the block last pushed and the cached bytes are the sum, a class never holds more than `BLOCKS_PER_CLASS`, and a push to one class leaves the others alone |
| 9 | done: `align_of::<Free>() <= GRID_BYTES` beside the size assertion |
| 10 | left: the first weekly runs of the new repository |
| 11 | answered by item 8, which gives the crate five harnesses; the move lane still sees the `proofs` workflow green on its first pull request |
| 12 | done: `Debug` (`a_cache_prints_counts_and_not_pointers`) and the `Drop` assertion (`a_cache_dropped_while_it_holds_a_block_is_noticed`), which fires only in a debug build. The consumer never drops its cache, since the thread block is freed as raw memory after a `give_back_all` |
| 13, 14 | done: two BUGS lines in the crate root |
| 15 | done: the test is `a_request_without_a_class_reaches_the_heap_as_asked_unless_empty` |

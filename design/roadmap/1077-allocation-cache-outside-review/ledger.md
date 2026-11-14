# The ledger

An appendix to [the milestone](../1077-allocation-cache-outside-review.md).

| item | state |
|---|---|
| 1, 3 | left: the move's, and decision 1 may close the move |
| 2 | done: the root doc states a class, the owner's three obligations, why the heap layout shapes the uncached path and the zeroed rule; the consumer, the generator and the note are gone; the BUGS name the leak on drop |
| 4 | done for the arithmetic: `class` compares before it rounds (`a_size_near_the_top_of_usize_has_no_class`); the bare index is decision 3 |
| 5 | done: the sentence on `Cache` and `an_all_zero_cache_is_empty` |
| 6 | done: `Cache`, `put`, `spill` and `drain` state the owner's rule |
| 7 | left: decision 2 |
| 8 | two harnesses done (`a_request_lands_in_a_class_that_holds_it`, `the_heap_layout_is_never_empty_or_smaller`) with their falsification patches; the bounded model of `put` and `take` is left, because it needs an arena model this lane did not build |
| 9 | done: `align_of::<Free>() <= GRID` beside the size assertion |
| 10, 11 | left: the first weekly runs of the new repository; item 11 is answered by item 8 once the move lands |
| 12 | `Debug` done (`a_cache_prints_counts_and_not_pointers`); the `Drop` assertion is left, because a thread block that is dropped without a drain would fail it in a debug build and the consumer is not this lane's |
| 13, 14 | done: two BUGS lines in the crate root |
| 15 | done: the test is `a_request_without_a_class_reaches_the_heap_as_asked_unless_empty` |

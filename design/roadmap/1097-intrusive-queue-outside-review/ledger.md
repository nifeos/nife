# Ledger

An appendix to [the milestone](../1097-intrusive-queue-outside-review.md). Each item is DONE with
the test or harness that holds it, or LEFT with the reason.

| item | state | where |
|---|---|---|
| 1 manifest | LEFT | the move's manifest rows |
| 2 front page points into nife | LEFT | the move rewrites the citations and strips the provenance blocks |
| 3 repository skeleton | LEFT | the scaffold belongs to the move |
| 4 `Queue` is `Send` for every `T` | LEFT | decision 1 |
| 5 provenance clause, owner-pointer examples, Miri-run test | DONE | the safety list of `Unqueued::new`; `a_token_from_the_owning_pointer_survives_later_borrows` |
| 5 the kernel's `mint_token` from the stored pointer | LEFT | kernel code; it wants its own lane with the QEMU gates, and the move does not wait on it |
| 6 seven public names | LEFT | decision 2 |
| 7 compile-fail doctest names its error | DONE | the `push_back` doctest is `compile_fail,E0382` |
| 8 dropping a non-empty queue | DONE | the paragraph on `Queue` |
| 9 `remove` and an `extract_if`-shaped method | LEFT | two new public names and two new harnesses; the maintainer decides whether the surface grows |
| 10 clear link after a pop | DONE | harness `a_popped_node_leaves_with_a_clear_link` with `falsifications/verification.a_popped_node_leaves_with_a_clear_link.patch` |
| 11 std traits | DONE | `PartialEq` between tokens and from `NonNull` to a token, `Debug` for `Queue`; `a_token_is_its_nodes_pointer` covers the pointer comparison |
| 12 acronyms | DONE | the crate root expands FIFO at first use |
| 13 one shared test node | LEFT | the two nodes differ in the tag's type, and the test module already shares the owning-pointer helper; a shared module waits for the move |

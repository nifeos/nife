# Ledger

An appendix to [the milestone](../1104-paging-outside-review.md).

Names minted by this work are provisional: `Flags::ALL`.

- 1 LEFT: manifest, README and licenses are the crate's move preparation, which waits on the move.
- 2 LEFT: Decision 1 (the published name) is the architect's.
- 3 LEFT: the move rewrites the record lines and patch paths, not this work.
- 4 LEFT: the documentation rewrite for a stranger belongs with the move, with item 1.
- 5 LEFT: every rename is a public name, so each waits for the architect's ratification.
- 6 LEFT: the aliases wait for the same ratified spellings as item 5.
- 7 DONE: `Mapper::new`'s `# Safety` states the allocator's obligations, and a misaligned frame
  draws `MapError::Misaligned` before anything writes through it
  (`a_misaligned_frame_from_the_allocator_is_refused` in `tests/mapping.rs`).
- 8 DONE: `Flags::ALL` lists every constructor, and every harness and host test iterates it. The
  review counted ten constructors; there are eleven, and
  `all_lists_every_public_constructor` counts them from the source so nobody can miss a twelfth.
  The Kani harnesses run over all eleven (`script/verify --only paging`).
- 9 LEFT: the exemption port belongs to the move.
- 10 DONE: `map_range` and `map_span` refuse a run that passes `u64::MAX` with
  `MapError::BadRegion`, before mapping anything
  (`a_range_or_span_past_the_top_of_the_address_space_is_refused_whole`). its callers' loops keep `index` under
  `LEVELS`, and no assertion was added.
- 11 LEFT: the README is item 1's move preparation.
- 12 PARTIAL: the IOMMU formats' `PageFormat` and the crate's BUGS now document `SPLIT_SHIFT`
  as a ceiling, and `BadRegion`'s doc names its three raisers. Moving `WritableAndExecutable`
  out of the enum is a public-item change and is LEFT.
- 13 DONE: `Mapper::walk` asserts `F::LEVELS <= MAXIMUM_WALK` at compile time.
- 14 LEFT: Decision 2.
- 15 PARTIAL: `MapError` has `Display` and `core::error::Error`
  (`a_map_error_displays_and_is_an_error`), and the crate's BUGS names the missing operations.
  Adding methods is not done.
- 16 DONE: the documentation pass (`invlpg`, the x86 halves, `ACCESS_*`, the PAT reset value cited
  to the Intel SDM).
- 17 LEFT: a test-harness conversion with no effect on the crate's contract; not reached.
- 18 DONE: both limits are in the crate's BUGS. No vendor document for the T-Head bits was found.
- 19 LEFT: not reached.

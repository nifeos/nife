# Ledger

An appendix to [the milestone](../1106-program-argument-contract-outside-review.md).

Names minted by this work: none (the two harnesses are
`parse_never_panics_and_iterates_len_items` and `a_parsed_page_holds_exactly_what_its_header_states`).

- 1, 2, 3 LEFT: the move's preparation (manifest, repository files, citations).
- 4 LEFT: renames, for the architect.
- 5 LEFT: `PAGE_CONTENT_BYTES` is a public rename, ratified in the decision below.
- 6 PARTIAL: two of the three harnesses, each with a falsification patch. The round trip needs
  item 12's builder shape and is LEFT. The README's "What is proven" is item 2's.
- 7 DONE: `a_page_read_through_its_address_matches_the_slice_reader`.
- 8 LEFT: the exemption file and the coverage figure belong to the move.
- 9 DONE: the crate root says single-field damage, and its BUGS records the two-field case
  (`a_consistently_shortened_header_reads_as_the_shorter_argv`).
- 10 LEFT: the narrow-target claim is recorded in the crate's BUGS; the checked conversion changes
  an unsafe function copied into `std`.
- 11 DONE: `parse` answers `ArgPage::EMPTY`.
- 12, 13 LEFT: public signatures (`PageBuilder::new`, `record_bytes`), and `grant_plan` is
  touched by another pull request.
- 14 DONE: the record walk runs until the records are used up and compares the count walked.
- 15 DONE: `src_holds_exactly_one_file`, and the constraint is in the crate's BUGS.

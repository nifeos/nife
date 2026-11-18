# Where this stands

An appendix to [the milestone](../1087-domain-name-system-outside-review.md).

Finished: items 2 to 4, 6, 7 (parts a to d), 8 and 10 to 16, and the `forbid(unsafe_code)` half of
item 5, by #3608 and #3658; the names ruled for `ttl`, `CNAME`, `OPCODE_MASK` and `record_type::A`
are applied (#3668). Item 1 is finished: every public name is ratified as built. Decisions 1 to 3 are decided by the maintainer, provisionally.

What remains, with what holds each:

- Item 5's `fuzz/` directory, item 9's `.github/mutants-exempts`, item 7's `.github/accepted-words`
  entries, the stranger run and the first full `mutation` run: release step, the move.
- The move itself: decision for the architect. The crate has no review package under
  `notes/crate-reviews/`; a package comes first, then the architect's review clearance.

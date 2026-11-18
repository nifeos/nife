# Where this stands

An appendix to [the milestone](../1105-pci-outside-review.md).

Waiting on calef: the repository, which clearing this review approves.

Finished: items 3 and 4 (#3519); the P2 `cap_len`, `Bdf` masking and `forbid(unsafe_code)` items
(#3567); the package name (#3588). Both decisions have rulings.

What remains, and what holds each:

- Item 1, the fixture tests that reach into nife: ready for a lane.
- Item 2, the staged tree's manifest, README, citations and the corrupt regenerated patch: a
  release step, the move (`script/crate-move` and its staged tree).
- Items 5 and 6, the renames: ready for a lane (two-way before the first tag, so the maintainer
  rules the spellings provisionally).
- Item 7, documentation for a stranger: ready for a lane.
- Item 8, one shared capability walk and the ceiling proof: ready for a lane.
- P2 `intx_interrupt_number`, the precondition and `intx_pin_through_bridge`: ready for a lane.
- P2 fuzz targets and their opt-in: a release step, the move (the opt-in needs the repository).
- P2 stale patch headers: ready for a lane.
- P2 proof table rows and the mutation survivors: a release step, the move.
- Creating `nifeos/pci_configuration_space`: a decision for calef (clearing this review).

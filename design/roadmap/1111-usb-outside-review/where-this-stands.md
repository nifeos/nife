# Where this stands

An appendix to [the milestone](../1111-usb-outside-review.md).

Waiting on calef: the move, which clearing the review package approves.

Finished: items 3, 5, 6, 7, 11, 13 (in the crate) and 14 (#3614), items 8, 9 and 12 (#3580).
The maintainer has decided Decisions 1, 2 and 3, provisionally. Decision 1 and the `Speed` half of
Decision 3 have landed (#3614).

What remains, and what holds each:

- Items 1, 2, 4 and 10: a release step, the move. They are the manifest, the name
  `universal_serial_bus` with its alias, the scaffold files with the README and the stranger test,
  and the two fuzz targets. Milestone 820 (`extensible_host_controller_interface`: proven xHCI
  logic, released on its own) owns the move. The 13 prose acronym findings of the review package go with it.
- Decision 2's work, `changes` emitting HID usages and `MODIFIER_CODES` leaving the public surface:
  ready for a lane.
- Decision 3's other half, `DeviceDescriptor::parse` answering `Result`: ready for a lane.
- The move itself: a decision for calef (clearing the review package
  `notes/crate-reviews/universal-serial-bus.md`, whose recommendation asks for a review of the
  names).

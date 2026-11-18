# Where this stands

An appendix to [the milestone](../1090-filesystem-contract-outside-review.md).

Finished: Decision B's renames (provisional names, in force), Decisions A and C with item 6 (#3577), items 4, 13 and 17 to 20, the six harnesses of
item 8 (#3566, #3637), and the crate's own half of item 11.

What remains, with what holds each:

- Item 5's exemption for `block::SIZE` (an opcode the register gate takes for a register) and the
  aliases for kept foreign spellings: the template's exemption file is not written, so not built.
- Item 16, the errno module's name: Decision B's table does not name it, so no lane may choose one.
- Items 1, 2, 3, 9 and 10, and the new repository's half of item 11: release step, the move.
- Items 7 and 22: other. Each proxy call site needs a refusal before the packers can mask, a design
  change across three components, and no lane holds it.
- Item 8's proofs that the four walkers terminate: other, not built.
- Item 12, where `rm` lands and whether `CLIENT_WINDOWS` is a contract fact: other; the placement is
  not yet written up.
- Items 14 and 15, the `req_blocks` decoder and an empty name in `dirent::encode`: decision for the
  architect. Each changes what the contract says on the wire, and nobody has written up a fork yet.
- Item 21, a fuzz crate per walker: other, not built; a lane of its own.
- The `.nife-attrs` name in §99 (where Apple's metadata lands: stream or sidecar): decision for the architect, since a
  decision's text changes only with its owner's approval.
- The move itself: decision for the architect. The crate has no review package under
  `notes/crate-reviews/`; a package comes first, then the architect's review clearance.

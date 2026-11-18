# Where this stands

An appendix to [the milestone](../1113-virtio-outside-review.md).

Nothing here waits on calef.

Finished: items 1 and 11 (#3662, the roles live in `components/src/block_driver_roles.rs`), items
2, 7, 12, 14, 15 and 16 (#3613), items 4, 5 and 8 (#3520), item 6 (#3559, #3574), item 13 (#3520,
#3613). Decisions 1, 2 and 3 have rulings, and #3628 built Decision 3.

What remains, and what holds each:

- Item 3, the ring layout constants move to `abi::virtio` (Decision 2): ready for a lane.
- Item 9, the names of the folded roles (still `DMA_VA`, `QSIZE`, `blk`, `S_*` and the rest):
  ready for a lane, since the program publishes no name.
- Item 10, the kernel comments and notes that cite `crates/virtio`, which no longer exists:
  ready for a lane.
- A QEMU test of a block request past the disk's capacity (Done when): ready for a lane; a host
  test of `transfer_first_sector` covers the refusal today.
- The shared library Decision 1 holds the name `virtio` for: waits on milestone 1062 (the device
  control planes leave the kernel), step 7.

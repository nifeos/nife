# Done when

An appendix to [the milestone](../1113-virtio-outside-review.md).

- The crate's roles live in the drivers package beside `block_driver.rs` with no crate boundary
  (Decision 1, ruled 2026-11-16), and the workspace and the package tables no longer list
  `crates/virtio`. No crate takes the name `virtio` until the library Decision 1 holds it for exists.
- The ring layout constants have one home in `abi::virtual_io` (Decision 2) and every driver in the
  tree, this one included, imports them.
- `find_file`, `pattern_byte`, `ip_checksum`, `build_dhcp_discover` and `parse_dhcp_offer` take a
  buffer and have host tests, including a DHCP OFFER whose options end at the last byte of the
  received length and a frame shorter than the previous one.
- A `blk::READ` or `WRITE` past the device's capacity answers an error through the Reply and the
  block server is still serving afterward, shown by a QEMU test.
- The two volatile accessors check bounds and alignment, or are gone in favor of `MappedWindow`.
- The handshake waits for `Status` to read 0 after reset. The driver does not check `Version`; the
  kernel refuses a device whose `Version` is not 2 in every build (Decision 3).
- One `submit` and one `complete` exist; the attackers and the flush build their chains through
  them.
- The architect ratifies or renames the names in item 9, and `script/names` reports no public   item
  without a block.
- The two kernel comments that cite `user/src/virtio.rs` cite the file that exists.

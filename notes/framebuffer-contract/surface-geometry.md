# The surface's geometry

An appendix to [the framebuffer contract](../framebuffer-contract.md): why
`graphics_contract::WIDTH` and `HEIGHT` are 1120 by 624, and what the frames around it cost.

## Not square, on purpose

A square surface makes a stride bug, a transposition and an x/y swap all invisible. The byte count
comes out the same, and the checksum of a transposed square can still be built out of the right
pixels. 1120x624 makes every one of those a size or content mismatch. The lower bound is the
device's: QEMU refuses a scanout smaller than 16 in either dimension.

The size is set against the terminal's cell: 80 columns by 24 rows, the floor milestone 142 (a
text display good enough that people use it instead of a GUI) sets for a terminal somebody would
live in. At `glyph_atlas`'s 14x26 cell that is 1120 = 80 * 14 and 624 = 24 * 26, both exact, so no
strip of the surface is outside the grid. It is 1.79:1.

## The last frame's padding

`SURFACE_BYTES` is not a whole number of 4 KiB frames: 2,795,520 bytes, 682.5 frames.
`SURFACE_PAGE_FRAMES` rounds up, so the last frame carries 2 KiB of mapped but unused padding.
Nothing reads or writes there, because `user_mode_runtime::mapped_window::MappedWindow` bounds
every access against `SURFACE_BYTES`, not the frame count. The nearest frame-exact height that
still holds 24 rows is 640, which would leave a 16-pixel strip outside the grid. An exact grid is
worth more than an exact frame.

What that costs: the `PageFrame` run capability the kernel mints over this surface names 683 whole
frames, so it names 2,048 bytes (683 * 4096 - 2,795,520) that are not part of the surface. Those
bytes are inside the same contiguous DMA allocation, not another object's memory, so this is
padding rather than a disclosure. It is written down because a capability that names more than the
object it stands for is the kind of fact the BUGS convention exists to record; notes/frames.md
carries it in its BUGS.

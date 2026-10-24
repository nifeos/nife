# The screen console's design, type by type

*Name: provisional, for the crate it describes.*

A page beside [`notes/serial-less-output.md`](../serial-less-output.md). It holds the reasoning
behind `crates/screen_console`'s types, whose doc comments link here. Why the console is not
`video_terminal` is in [`framebuffer-handoff-choices.md`](framebuffer-handoff-choices.md).

## The scale

Each font pixel is drawn as a square block at the largest integer scale that leaves `MIN_COLUMNS`
columns, which is 120. So 1920x1080 gets a scale of two, and anything under 1680 wide gets one. The
font is the one `bitmap_font` holds for the graphical terminal too, so early-boot letters match it.

## The screen can only be written

A framebuffer aperture on real hardware is mapped uncacheable or write-combining. Reading either is
slower by orders of magnitude: a read has to cross to the device and back, one word at a time, where
a write is posted and forgotten. A scroll that moved the picture up with `copy_within` over the
aperture would read most of 8 MB back for every new line on xenon. xenon's console was seen to
redraw in sections that way.

So the console's only handle on the screen is `PixelSink`, which has no way to read. Its slice is
private and its one method is a store, so a painter that reads the screen back does not compile. It
is rung 1 of the ladder in `AGENTS.md`: the wrong state is unrepresentable, and no test has to
remember to look for it. A `compile_fail` doctest on the type holds that.

## The text grid is a separate value

The console redraws from `Cells`, one byte per character cell, rather than from the screen. That is
what makes a scroll cost writes and no reads. `Cells` is a separate value from `ScreenConsole`
because of its size, `Cells::CAPACITY` bytes, about 35 KiB. The kernel's boot stack is 64 KiB, and
`notes/stack.md` records an incident from a 16 KiB array on it. The kernel keeps the grid in its
console's `static`, initialized at compile time, so it is never built on a stack. `ScreenConsole`
stays a small `Copy` value that holds no pointer to device memory, so a caller can put it in a
`static`.

## The aperture, and the width of a store

`Aperture` is the arithmetic a driver needs to copy a rectangle of pixels onto a screen the
firmware set up. It lives in this crate rather than in the driver because it is a pure function of
five numbers, and a stride bug is only catchable on the host. Clipping is in surface pixels, against
the screen divided by the scale. A screen that is not a whole multiple of its scale leaves fewer
than `scale` pixels uncovered at its right and bottom edges. The scale is the kernel console's
rule, so the shell after the handover has at most the boot tour's columns.

`Aperture::copy_wide` pairs two pixels into one `u64` store where the alignment permits. The
aperture is mapped uncacheable (on x86 there is no PAT programming yet). Under QEMU's TCG,
store-heavy guest code runs about 3.8 times slower per instruction than arithmetic
(`notes/benchmarks/icount-tick-scales.md`). The one-word-per-pixel copy was most of the `x86_64`
swish leg's 321 s. Pairing the pixels halves the store count and changes nothing else.

A `u64` is also the widest store this tree's `x86_64` target can legalize. The kernel and initial
ramdisk build for `x86_64-unknown-none`, whose spec is `-mmx,-sse,+soft-float`, so there is no
vector store to reach for; `xtask/src/archive.rs` records the target. With the base's misalignment
passed in, every geometry still takes its widest path. A padded or odd stride only earns head and
tail words on some rows. The host test that proves `copy_wide` equals `copy` pixel for pixel runs
on a little-endian machine, as all three of this tree's targets are.

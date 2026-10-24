# The display tests: what each one exercises

An appendix to [notes/framebuffer-contract.md](../framebuffer-contract.md). The doc comments in
`system_tests/src/user/display_tests.rs` state what each test proves. This page holds the detail
behind those claims that the parent and
[notes/glyphs/terminal-and-proof.md](../glyphs/terminal-and-proof.md) do not already carry.

*Name: provisional, named for its content. calef names files.*

## The text on the scanout

The script `video_terminal::script` is chosen so that a lucky pass is hard. Each part is there to
catch one failure:

- Four rows of text, because a one-row picture would hide a stride error.
- Three renditions, because a terminal that ignored SGR would draw every glyph correctly and still
  fail.
- A `\r\n` pair, which is what `line_editor::expand_output` puts on the wire for a Unix `\n`.
- Descenders and an underscore, the glyph rows a font table truncated to seven would lose.
- Keystrokes, delivered as `OPERATION_BYTES`. That is the terminal contract's driver half, byte for
  byte what `components/src/input.rs` sends and what the compositor forwards to a focused client.

`gpu_driver` makes one status report, and it covers its first flush, which is the terminal's blank
grid. So the test also checks that an empty terminal is a defined picture rather than whatever the
frames held at boot.

The test then scrolls the screen past the grid's own height. The terminal's fast path moves pixel
rows rather than re-rendering them, and the x86_64 swish leg spends most of its wall clock in that
path (`notes/benchmarks/icount-tick-scales.md`). The kernel's comparison against its own engine is
the end-to-end witness that a moved picture is the right picture. A one-digit typo in a tail line
is the negative control for the scrolled comparison.

Each `Vt` grid in the test is a function-local `static` rather than a stack local. At the 80x24
grid one `Vt` is over a hundred KiB, and a kernel thread stack is 24 KiB
(notes/stack-high-water.md). `Vt::new` is a `const fn`, so each initializer lands in `.bss`.

## The keyboard driver's authority

The keyboard driver holds no client's endpoint, so it cannot choose who receives a keystroke or
even name a client. It rings the same content-free doorbell every client holds. Nothing it says on
that doorbell carries a keystroke: a client that rang it forever could not type a character,
because typing is a page that client does not map. The driver's authority to type is the ring's
mapping, and the compositor's authority to deliver is the client endpoints it holds.

## The firmware screen

The OVMF gate proves the firmware-screen chain on one geometry: 1280x800, bgrx, an unpadded
stride and a page-aligned aperture. The in-guest test needs no device, so it runs on all three
architectures, and it presents what a real machine could and OVMF does not.

- An odd shape, 42 pixels wide and 650 tall. The surface is the screen's size at its scale, not
  the virtio contract's 1120x624, and the terminal lays out 3 columns by 25 rows over it.
- A stride padded by 60 bytes. With it the stride is 228, an odd number of words, so every other
  row is off qword alignment. That is the geometry the flush's wide-copy path has to survive.
- rgbx order, so red and blue must be exchanged. It is checked on a red and a blue cell, because
  every gray in the default picture reads the same in both orders.
- An aperture that is not page-aligned, so the driver's sub-page offset is what lands pixel (0, 0).

Clipping to a screen taller than the surface, and the scale-two layout, are proven by
`screen_console::Aperture`'s host tests. The suite cannot afford the frames a larger pretend screen
would take. That a real screen scans this out is the OVMF gate's claim and the bench step's in the
milestone's block.

## The frame pool

The suite runs one boot's frame pool and region slots through every test, and hands little back
(`user::holding`'s module note). A test that keeps parked services, regions or frames starves a
later test in the same boot, which shows up as a hang far from the cause. So the firmware-screen
test reclaims both services' endpoint region and frees the surface, the output page and the
pretend screen before it returns.

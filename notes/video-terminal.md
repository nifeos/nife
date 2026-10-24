# The VT engine: `crates/video_terminal`

This note carries the design that `crates/video_terminal`'s doc comments link to. The crate is the
grid a display terminal keeps: bytes in, a character grid out, plus the rectangle that changed. Its
font and its limits are in [glyphs.md](glyphs.md), and the line discipline it displays is in
[terminal-contract.md](terminal-contract.md). The stem `video-terminal` is a provisional name.

Two appendices hold the rest: [the handoff blob and the
component](video-terminal/handoff-and-component.md), and [the palette, the keymap and the test
script](video-terminal/palette-keymap-and-script.md).

## One definition, three witnesses

The engine is a value with no IO. It holds no endpoint, makes no syscall and owns no framebuffer;
`components/src/display_terminal.rs` feeds it and paints what it says. So anyone holding the same
bytes can compute the expected picture. The terminal component runs it to draw, the kernel-side
test runs it to predict the framebuffer, and `cargo xtask` runs it on the host to grade what QEMU
scans out.

A test feeds the real `line_editor`'s echo to this parser, so the sequences the line discipline
emits are checked against the engine rather than against the contract's prose.

## What it implements

- Printable bytes, with deferred wrap at the right margin, and UTF-8. A multi-byte sequence takes
  one cell, drawn as `glyph_atlas`'s missing-glyph box past printable ASCII.
- `CR`, `LF` (scrolling into the history ring), `BS`, `TAB` and `BEL`, which is ignored.
- `CSI A/B/C/D` cursor motion, and `CSI H` and `CSI f` absolute positioning.
- `CSI J` erase in display and `CSI K` erase in line, each with all three modes.
- `CSI m` (SGR). It covers reset, and bold and italic, drawn in their own faces. Dim, reverse,
  concealed and crossed-out each have their off switch. Underline comes in five styles (`4`, `21`,
  `4:0` to `4:5`) with its own color (`58`, `59`). Colors are the eight and their bright forms,
  and the 256-color and 24-bit forms in both the `;` and the `:` spelling, for foreground and
  background.
- `ESC c` (RIS), a full reset.

Blink is parsed and dropped, because the engine reads no clock. Overline and font selection are
dropped too: text drawn in the wrong style is better than text not drawn. Anything else is
swallowed whole, introducer and all, rather than printed as garbage.

Not here: an alternate screen, origin mode, scrolling regions, tab stops other than every eight
columns, mouse, and any reporting sequence. The engine never writes to its input, which is what
keeps it a value, and a half-answered device query is worse than an unanswered one. There is no
reflow, since the grid's capacity is fixed.

## Parsing

The parser has five states: ground, after `ESC`, inside `CSI`, inside a string sequence, and
inside a string after an `ESC`. A full VT state machine has a dozen, but most of the extra ones
distinguish sequences this engine swallows anyway.

The string state earns its place. `ESC ]` (OSC), `ESC P` (DCS) and `ESC X`, `ESC ^`, `ESC _`
carry arbitrary text, so a parser without the state prints the window title onto the screen. The
test feeds a title-setting sequence to hold this. A
string ends at BEL (xterm's terminator) or at `ESC \` (ST); an `ESC` followed by anything else is
part of the string. A two-byte escape the engine does not speak consumes its final byte too, so
it cannot leave a stray letter on the screen.

A `CSI` sequence carries at most sixteen parameters. One 24-bit color is five (`38;2;r;g;b`), a
foreground and a background together are ten, and a highlighter often adds a reset and a
rendition in the same sequence. A seventeenth marks the sequence ignored, since a list that long
belongs to a sequence this engine does not implement. The first parameter may be empty either way
(`CSI ;5H`, `CSI :2m`), so a separator before any digit opens the first parameter as well as the
next. A private-use introducer (`?`, `<`, `=`, `>`) or an intermediate byte marks the sequence
ignored, because its parameter list means something else. A control code inside a sequence is
dropped with the sequence. Real terminals execute it, but nothing that reaches this engine relies
on that, and dropping fails toward drawing nothing.

`CUP` parameters are one-based on the wire and zero-based in the grid. Cursor sequences default an
absent or zero parameter to one; SGR defaults an empty list to zero, a reset.

### SGR

The extended colors (`38`, `48` and `58`, then `5;n` or `2;r;g;b`) consume the parameters they
name, so the walk steps an index. A malformed one (a missing or out-of-range component, or a
color space other than 2 or 5) ends the sequence's effect there, which is xterm's behavior: the
parameters after it cannot be told apart from the broken color's own. The colon form is
ECMA-48's sub-parameters. `4:3` is a curly underline, and `38:2::r:g:b` (with its empty
color-space slot) or `38:2:r:g:b` is a 24-bit color. A parameter's sub-parameters belong to it,
so ones it does not use are skipped rather than read as the next attribute. The walk is bounded by
the parameter count, since every pass consumes at least one.

### UTF-8

The decoder runs ahead of the control-code match. Every control code and introducer the engine
understands is below `0x80`, so a byte with the high bit set can only be part of a multi-byte
character, and the two decoders never see each other's bytes. State persists across `feed`
calls, so a sequence split at a buffer boundary still decodes.

A lead byte follows RFC 3629: `0xc2..=0xdf` starts two bytes, `0xe0..=0xef` three, and
`0xf0..=0xf4` four. `0x80..=0xbf` are continuation bytes only, `0xc0` and `0xc1` could only encode
an overlong form, and nothing past `0xf4` is assigned, so those draw U+FFFD at once. A sequence cut
short by a byte that is not a continuation draws U+FFFD and reprocesses that byte, so it loses
exactly the bytes it claimed. U+FFFD is the conventional choice, the one
`String::from_utf8_lossy` makes.

### Printing and the cursor

Writing into the last column leaves the cursor on that cell and arms a pending wrap; the next
printable byte wraps first. Every real terminal does this. Without it a line that exactly fills
the width scrolls the screen before anything asked it to, and a `CR` right after it finds the
cursor a row too low. Cursor motion, `CR` and `LF` cancel the pending wrap.

Backspace does not wrap back to the previous row. A line discipline only backs up within the line
it echoed, and wrapping back would need a record of which rows ended in a wrap. A tab moves to the
next multiple of eight and stops at the last column rather than wrapping, as fixed-tab-stop
terminals do. `CSI 2 J` does not move the cursor, which is why a line discipline's `^L` sends
`CSI 2J` then `CSI H`. Erasing writes spaces in the current rendition, so `CSI K` on a colored
background leaves the background rather than a black gap. `ESC c` clears in the default rendition,
which is the difference between a reset and an erase.

New output snaps a scrolled-back view to live, as every terminal does. A scroll key alone goes
through `scroll_up` or `scroll_down` and never reaches `feed`, so it is unaffected.

## The grid's capacity

The grid is a fixed array, because the terminal component has no allocator and this crate is the
same code the kernel and the host run. The component asserts its own geometry at compile time, so
a screen too wide is a build error. A requested geometry is clamped rather than refused: the
alternative in a `no_std` component is a panic in a process that cannot report one.

`MAX_COLS` is 240. The terminal on a firmware screen fills it at
`screen_console::ScreenConsole::scale_for_cell`'s scale, and that rule never gives a screen
`2 * screen_console::MIN_COLUMNS` (240) columns or more. QEMU's 1280x800 OVMF screen is 91 columns
and xenon's 1920x1080 is 137. The virtio-gpu scanout lays out 80 by 24, the floor milestone 142 (a
text display good enough that people use it instead of a GUI) sets; at the 14x26 cell its 1120x624
scanout is exactly the grid.

`MAX_CELLS` is a budget rather than `MAX_COLS * MAX_ROWS`, because the grid is stored at its own
width. 9,600 holds every screen this tree drives with room; xenon's 137x41 is 5,617. So 240
columns get 40 rows, and 150 rows need 64 columns or fewer. The cost is paid once per terminal
instance from that program's own region: the kernel's `user::load` sizes a process's region from
its ELF segments, `.bss` included.

The scrollback is a ring of whole rows, oldest overwritten first, for the same no-allocator
reason. 300 rows is a working depth, not a derived one. The ring is a cell budget at the grid's
own width, 24,000 cells, so it is 300 rows deep up to 80 columns and shallower past that: 263 at
OVMF's 91 and 175 at xenon's 137. The kernel's test image holds eight `Vt` statics as witnesses
(`system_tests/src/user/display_tests.rs` and `compositor_tests.rs`), about 5.4 MB of `.bss`
against QEMU's 256 MiB.

The two budgets hold two numbers still. A `Cell` is twenty bytes: a four-byte `char` beside a
fourteen-byte `Attr` of alignment one. So the 33,600 cells make a `Vt`'s cells 672,000 bytes, 164
page frames. A handoff blob is four words a cell, at most 537,744 bytes, so `HANDOFF_PAGES` stays
132, the figure the swap reserve is sized on. A const assertion holds the cell at twenty bytes, so
a change that grew it fails at build and must be priced here.

### A `Vt` is never a stack value

At several hundred KiB a `Vt` is past a kernel thread stack (24 KiB) and a user process's stack
page (4 KiB). `Vt::new` is a `const fn` so that `static mut ... = Vt::new(...)` with constant
geometry is built by the compiler and placed in `.bss`. A call to a `const fn` outside a const
context runs as an ordinary function and needs a `Vt`-sized return value at runtime. So a caller
with a runtime geometry (`display_terminal` negotiating with its driver, a kernel test reading a
window's size off a control page) constructs once, typically at `(1, 1)`, and calls
`Vt::reset_to` to retarget in place. The `script` functions take `&mut Vt` for the same reason: a
kernel test calls them on a 24 KiB thread stack. `script/stack-frame-check` is the gate that
catches a violation.

## Damage: what changed, and what moved

A scroll is not a change to any cell; it changes where the cells are. Reporting it as a whole-grid
rectangle forces every consumer to re-render a screen whose pixels it already holds. On the
`x86_64` swish leg that re-render, per scrolled line, was most of 321 s
([benchmarks/icount-tick-scales.md](benchmarks/icount-tick-scales.md), the swish-check row). So
`Damage` has two fields and one invariant: move the picture up `scrolled` cell rows, then render
`rect`, and the result is the grid. A consumer that cannot move pixels uses `repaint_rect`, which
folds the scroll into a whole-grid box.

The changed box is a bounding box rather than a region list, the trade `compositor::Rect` makes:
two small changes far apart cost the box that holds both, a few extra glyph blits.

The cursor is the one thing on screen whose pixels do not move with its cell. A scroll carries the
rendered block up a row while the cursor stays on the bottom row, so the row the block landed on
must be re-rendered. `Vt::drawn` records where the last report drew the cursor, in current
coordinates, and each scroll moves it up a row. `take_damage` dirties the old and new cursor cells
only on a real difference, which keeps an engine at rest reporting `None`; a `BEL` is not a
one-cell flush. `feed` dirties the endpoints of each cursor move once per call rather than per
byte.

The cell model behind `put`'s comparison moves one row at a time, which leaves a copy of the
bottom row behind. A painter moves `scrolled` rows at once, so the two agree only when `scrolled`
is one. Past that, `take_damage` adds the exposed bottom rows whole, or a row wider than its
replacement would leave its tail on the screen. A full-grid box subsumes any scroll in the same
window, and `rows` scrolls between takes replace everything, so both report content with
`scrolled == 0`. That keeps `scrolled < rows` at every take.

## Rendering

`Vt::pixel` is the whole of rendering, a pure function of the grid and the scroll position. The
cursor is a block drawn by swapping the cell's two colors, not an overlay, so a test that predicts
the screen predicts the cursor too. It is not drawn while scrolled back, where its `(col, row)`
names a live cell that is not on screen.

`Attr`'s color resolution takes a rendition in a fixed order. Bold changes no color, since it is
a face. Dim moves the foreground halfway to the background per channel in sRGB; the midpoint is xterm's
amount. sRGB rather than linear light is deliberate: dim asks for half the apparent brightness, and
the linear-light midpoint of white and black is sRGB 188, which reads as barely dimmed. Linear light
is right for anti-aliased coverage, where the question is how much light a partly covered pixel
emits. Reverse swaps the two colors. Invisible paints the foreground in the post-reverse
background, so the text is there to copy and not to see. It comes last so the block cursor, which
toggles reverse, still shows on a concealed cell.

The line renditions ink the whole cell width, gutters included, so an underlined word is one
unbroken line; the underscore glyph's ink stops at the gutters. The patterned underlines read the
screen's `x` rather than the cell's, so a wave or a run of dots carries across cells. The 256-color
table above sixteen is xterm's, computed: the cube's levels are `0, 0x5f, 0x87, 0xaf, 0xd7, 0xff`
and the ramp is `0x08 + 10 * k` for 24 grays that stop short of black and white. Every emulator
that implements `38;5;n` agrees on these numbers.

## Mutants the engine cannot kill

Some guards are redundant on every reachable input, so a mutation survives them. They are kept for
the reader.

- `Vt::pixel` checks `col < self.cols` after `col == self.col`. The cursor is never at or past the
  margin, so the second test cannot change the branch.
- `Vt::erase_display` tests `to > from`, which always holds: `cols` and `rows` are at least one,
  and every cursor move clamps below the margin. So `>` relaxed to `>=` survives.
- `Vt::push_scrollback_row` is only called with row `0`, so `row * cols` and `row / cols` agree.
  It never runs while scrolled back, since `feed` resets the view before its byte loop.
- The UTF-8 accumulator's `|` cannot be told from `^`. `code << 6` has six zero low bits and
  `b & 0x3f` is six bits, so the operands never share a set bit.
- The `CSI` arm for private-use and intermediate bytes runs the same code as the catch-all. It is
  written out so the two families of swallowed byte are named separately.

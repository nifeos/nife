# Glyphs, the VT engine, and input

Milestone 29's remaining increment: the piece that turns a framebuffer into a terminal a person can
read. Rung one put pixels on a screen ([framebuffer-contract.md](framebuffer-contract.md)) and rung
two multiplexed the screen among mutually distrusting clients ([compositor.md](compositor.md)).
Neither could show a letter.

The code halves are `crates/glyph_atlas` (the terminal's font), `crates/video_terminal` (the grid engine, the keymap, and the
test script), `components/src/display_terminal.rs` (the terminal component), and `components/src/keyboard_driver.rs` (the keyboard
driver). This is the prose half.
The survey behind the font, the input path's design and the proof of text on the screen are in
[`glyphs/`](glyphs/README.md), and each section links the appendix it summarizes.

## The shape

```text
  virtio-input ──virtio (PCIe, IOMMU)──► kbd ──the input ring──► compositor ──OPERATION_BYTES──►┌───────┐
  (a keyboard)                                                                            │ display_terminal │
                                                         application ──OPERATION_WRITE──────────►└───────┘
                                                                                              │ glyphs
                                                                    its surface ◄─────────────┘
                                                                         │
                     display ◄──gfx FLUSH(damage)── compositor ◄──COMMIT─┘
```

Everything above the surface is text; everything below it is rung one's contract, unchanged.

## The terminal's font: DejaVu Sans Mono, rasterised at build time

`crates/glyph_atlas` is DejaVu Sans Mono (§104 (the rich-text font is DejaVu Sans Mono, and the
palette is Solarized)) at a 14x26 cell. Its four faces are what §166
(the rasteriser dependency, and whether the glyph atlas ships one face or four) ruled. `tools/glyph_atlas_generator`
rasterises it on the host; its test fails unless the checked-in tables are exactly what it makes
from `vendor/dejavu`. A glyph is a lookup and a blend; no witness below needs a font engine. The [determinism appendix](../design/roadmap/0142-a-text-display-worth-living-in/determinism.md)
of milestone 142 (a text display good enough that people use it instead of a GUI) says why.

## The boot console's font: ours, drawn in the Kaypro II's style

`crates/bitmap_font` is a 7x8 monochrome bitmap font and a pure function from `(byte, x, y)` to a
color. The kernel's boot console (`crates/screen_console`) and `board_console` draw with it; the
terminal does not.

It is an original drawing, made for this tree in `crates/bitmap_font/kaypro-style-7x8.art`. Nobody
holds a license over it and no obligation travels with it. It replaced `font8x8` (public domain, by
Daniel Hepper, from Marcel Sondaar's `font8x8.h`, from IBM's public-domain VGA fonts) on 2026-08-20,
after a poll calef ran was won by the Kaypro II's character generator.

The style is the Kaypro II's and the bits are not, a distinction the law makes. A *typeface as typeface* is listed by 37 CFR 202.1(e) among the things not subject
to copyright, along with "mere variations of typographic ornamentation, lettering or coloring", so
the look of a font is free to reproduce. A particular file of bitmaps is somebody's work. The ROM is
excluded on exactly that second ground, and the case is set out in
[`glyphs/kaypro-ii-rom.md`](glyphs/kaypro-ii-rom.md). The dumps in circulation state no license at all, `ivanizag/kaypro-disassembly` has no `LICENSE` file,
and this file's standing rule is that ambiguous is treated as obliged. So the ROM is not in this
repository, was not traced, and is not needed: it was used the way a person uses a reference,
which is by looking at the shapes and drawing your own.

The claim is checkable. The `.art` file is the drawing, every glyph is a picture
of `#` and `.`, and `crates/bitmap_font/src/glyphs.rs` is that file transcribed with a test
(`the_art_file_and_this_table_agree`) that parses it back and fails if the two ever drift.

Why the license question is worth this much care: a bitmap font is compiled into the kernel
image and into every binary that draws text, so its license is a license on the *artifact* rather
than on a build-time tool. That was the reason `font8x8` was chosen and it is the reason this one is
drawn rather than downloaded.

### The geometry, which is the machine's and is why the terminal got wider

Seven columns, of which the middle five carry ink and the outer two are gutter. That is what
the Kaypro's video board did in hardware, shifting out a zero, five ROM
bits and a zero (MAME's `kaypro_v.cpp`). Eight rows: 0 to 6 are the body with the baseline at row 6,
and row 7 is the one-row descender. Caps and digits fill rows 0 to 6, x-height letters rows 2 to 6.

That geometry is most of the argument for the font. 128 / 7 is 18 columns where 128 / 8 was
16, on the same 128x64 scanout, at the same 1024-byte table. Milestone 29's other candidate,
`gohufont-14`, has better letterforms and gives four rows of text, and four rows is not a terminal.
No larger scanout is reachable today (a `PageFrame` names one page and the display driver has nine
capability table slots left), so a narrower cell is the only lever there is.

The division has a remainder: 18 cells of 7 is 126, so two
pixels on the right of a full-width surface belong to no cell. `Vt::pixel` already answered for
them (a cell outside the grid is a blank on the default background), and `display_terminal` paints
its whole surface on its first frame so that something writes them. No surface has to be a
whole number of cells any more.

### What was done better than the ROM, and what the grid would not allow

Where the machine is weak for reasons the grid does not force, the drawing is not: one baseline for
every glyph, four distinct shapes for `Il1|`, and a slashed zero. What the grid forces is kept: `M`
and `W` are near mirrors, the descender is one row, and `_` does not join.
[`glyphs/font-design.md`](glyphs/font-design.md) has each choice.

### The options, with pictures

A survey (2026-08-19, lane `bench/font-options`) rendered every candidate with
`cargo run -p bitmap_font --example specimen`, priced each license (calef ruled the OFL priced, not
refused), and found the screen, not taste, was the binding constraint. On the 128x64 scanout then, a
14- or 16-row font gave four rows of text. The Kaypro II's ROM and most early machines' fonts are
excluded on license. calef's poll on 2026-08-20 chose the Kaypro's style. The appendices:
[`glyphs/font-survey-2026-08-19.md`](glyphs/font-survey-2026-08-19.md) (the candidates, licenses and
measurements), [`glyphs/kaypro-ii-rom.md`](glyphs/kaypro-ii-rom.md) (the ROM, found, rendered and
excluded), and [`glyphs/retro-fonts.md`](glyphs/retro-fonts.md) (the rest of the era).

## The VT engine: sans-IO, and checked against the real line discipline

`crates/video_terminal` keeps the grid: bytes in, a character grid out, plus the rectangle that changed. It holds
no endpoint, makes no syscall, and has never heard of a framebuffer, exactly as `line_editor` does for
the serial terminal ([line-discipline.md](line-discipline.md)).

What it implements is not a guess. The escape sequences a display terminal must understand are
the ones the line discipline already emits (DECISIONS §21 (the terminal is a userspace component)).
Rather than assert that from a hand-written list that could drift, the crate's interoperability test
runs the real `line_editor` and feeds its echo stream to this parser. It types a line, backs up,
inserts, deletes, kills, presses Enter and presses ^L. The grid must show the line the discipline
says it assembled. Two separately-correct
components now fail together or not at all.

On top of that: printable bytes with deferred wrap, `CR`, `LF` with scrolling, `BS`, `TAB`, `BEL`
(ignored), `CSI A/B/C/D`, `CSI H`/`f`, `CSI J` and `CSI K` in all three modes, `CSI m`, and `ESC c`.
Anything else is swallowed whole, blink among it. `CSI m` covers bold and italic (their own
faces), dim, reverse, concealed, crossed-out, underline in five styles with its own color (`4:3`,
SGR 58), and 16, 256 or 24-bit color, in the `;` and the `:` spellings. A cell is twenty bytes.

Three decisions inside it worth reading:

- Deferred wrap. Writing into the last column leaves the cursor there and arms a pending wrap;
  the *next* printable does the wrap. Without it, a line that exactly fills the width scrolls the
  screen before anything asked it to, and a `CR` arriving right after the last character finds the
  cursor a row too low. That is the difference between a grid and a terminal.
- Bold is a face, not a brightness. An eight-pixel cell had to brighten instead, as every
  terminal since the DEC VT has; at the atlas's 14x26 cell the weight shows, so SGR 1 draws
  `glyph_atlas::BOLD` and the color stays as it was set.
- The cursor is part of the picture, drawn by inverting its cell rather than overlaid. That keeps
  the screen a pure function of the state: a test that predicts the screen predicts the cursor too,
  and a cursor left in the wrong place is a failure rather than a cosmetic difference nobody notices.

A bug the tests caught before anything reached a screen is recorded, because it is a real terminal
bug and not a toy one. An OSC sequence (`ESC ]0;title BEL`, how every program sets a window title)
printed the title onto the grid, because the parser had no string state. It has one now, and the
test that found it feeds a title-setting sequence on purpose.

## The terminal: a client at both seams, and the same binary

`components/src/display_terminal.rs` serves the terminal contract's IPC half
([terminal-contract.md](terminal-contract.md)) against a grid and a font instead of a serial line.
One binary, two wirings, chosen by `arg0`:

| | `MODE_DISPLAY` | `MODE_WINDOW` |
|---|---|---|
| slot 0 | report endpoint | report endpoint |
| slot 1 | the **display** endpoint, WRITE (rung one) | the **doorbell**, WRITE (rung two) |
| slot 2 | the terminal endpoint, READ (it serves) | the terminal endpoint, READ (it serves) |
| mapped | the scanout, an application's output page | its control page, its surface, an output page |
| presents by | `gfx FLUSH(rect)` | `compose COMMIT` |
| knows | no device, no physical address | no device, no neighbor, not even its own position |

That is `painter`'s authority in the first column and `window`'s in the second, and it is the
answer to the question this increment was asked to check: *did the framebuffer contract need
changing to carry text?* No. Neither did the compositor's. Both carry pixels, and a terminal draws
pixels; `gpu_driver` cannot tell `display_terminal` from the client that painted a test pattern, and `compositor` cannot
tell it from the client that painted a coordinate function. The answer is a spawn literal rather than
an argument.

### One endpoint, and the deadlock that shaped the input path

A process here has one blocking wait point (DECISIONS §33 (the compositor's authority is memory,
not messages)), so output and keystrokes share one endpoint, told apart by opcode. A terminal that rang the compositor's doorbell on a keystroke
would deadlock against the compositor's `CALL` to it. So it does not ring: the input source's own
`COMMIT` makes the frame that shows the keystroke. [`glyphs/terminal-and-proof.md`](glyphs/terminal-and-proof.md)
has the reasoning.

## Input: the ring is the authority, the doorbell is not

`components/src/keyboard_driver.rs` is a confined userspace virtio-input driver. It holds the device, its interrupt,
its own DMA page, the doorbell, and the input ring's mapping. It holds no client's endpoint and
cannot name a client.

That split is DECISIONS §33 seen from the producing side:

- The power to type is the ring's mapping, which no client has. It is not the doorbell: every
  client holds that, everything sent on it is content-free, and a client that rang it forever could
  not produce one character.
- The power to decide who receives is the compositor's, expressed as which of the per-client
  input endpoints *it* holds it uses. The driver cannot influence it. A client receives a keystroke
  because it holds an input endpoint, and a client granted none has an empty capability table slot and is
  refused with `NoSuchSlot`, "there is nothing there".

So focus never becomes ambient: there is no verb that grabs the keyboard, no message that names a
recipient, and no page a client can write that would inject input. The parts that could be forged do
not exist rather than being guarded.

The keyboard rides PCIe, and here that is a choice rather than a constraint: both `virt` machines
do offer a `virtio-keyboard-device` on the virtio-mmio bus. It rides PCIe so it lands in the same
IOMMU domain the GPU does. A keyboard is the device whose DMA you would least like unconfined,
because its buffers are where every keystroke lands.

The scancode-to-byte mapping is `video_terminal::keymap`: a US layout's main block, shifted and unshifted, as a
flat table plus one bit of state (shift is *held*, so it has to be remembered between events).
Host-tested, because a keyboard layout is data and a wrong row is exactly what a table test catches.
Two rules there earn their tests: a release types nothing (the first bug every evdev driver has
is every character arriving twice), and Enter sends CR, not LF.

## How text on a screen is proved

The picture is a value three parties compute without talking to each other, from one script,
`video_terminal::script`. The terminal paints it, the kernel compares the framebuffer pixel for pixel
through the direct map, and the host compares QEMU's `screendump`. The host checker must reject the
same screen with an `o` for a zero, the typed input missing, every rendition ignored, and a blank
terminal. The host presses one key through QEMU's monitor, and a two-terminal compositor test makes
focus routing visible in the picture. [`glyphs/terminal-and-proof.md`](glyphs/terminal-and-proof.md)
has the witnesses, the ordering by test name, and why each choice makes a lucky pass hard.

## Honest limits

Stated plainly, because a demonstrator's caveats are part of the deliverable.

- **Scrollback: BUILT 2026-08-26** (milestone 142 increment 2). A ring of `video_terminal::
  SCROLLBACK_ROWS` off-screen rows plus a viewport (`Vt::scroll_up`/`scroll_down`, `Vt::
  view_offset`); new output snaps the view back to live. Not wired to a key: the route needs a new
  opcode, proposed in `design/roadmap/0668-scrollback-from-the-keyboard.md`.
- **UTF-8: BUILT 2026-08-26** (milestone 142 increment 2). The VT engine decodes UTF-8 in its ground
  state (a running `utf8_need`/`utf8_code` accumulator, invalid or truncated sequences drawing
  U+FFFD). The atlas covers printable ASCII, so a decoded non-ASCII `char` draws the font's
  missing-glyph box, one box for one character rather than one wrong picture per encoded byte.
- No line editing in the display terminal. It renders a stream and echoes keystrokes; it does not
  serve `OPERATION_READLINE`. A client that wants edited lines puts `line_editor` in front of it and prints the
  discipline's echo through `OPERATION_WRITE`, which needs no new protocol at all, because `line_editor`'s echo
  is exactly a byte stream this engine parses. That is not a hope: the `video_terminal` crate proves it on the
  host by running both.
- An 80x24 grid on the virtio-gpu scanout: 1120x624 is exactly 80 cells of 14 by 24 of 26.
  `MAX_COLS`/`MAX_ROWS`/`MAX_CELLS` are sized for the largest firmware screen, not the scanout.
- The atlas clips a glyph to its cell, so an oblique glyph loses what leans past it;
  `crates/glyph_atlas`'s BUGS has the measure.
- The boot console's 7x8 font has weak glyphs, named where a reader meets them. `M` and `W` are near vertical
  mirrors, because five ink columns leaves one way to draw each. `&` is the busiest glyph in the set
  and reads as a knot at a glance. `%` fills its corners heavily enough to look bolder than its
  neighbors. And `_` does not join across cells, so a rule drawn out of underscores is dashed. The
  first and the last are the grid rather than the drawing; the middle two are the drawing and could
  be improved by someone with a better eye.
- No box-drawing, no block glyphs, no line-drawing set. The font covers printable ASCII and
  nothing else, so a program that wants a frame draws it out of `-` `|` `+`.
- No reflow on resize, because nothing resizes. The roadmap named reflow; a fixed scene has
  nothing to reflow to.
- The keymap is a US layout's main block, plus the arrow and navigation clusters (2026-08-26
  and 2026-09-26), with xterm's shift forms. Still no keypad, no function keys, no compose, no dead
  keys, no other layout.
- No bell, visual or otherwise. `BEL` is consumed.
- No mouse. `virtio-tablet-pci` presents the same PCI device id as the keyboard, which is
  recorded in `crates/pci` so that a machine carrying both would be a known problem rather than a
  surprise. We attach only a keyboard.
- No key repeat of our own. The device's repeats are honored; nothing here generates them.
- The font survey's own tool and candidates have limits (wide glyphs clipped, narrow fonts drawn at
  their own advance, an unproved 8x16 authoring cost); they are recorded in
  [`glyphs/font-survey-2026-08-19.md`](glyphs/font-survey-2026-08-19.md).

## What adopting libghostty-vt would cost now

The recommendation, not a decision: adopt Ghostty's VT core as a *second* engine behind the C seam,
not a replacement, when there is a reason to want scrollback and UTF-8 rather than a Zig dependency.
It would buy a far more complete VT engine and the strongest form of the claim of milestone 23
(a capability-routed component OS). It would
cost a Zig toolchain and a rebuilt proof structure, because its ABI gives cells where ours gives
pixels. Keeping `crates/video_terminal` as the reference it is checked against is worth more than
either alone. [`glyphs/libghostty-vt.md`](glyphs/libghostty-vt.md) has the pricing.

## Where the pieces are

| piece | file |
|---|---|
| the terminal's font, and how it is drawn | `crates/glyph_atlas/src/lib.rs`, `tools/glyph_atlas_generator` |
| the outlines it is drawn from, and their license | `vendor/dejavu/`, `vendor/README.md` |
| the boot console's font as pictures, which is what to edit | `crates/bitmap_font/kaypro-style-7x8.art` |
| the boot console's font and its provenance | `crates/bitmap_font/src/lib.rs`, `crates/bitmap_font/src/glyphs.rs` |
| the VT engine | `crates/video_terminal/src/vt.rs`, and the cells, damage and palette beside it |
| the keymap | `crates/video_terminal/src/keymap.rs` |
| the test script, shared by three witnesses | `crates/video_terminal/src/script.rs` |
| the terminal component | `components/src/display_terminal.rs` |
| the keyboard driver | `components/src/keyboard_driver.rs` |
| enumeration | `kernel/src/pci.rs` (`find_input_device`) |
| the wiring | `kernel/src/user/display_service.rs` (`start_terminal`), `kernel/src/user/compositor_service.rs` (`spawn_terminal`), `kernel/src/user/keyboard_service.rs` |
| the tests | `system_tests/src/user/display_tests.rs`, `system_tests/src/user/compositor_tests.rs` |
| the host-side text check and its negative control | `xtask/src/scanout.rs` |
| the device lines | `helpers/qemu-runner-aarch64.sh`, `helpers/qemu-runner-riscv64.sh` (`NIFE_KBD`) |

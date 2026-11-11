# The terminal's input path, and how text on a screen is proved

*Name: ratified 2026-10-18 (calef, convention-rulings Rule 3).*

An appendix to [notes/glyphs.md](../glyphs.md), which keeps the design in summary. This file holds
why the terminal has one endpoint, the deadlock that shaped its input path, and the three-witness
proof of text on the screen with its ordering and keystroke details.

## One endpoint, because one wait point

A terminal has two classes of sender: an application printing, and an input source typing. DECISIONS
§33 (the compositor's authority is memory, not messages) recorded that a process here has exactly one
blocking wait point: one `RECEIVE`, no wait-any, and two threads cannot share an address space. So
telling them apart by endpoint is not available. They arrive on one endpoint and are told apart by
opcode, which is what `line_editor` already does.

The security consequence is stated rather than hidden. An application holding that endpoint could
send `OPERATION_BYTES` and forge a keystroke into its own terminal. It gains nothing, since the bytes
come back to the grid it is already printing on. The boundary that matters, one client's input not
reaching another's, is the compositor's, and is a capability there.

## The deadlock that shaped the input path

A terminal that answered a keystroke by ringing the compositor's doorbell deadlocks, and it takes two
keystrokes in one drain to do it. The compositor is blocked in its `CALL` to the terminal, while the
terminal is blocked in its `CALL` to the compositor. That is §33's known cost of
input-as-a-blocking-`CALL`, arriving in practice.

It does not need to ring. The compositor rescans every client's control page on every `COMMIT` from
anyone, and the input source rings `COMMIT` itself after it fills the ring. So the frame that delivers
a keystroke is the frame that shows it: the terminal paints, records its damage, bumps its sequence,
and replies. Application output is different, because nobody else is going to ring for it. So
`OPERATION_WRITE` does ring, and that is safe, because the caller blocked in `CALL` is the
application.

The result is better than the design the deadlock ruled out, which is worth saying plainly. A client
that does not have to ask for a frame after receiving input is one fewer round trip, and one fewer way
to stall the compositor.

## How text on a screen is proved

This is the part that took the most care, because text is where "it looked right" is most tempting
and least sufficient.

The picture is a value three parties compute without talking to each other. The script is
`video_terminal::script`, a constant in the contract crate: the same move `graphics_contract::pixel`
and `compositor::SCENE` make.

1. The terminal runs the engine over the bytes it was sent, and paints what it says.
2. The kernel runs the same engine over the same script, and compares the framebuffer pixel for pixel
   through the direct map. It never asks the terminal anything.
3. The host runs it a third time, and compares QEMU's `screendump` against the same definition.

The third is not decoration. `-display none` means nothing in the guest can see the device's own
surface. So a wrong pixel format or a wrong scanout rectangle would satisfy the first two. A wrong
format turns a *test pattern* into an odd-looking test pattern; it turns *text* into something nobody
can read.

The host checker has its own negative control (`cargo test -p xtask`), and its failure modes are the
terminal's rather than the driver's or the compositor's. It must reject:

- the same screen with one letter changed (`glyphs_ok` against `glyphs_0k`, an `o` for a zero, the
  closest pair of glyphs in the font and therefore the hardest case). This is the assertion that makes
  the whole thing mean something. A checker that could not tell those apart would report "readable
  text reached the scanout" for a terminal that drew the wrong text;
- the typed input missing, which is a screen that is correct as far as it goes;
- every rendition ignored, which is every glyph in the right cell in the wrong color, the picture a
  terminal that swallowed SGR as an unknown sequence would draw;
- a blank terminal, and the other two pictures on the same scanout.

The script is chosen so a lucky pass is hard. It has four rows (a one-row picture hides a stride
error), three renditions, and a `\r\n` pair (what `line_editor::expand_output` puts on the wire for a
Unix `\n`). It has descenders plus an underscore, the glyph rows a font table truncated to seven would
lose.

## Ordering, and what breaks it

Three pictures reach one scanout over one boot, and `cargo xtask` looks for them in order: the
composed screen, then the terminal's text, then rung one's pattern, which stays up until QEMU exits.
Tests sort by name, so the order is arranged by naming.
`a_backing_outside_the_grant_is_refused_by_the_iommu` (which resets the device) sorts before
`a_bitmap_font_and_a_vt_engine_put_readable_text_on_the_scanout`. That sorts before
`a_confined_userspace_driver_puts_a_known_pattern_in_a_framebuffer`. A reordering does not corrupt
anything; no dump matches, and the scanout check fails loudly.

## The one place the host presses a key

Nothing in the guest can press a key, so the host does. `cargo xtask` sends `sendkey` on the same
monitor connection the scanout check already holds open, every poll, from the start of the run. That
needs no synchronization, because QEMU drops key events until a driver sets `DRIVER_OK`.
`video_terminal::script::HOST_KEY` is the single definition of which key, so the side that presses and
the side that asserts cannot drift.

The keyboard test proves the path from a physical key event to a terminal byte. The compositor test
proves the path from the ring to a focused terminal's pixels. The seam between them is the ring, which
is exactly where §33 put the authority boundary. Naming the seam is better than one test that hides
it.

## And in the compositor, the routing is visible in the picture

`focus_routes_a_keystroke_to_one_terminals_grid_and_not_its_neighbours` puts two display terminals
side by side. It types `A` at the focused one, presses TAB, and types `B` at the next. The kernel then
compares every pixel of the composed screen against the two engines it ran itself. A keystroke
delivered to the wrong client is a wrong picture, not a missed assertion. The test also checks that
the two terminals' contents *differ*, so it cannot pass by the two scripts having become the same
text.

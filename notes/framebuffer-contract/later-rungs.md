# What the next two clients did with this contract

*Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes.*

An appendix to [notes/framebuffer-contract.md](../framebuffer-contract.md), moved there on
2026-10-11 (UTC) under §212 (a prose budget). It records rung two (milestone 33 (a compositor)) and the
display terminal (milestone 29 (a display terminal)'s text increment), the two clients built after
the contract, and what each
predicted change turned into. The predictions are the main page's list of seams left open.

## What rung two did with this contract (milestone 33)

Written back here on 2026-07-29, because a contract's real test is what happened when the next thing
implemented against it. The answer is worth recording: nothing in this rung changed.

`crates/graphics_contract` and `components/src/gpu_driver.rs` are byte-for-byte the same. The
compositor (`components/src/compositor.rs`) took `painter`'s place at this seam. It holds the display
endpoint and the scanout frames with exactly `painter`'s authority, and the driver cannot tell the
difference. The only addition on this side of the seam is a kernel wiring entry point that starts
the driver with no client (`display_service::start_driver`). That is because rung two's client is
spawned separately with a scene behind it.

Three of the four rung-two tests go further and replace `gpu_driver` with a kernel stand-in that
serves `INFO` and `FLUSH` over frames the kernel allocated. The compositor does not notice that
either. That is milestone 23 (a capability-routed component OS with live replacement)'s
swappable-component claim arriving as a side effect of a contract
rather than as a demonstration built on purpose. It also made the damage rectangle *observable*. A
real driver honors a rectangle and says nothing about it, so the stand-in is how "a one-window redraw
does not cost a whole screen" became an assertion.

Two predictions in the list came out exactly as written, and one was answered differently:

- Damage tracking: the compositor flushes one rectangle per frame, and the test poisons the rest of
  the scanout and finds the poison intact. As predicted, no driver change.
- Input: the keyboard's routing question turned out to be the compositor's, as predicted. The
  answer reuses `line_editor::proto::OPERATION_BYTES` verbatim.
- Several surfaces, differently. The prediction was "a compositor holding one endpoint per client
  surface needs a driver change and not a contract change". Rung two holds one endpoint for all its
  clients instead. A shared endpoint carries no sender identity, so per-client surfaces are
  identified by *memory* rather than by endpoint. Still no contract change, and still no driver
  change, but by a different route than this note guessed. See notes/compositor.md.

The scanout check grew accordingly: `cargo xtask` now proves two pictures over one boot, the
composed screen first and this rung's pattern second, both on both ISAs.

## What the display terminal did with this contract (milestone 29's text increment)

Written back here on 2026-07-30, for the same reason rung two's section exists: a contract's real
test is what happened when the next thing implemented against it. Nothing in this rung changed.

`crates/graphics_contract` and `components/src/gpu_driver.rs` are byte-for-byte the same again. The
display terminal (`components/src/display_terminal.rs`) takes `painter`'s place at this seam with
exactly `painter`'s authority: a report endpoint, the display endpoint, and the surface frames. It
draws glyphs instead of a coordinate pattern. It calls `FLUSH` with the rectangle of cells that
changed, which is what this note said a client does. The only addition on this side is a kernel
wiring entry point (`display_service::start_terminal`). The only thing the terminal adds over
`painter` belongs to the *terminal* contract rather than this one: an endpoint it serves, and a page
an application writes text into.

So the prediction in the list came out exactly as written, and the seam claim is now made twice in
one milestone. The same binary is also a compositor client, with exactly `window`'s authority, and
`compositor` cannot tell it from the client that painted a pattern either. See notes/glyphs.md.

The scanout check grew accordingly: `cargo xtask` now proves three pictures over one boot, in
order (the composed screen, the terminal's text, then this rung's pattern), on both ISAs.

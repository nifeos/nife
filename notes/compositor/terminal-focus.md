# Focus, cashed: two terminals and one keyboard

*Name: provisional.*

An appendix to [`notes/compositor.md`](../compositor.md), which states the conclusion.

The claim that a terminal is a client of this compositor was cashed on 2026-07-30 (milestone 29 (a
display terminal) and its text increment, notes/glyphs.md). The routing is now visible in the
picture rather than only at the endpoint.
`focus_routes_a_keystroke_to_one_terminals_grid_and_not_its_neighbours` puts two display terminals
side by side and types `A` at the focused one, presses TAB and types `B` at the next. The kernel
compares every pixel of the composed screen against the two VT engines it ran itself. A keystroke
delivered to the wrong client is a wrong picture. Two things came out of it that this note had not
foreseen:

- The producing side of "who may deliver input" got a real driver. `components/src/keyboard_driver.rs` is a confined
  virtio-input driver holding the ring's mapping and the doorbell, and nothing else. It holds no
  client endpoint and cannot name a client, so it cannot influence focus; and the doorbell it rings
  carries nothing, so the ring's mapping really is the whole of its power to type.
- A client must not ring the doorbell in response to input. The compositor is blocked in its
  `CALL` to that client, so a client that answered a keystroke by ringing deadlocks the pair as soon
  as two keystrokes arrive in one drain. It does not need to: this compositor rescans every control
  page on every `COMMIT` from anyone, and the input source rings `COMMIT` itself, so the frame that
  delivers a keystroke is the frame that shows it. That is the blocking-`CALL` cost this note records
  below, met in practice, and the workaround turned out better than the design it ruled out.

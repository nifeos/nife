# Shared-page audit: findings 4, 5 and 7, the compositor

*Name: provisional, minted 2026-10-11 (UTC) by lane/next-ten-longest-notes.*

An appendix to [notes/shared-page-audit.md](../shared-page-audit.md), the tree's second security
audit (2026-08-04). The parent states the lens and its four questions: (a) the window, (b) what can
land in it, (c) the corrupted state, (d) whether it is reachable. Its honest summary has every
finding's disposition.

## 4. Two unchecked arithmetic sites in the compositor's window client

(a) The windows. `fixtures/src/window.rs` reads its geometry out of the control page the compositor
publishes:

```rust
let w = rd32(CTL_VA + ctl::WIDTH);
let h = rd32(CTL_VA + ctl::HEIGHT);
if w == 0 || h == 0 || w * h * 4 > compositor::MAX_SURFACE_BYTES { die(E_GEOMETRY); }
```

`w` and `h` are `u32`. `w * h * 4` wraps in a release build, so `w = h = 0x10000` computes `0` and
passes the check the moment before the paint loop writes `2^32` pixels past a two-frame surface. The
check is written to catch exactly this ("a control page claiming more than that describes memory we
were not granted") and the arithmetic is what lets it through.

The second is in the same file's input loop: `let n = line_editor::proto::len(w0);` and then
`(bytes >> (8 * k))` for `k` in `0..n`, with no clamp. `proto::len` is a full 32-bit field, so
`8 * k` passes 63 at `k == 8`. Every other consumer of `OPERATION_BYTES` clamps to 8
(`display_terminal.rs`, `line_editor.rs`); this is the one that does not.

(b)/(c)/(d) Reachable? No. Both values are written by the compositor, which is the trusted
party in this direction, and it publishes correct geometry and always sends a count of 1. They are
latent, in a test witness program, and they are recorded because a bounds check that its own
arithmetic can defeat is worth naming wherever it appears.

`display_terminal.rs` validates the same geometry by division (`w / GLYPH_W <= MAX_COLS`), which
cannot overflow. That is the shape to copy.

Disposition: fixed in this lane, both, because each is one line and neither can change behavior
on any path the tests exercise.

## 5. The compositor composites surfaces whose owners are not blocked

(a) The window. `components/src/compositor.rs`'s `serve_frame` iterates every committed window, and
`source(i)` builds a `&'static [u32]` over client `i`'s surface. The invariant claimed next to it is:

> The caller is blocked in `CALL` throughout, which is what makes reading a client's pixels safe
> without a lock: the client that rang cannot be writing while we read.

(b) What can land in it. That covers the caller and not the other clients. `paint` reads
every window's surface, and the keyboard driver's `COMMIT` rings the doorbell while no window
client is blocked at all. Clients hold `Flags::user_data()` on their own surfaces and control pages.

The damage rectangle has the same shape one level down: four independent `rd32`s of `DAMAGE_X/Y/W/H`,
which a non-calling client can be mid-way through writing, so the compositor can assemble a rectangle
that never existed. The client's `SEQ` fence orders the *client's* stores and cannot stop the
compositor sampling between them on somebody else's `COMMIT`.

(c)/(d) Reachable, and bounded to tearing. The slice length is `SCENE[i].pixels()`, a
compile-time constant, and every clip uses constant geometry, so no client-supplied value ever
indexes anything. The observable effect is a half-drawn window or a wrong damage rectangle. It is
the same limit already recorded for a capture client reading a mid-composite screen.

Disposition: recorded-accepted, in `notes/compositor.md`'s BUGS section, where a reader meets
the frame protocol. Making it a real guarantee has two routes. One is compositing only the caller's surface, which
breaks the contract, since a `COMMIT` from the input source must repaint everything. The other is
double-buffering per client, which is a design decision and not an audit's to take.

One hardening this audit does recommend and did not take: the compositor holds
`Flags::user_data()` on every client surface and never writes one. Read-only there would make
"the compositor cannot deface a client's window" a mapping rather than a discipline, exactly as
`ROLE_CAPTURE`'s read-only screen already does. It is a one-word change in
`kernel/src/user/compositor_service.rs` with a real behavioral risk if any path does write, so it
belongs with a test that proves the fault, not in an audit's diff.

## 7. The compositor is the only reader in its subsystem with no acquire side

(a) The window. Three writers in this subsystem publish data and then publish an index that
advertises it, each with a fence between and a comment explaining the fence:

- `fixtures/src/window.rs`: writes the damage rectangle, `fence(SeqCst)`, writes `SEQ`. "The sequence
  bump must be visible after the pixels and the rectangle it describes, or the compositor could
  composite a frame we have not finished writing."
- `components/src/display_terminal.rs`: the same, with the same comment.
- `components/src/keyboard_driver.rs`'s `ring_publish`: `fence(SeqCst)`, then writes `TAIL`. "The bytes must be visible
  before the tail that advertises them."

`components/src/compositor.rs` is the reader of all three, and it read with `rd32`, a plain
`read_volatile`, with no fence anywhere. `serve_frame` loads `SEQ` and then loads the four damage
fields and (via `paint`) the pixels. `drain_input` loads `TAIL` and then loads the bytes.

(b) What can land in it. Nothing needs to. `read_volatile` guarantees the access happens and
guarantees no ordering at all, and this kernel runs on aarch64, which is the weakly ordered one
(DECISIONS rule 4). The dependent loads may be satisfied before the load that gates them.

(c) The corrupted state. A fresh sequence beside the previous frame's rectangle or pixels; a
fresh tail beside bytes that are not yet there. Exactly the outcome each producer's comment says its
fence prevents.

(d) Reachable? On real weakly ordered hardware, yes, and it has never been observed, which is
what a memory-ordering bug looks like right up until it is not. QEMU's TCG does not reorder, and
this system has not yet run on a physical board.

The evidence that this is an oversight rather than a judgment is that the kernel's own stand-in
for the input ring gets it right. `kernel/src/user/keyboard_service.rs`'s `take_typed` reads the
tail, then `fence(SeqCst)`, then reads the bytes, with the comment "The tail is published after the
bytes it advertises; read it before them." Two readers of one contract, one fenced.

Disposition: fixed in this lane, two `fence(Acquire)`, matching `clock_contract`'s reader, which is
the in-tree precedent for the acquire side of exactly this pattern.

Relationship to milestone 80. That lane found the same failure shape in the clock page's
seqlock, on the writer's side (the claim is not ordered ahead of the data, and neither `AcqRel`
nor `SeqCst` on the claim fixes it; it needs a release fence). This finding is a different page, a
different pair, and the reader's side, so it is fixed here rather than folded into that lane. The
two together say something neither says alone: this tree writes release-side fences by instinct
and forgets the acquire side, and a comment explaining a one-sided fence reads exactly like a
comment explaining a correct one. That is a lint's worth of pattern, not a bug's.

# The compositor: the scene, the clips and the composite

This appendix to [`notes/compositor.md`](../compositor.md) carries the reasoning behind
`crates/compositor`'s arithmetic. The crate's doc comments state what each item does and link here.

## The scene is fixed

A compositor normally learns its windows from clients that ask for surfaces and from a user who
moves them. This rung's subject is authority routing, not window management, so `SCENE` is a
constant. The compositor, the clients, the kernel test and the host-side scanout check all read
it, which makes the composed screen a value that can be predicted and then checked pixel for pixel.

The geometry is chosen so that a wrong composite cannot pass:

- Three different sizes, so a compositor that assumed one geometry for all surfaces fails.
- Overlap: windows 1 and 2 each cover part of window 0, so stacking order is observable.
- Clipping: window 2 hangs off the left edge with a negative origin. A compositor that clipped
  only against the surface, or only against the screen, produces a different picture. The host
  tests cover the other edges with synthetic windows, so the live scene need not carry every case.

Each window holds a grid of a few `video_terminal` cells (14x26): 5x2, 5x2 and 2x3, each with a
strip no cell covers. The two terminal windows are five columns because their scroller lines
(`video_terminal::script::window_scroll_line`, `wN-MM`) are five characters. A narrower window
wraps them and scrolls the window's own digit out of sight, and the two terminals then show the
same picture.

## Two clips, two bugs

`damage_to_screen` clips a client's damage to its surface, places it, then clips it to the screen.
Without the first clip, a client that reports damage past its own edge makes the compositor read
pixels it does not own. Without the second, a window that hangs off the screen makes the compositor
write outside the framebuffer. The client's number is untrusted input to the first clip, which is
why it is a clip and not an assertion.

`Rect` is half-open: a rectangle covers `x .. right()`. Every classic clipping off-by-one is a
confusion between that edge and the last column. `right()` and `bottom()` saturate because a damage
rectangle is read from a client's control page as `(i32, i32, u32, u32)` and reaches them before any
clip. With `x = i32::MAX, w = 1`, the sum `x + w` passes `i32::MAX`, and `w as i32` is negative for
any `w` above `i32::MAX`.

## The composite

`composite` is the painter's algorithm: background first, then each window in stacking order, each
clipped to the damage. Windows are opaque, so blending is a copy and the last writer wins, which is
what makes the stacking order observable in the result.

An empty source is a window that is not on screen yet. The compositor passes `&[]` for a client that
has never committed, so the first paint, before any client has run, is the background alone rather
than a screen of somebody's uninitialized frames.

Every pixel outside the damage is left exactly as it was, which is the point of a damage rectangle.
The host tests and the kernel test both check it by poisoning the rest of the screen and finding the
poison intact afterwards ([proofs.md](proofs.md)).

`expected_screen_pixel` is the independent definition of the composed screen. `composite` paints up
the stack and it looks down the stack, so the two agree only if both are right.

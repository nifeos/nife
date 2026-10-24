# A window can tear too

*Name: provisional.*

An appendix to [`notes/compositor.md`](../compositor.md), whose limits list states the conclusion.

A window can tear too, and the reason is sharper than the screenshot's (milestone 43 (a second
security audit), notes/shared-page-audit.md finding 5). `serve_frame` composites every committed
window, not only the one that rang, and the input source's `COMMIT` arrives when no window client is
blocked at all. So the invariant written next to `source()` ("the client that rang cannot be writing
while we read") covers the caller and nobody else: a client that did not ring is runnable and holds
its surface and its control page read-write throughout. The damage rectangle is four independent
32-bit loads, so it can be sampled mid-write and describe a rectangle that never existed. The
client's `SEQ` fence orders that client's stores and cannot stop the compositor sampling between
them on somebody else's frame.

For reads it is bounded to tearing. Until milestone 719 (compositor confinement claim 25) a
rectangle could also panic the compositor through `Rect::right`; see
[compositor-claim-25.md](../compositor-claim-25.md). Every slice length and every clip comes from
`compositor::SCENE`, a compile-time constant, so no client-supplied value indexes anything. The
worst read is a half-drawn window, never a pixel outside a surface. A
guarantee means per-client double buffering, a design decision this note does not take.

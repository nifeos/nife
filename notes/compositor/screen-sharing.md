# The capture client is the screen-sharing case

*Name: provisional.*

An appendix to [`notes/compositor.md`](../compositor.md), which states the conclusion.

A capture client shows the other side of the same coin. It holds a read-only mapping of the screen
and of the window list. So it can screenshot and enumerate with no server involved and no verb to
call, and its attempt to *write* the screen faults. A thing that may look at the screen may not draw
on it.

That client is also the screen-sharing case, not a separate mechanism waiting to be built. It is an
ordinary window client, with its own surface and no special relationship to the compositor. It was
*additionally* granted the screen read-only in its spawn literal. That is exactly the shape of a
screen-sharing app or a recorder, and exactly the authority such a thing needs and no more. The
three items the roadmap asks for (enumeration, screenshots, screen sharing) are therefore one grant
with three uses rather than three features. That is the point of building on mappings instead of
verbs. And because the grant is a frame mapping, it is revocable through the machinery milestone 13
(capability revocation) already built (`PageFrame::REVOKE`), rather than by asking a server to stop
honoring a request.
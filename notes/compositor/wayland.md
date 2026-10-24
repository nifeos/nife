# The compositor and Wayland

*Name: provisional.*

An appendix to [`notes/compositor.md`](../compositor.md), which states the conclusion.

That is the difference in kind from Wayland, and worth stating precisely because Wayland is the prior
art and is often described as if it had already solved this. A Wayland compositor holds a socket per
client and every request arrives on it *with the client's identity attached by the transport*; the
compositor then decides, in code, what that client may do. Its security properties are properties of
that code. Here the compositor holds a capability per client too (an input endpoint), but the
authority to *read* anything is a mapping the kernel made at spawn, and no code path in the compositor
can widen it. Wayland's model approximates capability routing; this is capability routing.

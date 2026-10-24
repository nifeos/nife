# The primitive this rung wants: what would change

*Name: provisional, after the main page's own heading.*

An appendix to [`notes/compositor.md`](../compositor.md), which states the conclusion.

What would change if the primitive existed, in either of its two forms (a wait-any / poll on several
endpoints, or threads sharing an address space):

- the compositor could hold one endpoint per client and get unforgeable sender identity for free,
  which would let a reply carry per-client data and let a bad damage rectangle be *refused* to its
  author rather than clipped;
- a screenshot could be a served request that copies a consistent snapshot into the requester's
  buffer, instead of a live read-only mapping that can be read mid-composite (see the limits below);
- input delivery could stop being a blocking `CALL` into a client, which is today the one place a
  misbehaving client can stall the compositor.

# The same shape in another medium

*Name: provisional. Named after the main page's own heading.*

An appendix to [`notes/citations.md`](../citations.md), which states the conclusion.

Worth naming, because it says what the general defect is and this gate only covers part of it.

Milestone 112 (the SAFETY comments that bind nobody) found a `// SAFETY:` comment in
`components/src/virtio_net_transport.rs` whose paragraph described `invoke` and capability validation, sitting
above a `write_volatile` into a DMA page. It described a different operation entirely, and it passed
`clippy::undocumented_unsafe_blocks` for as long as the file existed, because that lint asks whether
a comment is *present*, never whether it is *about the thing underneath it*.

That is the same failure as `milestone 24` resolving to a board: a reference that is well-formed and
wrong, sitting under a check that only asks whether it resolves at all. A comment that names its
subject can be compared against it; one that does not is unverifiable by construction. Every gate
in this file is an instance of that one idea.

SAFETY comments are milestone 112's territory and not this script's, deliberately. The point of
recording it here is that the citation gate is one member of a family, and whoever adds the next
member should recognize the shape rather than rediscover it.

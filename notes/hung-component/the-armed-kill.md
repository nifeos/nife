# The hung component: the kill that a blocked thread never spends

*Name: provisional.*

An appendix to [`notes/hung-component.md`](../hung-component.md), whose taxonomy names case (c): a
thread blocked on an endpoint its supervisor cannot reach. This is the mechanism that made case (c)
impossible to tear down when the note was written. Milestone 133 (ending a permanently blocked
thread, and deciding who may) closed it on 2026-09-04, when the region holder's `DESTROY` began
finishing a `Blocked` resident in place.

Case (c) is not speculation and it is not a gap somebody should close casually.
`reap_region_objects` says it in its own words, in the comment added on 2026-08-16 that fixed the
(b) case:

> A server whose endpoints came out of the region being destroyed dies; one blocked on somebody
> else's endpoint still does not, and `reclaim_region`'s caller is told so by the refusal rather
> than by a hang.

The mechanism is worth spelling out because it is the load-bearing fact for question 4 in the
parent. `MemoryRegion::DESTROY` on a region holding a live thread does not fail passively: it marks the
thread `killed` and refuses, so the owner's retry reclaims a runaway. That is the amendment to §16
(object revocation), and the forcible `^C` of §24 (interrupting the foreground process). The kill is
spent in `schedule()`, at the top of the decision, and only for a thread whose state is `Running`. A
thread that is `Blocked` forever never reaches `schedule()` again. So the kill is armed and never
lands, the refusal is permanent, and the retry loop the shell's escalation performs runs forever.

Which is why the "stronger right" §32 points at was, when written, insufficient for case (c).

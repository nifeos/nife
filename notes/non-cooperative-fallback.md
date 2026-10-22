# The non-cooperative fallback for a dependent

*A proposal from the lane for milestone 23 (a capability-routed component OS with live
replacement), 2026-09-26. Status: **BUILT** 2026-09-27, after calef ruled it on 2026-09-26 (DECISIONS §231 (a swap's warning to a dependent is advisory, and the supervisor never waits for it)). The measurement behind it is
built and gated: `swapper`'s `ROLE_UNWARNED` and the guest test
`a_dependent_that_is_never_warned_loses_nothing_and_only_waits`. Read
notes/dependency-orchestration.md and notes/hung-component.md first.*

## The question as the block states it

`capability_plan::dependents` names who must be warned before a contract is swapped. On the one edge
this tree has, the warning is `broker`'s `BOP_DOWN`: stop calling through to the backend and start
buffering. The block's open question is what a supervisor does when the dependent it must warn does
not answer. It files that as "the same open decision one level out" as the hung component's two:
how a supervisor notices a hang, and what it may do to a component that never cooperates.

## The premise is half wrong, and the half that is wrong is measured

**A dependent's warning is an availability courtesy, not a correctness step.** A dependent is only
ever a component that forwards synchronously while serving others (`Requirements::depends_on`'s own
rule). If it is not warned, its forwarded `CALL` to the swapped contract parks on that endpoint's
sender queue for the down window, and the replacement drains it. That is §41 (the endpoint is the
broker, and a device is revoked by taking it back) doing for the dependent exactly what it already
does for a pure consumer. Its own callers wait for the down window; nothing is lost.

`ROLE_UNWARNED` runs the queued system with the graph still naming `broker` and the operator never
sending `BOP_DOWN`. The producer's verdict is clean (every request answered, right, in order, none
refused), the witness page shows no gap and one version change, and `WAS_BUFFERED` is zero, which
proves the warning really was absent. So **proceeding unwarned costs latency, not work**, and the
latency is bounded by the down window.

That removes one of the two decisions from this question entirely. **Nothing has to be done *to* an
unwarnable dependent**: the fallback does not need §32 (a supervisor may collect a corpse) widened,
a kill, or any authority the operator lacks. What remains looks like the detection half, how long a
supervisor waits for an acknowledgement, and the options below show it need not wait at all.

## The defect that makes the remaining half urgent

The warning is a `CALL` (`swapper.rs`, `queued()`). A dependent that does not answer does not just
go unwarned: it hangs the supervisor, which is now a caller stranded mid-`CALL` and, per
notes/hung-component.md's question 3, unwakeable and unreclaimable. So today one hung dependent
converts a routine swap into a hung operator. The only supervisor that warns anyone is the test
operator, so nothing shipped is exposed, which is why this is a `BUGS` entry
below and a comment at the `CALL` in `swapper.rs`, and not an emergency.

## Options

| | option | needs | verdict |
|---|---|---|---|
| B | Warn with a signal, never wait: the supervisor writes the wanted state (down or up) on a read-only page it shares with the dependent and signals a notification bound to the dependent's thread | milestone 151 (notification objects: async multiplexing without wait-any), `SIGNAL` and `BIND`, already specified by §101 (notification objects: async multiplexing without wait-any) | recommended |
| A | Warn with a `CALL` bounded by a deadline; on expiry, proceed unwarned | a deadline on `Rendezvous::CALL`, which is one of the three shapes of milestone 106 (a wait that ends on either the interrupt or the deadline), and not the direction calef set on 2026-09-05 (a userspace timer service signaling a notification) | second choice: a notification-based timed wait bounds a *wait*, not an outstanding `CALL`, so the directed shape does not serve this consumer |
| C | Treat an unwarnable dependent as hung and replace it first, with `ROLE_HUNG`'s recipe | detection, plus a fresh region per hang with the old one stranded | strictly more expensive for the same information, and recursive: the dependent's own dependents then need warning |
| D | Give the supervisor a right to force the dependent | a new decision under §32 | refuted by notes/hung-component.md: a permanently blocked thread never reaches `schedule()`, so the kill a destroy arms is never spent. That reason was already stale when written: milestone 133 (ending a permanently blocked thread, and deciding who may) made the region holder's `DESTROY` finish a blocked thread on 2026-09-04 (corrected 2026-10-08). D still loses, because `ROLE_UNWARNED` measured that an unwarned dependent loses nothing, so there is nothing to force |
| E | A helper process makes the blocking `CALL` so the operator never does | nothing new | the helper is stranded instead, costing an unreclaimable region per hang to avoid a primitive §101 already decided |

Why B is safe in every ordering, which is what the measurement buys. A signal can land late: the
backend may already be gone, or already replaced, before the dependent looks at the page. Every such
ordering degrades to some mix of pass-through and buffering, and pass-through across a swap is the
case `ROLE_UNWARNED` measured as lossless. So the supervisor needs no acknowledgement, and needing
none is what lets it never block. The state page rather than a bare signal is there because §101's
first notification has no badges (milestone 151 puts them out of scope), so one signal cannot say
"down" versus "up"; the page says it, and the signal says "look". That is the tree's own idiom for
moving a fact one way: the clock page and §111 (inert configuration is a read-only page)'s page.

What the tree already does in the analogous case (question 2): the hung-component work found the
drain step redundant against a component that is not receiving, and proceeded without it. B is the
same move one edge out: the step that needed cooperation stops needing it, and the stable endpoint
absorbs the difference.

Prior art (question 3). Erlang/OTP's supervisor `shutdown` child spec, read 2026-09-26 at
erlang.org/doc/apps/stdlib/supervisor.html: with an integer timeout the supervisor sends `shutdown`
and waits; "if no exit signal is received within the specified number of milliseconds, the child
process is unconditionally terminated". That is option A with a kill on the end. Kubernetes'
`terminationGracePeriodSeconds` and systemd's `TimeoutStopSec` have the same shape (recalled, not
re-read). seL4's bound notification, which §101 adopts, is B's mechanism. The difference from all of
them is that nife's fallback needs no kill and no deadline, because the measurement says an unwarned
dependent loses nothing.

Cost (question 5). B: milestone 151, which is owed anyway (§101's retrofit table names the
compositor, the file server, the network stack and the shell's exit wait), plus one page and one
notification per dependent. A: a kernel-surface change to `CALL` that the architect has already
steered away from. The correctness cost of the unwarned path is measured above as zero. Its latency
cost is the down window, not measured in time: the guest test asserts program order, never
elapsed time, on purpose (notes/hung-component.md).

Reversibility (question 6). Reversible until a second forwarding dependent exists. It retires
`BOP_DOWN`/`BOP_UP` as `CALL` opcodes in `hot_swap_contract`, a contract two programs in the swap suite
speak and nothing else does.

Would we choose B at equal cost (question 7)? Yes, and it is also the cheaper one given that 151
is owed. A is the option that makes the supervisor wait for something it has just measured it does
not need.

## The recommendation

**Rule that a dependent's warning is advisory and must never block the supervisor: signal it through
a bound notification plus a state page, and swap without waiting for an answer.** Build it after
milestone 151. Tracked as `design/roadmap/0682-warn-a-dependent-without-blocking.md`. Until then, record the stranded-operator defect as a known limit and do not build a
workaround (E).

What is blocked until this is answered: nothing in milestone 23 beyond the one `Outstanding`
line in its block. If the answer is no, the supervisor keeps a blocking warn, which is safe for
every dependent that is alive and hangs the operator on one that is not.

What this does not decide. What may be done to a component that never cooperates stays open,
and stays §32's. This proposal only shows the dependent case does not need it.

## Built (2026-09-27)

The recommendation, as ruled. `hot_swap_contract::BROKER` declares a read-only warning page;
`swapper` writes `WANT_DOWN` or `WANT_UP` on it and signals a notification bound to `broker`'s
thread before it runs, and never waits. `broker` reads the page on every wakeup, message or
signal, so a signal it has not seen changes only latency. `BOP_DOWN` and `BOP_UP` are gone. Three
guest tests cover the orderings on all three architectures: on time (the queued channel, which
still buffers and drains everything it buffered), never (`ROLE_UNWARNED`), and late
(`ROLE_LATE_WARNING`, warned only after the replacement is serving), and none loses a request.

## BUGS

~~A dependent that does not answer its warning hangs the supervisor.~~ Closed 2026-09-27: the
warning is a page and a signal, and `swapper` never waits for either (see "Built" above).

The steady-state price moved and is not measured. `broker` now reads its warning page on every
wakeup, one load per request. `broker_rtt` models the broker as a kernel thread, so it does not see
that load; a benchmark of the real program would.

B leaves the dependent's callers waiting for the whole down window when the signal is late. That
is latency, and it is the price of never blocking the supervisor. A late signal cannot be detected
by the supervisor under B, by construction.

The measurement covers one edge shape. `broker` is the only forwarding dependent in the tree.
Consider a dependent that holds its own caller's reply capability across the forwarded call, rather
than answering from the forward's return as `broker` does. It would strand its caller for the down
window in the same way, and still lose nothing. That is argued, not run.

## See also

- notes/dependency-orchestration.md, notes/hung-component.md, notes/state-handoff.md
- design/roadmap/0106-deadline-wait.md and design/roadmap/0263-can-a-timer-be-a-capability.md
- design/roadmap/0151-notification-objects.md, the recommended option's prerequisite

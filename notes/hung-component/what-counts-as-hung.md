# The hung component: what counts as hung, argued

*Name: provisional. Named after the parent's question 2.*

An appendix to [`notes/hung-component.md`](../hung-component.md), whose question 2 states the
answer: the bound is held by the supervisor. This is the argument, owner by owner, for what a bound
should be denominated in, and where the built one, a duration on the swap's quiesce, sits against it.

Take the three candidate owners in turn, because the manifest answer is genuinely attractive and it
is wrong.

The manifest's, and no. `capability_plan::Requirements` is where a deadline looks like it belongs:
one declaration, shipped with the contract, read by every supervisor. It fails on the tree's own
evidence. A contract cannot know the machine. The same `OPERATION_PUT` under QEMU TCG, under HVF,
and on a VisionFive 2 differ by orders of magnitude (notes/cpu-models.md, the matrix of milestone 59
(the CPU-model matrix)). A wall-clock number compiled into a `*_proto` crate would be a shipped
version of exactly the load-sensitive assertion milestones 62 and 78 exist to remove. It also fails
the manifest's own test: notes/component-manifest.md's line is that a manifest declares authority,
and a deadline is not authority, it is a service-level claim about a machine the declaration has
never met. Fuchsia does not put timeouts in a `.cm` either.

The client's, and not alone. The client is the party harmed, which is a real argument for its
deadline being the one that matters. But a client cannot act on it, so a client deadline is only
ever a report. Worse, a client on the default rung of the latency ladder is *blocked* by the time it
would like to give up: a `CALL` takes no deadline, so it has no moment at which to notice.

The supervisor's, denominated in progress. This is milestone 62 (tests)'s argument, one level out,
and the fact that this tree has already made it about its own test harness is why it is a
recommendation rather than a preference. 62's block:

> The heartbeat is bumped once per test, at the test's start, and never while a test runs. So "no
> progress for 60 s" cannot distinguish a genuine deadlock from a test that is simply *slower* than
> 60 s. [...] The fix: a per-test *progress* heartbeat [...] so a slow test keeps the watchdog fed
> and a wedged one does not.

And its closing line, which is the general form: *"a bound expressed in something other than the
property under test."* A duration is that. A monotone count of work completed is not.

Concretely, and it needs nothing that does not exist: a component publishes a monotone progress
counter in a page its supervisor owns. §41 (the endpoint is the broker, and a device is revoked by taking it back) already has that page, in milestone 23 (a capability-routed component OS with live replacement)'s system. `hot_swap_contract::LOG_VA` is a frame
the operator retyped from its own budget and mapped read/write into every instance, and every
instance stamps it as it serves. Reading it costs the watcher zero syscalls and the component one
store. A verdict is then: *work is owed (a request went in and no counter moved) and the counter has
not moved across k observations.* No clock is consulted, and the number k is the supervisor's, set
by somebody who knows the machine it is running on.

Two honest costs, and the first is the one to design against:

- The counter's granularity is the false-positive rate. A component that bumps once per request
  looks hung throughout a single legitimately long request. One that bumps inside its work loop does
  not. This is 62's own finding and it does not go away by moving up a level; it becomes a property of
  the contract, which is at least a property somebody writes down.
- Separating k observations needs the watcher to wait, which milestone 106 (a wait that ends on
  either the interrupt or the deadline)'s timed wait now provides: a `Timer` signaling a
  notification bound to the watcher's thread.

## What is built, and how it departs from this

`swapper`'s operator bounds one thing, its `OPERATION_QUIESCE`, and in time: `QUIESCE_BOUND_MS`, 250
ms, chosen as about forty times the slowest healthy answer measured on three ISAs under a loaded
host (the measurements are under "How the quiesce bound was measured" below). It reads no progress counter. Two things make the
duration acceptable for this one question where the argument above rejects it in general. The
quiesce is owed work of known size. It waits in the endpoint's FIFO behind at most the requests
already queued, one per client here, so the bound covers one answer plus that queue rather than a
component's whole workload. And the question is asked only at a swap, by a supervisor about to
replace the component anyway, so a false conviction costs a replacement the operator had already
decided on, plus a `Gone` to a caller mid-request.

The departure is real, and a component with a long queue ahead of the quiesce or slow requests
would meet it: its quiesce could miss a time bound while its counter still moved. The progress
counter above is the answer for that component, and for any watch that runs between swaps, where
neither condition holds. Neither is built.

A citation to fix while passing through, because it is the exact failure `script/decisions
--check` cannot catch: milestone 106's block attributes the fork's three candidate shapes to
"DECISIONS §51 (sink)" twice, and §51 is *the sink protocol*. The three shapes are in milestone 51
(wall)'s roadmap block (`design/roadmap/0051-wall-clock-time.md`, its rejected-alternatives list).
§N and milestone N are colliding schemes, the gate proves only that a cited §N resolves to *some*
section, and a well-formed wrong citation is invisible to it. Corrected in 106's block by this lane.

## How the quiesce bound was measured

A healthy incumbent's answer to `OPERATION_QUIESCE` was timed the way `quiesce_within` in
`components/src/swapper.rs` times it: from starting the stand-in to receiving its forwarded answer.
That includes the one client request the quiesce can queue behind. Ten runs per leg, on 2026-10-18
(UTC), under `script/test`, on a 16 GB Apple Silicon host with a load average of 25 to 34 from other
lanes' builds:

| ISA | counter | fastest | slowest |
|---|---|---|---|
| aarch64 (4 cores) | 62.5 MHz | 1.2 ms | 3.4 ms |
| riscv64 (4 cores) | 10 MHz | 1.2 ms | 2.4 ms |
| `x86_64` (2 cores, and the 1-core leg) | TSC, about 1 GHz | 1.1 ms | 6.5 ms |

`QUIESCE_BOUND_MS` is 250 ms, about 38 times the slowest of the forty, and 25 scheduler ticks. The
deadline fires at the first 10 ms tick at or after it (milestone 106's `BUGS`), so the real wait is
250 to 260 ms, and the hung test prints what it was.

This is wall time under QEMU, not instructions. `script/test` runs TCG without `-icount` (only
`script/bench` and `script/icount` pass it), so the guest counter follows the host clock and a
loaded host stretches a healthy answer. The margin is for that. The bound is also this fixture's
alone: its incumbent answers a request with eight multiplications, and a component whose requests
take longer needs a bound measured against it. A bound that convicts a healthy component destroys
it mid-work, and the caller it was serving is answered `Gone` and asks again, which is safe only for
a request that may be repeated. A bound that is too long costs only a longer down window.

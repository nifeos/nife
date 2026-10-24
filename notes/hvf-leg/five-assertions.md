# The HVF leg: five assertions about "has it happened yet"

*Name: provisional. After the parent's own heading.*

An appendix to [`notes/hvf-leg.md`](../hvf-leg.md). It records the five tests the physical core
found, from milestone 81 (an HVF leg: the test suite on the physical core) on 2026-08-04, how each
was fixed, and the settle windows the leg makes weaker rather than flakier.

## What the physical core found

Every failure belonged to the milestone-78 family (`notes/load-sensitive-assertions.md`), and every
one was found from the opposite direction. That note's diagnostic is that a slow machine produces a
deficit. HVF is a *fast* machine, and it produces the same deficit for the mirror-image reason. A
yield count is not a duration in either direction. On a loaded host this core burns cheap yields
while another core is descheduled. On the physical core it burns them in nanoseconds while another
core has not been dispatched yet.

They arrived one per run, over five runs, so the leg is a *sampler* of this defect, not a detector
of it. The exception was one fixed on inspection, because it had the same shape a few lines from a
failing sibling.

### The revoked waiter

`sched::a_blocked_waiter_wakes_with_an_error_when_its_endpoint_is_revoked` failed with "the revoked
waiter never woke". One `yield_now()` was the entire wait for the waiter to block on the endpoint.
The comment said "single core: one yield lets the waiter run", which stopped being true when
DECISIONS §28 (SMP placement) scattered placement across cores. Under HVF the reclaim ran before the
waiter had ever been scheduled, so there was nothing to wake.

The test now waits, on the clock, for the endpoint's receive queue to actually hold the waiter. That
needed a new `#[cfg(test)] sched::endpoint_waiting_receivers` beside the existing
`endpoint_waiting_senders`. Its second wait (50 yields for `WOKE`) is now `wait_for` too.

### The worker that made no progress

`sched::other_threads_run_while_one_is_blocked` failed with "a worker made no progress while another
thread was blocked on IPC". It spent 100 yields on this core, then asserted the worker on another
core had incremented a counter. It is now `wait_for(|| PROGRESS > 0)`.

Nothing is weakened. If a blocked thread were requeued and starved the worker, the counter stays at
zero for the full two seconds. The assertion then fails with the message it always had. Its teardown
also changed from 20 yields to waiting for both of its threads to be gone. This test's late-landing
teardown is the neighboring state that made *other* tests' accounting fail in milestone 78
(load-sensitive assertions).

### The corpse that never parked

`user::reap_tests::reaping_an_uncollected_corpse_leaves_no_ghost_on_the_endpoint` failed with "the
corpse never parked on its supervision endpoint". It spent 4000 yields waiting for a dying child on
another core to park its death message. This one passed three HVF runs and failed the fourth, which
is worth stating: the leg samples this defect, it does not detect it. It is now
`wait_for(|| endpoint_waiting_senders(fault_ep) == 1)`.

`user::supervision_tests`'s reclaim after a respawn had the same shape: 2000 yields and no assertion
at all. So a timeout silently left an unreclaimed region for a neighbor to trip over. It was not a
failure. It was fixed on inspection, and it now asserts.

### The spinner that never ran

`sched::a_thread_that_never_yields_is_preempted_anyway` failed with "the spinner never ran at all".
It is not a yield count. It is the most interesting of the five, because it is a vacuity guard that
the faster machine turned into a failure.

The test spawns a hostile spinner and a polite thread on the same core, and waits for the polite one
to report. It sets `STOP`, and then checks the spinner had spun at least once. A polite thread
running on a core nobody was monopolizing proves nothing about preemption. Which of the two runs
first is a race, and on the physical core it came out the other way. The polite thread finished,
`STOP` was set, and the spinner was killed before its first increment. It now waits (its own bounded
second) for the spinner to have spun before stopping it. So the guard fires on a genuinely dead
spinner rather than on a scheduling order.

### Shared, and what was not found

`user::tests::wait_for` became `pub(super)` so the two `user` modules could use the existing
implementation instead of becoming the sixth and seventh copies of it.

None of these was a timer-behavior failure. That is the finding worth stating plainly, because
timers were what the milestone expected to be perturbed.

## The settle windows the leg makes weaker, not flakier

Three sites spend 400 yields to "let it settle" and then assert that nothing more happened:
`user::c_seam_tests` line ~122, `user::authority_tests` ~121 and `user::live_swap_tests` ~185, each
asserting `endpoint_waiting_senders(report) == 0`. These do not fail under HVF and will not. A
shorter real settle window makes a negative assertion easier to pass, so the leg quietly proves
*less* there rather than going red.

They are left alone deliberately. Turning a settle window into a wall-clock duration is a design
question (how long is "nothing more happened"?), and not the mechanical fix the four above took.
They are recorded here so the next reader does not mistake their silence for a clean bill.

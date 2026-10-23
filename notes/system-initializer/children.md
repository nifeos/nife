# The system initializer's builders after the boot

An appendix of [the system initializer note](../system-initializer.md): how the spawn service builds
caretakers and `graphical_terminal` sessions, and how it takes their memory back.

A directory grant is delivered by a caretaker built in the client's own region, together with the
two endpoints between them. The caretaker's serve loop never returns, so a region of its own would
never come home, and §16's LIFO rule would pin every region above it. Inside the client's region,
the client's one reap ends both. The endpoints must come from that region too:
`sched::reap_region_objects` sweeps a region's endpoints before its threads, and that sweep is what
wakes a caretaker parked in `RECEIVE`.

The caretaker is built with no fault slot. Its region is collected by the client's reap, and a
second death message for a thread that reap already took would have `job_undertaker` collect a tid
the scheduler no longer knows, which it traps on. A `graphical_terminal` session's drivers and line
discipline are unsupervised for the same reason; the session program alone is born on `deaths`, and
its reap ends them all.

The progenitor has no second thread, so a receive that never completes is a prompt that never
takes another command. The caretaker's readiness endpoint is what bounds the build: it answers
`DESCENT_REFUSED` rather than trapping, so a mistyped directory costs a refusal. A session's
one-time reports are taken in the order the drivers send them (`UP`, `TERM_UP`, the driver's
`FLUSHED`, `KEYBOARD_UP`), because a report nobody receives parks its sender for ever.

`reclaim` retries a region's destroy, because a destroy of a region holding a live thread is
refused with §16's kill armed and one preemption later succeeds. It is bounded and silent where
`job_undertaker` traps: this is the spawn service, and a trap here would take the prompt down over
one command's memory. A session that could not come up is a refused spawn for the same reason, never
a trap.

A session without a keyboard reads the boot's line discipline through a copy badged
`proto::RAW_ONLY_BADGE`. The discipline answers only raw-mode operations on that badge, so the
session cannot `OPERATION_BYTES` a line that the boot shell would then run with its own authority.

## How many times `reclaim` retries

`RECLAIM_ATTEMPTS` is sixty-four yields, and that count does not reliably cover one preemption.
`login_test_client` carried the same loop over the same refusal and measured it on 2026-08-27 (UTC);
see its `destroy_with_retry` and [load-sensitive-assertions.md](../load-sensitive-assertions.md). A
`yield_now` that finds work on its core returns in about 130 us, so sixty-four can pass in 8 ms,
while the preemption being waited for is a 10 ms tick away. At about twice oversubscribed, the count
stopped covering the tick in about one run in three. That client now waits on the property with a
clock. Here the failure has never been observed, and it would be invisible: `reclaim` gives up
quietly and strands `DIR_JOB_REGION_PAGES` until the machine stops. The fix is the same
clock-bounded wait and a way to notice when it expires.

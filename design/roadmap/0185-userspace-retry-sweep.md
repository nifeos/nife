---
status: NOT-STARTED
raised: 2026-08-27
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 185. Sweep userspace's bounded retry loops onto a clock

Four bounded retry loops in userspace count attempts instead of watching a clock. They are siblings
of `fixtures/src/login_test_client.rs`'s `destroy_with_retry`, over the same refusal. That one was a
fixed 64-attempt loop. It gave up before the tick it waited for arrived, at roughly 2x host
oversubscription. See notes/load-sensitive-assertions/caretaker-teardown-wait.md, "The disposition,
2026-08-28".

The shape is already built once, in the same lane's fix to `login_test_client.rs`:
wait on the property (the region actually coming down) with a clock-bounded watchdog rather than a
syscall count. This milestone repeats that pattern at four more sites; it is not a design fork.

## What this is, in brief

DECISIONS §16 (object revocation) says a `MemoryRegion::DESTROY` on a region that still holds a
live thread is refused. The refusal *arms* a kill on that thread that lands at its next
preemption. That is a timer tick away (10 ms at 100 Hz, milestone 62's own unit). Five places in
userspace wait out that refusal with a fixed count of attempts, yielding between each. None of
them has a clock in it. A count of syscalls is not a way to buy a tick: `yield_now` returns in
about 130 microseconds when this core has other work and parks until the next tick when it does
not. Which kind a given call gets is a scheduling outcome the host decides, not something the
loop controls.

The measurement that settles it is in `notes/load-sensitive-assertions/caretaker-teardown-wait.md`,
"The measurement, and the number that settles it". Across four logouts in one run, the **failing**
wait was the **shortest** one in the table (64 attempts, 8.26 ms, gave up), while a **passing** wait
in the same run took 39.3 ms over only 2 attempts. Attempts and elapsed time are not the same axis.
A loop bounded on the wrong one fails exactly when the host is busiest. That is the condition
this project's own CI and merge-queue runners sit in whenever more than one lane gates at once.

## The five sites, verified against `main` on 2026-08-27

All five wait on the same refusal (`sched::reap_region_objects`/`reclaim_region` under load,
`§16`'s kill armed and not yet delivered) and none has a clock. The split that matters is what
happens when the count runs out:

| site | function | constant | on exhaustion |
|---|---|---|---|
| `crates/system_initializer/src/children.rs` | `reclaim` | `RECLAIM_ATTEMPTS` (64) | returns; strands the region's pages until the machine stops |
| `components/src/login.rs` | `reclaim` | `RECLAIM_ATTEMPTS` (64) | returns; strands the region's pages |
| `components/src/swish/running.rs` | `await_screen` | `SCREEN_REAP_ATTEMPTS` (1024) | returns; leaks one job's worth of init's pool |
| `components/src/job_undertaker.rs` | `collect` | `MAX_ATTEMPTS` (1024) | **`user_rt::trap()`**, taking the process down |
| `components/src/timetable.rs` | `collect` | `REAP_ATTEMPTS` (1024) | **`user_rt::trap()`**, taking the process down |

**The two that trap are why this is a milestone and not a tidy-up.** Under load, `job_undertaker`
and `timetable` do not degrade, they kill the process reaping a corpse, which is a louder failure
than the thing the loop was written to guard against. `job_undertaker`'s own comment argues that
"one preemption is enough and this yields between attempts," which is correct about the mechanism.
It does not argue that a thousand yields reliably covers that one preemption. No measurement of
this specific site backs that assumption the way `login_test_client.rs`'s A/B backs the claim that
sixty-four did not. Not measured here, deliberately: the two trapping sites are at sixteen times
`login_test_client`'s old attempt count, which buys margin but not, per that A/B, a guarantee.

`crates/system_initializer`'s site already carries a BUGS entry recording this exact measurement
against itself (added by PR #562's lane, on `RECLAIM_ATTEMPTS`): "one preemption is enough and
sixty-four attempts do not reliably buy one." That entry is the clearest single argument for this
milestone and should not need re-deriving; it is reproduced here because a BUGS section is read at
the call site, not at the roadmap.

## Why the fix is a clock, not a bigger count

A bigger count trades one failure mode for a slower version of the same one. `job_undertaker` and
`timetable` are already at 1024 attempts (16x `login_test_client`'s old 64). The measurement
above shows attempt count does not track elapsed time at all, so no fixed number is safe against an
arbitrarily loaded host. The fix that already shipped once, in `login_test_client.rs`'s
`destroy_with_retry`, is the pattern to repeat: wait on the property (`DESTROY`/`reap` actually
succeeding) with a watchdog denominated in counter time, not attempts. The watchdog is what turns
a wedged wait from a silent hang or, worse, a trap, into a bounded, diagnosable failure. A genuinely
stuck caretaker still gets reported, at the ceiling, instead of being indistinguishable from an
ordinary loaded run that just needed a few more milliseconds.

## The constraint that shapes the work: no delivered-tick counter in userspace

Milestone 62 re-denominated the kernel's own equivalent waits in delivered timer ticks rather
than wall clock, specifically because a tick-counted deadline cannot be fooled by a descheduled
guest the way a wall-clock one can. That unit is not reachable from a process. Nothing publishes
the kernel's per-core tick count across the syscall boundary. A userspace loop can only read the
raw counter (`user_rt::now()`, backed by `CNTVCT_EL0` on aarch64, `rdtime` on riscv64, `rdtsc` on
x86_64) and `user_rt::cntfrq()` to convert it. `login_test_client.rs`'s fix already documents this
gap in its own BUGS section rather than pretending the wall clock is the right unit. The four sites
here inherit the same limitation and should record it the same way, once each.

**This is recorded as the open question, not as a plan.** Giving a process a delivered-tick reading
would be a syscall ABI addition (an `svc` returning what the kernel currently keeps to itself in
`TICKS`/`PerCpu`). The syscall surface is a boundary calef decides, not a lane's call to make
while fixing five retry loops. Until that exists, safety here comes from margin (a watchdog wide
enough that the wait never legitimately approaches it) rather than from the right unit. That is
exactly the trade `login_test_client.rs`'s own BUGS section names for itself.

## What a lane taking this milestone should do

1. Port `destroy_with_retry`'s shape to each of the four remaining sites. Replace the fixed-count
    loop with a wait on the actual property (`DESTROY`/`reap` returning success) bounded by a
    clock-based watchdog (`user_rt::now()` against `user_rt::cntfrq()`), sized with the same kind of
    measurement `login_test_client.rs` did rather than picked by feel.
2. Decide per site what the watchdog's own exhaustion should do. The three that return today may
   reasonably keep returning (a stranded region is a slow leak, not a correctness failure). A
   clock-bounded return is still a stronger diagnostic than a silent one: it can report how long it
   waited, the way `login_test_client.rs`'s failure message now does. The two that trap need their
   own judgment call about whether trapping is still right once the wait is denominated correctly.
   That is engineering within this milestone's gate, not a fork requiring calef's sign-off, but
   worth writing down per site rather than assumed uniform.
3. Record the delivered-tick gap in each touched constant's own BUGS section, the way
   `crates/system_initializer`'s `RECLAIM_ATTEMPTS` already does, rather than leaving it only here.

## What this does not decide

Whether userspace ever gets a delivered-tick counter of its own. That is a syscall ABI question
outside this milestone's gate; if a future lane wants it, it is a design fork for calef, not
something to fold into a retry-loop sweep.

## Prior art

`fixtures/src/login_test_client.rs`'s `destroy_with_retry` (PR #562's fix) is the direct precedent and
the shape to copy, including its BUGS section's own honesty about the wall-clock unit. Milestone 78
(the load-sensitive assertions) is the family this belongs to, extended into userspace for the
first time. Milestone 62 (tests that assert on time) is where "wait on the property, bound with a
clock" was first established, in the kernel, with the stronger unit this milestone cannot use.

## BUGS

Not started; nothing built yet to carry its own BUGS section.

## Index row

Milestone 78's login-teardown fix (PR #562) found four siblings. A fixed attempt count, no clock,
waiting out the same DECISIONS §16 refusal. Two return quietly on exhaustion (a stranded region);
two call `user_rt::trap()`, killing the process under load instead of degrading. Fix is the same
one already built once, ported four times. Userspace has no delivered-tick counter, so the open
question (an ABI addition) is recorded, not decided.

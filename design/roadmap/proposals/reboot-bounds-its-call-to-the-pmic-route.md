---
status: PROPOSED
raised: 2026-11-11
milestone_dependencies: 1062
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# `reboot` bounds its call to the PMIC route

Raised by lane/timed-call-proposal. On 2026-11-11 (UTC) the architect accepted, on #3499, that the board's PMIC route carries the timeout for now. The route waits two seconds after its write and then answers `StillRunning`, and `reboot`'s own call has no timeout until the system has a timed call. This file is the follow-on work that ruling named.

Reuse: the timed call, once a lane builds it; `retype_sleeper` and `timer_arm` in `user_mode_runtime`; the spawn service's grant plan. No new dependency.

Name: provisional. Every new name below is the lane's, not a ruling.

## Blocked on the timed call

This waits on [a caller gives up on a call at a deadline](a-caller-gives-up-on-a-call-at-a-deadline.md), which is unbuilt and has no number yet. At promotion, the integrator adds that milestone's number to `milestone_dependencies` above. It also waits on milestone 1062 (the device control planes leave the kernel), whose step 5 builds the route.

## The problem

`reboot` calls the route with a plain `CALL` (`start` in `components/src/reboot.rs`). A route that stops answering leaves `reboot` blocked, so the machine neither restarts nor says why. Both programs' `BUGS` record it. The route's two-second wait bounds only a route that still runs.

## The work

- The spawn service grants `reboot` what a bounded call needs. With the shape ruled on 2026-11-11 (UTC), `Rendezvous::CALL_BOUND`, that is a `Timer` and a notification bound to `reboot`'s thread, either granted or retyped from a small memory region with `retype_sleeper`. The grant plan gains the slots, and the capability contract table in `reboot`'s module doc gains its rows.
- `reboot` arms the timer and makes the bounded call to the route. The bound is longer than the route's own wait, so a healthy route always answers first. A provisional name is `ROUTE_CALL_BOUND_SECONDS`, at 3.
- When the bound ends the call, `reboot` writes why on the diagnostics stream and invokes the kernel's `REBOOT`, as it does after any other route answer. The message says whether the route ever took the request.
- The `BUGS` lines in `reboot` and in the route that say the call has no timeout go away.
- swish-check's reboot phase keeps passing on QEMU, where no machine has a route. A host test, or a system test with a stand-in route that never answers, proves that `reboot` reaches `REBOOT` after the bound.

## The route keeps its two-second wait

Ruled 2026-11-11 (UTC), on #3508: the route keeps its own two-second wait once `reboot` bounds its call. Only the route can tell a write that did not reset the board from a route that never answered, and `reboot` reports the two differently. So the route still answers `StillRunning` after two seconds, and `reboot`'s three-second bound covers a route that never answers.

## What drives it

The architect's ruling of 2026-11-11 (UTC) on #3499 named this as follow-on work. A board whose route stops answering cannot restart without it.

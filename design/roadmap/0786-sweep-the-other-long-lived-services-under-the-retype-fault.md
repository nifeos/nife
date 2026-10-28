---
status: NOT-STARTED
promoted_from: sweep-the-other-long-lived-services-under-the-retype-fault
raised: 2026-10-04
milestone_dependencies: 757
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 786. Sweep the other long-lived services under the retype fault

Raised 2026-10-04 (UTC) by the lane for milestone 757 (a test kernel fails a process on its Nth
retype), provisional. *(Title and slug are drafts.)*

## Why

Milestone 757 built the fault and swept one `login` exchange. Its first sweep found a slot leak in
`supervision_contract::build_child_space` that had been recorded for five weeks with no test able
to reach it. Of milestone 745 (count the error paths no test reaches)'s 75 cleanup paths, the sweep reached `login`'s; 27 are in
`system_initializer` and 10 in `swish`, and `login` has two exchanges the sweep does not run.

## What to build

1. `login`'s start-up (the three splits in `_start`) and a login that opens a schedule
   (`Durable::open`, `rederive`). Each needs a fresh `login` per N, and a `login` cannot be torn
   down today (`kernel/src/user/holding.rs`' BUGS: it parks on a front door its budget did not pay
   for). Either give the harness a front door from `login`'s own budget, so `Holding` can end it,
   or sweep from a parent that can destroy the region `login` runs in.
2. `system_initializer`'s session build, which `swish-check`'s boot exercises, swept by arming the
   initializer's thread across one session start.
3. `swish` spawning a pipeline, where a failed child build is a common, user-visible event.

Same census as 757: the target's capability table and the usage of every region it holds, before
and after each N.

## Index row

The retype-fault sweep found a five-week-old slot leak in its first run but covered only one `login` exchange of 75 cleanup paths. The rest are in `login`'s start-up, `system_initializer` and `swish`, and need a harness that can tear a `login` down.

---
status: NOT-STARTED
raised: 2026-09-26
promoted_from: tests-retype-their-rendezvous-from-their-own-region
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 671. Tests retype their rendezvous from their own region, so the registry stops filling

*Title and slug are drafts.*
Filed by the lane of milestone 601 (the region table prints its
peak), from the rendezvous ledger at `sched::PEAK_RENDEZVOUS`. The title is provisional.

Test and service wiring; no syscall surface, no dependency.

## What was measured

On `main` at `484f3ebe` plus #1347, the aarch64 suite ends with 505 of 512 rendezvous live, 7
spare. riscv64 ends at 491. 466 of the aarch64 505 sit on the kernel's own chunks, which are never
freed, so the number only climbs. The lane of milestone 152 (durable delegation) hit the ceiling in
`timetable_tests` with one more test, and fixed its own case by retyping each run's report
endpoint from that run's scratch region.

The largest callers of `sched::create_rendezvous` are in the ledger: the scheduler's own tests
(46), `user::tests` (45), `ntp_tests` (37), `login_tests` (36), and the file-service tests (51
between three modules).

## The work

Where a test or a test-wired service creates a rendezvous for something that dies with the test,
retype it from the test's own region (`sched::create_rendezvous_from`), so reclaiming the region
reclaims the endpoint. Services that live for the boot keep kernel-chunk rendezvous, and say so.

## Done when

The `rendezvous:` line on aarch64 shows the kernel-chunk count down to the services meant to live
for the boot, with the spare back above 100.

## Index row

The aarch64 suite ends with 505 of 512 rendezvous live, 466 of them on the kernel's never-freed chunks. Proposed: each test retypes its rendezvous from its own region so the registry stops filling.

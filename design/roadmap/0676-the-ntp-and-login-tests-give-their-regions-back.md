---
status: NOT-STARTED
raised: 2026-09-26
promoted_from: the-ntp-and-login-tests-give-their-regions-back
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 676. The NTP and login tests give their regions back, or say why they keep them

*Title and slug are drafts.*
Filed by the lane of milestone 601 (the region table prints its
peak), from the per-test measurement that built the ledger at `memory_region::MAX_REGIONS`. The
title is provisional.

Test wiring and service teardown; no syscall surface, no dependency.

## What was measured

On aarch64 at `484f3ebe` plus #1347, the suite ends with 221 regions live that were not live when
the first test started, against a ceiling of 256. Two modules hold a third of them:

| module | aarch64 | riscv64 | `x86_64` | per test |
|---|---|---|---|---|
| `ntp_tests` | 39 | 39 | 5 | 5, 5, 9, 9, 11 |
| `login_tests` | 33 | 33 | 0 | up to 12 (`the_login_service_serves_past_the_old_capability_table_ceiling`) |

The five NTP tests also keep about 760 frames between them on aarch64. Neither module is on the
held list in `notes/frames.md` except the login service itself, which is wired once and shared.
`x86_64` has no network under QEMU, which is why its column is small.

## The work

For each test: either hand back what it spawned (`user::holding::Holding`, the reclaim path the
frame ledger already names), or add it to `notes/frames.md`'s held list with the reason it must
stay. The region peak is 225 of 256 with #1347; recovering the NTP residue alone takes it under 190.

## Done when

Every region kept by these two modules is either returned by the end of its test or listed as
deliberately held, and the `regions:` line on aarch64 shows the drop.

## Index row

The suite ends with 221 regions live against a ceiling of 256, and the NTP and login tests hold a third of them. Proposed: they give their regions back at teardown, or say why they keep them.

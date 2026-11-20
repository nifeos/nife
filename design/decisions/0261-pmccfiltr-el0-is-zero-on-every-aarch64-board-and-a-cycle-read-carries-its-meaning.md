---
status: DECIDED
raised: 2026-09-19
decided: 2026-10-07
ratified_by: calef
---

# 261. PMCCFILTR_EL0 is zero on every aarch64 board, and a cycle read carries its meaning

*Section number provisional until the merge queue lands it; 257 was the highest on `main` when this
was written, and open PRs #1846, #1847 and #1851 hold 258, 259 and 260. Minted for milestone 353 (the aarch64 half of milestone 74), on 2026-10-07 (UTC). Milestone 74
(Cycle counters) is the parent.*

## The rulings

Two forks, both calef's, both answered "Yes" on 2026-10-07 (UTC) to the options as recorded in
[the 353 block](../roadmap/0353-the-aarch64-half-of-74.md) ("The rulings, 2026-10-07 (UTC)").

**Decision A: A1.** `PMCCFILTR_EL0` is `0` on every aarch64 board, by policy. The kernel writes `0`,
so `PMCCNTR_EL0` counts EL0 and EL1 and not EL2, which matches riscv64 and x86_64 (both count user
and kernel). This supersedes the 2026-09-19 ruling to wait for argon's firmware value.

Why: the register's reset value is architecturally UNKNOWN and firmware-dependent. argon's value
says what argon's firmware does and what seL4's TX1 figures were measured under; it cannot set
nife's policy. argon's firmware value is still read at first boot, for milestone 25 (cross-OS
comparison) only. If firmware left `P` set, milestone 25 runs nife a second time with the filter
matched to seL4's and labels that run as such.

Decision B: B4, step 1. One public function in `crates/user_mode_runtime`,
`user_mode_runtime::cycle_reading`, returns `CycleReading { count, meaning }`. The meaning per
architecture:

| Architecture | `count` is | `meaning` |
|---|---|---|
| aarch64 | `PMCCNTR_EL0`, EL0 and EL1 per A1 | core cycles, user and kernel |
| riscv64 | the cycle counter | core cycles |
| x86_64 | the TSC | constant-rate reference cycles, not core cycles |

`rdpmc` and `CR4.PCE` are not reopened on x86_64. A program knows whether it may read from its own
manifest's grant, so step 1 changes no system call surface.

Deferred: step 2 (a kernel-provided "may I read" page or method, a later system-call-surface fork) and
the riscv64 flag for a kernel probe handed `hpmcounter3`, which a process cannot learn without that
page or call. Both are proposed in
[milestone 856 (a program asks whether it may read the cycle counter)](../roadmap/0856-a-program-asks-whether-it-may-read-the-cycle-counter.md)
and not built.

## Names

`PMCCFILTR_COUNT_EL0_AND_EL1`, `CycleReading` and `abi::cycle_counter::CycleMeaning` are provisional
(naming-authority); a rename is an architect's call.

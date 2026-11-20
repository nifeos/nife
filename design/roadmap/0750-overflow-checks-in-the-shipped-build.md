---
status: BUILT
raised: 2026-10-04
built: 2026-10-04
milestone_dependencies: 744
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 750. Overflow checks in the shipped build

Raised 2026-10-04 (UTC) by calef, ruling on the options of milestone 744 (measure the kernel's
overflow checks): "Yes. Launch a lane for the fast paths." The number 750 is provisional until the
integrator mints it at merge; the title and the slug are drafts.

## Why

Milestone 744 measured that every tested build checks integer overflow and the shipped one did not,
so the build a customer boots meant something different by `a + b` from every build that was ever
tested. calef chose option A with option B's programs, and option C to keep the fast paths in their
footprint band.

## What was built

[notes/overflow-checks.md](../../notes/overflow-checks.md), "The ruling, and what landed", has the
numbers.

1. `overflow-checks = true` in every release profile that builds shipped or tested code: the root
   workspace, `std_exerciser`, `redoxfs_server` and `cryptography_exerciser` (each with its `std`),
   the ripgrep helper by environment, and the two probe scripts' generated manifests.
2. The IPC and system call fast paths on all three ISAs written so that each operation's overflow
   behavior is explicit and justified at its site; `script/fastpath-footprint` passes with its
   baselines unchanged.
3. `SUITE_PAGE_FRAME_BUDGET` re-derived from a CI run with checks on: 26,705.
4. `cryptography_exerciser` and `rg` exercised with checks on, on all three ISAs.
5. icount and HVF release benches re-measured; `ipc_rtt_el0`'s earlier +5.5% did not reproduce,
   and CoreMark's +2.4% did.

No overflow panicked anywhere.

## Follow-on

- **Recorded.** riscv64 and x86_64 have no release cycle measurement, and `cryptography_exerciser`
  and `rg` are still exercised only by hand, in the BUGS section of `notes/overflow-checks.md`.
- **Recorded.** `phys_to_virt`'s add check on x86_64 is weaker than the direct map's range, in the
  same BUGS section.

## Index row

Overflow checks on in every release profile, with the IPC and system call fast paths written explicitly
so the footprint gate still passes; budget re-derived, benches re-measured, no overflow found.

---
status: NOT-STARTED
raised: 2026-09-10
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 273. Port `ipc_thread_scaling` and `app_displacement` to x86_64, or record why not

Not a design fork. These are two existing benchmark functions gated
`#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]` in `kernel/src/bench.rs`, and this
is either extending the `cfg` to x86_64 or recording a reason the extension does not apply.

## What is excluded, and why this is different from the other three items

`ipc_thread_scaling` (milestone 134's E1, IPC latency versus thread count) and `app_displacement`
(E4, working-set eviction under concurrent IPC load) do not run on x86_64 at compile time, not
as a runtime skip. Milestone 134 built both on 2026-08-22, when x86_64 was earlier in its port; no
comment near either `#[cfg]` states a reason, which was checked directly (2026-09-10) rather than
assumed.

**This is the one item in the inventory where "why not" has not been established.** The other three
each have a stated, checked cause. This may be an oversight from before x86_64 was a full `bench`
target, or there may be a real reason (both call `real_single_hart_or_skip`, and if x86_64's
core-pinning story differs that could be it) that simply was never written down.

## What this needs

1. Find out why, before building anything. Read `real_single_hart_or_skip` and whatever else
   both functions depend on, and check whether it already supports x86_64 or would need porting.
2. If nothing blocks it, extend the `cfg` to include x86_64 and take the measurement, the same
   register-of-measures shape milestone 134 already uses for the other two architectures.
3. If something does block it, record the reason in `kernel/src/bench.rs` next to the `cfg`,
   the same way every other architecture gap in this tree is supposed to read at the thing itself
   rather than only in a roadmap block.

## BUGS

- This block does not know its own size yet. It may be a one-line `cfg` change or a real port;
  step 1 is what decides which, and it has not been done.

## Follow-on

- **Milestone 186.** If x86_64 was simply missed rather than deliberately excluded, this is a
  twelfth instance of 186's own pattern (a parity gap that predates x86_64 fully joining the tree)
  and should be named there once confirmed.

## Index row

calef, 2026-09-10. The one gap in the inventory with no stated reason, checked and confirmed
absent.

---
status: NOT-STARTED
raised: 2026-09-26
promoted_from: the-timebase-page-moves-beside-the-stack
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 680. The timebase page moves beside the stack

*Title and slug are drafts.*
Raised by the lane for milestone 206 (a program image has under 896 KiB)
while drawing the user address-space map, `crates/address_space_map`.

One constant in a protocol crate both sides already share, a std farm rebuild, and a
benchmark re-baseline. No syscall surface, wire format or dependency.

## The finding

`counter_frequency_protocol::PAGE_VA` is the one page the kernel maps into every `x86_64` and
`riscv64` process that is not on the map. It sits at seven-eighths of each architecture's low half
(`0x7000_0000_0000`, and `0x38_0000_0000` under Sv39), a siting written before the map existed to keep
it away from the low megabytes where fixtures picked addresses.

An address alone in a far corner is alone in its page tables. `notes/benchmarks/spawn-el0.md`
measured exactly this for the current-CPU page: three fresh tables cost 1,245 ticks a spawn, one cost
741. The timebase page pays the three-table price on two architectures today.

## The fix

Place it in the map's `PROCESS_PAGES` band, beside the current-CPU page and in the stack's leaf
table, where it costs only its leaf. That band exists for pages the kernel maps into every process
unasked, and it has fifteen free pages. The Sv39 special case in the crate goes away with it: one
number for three architectures, as the current-CPU page already has.

## What it costs

The crate is generated into std's PAL, so the move rebuilds the farm. `spawn_el0` should get cheaper
on `x86_64` and `riscv64` by roughly two tables' worth, which the icount tripwire will read as a
change beyond tolerance; the baselines are re-saved with the attribution beside them, as the
current-CPU page's own move was.

## Index row

The counter-frequency page sits alone in a far corner of every x86_64 and riscv64 address space, which costs fresh page tables per spawn. Proposed: move its address beside the stack, one constant in a shared protocol crate plus a std farm rebuild and a benchmark re-baseline.

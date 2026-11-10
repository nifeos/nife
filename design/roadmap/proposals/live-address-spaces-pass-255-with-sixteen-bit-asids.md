---
status: PROPOSED
raised: 2026-11-10
milestone_dependencies: 1055
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# Live address spaces pass 255 with 16-bit ASIDs

Raised by the lane for milestone 1055 (the kernel's object tables grow fourfold), which raised the
address-space registry to 1,056 slots.

Name: provisional.

Reuse: `crates/address_space_identifier`, the ASID plumbing in `kernel/src/arch/*/mmu*`, and the
probe of the hardware ASID width that milestone 58 (RISC-V TLB shootdown, and the flush that makes ASIDs pointless) added for riscv64.

## Background

Every user address space holds one ASID for life. The allocator hands out 255 numbers (8-bit ASIDs,
the width every aarch64 implementation has), so at most 255 spaces can be live at once, however many
slots the registry has. The registry has 1,056. Past 255 live spaces, the kernel refuses to create a
space, cleanly, as it does when the registry is full, and the process and thread tables above that
level stay unused.

## The work

1. Probe the hardware width on aarch64 (`ID_AA64MMFR0_EL1.ASIDBits`) and set `TCR_EL1.AS` when it
   is 16 bits, as the notes already name. Widen the allocator's bitmap to 65,536 numbers behind that
   probe.
2. On riscv64, use the width `satp.ASID` reports. On x86_64, where the kernel flushes everything on
   a switch, give each space a number only to satisfy the same interface.
3. Measure the `spawn_el0`, `map_el0` and context-switch rows on each architecture. A wider bitmap
   makes the allocation scan longer, so the allocator may want a next-free hint.

## Done when

- 1,024 live address spaces can exist on an architecture whose hardware has the width, and the others
  keep the smaller limit with the probe recorded.
- No bench row leaves its band.

## Index row

The address-space registry has 1,056 slots and the ASID allocator has 255 numbers. This widens the
numbers where the hardware allows.

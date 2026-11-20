---
status: BUILT
raised: 2026-10-13
built: 2026-10-13
---
# 915. NMI, double fault and machine check run on their own stacks

*Name: provisional. The number 915 is provisional until the merge queue lands it, and the title,
the slug, the module `kernel/src/arch/x86_64/ist.rs`, the symbols `isr_paranoid` and
`x86_paranoid_dispatch`, and every test name below are drafts. Branch
`milestone/915-nmi-double-fault-and-machine-check-run-on-their-own-stacks`.*

Reuse: none exists to take. The shape is Linux's x86_64 "paranoid entry" and its per-CPU IST
stacks (`arch/x86/entry/entry_64.S`, `arch/x86/kernel/dumpstack_64.c`), read for the approach; the
code is written against this kernel's own `TrapFrame`, per-CPU block and stack conventions, which
no outside implementation shares.

## What is true

On x86_64 the NMI (vector 2), the double fault (8) and the machine check (18) never run on the
stack they interrupted.

- Each core has three IST stacks, 16 KiB each over an unmapped 4 KiB guard page, in their own
  `(NOLOAD)` region `.ist_stacks` (`kernel/link-x86_64.ld`). `segments::init` installs them in that
  core's TSS at bring-up through `ist::install`, before the core loads its IDT. The fine kernel map
  maps each stack and leaves each guard unmapped, and `mmu::fine_map::verify` refuses a map in
  which an IST guard translates.
- The three gates select them: vector 8 selects IST entry 1, vector 2 entry 2, vector 18 entry
  3. No other gate selects an IST entry.
- Their entry does not trust the saved `CS`. The stubs for the three vectors jump to
  `isr_paranoid` (`trap.s`) instead of `isr_common`. The top word of each IST stack holds its core's
  per-CPU pointer; `isr_paranoid` reads `IA32_GS_BASE`, writes that pointer if the two differ, and
  writes the value it found back before `iretq`. That is correct from ring 3, from ring 0, and in
  the two windows where `CS` says ring 0 while the GS base is the user's (around the `swapgs` at
  `system_call` entry, and before `iretq` on the way out).
- An NMI never nests on its stack, by rule rather than by machinery: the NMI handler takes no
  exception that returns, so only its own `iretq` unblocks NMIs. `x86_paranoid_dispatch` checks
  the rule on every NMI and panics if the interrupted `rsp` is on an NMI stack. `ist.rs`'s header
  says why Linux's nested-NMI frame copy is not built.
- A machine check is fatal from either ring, and so is a double fault. Neither reaches the
  user-fault path, the deferred `schedule()` or the per-CPU interrupt stack.

aarch64 and riscv64 have no equivalent to build: neither takes an asynchronous exception on a stack
the kernel did not choose. [notes/stack.md](../../notes/stack.md), "Where an asynchronous
exception lands, per architecture", has the reasoning for each.

## How it is proven

Kernel tests in `kernel/src/arch/x86_64/ist.rs`, run on the x86_64 leg under PVH (two cores) and
OVMF. Each claim carries a replayable falsification under `kernel/falsifications/`, and each went
red when replayed with `script/test --arch x86_64 --no-host --test <name>` on 2026-10-13 (UTC).

| test | claim | falsification |
|---|---|---|
| `every_online_core_has_its_own_three_interrupt_stack_table_entries` | each online core's three IST entries are nonzero, inside that core's own stack for the vector, carry its per-CPU pointer, and are mapped | `install` indexes the next core's stacks |
| `every_interrupt_stack_table_guard_page_is_unmapped` | every IST guard page is a hole in the live kernel map | the map covers the guards and `verify` stops checking |
| `the_nmi_double_fault_and_machine_check_gates_select_their_own_stacks` | gates 8, 2 and 18 select entries 1, 2 and 3; no other gate selects any | the NMI and machine-check gates swap entries |
| `an_nmi_finds_its_per_cpu_block_whatever_the_gs_base_held` | a self-NMI sent while `IA32_GS_BASE` holds a decoy reads this core's per-CPU pointer | `isr_paranoid` skips the GS write |
| `an_nmi_leaves_the_gs_base_it_found` | that NMI returns with the decoy still in `IA32_GS_BASE` | `isr_paranoid` skips the restore |
| `the_ist_stack_region_is_the_size_the_linker_reserved` | the linker reserved at least what `ist.rs` lays out | none: a layout check |

## BUGS

- `script/stack-depth-check` does not measure the IST stacks. They are the size of the per-CPU
  interrupt stack, which carries the same panic path and is measured.
- The IST stacks cost 480 KiB of address space for eight seats whether or not the cores exist,
  like the interrupt and secondary stacks beside them.
- Recovery from a correctable machine check is not built; every machine check panics.

## Follow-on

- **Recorded.** The unmeasured IST stack depth and the fatal machine check are limitations, in
  `kernel/src/arch/x86_64/ist.rs`'s and `kernel/src/arch/x86_64/exceptions.rs`'s own docs and in
  BUGS above.

## Index row

An NMI arrives on every TLB shootdown and can land in the two instructions `syscall` runs on the
user's stack pointer, and a double fault happens when a stack is unusable. On x86_64 these vectors,
and the machine check, switch to per-core stacks of their own and find the kernel's per-CPU
block without trusting the interrupted state.

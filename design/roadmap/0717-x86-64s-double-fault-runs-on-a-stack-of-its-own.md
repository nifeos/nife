---
status: NOT-STARTED
raised: 2026-10-03
promoted_from: x86-64s-double-fault-runs-on-a-stack-of-its-own
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 717. x86_64's double fault runs on a stack of its own

*Title and slug are drafts.*
Raised at calef's request, from the first of two findings in `notes/ci.md`. The second is the
sibling proposal
[x86_64 names a kernel stack overflow the way aarch64 and riscv64 do](0716-x86-64-names-a-kernel-stack-overflow-the-way-aarch64-and-riscv64-do.md).
Title and slug are drafts.

## The finding

The double-fault safety net is documented, selected by the IDT, and never installed.

- The IDT gate for vector 8 selects IST slot 1: `let ist = if vector == 8 { IST_DOUBLE_FAULT } else
  { 0 };` (`kernel/src/arch/x86_64/exceptions.rs:411`), with `IST_DOUBLE_FAULT = 1`
  (`arch/x86_64/segments.rs:46`).
- The TSS has the seven entries (`ist: [u64; 7]`, `segments.rs:94`) and zeroes them at init
  (`segments.rs:117`).
- `set_interrupt_stack` (`segments.rs:298`) is the only writer after init, and
  `grep -rn set_interrupt_stack kernel xtask crates` finds no caller outside its own definition.
  The compiler agrees: `notes/ci.md` on #1472 records the `never used` warning as how this was found.

So a double fault today loads `rsp` from `TSS.ist[0] == 0`. The CPU cannot push its frame there,
and the result is a triple fault. The runner passes `-no-reboot` (`helpers/qemu-runner-x86_64.sh:289`),
so under QEMU that is a silent exit with no report; on a real machine it is a reboot. The comment on
`exceptions::initialize` (`exceptions.rs:388` to `392`) states the intent exactly: a handler pushing
onto the same broken stack "triple-faults". The code does not deliver it.

The commonest trigger is the one the IST exists for: a kernel stack overflow whose exception frame
push lands in the guard page (the 48-byte frame needs room the stack no longer has).

### What the other two architectures do

Neither has a double fault, so neither can reset the machine for this. They fail differently and
each is recorded:

- aarch64: a store into a guard page from the vector's own `SAVE_CONTEXT` re-enters the same vector
  with `sp` another 272 bytes lower, repeatedly, until a frame fits in the slot below
  (`kernel/src/stack.rs`, doc on `warn_if_guard_page`, "aarch64 has no double fault").
- riscv64: the entry stub does `addi sp, sp, -288` and then stores
  (`kernel/src/arch/riscv64/trap.s:52` to `54`). I read that it can fault the same way; I did not run
  it, and `kernel/src/interrupt_stack.rs`'s BUGS entry says an overflow whose first fault is the
  vector's own frame store "still cascades" on the ports that have the interrupt stack.

Both ports eventually print. x86_64 resets. That is the parity gap this closes (§19
(architectural parity is a tenet)).

## Proposal

1. Allocate one stack per core for the double fault, in its own region with an unmapped guard page
   below it. It must not be the interrupt stack (`interrupt_stack.rs`): `set_interrupt_stack`'s
   Safety contract (`segments.rs:295` to `297`) forbids two vectors sharing an IST stack, and a
   double fault taken while the interrupt-stack handler runs is exactly that case.
2. Call `set_interrupt_stack(IST_DOUBLE_FAULT, top)` on every core, after `segments::initialize` has
   loaded that core's TSS (`set_interrupt_stack` indexes by `cpu::id()`).
3. Give vector 8 its own body that prints a fault report from that stack and halts. It prints the
   frame (`rip`, `rsp`, `cs`, `ss`, `rflags`; the error code is architecturally zero) and does not
   return. The report is deliberately minimal and takes no lock and no allocator.
4. Exempt vector 8 from `x86_trap_dispatch`'s interrupt-stack move, the way NMI is exempted
   (`exceptions.rs:668` to `686`): `top_for_trap` (`interrupt_stack.rs:171`) would otherwise find `rsp`
   outside the interrupt-stack region and move the handler off the IST stack onto a different one.
   This is read from the code, not measured.

The guard-page naming (is the faulting `rsp` or address in a kernel stack's guard page, and whose)
belongs to the sibling proposal and is what makes this report useful rather than merely present.

## Proof plan

- A kernel test that deliberately overflows a kernel stack on x86_64 under QEMU and must reach the
  double-fault report. The runner already passes `-no-reboot`, so a regression shows as an exit with
  no report line, which the test treats as failure. The harness piece is open: I found no existing
  expected-fatal-exit test (`grep` for `should_panic`/`expect_panic` in `kernel/src`, `xtask/src`
  and `system_tests` found none), so the lane must say how a test passes by observing a fatal report.
- Host-side: a test that the stack top handed to `set_interrupt_stack` is per-core, distinct from
  `interrupt_stack::span(id)`, and 16-byte aligned. Pure arithmetic, so it belongs in a crate or a
  `#[cfg(test)]` module.
- The `never used` warning on `set_interrupt_stack` is the tripwire that found this; wiring the call
  ends it, so no allowance is added.

## Prior art outside the tree

Read from source on 2026-10-03 (UTC):

- Linux, `arch/x86/kernel/cpu/common.c`: `tss->x86_tss.ist[IST_INDEX_DF] = __this_cpu_ist_top_va(DF);`
  per CPU, and `arch/x86/include/asm/cpu_entry_area.h` lays out a separate `DF` exception stack with
  a `DF_stack_guard` beside it, one per CPU. `exc_double_fault` in `arch/x86/kernel/traps.c` reads
  `CR2`, checks it against the guard-page info and calls `handle_stack_overflow`, then falls through
  to `PANIC: double fault`. Linux therefore does both halves this pair proposes, in one handler.
- blog_os (phil-opp), `src/gdt.rs` and `src/interrupts.rs` at tag `post-06`: a statically allocated
  `4096 * 5` byte stack stored in `interrupt_stack_table[0]` and selected with
  `.set_stack_index(DOUBLE_FAULT_IST_INDEX)`. Note it has no guard page below that stack, and one
  static stack serves every core, which is fine for one core and not for this kernel.

## Cost, risk, reversibility

- Cost: one small per-core region (blog_os uses 20 KiB, so that is a ceiling to measure against) and
  about 80 lines. This is an estimate, not a diff.
- Risk: low. The handler runs only after the machine was about to reset anyway, so a mistake in it
  cannot make a working boot worse. The one live-path edit is the `x86_trap_dispatch` exemption.
- Reversibility: fully; all of it is code in `arch/x86_64/`, and no wire format, name or syscall is
  touched. The new region's linker symbol is a provisional name.
- Would we choose this if both options cost the same? Yes. The alternative, leaving it, is the one
  case on this port where the kernel's own comment promises a report and delivers a reset.

## What it unblocks

The sibling proposal's most common case. A kernel stack overflow with `rsp` near the bottom reaches
the double fault before it can reach the page-fault handler, so its report is only seen through this
stack.

## Index row

x86_64's IDT gate for vector 8 selects IST slot 1, but the TSS entry is zero and never installed. Proposed: give the double fault a stack of its own so the safety net exists.

---
status: NOT-STARTED
raised: 2026-10-03
promoted_from: x86-64-names-a-kernel-stack-overflow-the-way-aarch64-and-riscv64-do
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 716. x86_64 names a kernel stack overflow the way aarch64 and riscv64 do

*Title and slug are drafts.*
Raised at calef's request, from the second of two findings in `notes/ci.md`. The first is the
sibling proposal
[x86_64's double fault runs on a stack of its own](0717-x86-64s-double-fault-runs-on-a-stack-of-its-own.md).
Title and slug are drafts.

## The finding

aarch64 and riscv64 say `*** KERNEL STACK OVERFLOW ***` and name the stack when a kernel fault lands
in a guard page. x86_64 has the stacks and the guard pages and never asks the question.

- The diagnostic is `stack::warn_if_guard_page(addr, interrupted_sp)` (`kernel/src/stack.rs:209`),
  built on `stack::guard_page_at` (`stack.rs:109`), which names the boot, secondary, thread and
  interrupt stacks (`enum GuardPage`, `stack.rs:75`).
- Its callers are exactly two: `arch/aarch64/exceptions.rs:726` and `arch/riscv64/exceptions.rs:503`.
  `grep -rn warn_if_guard_page kernel/src` finds no x86_64 caller.
- x86_64's fatal kernel-exception arm (`arch/x86_64/exceptions.rs`, the `vector =>` arm of
  `x86_trap_body`, near line 888) prints vector, error code, `rip`, `rsp`, `rflags` and, for vector 14,
  `cr2` (`faulting_address`, line 605), then panics. It never says that `cr2` is a guard page.
- The guard pages exist on x86_64: the `verify` asserts in `mmu/fine_map.rs` assert the boot and
  secondary guards are unmapped.
- The cost is already visible as eight dead-code warnings (`GuardPage`, both `guard_page_at`s,
  `thread_stack_site`, `warn_if_guard_page`, `print_text_words`, `stack_area_span`, `is_mapped`),
  recorded in `notes/ci.md` on #1472 as one chain with one cause. `is_mapped` is in
  `mmu/kernel_map.rs`, and its own doc names the shared caller `stack::print_text_words`.

Why it matters: the 2026-08-14 and 2026-08-16 incidents (`notes/stack/kernel-stack-overflows-2026-08-14.md`,
`notes/stack/guard-page-faults-2026-08-16.md`) were each read as a memory fault until the kernel
named the guard page. x86_64 would repeat that, and under DECISIONS §19 (architectural parity is a
tenet) a capability that works on two ISAs and silently not the third is the bug.

## Is the dependency on the double-fault proposal real?

**Partly, and it decides the order.** From the code and the SDM's exception-delivery rule, not from a
run:

- A kernel-mode page fault pushes a 48-byte frame at `rsp` (no ring change, so no stack switch;
  the vector has no IST, `exceptions.rs:411`). If `rsp` is within that distance of the guard page,
  the push itself faults and the CPU raises a double fault. An overflow caused by `call` or `push`
  leaves `rsp` at the bottom of the stack, so this is the common case.
- With no double-fault stack (the sibling proposal) that case is a silent reset, and a diagnostic
  wired only into the page-fault handler never runs for it.
- The page-fault handler does run when `rsp` still has frame room, for example a large local written
  past the bottom by a store rather than a push, or `sp` arithmetic that skipped into the guard.
  That subset works without the sibling.

So this proposal is independently shippable and fixes a real subset, but the headline case needs the
sibling's stack, and the check has to be callable from both vectors (14 and 8). Linux is built the
same way: its `exc_double_fault` reads `CR2` and calls `handle_stack_overflow` (see Prior art). The
recommendation is to land the sibling first, or to land this with the shared check and the page-fault
arm and let the sibling add the second call site. Either order is fine; shipping this alone must not
claim to cover overflow in general. `CR2` being the delivery-fault address at a double fault is what
Linux's code assumes; I have not measured it on QEMU, and the proof plan does.

## Proposal

1. In the fatal arm of `x86_trap_body`, when `vector == 14`, call
   `crate::stack::warn_if_guard_page(faulting_address(), frame.rsp)` before the register dump,
   mirroring the aarch64 and riscv64 order. The interrupted `rsp` is exact here (`frame.rsp`), so the
   milestone 124 (a thread is born where it lives) correction, never reading the live `sp`, holds for free.
2. Keep the check a function both vectors call, so the sibling's double-fault report can use it.
3. Delete the per-item dead-code allowances this makes unnecessary, and leave no allowance behind:
   an allowance would launder a parity gap into "fine", which is what `notes/ci.md` says.

## Proof plan

- A test on all three ISAs that the diagnostic fires: take a fault whose address is inside a known
  guard page and assert the `KERNEL STACK OVERFLOW` report names the right `GuardPage` variant.
  `guard_page_at` is pure given the layout, so the three-ISA test can call it directly for each
  kind (boot, secondary, thread, interrupt). The per-ISA proof is that the live handler reaches it,
  which needs a deliberate fault, and that has the same harness gap as the sibling: no expected-fatal
  test exists today (searched `kernel/src`, `xtask/src`, `system_tests`). Today the only text proof
  of the report is a CI log, quoted in the notes above.
- x86_64 under QEMU: a deliberate overflow with `rsp` kept above the frame room (a large store), so
  it reaches vector 14 without the sibling, must print the report.
- With the sibling landed: the `call`-recursion overflow must print the same report from the
  double-fault stack. Measure `CR2` there.

## Prior art outside the tree

Read from source on 2026-10-03 (UTC):

- Linux `arch/x86/kernel/traps.c`: `handle_stack_overflow` prints `BUG: %s stack guard page was hit
  at %px (stack is %px..%px)` and dies; `exc_double_fault` reads `CR2`, calls `get_stack_guard_info`
  and then it. Its comment there says the test can misfire if a `#GP` delivery fails while `CR2`
  happens to point at a guard page, and accepts that because the machine panics either way. That
  false-positive is worth copying into this tree's `BUGS` text.
- blog_os (phil-opp), tag `post-06`: no guard-page naming; the double-fault handler panics with the
  stack frame only. I read only that tag, so what later posts add is unchecked.

## Cost, risk, reversibility

- Cost: one call site and the deletion of dead-code allowances, plus the tests. Small.
- Risk: the diagnostic must not itself fault in the handler. The shared function takes no locks and
  touches no allocator by its own contract (`stack.rs`, doc on `guard_page_at`), and
  `print_text_words` is already guarded by `is_mapped`.
- Reversibility: fully; code only, in `arch/x86_64/exceptions.rs`. Nothing is named or wired across
  programs.
- Would we choose this if both options cost the same? Yes. The only alternative is a scope note
  recording the gap, and a three-ISA kernel with a one-ISA diagnostic is the case §19 refuses.

## Index row

aarch64 and riscv64 print `KERNEL STACK OVERFLOW` and name the stack when a fault lands in a guard page, and x86_64 has the guard pages and never asks. Proposed: call `stack::warn_if_guard_page` from x86_64's fault path.

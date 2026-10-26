---
status: NOT-STARTED
raised: 2026-09-20
promoted_from: what-else-assumes-arch-init-runs-on-every-core
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 538. What else assumes `arch::initialize` runs on every core

*(Number provisional until the merge queue lands it.)* Promoted from the proposal `what-else-assumes-arch-init-runs-on-every-core`, filed 2026-09-20, on calef's instruction of 2026-09-20 to give every proposal on `main` a number. The text below is the proposal's own, unedited except for this paragraph. The argument is its author's and promotion is not the moment to improve it. Found by milestone 447 (a thread's vector registers are its own),
which hit the instance and named the class in its own report. The instance is fixed in that
milestone; the class is not audited, and this is that audit.

Three call sites and a reading of each boot path.

## The instance, which is evidence rather than anecdote

447 put its floating-point initialization in `arch::initialize`, on the reasonable assumption that a
hook by that name runs when a core comes up. It was **green on aarch64 and x86_64 and silently wrong
on RISC-V**, where the boot hart installs `stvec` through `arch::exceptions::initialize()` directly
and never passes through `arch::initialize` at all.

Silent is the word that matters. OpenSBI hands the kernel a hart with `sstatus.FS` already open,
so no thread ever took the first-use trap, so nothing faulted: two threads shared a register file
and every test passed. The lane found it by reasoning about the boot path rather than by a failure.
That is the kind of discovery that does not repeat reliably.

## What to audit

`arch::initialize()` has three call sites (`kernel/src/main.rs` twice, `kernel/src/smp.rs` once).
For each architecture, establish which cores actually reach it, and then read what is initialized
there against that answer. The question for every item is the one 447 had to ask: is this per-core
state, and does every core run this code?

Three failure shapes to look for, because they are what makes this worth a lane rather than a grep:

- Per-core state set on one core. The 447 shape exactly.
- State that happens to be correct by inheritance, like `sstatus.FS` arriving open from firmware.
  The bug is invisible until the firmware changes or a second core boots differently.
- Initialization that is per-core on one architecture and global on another, which reads as a
  portability question and is really a correctness one.

## What would make it stay fixed

Open, and worth more than the audit. The name `arch::initialize` is a claim about when it runs, and
on one architecture that claim is false; a reader meeting the name cannot tell. It is exactly the
sort of thing to decide with the audit's findings in hand rather than before them. The question is
whether the answer is a
rename (calef's), a comment at each call site, or a structural change that gives per-core bring-up
its own hook.

## Index row

447 put its floating-point initialization in `arch::initialize`, on the reasonable assumption that a
hook by that name runs when a core comes up.

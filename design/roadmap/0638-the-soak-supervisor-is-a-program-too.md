---
status: NOT-STARTED
raised: 2026-09-21
promoted_from: the-soak-supervisor-is-a-program-too
milestone_dependencies: 523
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 638. The soak supervisor is a program too

*Title and slug are drafts.*
Raised by calef, asking whether any other special kernel build
should be a userspace program instead, after milestone 523 (moving the job-mix supervisor into userspace) was
minted for the same reason.

It is gated on milestone 523, not because the code depends on it but because 523 is the experiment: it
proves or disproves that a userspace supervisor can spawn the pool, hold the cycle grant and print a
machine-readable transcript. If it cannot, this proposal is answered before it starts.

## The case

`soak_test` and `job_mix` are the same shape, which is not a coincidence:
`kernel/src/job_mix.rs`'s own header says it was *"shaped like `kernel/src/soak.rs`"*, and both
replace the end of the boot tour. That is why the two features are refused together by
`script/board-image`, and a mutual exclusion between two workloads is a symptom of both living in
the wrong place. Nothing about running a workload requires being the kernel.

What it would buy is what 523 buys, applied to the risk that most needs it:

- **No special image.** `design/fatal-risks.md`'s risk 5 (it cannot be made reliable on multicore,
  and the bugs appear only on silicon) is the hardest entry on that list precisely because its
  experiment is expensive, hardware-bound and produces a confidence rather than a verdict. A soak
  that runs from the prompt can be started on any boot, on any board, without a card or a bench
  evening, and can be left running while somebody does something else.
- **No build-time seal problem.** Milestone 523's own origin was a `NOT SEALED` failure that was
  a *false* failure: the linker dropped the kernel's trust root as dead code because the job-mix
  tour never verifies, and the seal checker's byte-scan then found nothing. Every special-build
  kernel is exposed to that class; a program in the archive is not.
- The two workloads stop excluding each other, which is the thing the current shape makes
  structurally impossible.

## What has to be established, and 523 establishes most of it

- The cycle grant reaching a program declaratively, which milestone 229 (build the cycle-counter grant) built and DECISIONS §139 (cycle counter authority) authorized.
- What moves into the measurement. A userspace supervisor's own scheduling becomes part of what
  is observed. For the job mix that is arguably a feature; for a soak, whose job is to run forever
  and notice a wrong answer, it matters less. Say which it is rather than assuming it carries over.
- Whether a soak needs to outlive its supervisor. A kernel-side soak cannot be killed by the
  thing it is testing. A userspace one can be, and on a machine whose scheduler is under
  investigation that is a real difference rather than a theoretical one. This is the strongest
  argument for leaving it where it is, and it should be answered rather than waved at.

## What this does not propose

Touching `reboot_soak_test`, which composes `soak_test` with `board` and is about a power cycle
rather than a workload.

## Index row

The soak test is a special kernel build that replaces the end of the boot tour, the same shape as the job-mix build. Proposed: make it a userspace supervisor program too, gated on milestone 523 (moving the job-mix supervisor into userspace) proving the idea.

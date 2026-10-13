---
status: NOT-STARTED
raised: 2026-09-09
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 269. `machine` at the prompt: ask what this computer is without rebooting it

calef, 2026-09-09: *"If we can get every OS to boot to swish, then we can run machine to get the
unified display of each machine."*

Milestone 268 builds the kernel-side description and settles what questions it
answers. This puts the same answers behind a program.

Depends on 268 and on every architecture reaching a prompt (milestone 182 for x86_64).

## Why it is a separate milestone rather than the same one

The two have different audiences and different failure modes, and only one of them can be a
program at all. 268's description prints in the kernel before userspace, because the moment you
need it most is when you are bringing up hardware that does not work yet and there is no userspace to
run anything from. A boot that dies before the prompt still prints it. That is the whole reason it
is where it is.

This one is for the machine that did boot, where the operator wants to look again without power
cycling, and where the output can be paged, filtered and piped like any other program's.

Splitting them means the same facts get described twice, which is the §76 status-in-two-places defect
in embryo. That is this milestone's central design problem, not an afterthought: the two must
share their source, or they will disagree, and the one that disagrees will be the one nobody boots.

## The question that decides the shape

Where do the facts come from when the program asks?

The kernel is the only thing that knows most of them. So either the description is captured at boot
and handed to userspace somehow, or the program asks the kernel at the time. The second is the
kernel-introspection case that DECISIONS §149 **deliberately declined to settle**, and this is
exactly the concrete need §149 said should exist before that question is answered in the abstract.

So this milestone is where that decision gets made, with a real consumer in hand.

## BUGS

- Nothing here is designed yet. This block records a decision and a dependency, not a plan.
- **It may be blocked on a decision that does not exist yet**, and that is deliberate: §149 refused
  to decide kernel introspection without a consumer, and this is the consumer.
- "The same answers" is the requirement and nothing enforces it. If the program and the boot
  path each format their own, they will drift, and the drift will be invisible because almost nobody
  compares a boot log against a command's output.

## Follow-on

- **Milestone 391.** Numbered on 2026-09-19 by milestone 433's drain of the pile. It has no
  `DECISIONS` section yet, and that is deliberate rather than an omission:
  `design/decisions/0149-kernel-served-console-endpoint.md` separated it from the console case so the
  general question would not be settled on the momentum of the narrow one, and said it should be
  answered when a real consumer exists. This milestone is that consumer, so the section gets written
  here rather than before.

## Index row

calef, 2026-09-09. The boot description serves the machine that did not come up; this serves the
one that did, and is the first real consumer of kernel introspection.

---
status: NOT-STARTED
raised: 2026-09-09
milestone_dependencies: 269
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 391. Kernel introspection over an endpoint, rather than one syscall per fact

Premise re-read against the tree on 2026-09-19 and still true: nothing has answered the fork.
`design/decisions/0149-kernel-served-console-endpoint.md` still refuses to settle it on the
console's momentum, and milestone 269, which that refusal named as the consumer to decide it
against, is still `NOT-STARTED`. So this block becomes a `design/decisions/` section when 269 is
taken, not before.

This is a design fork about the kernel's shape, not work to schedule.
The decision it waits on is [§149](../decisions/0149-kernel-served-console-endpoint.md). §149
(may the kernel answer on an endpoint) is
`DECIDED` and what it decided about *this* question is to refuse it. It separated the console case
from kernel introspection in a six-row table, on the ground that the console is **forced** by
hardware and introspection is **chosen**. And it says in its own words that *"a reader citing this
section for the general case is misreading it"* and that the introspection question *"should get its
own section when there is an actual fact to expose rather than in the abstract."*

So no `design/decisions/` section is minted here, and that is the answer rather than an omission.
Milestone 269 (`machine` at the prompt) is the consumer §149 named. It is still `NOT-STARTED`. Writing this fork up before
it is taken would put the general case in front of calef on the narrow case's momentum, which is the
exact thing §149 refused. The section gets minted when 269 is taken.

## In brief

Some facts exist only in the kernel: preemption counts, scheduler state, the machine description
milestone 268 prints at boot. Today there is no way for a program to ask for any of them, and the
obvious route is a syscall per fact, which grows a surface DECISIONS §10 and §16 exist to keep
narrow.

The alternative is a kernel thread parked on a rendezvous, answering questions. That mechanism
was verified while deciding the console case (`design/decisions/0149-kernel-served-console-endpoint.md`):
`inter_process_communication::Rendezvous` is generic over `T: Node` and has no privilege level in it, and kernel threads
already exist. It needs no new object type and no new syscall number.

## The argument that makes this more than tidiness

In a capability system, an endpoint is a better security primitive than a syscall, and it is easy
to get this backwards. A syscall is reachable by every thread in the system, so restricting who may
read the scheduler's state means writing a check inside the kernel and a rule about who should call
it. An endpoint is reachable only by a program that was handed the capability, and the right can be
revoked. One is rung one of AGENTS.md's ladder; the other is rung four wearing a syscall's clothes.

It also changes the growth rate of the boundary. Over a syscall, each new fact the kernel learns is a
new method to design, document and never remove. Over an endpoint, it is a message.

## What argues against it

- The kernel becomes a parser of untrusted input, inside the trusted computing base. A syscall is
  already that, so this is more of it rather than new, but it is code that must be verified rather
  than argued about, and milestone 193 only recently put `kernel/src` within reach of the prover.
- It erodes §14's minimal-core claim. seL4's position is that the kernel provides mechanism and
  no services. Read-only introspection is the cheapest possible violation of that, since there is no
  userspace owner these facts could belong to, but it is not zero and a demonstrator's claims are
  what it sells.
- "Only the kernel knows it" is sometimes a smell. For some of these facts the right answer may
  be to give a userspace owner the data rather than to build a way to ask the kernel.

## Why it is not decided yet, deliberately

`design/decisions/0149-kernel-served-console-endpoint.md` refused to settle this while deciding the
console, on the grounds that the console is *forced* by hardware and this is *chosen*. Deciding a
chosen thing on the momentum of a forced one is how a narrow surface stops being narrow.
It said the question should be answered when a real consumer exists rather than in the abstract.

Milestone 269 is that consumer, and this proposal should be promoted to a `DECISIONS` section
when that milestone is taken rather than before.

## BUGS

- No fact has been named as the first one to expose. Preemption counts are the example calef
  raised, and nobody has checked whether a program would actually want them.
- **The denial-of-service shape is inherited from §149 and not re-examined here.** A read-only
  service that never blocks inside a handler is a weaker version of the same problem, but "weaker" is
  an assertion until somebody looks.

## Index row

Some facts exist only in the kernel (preemption counts, scheduler state, the machine description
milestone 268 (every architecture boots the same way: describe) prints at boot) and no program can ask for them. The obvious route is a syscall per
fact, which grows the surface DECISIONS §10 (process model) and §16 (object revocation) keep narrow.
The alternative is a kernel thread parked on a rendezvous, answering questions, with no new object
type and no new syscall number, because `ipc::Rendezvous` is generic over `T: Node`. The argument is
easy to get backwards. A syscall is reachable by every thread, so restricting who may read
scheduler state means a check inside the kernel. An endpoint is reachable only by a program handed
the capability, and the right can be revoked. Against it: the kernel becomes a parser of untrusted
input inside the trusted computing base, and "only the kernel knows it" sometimes means the data
wants a userspace owner. §149 refused to settle a chosen thing on a forced thing's momentum, and
milestone 269 is the consumer that decides it.

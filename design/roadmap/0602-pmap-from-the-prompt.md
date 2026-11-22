---
status: NOT-STARTED
raised: 2026-09-26
milestone_dependencies: 47
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 602. `pmap` from the prompt

calef, 2026-09-26 (UTC), on #1365: *"take pmap out of milestone 126, and
pick among A to D later."* The choice among the four options below is deferred, not refused.
Promoted the same day from the proposal note
[`notes/process-view/pmap.md`](../../notes/process-view/pmap.md), written by the lane
`proposal/126-pmap`. *(Number and title provisional: the integrator mints the number at merge,
and the title is a draft until an architect names it.)*

The open decision is the choice among A to D, which no
`design/decisions/` section records yet. Milestone 47 (navigation and naming) is where the note's
source files the operand that `pmap <tid>` needs.

## The goal

A person at the shell prompt lists what another running job has mapped. Milestone 126 (the
`procps` package: who else is running) built the kernel method and the program:
`moho_abi::address_space::LIST`, gated by `ENUMERATE` under §114 (`ENUMERATE` extends to the
address-space object), and `crates/pmap` with `components/src/pmap.rs`, proven in
`kernel::user::pmap_tests`. Nothing live can be handed to the program. `ThreadControlBlock::CONFIGURE`
takes a space out of the registry `LIST` reads, and no spawner keeps a view of a child's space.

## The open decision

The note has the premises, the costs, the prior art and the refused options. This is its table.

| | A: retain the space | B: ask the domain | C: a size record | D: refuse |
|---|---|---|---|---|
| New method number | none (`LIST` exists) | one, on `Rendezvous` | none; one `SURVEY` record value | none |
| Kernel lifecycle change | yes, and `MAP_INTO` must be settled | no | no | no |
| Progenitor slots | one per live job, 1 spare | none (slot 7 reused) | none | none |
| Who gains power | nobody but `pmap` | every `ENUMERATE` holder on a domain, or a new rights bit | `ps`, `top` gain a size | nobody |
| Unit of consent | one space | the whole domain | the whole domain | n/a |
| What the user gets | the map | the map | the size | nothing new |
| Overturns a record | extends §142 past `Nothing` | none; widens §114's bit to a new object | none | none |
| Reversible | no: lifecycle and a wire field | no: a method number | mostly: one additive value | yes |

A keeps a narrowed view of each job's space in the progenitor. B adds a `Rendezvous` method over the
supervision domain, the way `ps` reads it. C gives a per-member page count on `SURVEY` and composes
with any of the others. D records that `pmap` does not reach the prompt. §142 (what a spawner retains
over a child after `START`) and the system call surface are both touched, so the note gives options and
no winner.

## What a lane can do before the ruling

Nothing on A or B. Each spends something irreversible, a lifecycle rule or a method number. Measuring
the two unmeasured costs in the note is safe and would help the ruling: the live count of address
spaces on a booted system, against `MAX_USER_SPACES` of 32, and what a walk that cannot outlive a
bound space costs.

## BUGS

- `pmap` runs nowhere but its kernel test. `crates/pmap`'s `BUGS` record it where a reader of the
  program meets it.

## Index row

`pmap` from the prompt: a person lists what a running job has mapped. The kernel method and the
program are built; how the program is handed a live space is calef's choice among four options,
deferred on 2026-09-26.

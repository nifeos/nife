---
status: PROPOSED
raised: 2026-10-20
milestone_dependencies: 890
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# The self-test proves a supervised death and a teardown that leaks nothing

*(Title and slug provisional.)*

Reuse: the spawn path `user::run` and the supervision protocol the progenitor already speaks
(`supervision_protocol`); nothing new is taken.

Raised 2026-10-20 (UTC) by lane `milestone/890-tours-to-self-test`. calef ruled on 2026-10-19
(UTC), on #2434, that every claim the per-port tours made becomes a check in `self_test::run`.
Two of `x86_64`'s did not, and milestone 890 (riscv64 and `x86_64` join the shared boot path)
records them in its BUGS.

## The two claims

- A faulting user-level thread's death reaches its supervisor as a message, naming the thread,
  the faulting pc and the address. `x86_64`'s tour built a child by hand from untyped, with a
  supervision endpoint, and received the FAULT event.
- Tearing down a process returns every frame it cost. The same tour built two children, tore
  both down, and did it again, printing the second round's frame cost, which was zero.

## Why they are not checks yet

No portable spawn takes a supervisor: `user::run` and `user::Spawn` have no supervision endpoint,
and the tour used `x86_64`-only hand-assembled children. A teardown check measured once, before a
spawn and after its teardown, also counts whatever else allocated in the window, which is why the
tour ran two rounds and reported the second.

## What would do it

A spawn helper that takes a supervision endpoint on all three ports, then two checks: `supervised`
(the FAULT event arrives with the right thread, pc and address) and `teardown` (a second round of
spawn and teardown costs zero frames). Both name checks that apply on every machine, so neither is
ever absent. `system_tests` already proves both claims; this moves them onto every boot, boards
included.

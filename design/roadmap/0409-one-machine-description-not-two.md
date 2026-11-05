---
status: NOT-STARTED
raised: 2026-09-14
promoted_from: one-machine-description-not-two
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 409. One machine description, not a description and a narrative saying the same thing

The machine description prints on all three architectures, and on two of them the arm above it had
already said most of the same things in its own words.

It reads the tree and moves lines within one function.

**Premise re-checked 2026-09-19 and still true.** `kernel/src/main.rs` calls
`arch::isa::print_summary()` in both the x86_64 and riscv64 arms and again inside
`print_machine_description`, which still carries its `#[cfg(not(any(test, feature = "bench")))]`.
So the duplication and the reason it cannot simply be deleted both stand as written.

## In brief

`kernel/src/main.rs`'s riscv64 and `x86_64` arms narrate their own bring-up, one line per step as
each piece comes up: `isa`, `firmware`, `memory`, `paging`, `pci`, `apic`, `clocks`, `io apic`. Then
`print_machine_description` answers the eight questions in one block, and several of the answers are
the same facts in a different layout. A riscv64 boot prints its `isa` and `firmware` lines twice.

The duplication is noise rather than a defect, and it is the honest cost of getting parity first:
the description had to land on all three before anything could be trimmed from either.

## What makes this less trivial than it looks

**The arms' lines serve an audience the description does not reach.** A `test` boot exits through
semihosting and a `bench` boot diverges into `bench::run`, and `print_machine_description` is
`#[cfg(not(any(test, feature = "bench")))]` precisely because neither is a boot anybody reads to
bring up a board. The riscv64 arm's `isa::print_summary` call carries a comment saying it is placed
where it is *"so that the test, shell and bench boots report it too"*. Delete it and those two
configurations stop saying what machine they ran on, which is a real loss on a bench.

And the timing differs, which is sometimes the point. The arms print each fact *at the moment
that piece comes up*, so a boot that dies halfway leaves a transcript that says how far it got. The
description prints everything at once, after everything is up, so a boot that dies before it prints
nothing at all. On a board those are different instruments, and milestone 267's lint exists because
somebody once moved a line between them.

## Roughly what to do

Decide, per duplicated line, which instrument it belongs to, and say so where it sits:

- facts a dying boot needs *early* stay in the arm;
- facts a healthy boot needs *collected* move to the description;
- facts a `test` or `bench` boot needs get a third home, or the description's `cfg` gets revisited
  (milestone 267's lint guards its body, not its signature, so that is a decision rather than a
  prohibition).

Measured, not guessed: a boot transcript per architecture before and after, in the block.

## Not a leveling-down

Nothing here removes a capability. The question is only whether one fact is printed once or twice,
and the default answer where it is unclear should be twice: a duplicated line costs a reader a
second, and a missing one costs a bench session.

## Index row

`kernel/src/main.rs`'s riscv64 and x86_64 arms narrate their own bring-up a line at a time, and
`print_machine_description` then answers the same eight questions in one block, so a riscv64 boot
prints its `isa` and `firmware` lines twice. The duplication is not simply deletable. The arms print
each fact as that piece comes up, so a boot that dies halfway still says how far it got, and they
are all a `test` or `bench` boot reports, since the description is excluded from both. The work is
to decide, per duplicated line, which instrument it belongs to, say so where it sits, and show a
boot transcript per architecture before and after.

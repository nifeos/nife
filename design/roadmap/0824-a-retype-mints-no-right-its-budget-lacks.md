---
status: BUILT
raised: 2026-10-07
built: 2026-10-12
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 824. A retype mints no right its budget lacks

*(The title and slug are provisional.)*

## In brief

`MemoryRegion::RETYPE` hands back a `PageFrame` with `Rights::ALL`, whatever the budget's rights.
calef ruled on 2026-10-07 (UTC), option (a): the frame's rights are the intersection of the budget's
rights and `Rights::ALL`. A `WRITE`-only budget mints `WRITE`-only frames, and `GRANT` can be
delegated but never minted.

## Why

Milestone 633 (an outside agent attacks the confinement claim)'s second pass found it, and its third
booted it as a characterization on all three ISAs: a holder of a budget that cannot delegate retypes a
page and then delegates it. It is the holder's own memory, so it is a scope gap, not a reach into
another domain. Claim 3 of `notes/confinement-claims.md` covers `SPLIT` and says nothing about
retype. Two options were refused with the ruling. Revoking after the fact comes too late, because
the delegation window opens at the mint. Narrowing `GRANT` alone is the intersection with a special
case in it.

## The work

- Intersect the rights at every retype arm in `kernel/src/syscall.rs` that mints from a budget
  (`RETYPE`, and `RETYPE_OBJ` if the same reasoning applies; say which and why).
- Flip `confinement_attack_tests::a_grant_less_budget_mints_a_grant_bearing_frame` from a
  characterization to a held assertion, renamed to say what it now holds, with a replayable
  falsification that restores `Rights::ALL`.
- Extend claim 3's row, or add one, so the table states the retype half.
- Record the method's new semantics in `design/decisions/`, as §10 (process model:
  capability-based, microkernel) requires of a method's behavior. Check every caller that
  retypes from a narrowed budget and then delegates, since it will now be refused.
- Remove the ruling's note from `cap::memory_region_cap`'s `BUGS`.

## Result

BUILT, 2026-10-12 (UTC), by lane `milestone/824-retype-mints-no-right`. `RETYPE` and every
`RETYPE_OBJ` arm (rendezvous, address space, thread, notification, timer, process) mint through
the budget's `Cap::mint_child`, the inheriting mint `split_never_widens_rights` proves. All six
object arms, because milestone 903 (a seventh outsider pass attacks the confinement claim) showed
the same `Rights::ALL` mint in each.

- The test: `confinement_attack_tests::a_retype_mints_no_right_its_budget_lacks` replaces the
  frame-only characterization. A `WRITE`-only budget and a spend-only one each mint a frame and
  every object kind with exactly the budget's rights, and the frame cannot be `SLICE`d. Two controls
  show the mint is the budget's rights, not a fixed set. `seventh_pass_retype_tests` flipped to a
  held assertion in the same commit. Each has a replayable falsification, red on aarch64.
- The callers: What a budget carries is now what everything carved from it carries, so the
  budgets changed shape. The root became `Rights::ALL`, because the progenitor's processes need
  `BIND`. The spend-only leaf budget became every right but `GRANT` (`abi::rights::SPEND_ONLY`,
  `cap::memory_region_cap`), since `WRITE` alone left a leaf's own notification unwaitable. Every
  builder or lender is handed `abi::rights::ALL`: the shell, login's session budget, the line
  editor's supervisor, a `std` program's socket-page region, and the kernel-side
  harnesses whose programs lend (`cap::memory_region_lending_capability`).
  `notes/memory-regions.md` has the three shapes.
- Claims row 3 now states the retype half and cites both tests.
- `cap::memory_region_cap`'s `BUGS` entry is gone; the doc says what the budget is now.

Names `SPEND_ONLY` (abi and kernel), `abi::rights::ALL` and `memory_region_lending_capability` are
provisional. The test names are provisional.

Outstanding, for the maintainer: the decisions section §10 (process model: capability-based,
microkernel) asks for, stating what a retype mints and that §16 (object revocation)'s sentence
"`RETYPE` mints a frame `READ|WRITE|GRANT`" no longer holds. A lane does not
edit `design/decisions/`. Risk 7's appendix also wants the citation under §216 (fatal-risk facts are
correctable, and verdicts are the architect's).

## Done when

The test holds on aarch64, riscv64 and x86_64, goes red under its record, and the claims table and
the decision record say what a retype mints.

**Reuse:** seL4's retype gives the new object full rights over itself (recalled, not read); this
ruling deliberately departs from it, so the decision record should say why.

## Index row

A frame retyped from a budget carries only the budget's rights, so `GRANT` is delegated and never
minted. Closes the near miss on claim 3 that milestone 633's passes carried.

## Follow-on

- **Recorded.** A builder handed `GRANT` on its budget so it can lend what it carves can also lend
  the budget itself. The sharpest case, a networked `std` program's job region, is in the `BUGS`
  of `crates/system_initializer/src/std_layout.rs`, and the narrower shape with its cost in
  `notes/memory-regions.md`.
- **Recorded.** The decisions section §10 asks for, and the correction of §16's sentence, are left
  to the maintainer; the reasoning they need is in `notes/memory-regions.md`, "What a retype
  mints".

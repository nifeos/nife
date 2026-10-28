---
status: NOT-STARTED
raised: 2026-09-05
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 390. The kernel's own spawn cannot put a capability at the slot a manifest names

Filed 2026-09-05 as an unnumbered proposal by milestone 111's lane, which
hit this and worked around it; numbered 2026-09-19 by milestone 433's drain of the proposal pile.
Premise re-read against the tree on 2026-09-19 and still true, to the digit: `kernel::user::Spawn`
in `kernel/src/user.rs` still carries `arg0`, `arg1`, `arg2`, `grants` and `maps` and no `placed`,
its `grants` field still documents "granted into slots 0, 1, 2, ... in order". There are still
exactly 91 `Spawn { .. }` literals in `kernel/`. The three named slots are still
`grant_plan::DOMAIN_SLOT` at 7, `DIAGNOSTICS_SLOT` at 8 and `ENTROPY_SLOT` at 9, and the userspace
builder this would mirror is still `supervision_contract`'s `placed: &'a [(u64, u64, u64)]` loop.
*(Number provisional until the merge queue lands it.)*

The mechanism already exists one layer up
(`supervision_contract::ChildEndowment::placed`) and the kernel side is a field and a loop. What needs
deciding is whether the 91 `Spawn { .. }` literals get a `..Spawn::new()` idiom or an added field,
which is a taste call inside the kernel and not a design fork.

In brief. `kernel::user::Spawn` grants capabilities into a child's table at slots 0, 1, 2, ... in
order, and offers no way to place one at a slot the caller names. Three named slots now exist
(`grant_plan::DOMAIN_SLOT` at 7, `DIAGNOSTICS_SLOT` at 8, `ENTROPY_SLOT` at 9), and **no test under
`script/test` can spawn a program holding any of them**. The only builder that can place at a
named slot is `supervision_contract::build_child`, which runs in userspace inside
`crates/system_initializer`. So every claim about a named slot's *endowed* direction is proven only
by `script/swish-check`, which boots the real init twice and is one gate rather than the suite.

## What it already costs, twice, in the tree as it stands

- **`date`'s declared second stream** (DECISIONS §67 (a program's second stream is a declaration, not a number)). `xtask`'s own swish-check list says it
  plainly: "the guest tests wire the shell from the kernel, whose `Spawn` fills a capability table
  from zero and cannot place a capability at the slot a manifest names, so `date` there never
  receives a second stream." Four assertions about `2>` live in swish-check for that reason alone.
- Milestone 111's entropy endowment. The refusal direction is a guest test on all three
  architectures (`kernel::user::uuid_tests`), because an *empty* slot needs no placement. The
  endowed direction has no guest test at all, on any ISA, and the milestone's own `BUGS` records it.

Both are the same missing feature wearing different clothes, and the count only goes up. A fourth
named slot inherits the gap for free.

## Why this is worth a lane rather than a shrug

**It is a parity claim that no ISA runs.** DECISIONS §19 says a kernel capability ships on every
supported architecture proven by the same suite, or a scope note records the gap. Here the gap is
not per-ISA, it is total: the suite proves the refusal everywhere and the grant nowhere, and the one
thing that does prove the grant runs on two architectures rather than three (`script/swish-check`
has no x86_64 leg).

**And swish-check is a boot, not a unit.** It types at a prompt and greps a transcript, so it can
say "the row printed" and cannot say "the capability carried exactly `WRITE` and not `READ`". A
guest test holds the `Cap` it granted and can assert the rights on it, which is the half that
actually distinguishes an over-grant from a correct one. Milestone 126 found a real `READ`-instead-
of-`ENUMERATE` over-grant on a named slot by reading code, and nothing in the suite would have
caught it.

## The shape

`Spawn` grows a `placed: &'a [(u64, crate::cap::Cap)]` beside `grants`, and `run` inserts each one
at the slot named after the positional grants are laid down, which is exactly what
`supervision_contract::build_child` already does for the userspace path. The two loops should read the
same, because they are the same operation on the same table.

The only real work is the 91 existing `Spawn { .. }` literals. Two options and neither is
interesting: give `Spawn` a `new()` returning the all-empty endowment so sites end in
`..Spawn::new()` (which is `ChildEndowment`'s own idiom, and makes the next field free), or add the
field and update 91 sites once. The first is better and is why this is a lane rather than a patch.

## What it unblocks

- A guest test that `uuid` holding a real entropy endpoint prints a v4 identifier, on all three
  architectures, with the rights on the capability asserted rather than inferred from a transcript.
- The same for `ps`/`pgrep`/`watch`'s domain slot, which today is proven by
  `kernel::user::survey_tests` building its own domain rather than by a spawn that mirrors init's.
- `date`'s second stream under `script/test`, retiring four swish-check lines that exist only
  because nothing else can run them.
- Every future named slot, which currently starts life untestable.

## What it does not do

It does not make `Spawn` a second implementation of the loader. Init stays the ELF loader the shell
directs (milestone 19d), and this changes only what the kernel's own test-support spawn can express
about a table it is already filling.

## Index row

`kernel::user::Spawn` grants capabilities into a child's table at slots 0, 1, 2 and so on in order,
and offers no way to place one at a slot the caller names. So no test under `script/test` can spawn a
program holding any of the three named slots (`grant_plan::DOMAIN_SLOT` at 7, `DIAGNOSTICS_SLOT` at
8, `ENTROPY_SLOT` at 9). The only builder that can place at a named slot runs in userspace inside
`crates/system_initializer`, which means every claim about a named slot's *endowed* direction is
proven by `script/swish-check` alone. One gate rather than the suite, on two architectures rather
than three. That makes it a total parity gap rather than a per-ISA one, which is the case DECISIONS
§19 (architectural parity is a tenet) is about. Swish-check is a boot rather than a unit, so it can say the row printed and cannot
say the capability carried exactly `WRITE` and not `READ`. Milestone 126 found a real
`READ`-instead-of-`ENUMERATE` over-grant on a named slot by reading code, and nothing in the suite
would have caught it. It already costs the tree twice, in `date`'s declared second stream and in
milestone 111's entropy endowment, and a fourth named slot inherits the gap for free. The shape is a
`placed` field beside `grants` and a loop that reads the same as
`supervision_contract::build_child`'s, since it is the same operation on the same table. The only
real work is giving `Spawn` a `new()` so the 91 existing literals can end in `..Spawn::new()` and
the next field after this one is free.

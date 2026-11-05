---
status: NOT-STARTED
raised: 2026-09-04
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 373. The board-only kernel features nothing compiles, until somebody is standing at the board

Checked on 2026-09-19, half the premise has stopped being true, which narrows the work and does
not close it. `script/lint`'s per-feature
clippy loop now builds `board`, `fastpath_pad` and `soak_test` on both aarch64 and riscv64, and a
second loop builds `shell`, `soak_test`, `job_mix` and `board` on x86_64, so the individual features
are no longer uncompiled. Two things are still exactly as this file describes them. `single_hart`
and `reboot_soak_test` appear nowhere in `script/lint`, so nothing but a person's own
`script/board-image` run has ever compiled either. And no combination is built: every one of the
six feature sets below is a combination, clippy builds each feature alone, and a `board,bench,single_hart`
card is a configuration nothing in CI has ever seen. Two feature names in the body are pre-297
spellings: `soak` is `soak_test` and `reboot_soak` is `reboot_soak_test`.

It is a build matrix, and the expensive question (which of these should also *run*
somewhere) is deliberately left out of it.

## In brief

`kernel/Cargo.toml` carries feature flags whose only consumer is a microSD card:
`board`, `soak`, `job_mix`, `reboot_soak`, `single_hart`, and `fastpath_pad` beside them. Nothing
in CI compiles any of them. `script/lint`, `script/test`, `script/bench` and every workflow build
the default and test kernels; a card build happens when a person runs `script/board-image`, which is
minutes before they walk to the bench.

So an ordinary refactor in `kernel/src/sched.rs` or `kernel/src/smp.rs` can break a card build, and
the tree stays green for as long as nobody writes a card. The failure surfaces as a bench session
that starts with a four-minute cargo error instead of a boot. That is the most expensive place in
this project to discover a compile error: the board is powered, the card is out, and the person is
not at their desk.

## What it would be

A CI job, or a leg of `script/ci-build`, that runs `cargo build -p kernel --release --target
riscv64imac-unknown-none-elf` once per feature set the tree actually writes to a card:

```
board
board,soak
board,soak,reboot_soak
board,job_mix
board,bench,single_hart
board,bench,single_hart,fastpath_pad
```

Six release builds of one crate. It proves compilation and nothing else, which is the point: it is
the cheap half, and it catches the failure that costs the most.

## What it is not

**It is not "run the soak in CI".** These features exist because their workloads need a board;
`reboot_soak` will not even compile off riscv64 by deliberate design, and a `job_mix` sweep under TCG
is the rehearsal `script/job-mix` already provides. Compiling is the part that generalizes.

**And it is not a fourth architecture's problem.** `fastpath_pad` has no x86_64 module and that is
already recorded in `script/fastpath-footprint`'s BUGS; a matrix would make the gap visible rather
than fix it.

## The same shape one level out

The list above is hardcoded, which is finding 11 of notes/architecture-list-sweep.md wearing new
clothes. The seventh card feature somebody adds is measured here only if they remember to type it.
Deriving it from `[features]` is possible and is a second decision, because not every feature in
that file belongs on a card.

## Index row

`kernel/Cargo.toml` carries feature flags whose only consumer is a microSD card. An ordinary
refactor in `kernel/src/sched.rs` or `kernel/src/smp.rs` can break a card build while the tree stays
green, and the failure surfaces as a four-minute cargo error at the start of a bench session.
`script/lint` clippy-builds most of these features individually. `single_hart` and
`reboot_soak_test` are compiled by nothing, and no combination a card uses is built. Six release
builds of one crate would cover the set. They prove compilation and nothing else: running a soak in
CI is not the ask, since these workloads need a board and `reboot_soak_test` will not compile off
riscv64 by design. The list is hardcoded, as finding 11 of notes/architecture-list-sweep.md
describes. Deriving it from `[features]` is a second decision, because not every feature belongs on
a card.

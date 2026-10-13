---
status: NOT-STARTED
raised: 2026-10-09
promoted_from: a-suppression-carries-its-reason-and-expects-to-be-needed
milestone_dependencies: none
decision_dependencies: 38
machine_requirements: none
specific_machine: none
needs_person: no
---
# 893. A suppression carries its reason in the attribute, and fails when it is no longer needed

*(The title and slug are provisional.)*

One of five lint milestones. Its numbers are in [notes/lint-census-2026-10-09.md](../../notes/lint-census-2026-10-09.md).
It reuses the lint ratchet specified in
[the panic proposal](0896-panics-in-the-kernel-and-the-parsers-are-counted-and-fall.md).

Reuse: rustc's `#[expect]` and its `reason =` field, and clippy's `allow_attributes`,
`allow_attributes_without_reason` and `ignore_without_reason`. Nothing is written.

## The ruling, 2026-10-09 (UTC)

calef ruled on the thresholds on 2026-10-09 (UTC). Suppression counts are frozen per file at today's
value and may only go down. That is the mechanism. Zero stays the end goal, with no date attached,
so done criteria 3 and 4 below state it.

## The problem

§38 (a suppression is scoped to an item and carries a reason) has two halves. `script/lint`
enforces the first: no module-wide `#![allow(dead_code)]`. The second is that the attribute says
which configuration has no caller. §38 calls it a review expectation, and nothing checks it. An
`allow` also stays silent when the thing it suppresses goes away. §38's own triage found what that
costs: a doc comment asserting a test that had moved, and a confinement check with no caller on
riscv64.

Measured over the note's non-test scope:

- 457 `allow(...)` attributes, 354 of them in the kernel. `kernel/src/user.rs` alone has 50.
- 344 suppress `dead_code`. 296 of those already take §38's preferred form,
  `#[cfg_attr(<configuration>, allow(dead_code))]`, so the predicate states the claim. 48 are bare
  `#[allow(dead_code)]`, 38 of them in the kernel.
- 28 suppress a `clippy::` lint and 7 an `unused*` lint.
- Clippy, across the five compiled configurations, flags 380 `allow_attributes_without_reason`
  hits (294 kernel) and 318 `allow_attributes`.
- One `#[expect(...)]` exists. Counting test code too, 11 of 577 `allow` attributes already carry
  `reason =`.

The request's other two counts did not survive measurement:

- `TODO`, `FIXME` and `XXX`. A gate already exists: `script/lint`'s "TODO markers cite a
  milestone", from milestone 94 (the untracked-work sweep). It finds no marker in Rust. The word
  appears on 107 tracked lines at the base, 97 of them prose in `design/`, `notes/` and `script/`.
  The 7 in Rust are prose too. At the census, four of them pointed at a TODO that no longer
  existed: `kernel/src/sched.rs`, `kernel/src/thread.rs` and `crates/paging/tests/mapping.rs` cited
  "the TODO on `paging::unmap`", and `crates/elf/src/lib.rs` cited "the TODO in the kernel's
  loader". 1c7849691 fixed all four (done criterion 5).
- `#[ignore]`. There are 3 attributes, not 6, and all 3 carry a reason.

## The mechanism, and its rung

Rung two, and the compiler is the gate. `#[expect(lint, reason = "...")]` makes two claims a
reader can check: why, in the reason, and that the lint fires, which rustc tests. When the dead
code gains a caller, `unfulfilled_lint_expectations` fails `-D warnings`. That is the half §38's
`cfg_attr` form could not do: a `cfg_attr(..., allow(dead_code))` on an item that became live
stays quiet.

The migration runs through the lint ratchet, with `allow_attributes_without_reason` and
`allow_attributes` counted per file. When both reach zero, they move into `[workspace.lints]` and
leave the ratchet, as `undocumented_unsafe_blocks` went in once its 205 sites were fixed.

`ignore_without_reason` costs nothing: all 3 `#[ignore]` attributes already comply. It goes in
`[workspace.lints]` now, and that fits §61 (a lint is adopted on evidence from this tree) as
written.

## Done when

1. `ignore_without_reason` is in `[workspace.lints.clippy]`, with its count beside it.
2. The ratchet counts `allow_attributes_without_reason` and `allow_attributes` per file, frozen at
   each file's count on 2026-10-09 (UTC), and a row may only go down.
3. Every `allow` in scope is an `#[expect(..., reason = "...")]`, or an `allow` with a reason and a
   comment saying why it cannot be an `expect`. Zero is the goal, with no date attached.
4. `allow_attributes_without_reason` and `allow_attributes` are in `[workspace.lints.clippy]`,
   and the ratchet's rows for them are gone.
5. The four comments that cite a missing TODO are corrected to say where the work went, or
   deleted. **Done 2026-10-09 (UTC).** The `paging::unmap` TODO was resolved as a design in
   d06791f73 (2026-07-22), so the three that cited it now cite `paging::Mapper::unmap` and
   notes/teardown.md. The loader TODO `crates/elf` cited was never written: the reference dangled
   from 0c7793dd7, in milestone 7c (user mode: EL0, capabilities, the ELF loader, and IPC). It now
   cites the linker script that keeps segments apart.
6. §38 is amended to say the reason now lives in the attribute, since its second half becomes a
   gate.

## Open questions for an architect

1. Where an `expect` cannot hold in every configuration its `cfg_attr` selects, which wins: a
   narrower predicate, or a recorded `allow`? An `expect` must be true in every build that sees
   it, and an `allow` need only be harmless. How many of the 296 break is a measurement the lane
   takes first.
2. The x86_64 kernel pass in `script/lint` runs `-A dead_code`. Whether a command-line `-A`
   leaves an item's `#[expect(dead_code)]` unfulfilled was not tested here; the lane checks it
   before converting the kernel.
3. Should a reason name the caller ("the system tests call it") or the configuration? §38 treated
   the `cfg` predicate as the reason. The recommendation is the caller, since the predicate already
   names the configuration.
4. Should the lint ratchet count an `#[expect]` on a ratcheted lint as a hit? If it does not, a lane
   can lower a panic count by suppressing it. The recommendation is that it counts.

## Where it sits in the ranking

The customer path is vacant, so the tie breaks toward the fatal risks. This serves risk 3 (the tests
do not test anything, and the quality is illusory). An unchecked suppression is a claim nobody
checks, the same shape as a test that cannot fail. It ranks fourth of the five: the gate is the
compiler's, and most of the work is mechanical once open question 1 is answered.

## Index row

Every lint suppression carries its reason in the attribute and is an `#[expect]`, so it fails the
day it is no longer needed. Counts are frozen per file and may only fall.

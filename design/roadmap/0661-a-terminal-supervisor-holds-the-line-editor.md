---
status: BUILT
raised: 2026-09-26
built: 2026-09-27
promoted_from: a-terminal-supervisor-holds-the-line-editor
---
# 661. A terminal supervisor holds the line editor

Status BUILT 2026-09-27: built on PR #1382. *(Title and slug are drafts.)*

Raised by the lane for milestone 23 (a capability-routed component
OS with live replacement) on reaching step 4 of the `line_editor` swap, "the swap in
`system_initializer`". The swap cannot live there as that process is built today.

Where the authority to replace the terminal lives is a design fork, and the
recommended answer adds a program, whose name is an architect's.

## Ruled

calef, 2026-09-27T06:31Z: option A. Built the same day on PR #1382: `terminal_supervisor` (name
provisional, calef's to name), with its guest test on all three architectures. The trigger is
`0694-the-installer-asks-the-terminal-to-swap.md`.

## Why it cannot be `system_initializer`

Two facts, both recorded in its own source.

- **Its capability table is full.** `kernel::cap::CAPABILITY_TABLE_PEAK_MEASURED` is 23 of 24, and
  that constant's comment says the next permanent capability "should buy a slot back rather than
  spend the last one". A swap needs the terminal's endpoint, its output-sink objects, the two
  client pages, a control endpoint and a handoff run held for the life of the boot: six or more.
- **It gives the terminal away on purpose.** After the shell is built it frees `term_in`, then
  `term_ep` and `term_out` once their last use is done, "and after that this process holds no way
  to reach the terminal at all" (`crates/system_initializer/src/lib.rs`, milestone 22 (trusted init:
  verify it, and shrink what a broken one can do)). A progenitor that kept the power to replace the
  terminal would undo that.

## Options

- A. A terminal supervisor (recommended; provisional name `terminal_supervisor`). A small
  program `system_initializer` builds and hands every terminal-side object plus a budget for one
  replacement. It builds `line_editor` itself, holds its endowment, its control endpoint and the
  handoff run, and runs the §209 (state handoff is an opaque blob over a granted frame, and it is
  optional) swap when asked on one endpoint. `system_initializer` keeps that one endpoint, or, if
  the trigger is the package activation path of milestone 198 (a package manager, and the trivial
  install), hands it to whatever serves activation. It is `swapper`'s shape (an unprivileged
  operator that holds exactly the objects it swaps), which is what DECISIONS §41 (the endpoint is
  the broker, and a device is revoked by taking it back) already argues for.
- B. Raise the table to 32. The rule milestone 230 (`script/shell-check` is red on `main`) left is to buy slots back rather than raise it, and
  B would still leave the progenitor holding terminal authority it deliberately gives up.
- C. Swap only under the kernel test harness. Proves the protocol, which
  `terminal_quiesce_tests` already does for the incumbent's half, and ships nothing a person can use.

## What is blocked until this is answered

Step 4 of the `line_editor` swap and its guest test. Steps 1 and 3 are built on
`milestone/23-line-editor-swap`; step 2 waits on PR #1338.

## If the answer is A

The swap is triggered by whatever activates a new `line_editor` build. That is milestone 198's
installer, so the trigger is a message between two lanes, and its wire shape (one verb, one image
name) is the other thing an architect sees before it is built.

## Follow-on

- **Milestone 694.** Milestone 694 (the installer asks the terminal to swap). The installer asking the terminal to swap, which nothing on a real boot does yet: `design/roadmap/0694-the-installer-asks-the-terminal-to-swap.md`.

## Index row

`system_initializer` cannot hold the authority to replace the terminal, because its capability table is full. Ruled option A by calef on 2026-09-27: a separate terminal supervisor program holds it, built on PR #1382.

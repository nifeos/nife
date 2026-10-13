---
status: BUILT
raised: 2026-09-26
built: 2026-09-27
promoted_from: swap-line-editor-live-under-system-initializer
---
# 669. Swap `line_editor` live under `system_initializer`

Status BUILT 2026-09-27: built on PR #1382. *(Title and slug are drafts.)*

Raised by the lane for milestone 23 (a capability-routed component
OS with live replacement), which built state handoff and then checked why the interactive stack is
still not swapped. The full account is notes/interactive-stack-swap.md.

Two changes to the terminal contract, which eleven programs speak, so both are
wire-format calls for an architect: an additive quiesce opcode in `line_editor::proto`, and what a
reader parked in `OP_READLINE` is told when a swap begins. The recommendation for each, with the
options that lost, is in the note.

## Ruled

calef, 2026-09-26: "1a and 2b". Fork 1: an additive `OP_QUIESCE` in `line_editor::proto`, sent
only by a supervisor, riding the served endpoint's FIFO. Fork 2: a new reply flag meaning "ask
again" (`FLAG_RETRY`, provisional); a reader handles it by re-issuing the same request.

Two things follow that the ruling did not say. The handoff grows a page count rather than dropping
history: the coordinator's reversible default, not calef's ruling, and it needs
`0659-a-region-retypes-a-frame-run.md` first. And calef's option D on #1361 (the shell edits its own line) moves `swish` to raw mode,
parked in `OP_READRAW`. So `FLAG_RETRY` must answer a parked `OP_READRAW` too, and the swap test's
witness is a line-mode client of `line_editor` as well as `swish`.

## Built

2026-09-27, on PR #1382, under a terminal supervisor rather than `system_initializer` (calef's
option A of `0661-a-terminal-supervisor-holds-the-line-editor.md`). What this proposal still carries is
the record of the two forks; an integrator promoting it can mark it built.

## What to build

- A `component_plan` declaration for `line_editor`, after the ELF-note manifest lane (PR #1338)
  settles where a manifest lives, so the declaration is written once in its final home.
- A handoff page count in `component_plan::Handoff`: `line_editor`'s state with history is a little
  over one page. Or drop history across a swap and record it as a limit.
- The swap itself in `system_initializer`, which already holds every object `line_editor` was built
  from: `swapper`'s `ROLE_HANDOFF` sequence (drain, absorb or roll back, retire) against a real
  component, with `swish` parked at the prompt across it.
- A guest test with `swish`'s own view as a witness: the person's half-typed line survives, and no
  keystroke is lost.

## What it unblocks

The last `Outstanding` line of milestone 23's block, for the one interactive component a userspace
supervisor can reach today.

## Follow-on

- **Milestone 694.** Milestone 694 (the installer asks the terminal to swap). The trigger that asks the terminal supervisor to swap, which nothing on a real boot does yet: `design/roadmap/0694-the-installer-asks-the-terminal-to-swap.md`.

## Index row

The interactive stack is still not live-swappable. Ruled by calef on 2026-09-26 (an additive `OP_QUIESCE` and a retry flag): two changes to the terminal contract that eleven programs speak.

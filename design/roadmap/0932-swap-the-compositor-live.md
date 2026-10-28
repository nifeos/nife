---
status: NOT-STARTED
promoted_from: swap-the-display-terminal-live
raised: 2026-10-19
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 932. Swap the compositor live

*(Number, title and slug provisional.)*

Milestone 23 (a capability-routed component OS with live replacement) swaps `line_editor` and
`display_terminal` under `terminal_supervisor`. The compositor is the interactive stack's third
component, and calef split it out on 2026-10-19 (UTC), "Split": it waits on a boot that runs the
compositor, which no milestone schedules yet. That missing milestone is the `unwritten`
dependency above.

## What the tree says

- No boot builds the compositor. Only `kernel/src/user/compositor_service.rs` spawns it, under the
  tests of milestone 33 (a compositor: one screen, mutually distrusting clients). The graphical
  session of milestone 632 (graphics on demand) runs `display_terminal` on the driver directly,
  with no compositor in the path.
- Its state is small. Each client's surface is a frame the client owns, so a replacement wired from
  the same capabilities gets the same surfaces. What it would carry is the window list, the focus
  and each client's acknowledged sequence.
- Its quiesce is a contract change. `compositor::proto` has `HELLO` and `COMMIT` and no
  `OPERATION_QUIESCE`, and every windowed client speaks it, so the opcode is an architect's.
- A replacement must republish every client's control page before a client's next `COMMIT` reads
  it.

## The work, once a boot runs it

- The quiesce opcode on `compositor::proto`, ruled first.
- Its spawner supervises its swaps, the rule milestone 23 applies to the terminal: whoever builds
  the compositor on that boot holds it, through `hot_swap_contract` and `supervision_contract`.
- A test in the shape of `system_tests::user::terminal_display_swap_tests`: two windows drawn
  before a swap and after it, compared pixel for pixel with the scene the kernel computes.

Swapping it under milestone 33's harness alone would measure the harness, which is why it waits.

## Reuse

`terminal_supervisor`'s bounded quiesce and its bays, and §209 (state handoff is an opaque blob
over a granted frame, and it is optional) for the window list. Prior art, recalled rather than
read: a Wayland compositor restart loses every client, which is the outcome a live swap exists to
avoid.

## Index row

The compositor is the one interactive component milestone 23 does not swap, because no boot runs it. Swap it live once one does: a quiesce opcode on `compositor::proto`, its spawner as supervisor, and a two-window pixel test across the swap.

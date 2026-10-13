---
status: NOT-STARTED
raised: 2026-09-27
promoted_from: the-installer-asks-the-terminal-to-swap
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 694. The installer asks the terminal to swap

*Title and slug are drafts.*
Raised by the lane for milestone 23 (a capability-routed component
OS with live replacement) after building `terminal_supervisor`, which can replace `line_editor` live
but which nothing on a real boot asks to. calef asked for the trigger to be written up as the next
ask rather than built (2026-09-27).

Two programs would agree on a message, and one image would travel between them,
so both the wire shape and the route are an architect's.

## What exists

`terminal_supervisor` serves a swap endpoint when it is started with one, and answers `SWAP` with
`SWAPPED` or `ROLLED_BACK` (`line_editor::component::supervisor`). The kernel's guest test holds that
endpoint. On a boot, `system_initializer` passes none, so the supervisor builds the terminal and
waits. And a replacement today is the same image the supervisor was handed at birth.

## What the trigger needs to decide

1. **Who holds the swap endpoint.** The activation path of milestone 198 (a package manager, and
   the trivial install) runs in `system_initializer`'s spawn service, which is where a new
   `line_editor` build becomes the active one. Recommended: `system_initializer` retypes the swap
   endpoint when it builds the supervisor, hands it `READ`, and keeps `WRITE` for the activation
   path. That is one permanent capability in a table measured at 23 of 24
   (`kernel::cap::CAPABILITY_TABLE_PEAK_MEASURED`), so it has to buy a slot back, and the note on
   that constant names the two candidates.
2. How the new image reaches the supervisor. The supervisor cannot read the archive or the
   package store. Recommended: the activation path copies the vouched bytes into a frame run it
   retypes with `MemoryRegion::RETYPE`'s page count. It sends that capability with the request
   (`SEND_CAP`), so `SWAP` carries its image and the supervisor maps it where it maps its own.
   Refused: a second copy at build time, which would make the supervisor carry every future build.
3. What an activation does when the swap rolls back. The new build refused the old state, so
   the old instance serves on. The activation set would then say the new build is active while the
   old one runs; the recommendation is that activation waits for `SWAPPED` and records a refusal as
   a failed activation, which the installer already has a verdict for.

## What it unblocks

A person installing a new `line_editor` and the terminal they are typing in changing under them,
losing nothing: milestone 23's claim on a component a person uses, end to end.

## Index row

`terminal_supervisor` can replace `line_editor` live but nothing on a real boot asks it to. Proposed: the installer asks it to swap, which fixes a message and a route two programs agree on.

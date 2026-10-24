# Swapping the interactive stack

*Which of the interactive stack's three components can be swapped live, for milestone 23 (a
capability-routed component OS with live replacement). `line_editor` and `display_terminal` are
swapped live by `terminal_supervisor`. The compositor is not, because no boot runs it: milestone
932 (swap the compositor live).*

## What the tree says, checked

| component | who builds it on a real boot | who could swap it | state it holds |
|---|---|---|---|
| `line_editor` | `system_initializer`, out of its own budget (`crates/system_initializer/src/lib.rs`, item 3) | `system_initializer`: it holds every object it routed | the edit line, the kill buffer, eight lines of history, up to four queued lines, raw or cooked mode, the interrupt count: under 4 KiB, measured (see BUGS) |
| `display_terminal` | `system_initializer`, at a `graphical_terminal` launch (`build_graphical_terminal_session`; no boot builds it, per milestone 632 (graphics on demand)); `terminal_supervisor`, when a spawner hands it the image | `terminal_supervisor`, when it built it: only the kernel's test does that today | the character grid and scrollback, the cursor, the rendition and the parser's state (`video_terminal::Vt`), up to 132 pages as a blob |
| `compositor` | no boot builds it. Only the tests of milestone 33 (a compositor: one screen, mutually distrusting clients) spawn it (`kernel/src/user/compositor_service.rs`, `#[cfg_attr(not(test), ...)]`); the graphical stack (option A of milestone 177 (wire the graphical terminal stack into the real interactive boot), built at a `graphical_terminal` launch) has "no compositor in this path" | nobody outside a test | nothing it could not rebuild: each client's surface is a frame the client owns |

All three run under the kernel test harness. Outside it, two are built and only `line_editor` has
a supervisor that could swap it.

`display_terminal` and `compositor` have no `capability_plan` declaration. `display_terminal`'s
slots are one list every builder reads (`video_terminal::component`), because two of them are a
map budget and frame runs, which `capability_plan::Direction` has no word for. A declaration for
either is written against the ELF-note manifest of milestone 597 (a program carries its manifest
in an ELF note).

## line_editor: swapped live

Its supervisor exists, its state fits a page, and §209 (state handoff is an opaque blob over a
granted frame, and it is optional) is built (notes/state-handoff.md). The terminal contract is a
wire format eleven programs speak (`swish`, `rmle`, `mdr`, `window`, `input`, `keyboard_driver`,
`console`, `display_terminal`, `compositor`, `terminal_sink_caretaker` and `line_editor` itself),
so its two changes were calef's. He ruled them on 2026-09-26, and on 2026-09-27 option A of
milestone 661 (a terminal supervisor holds the line editor):

- **A quiesce request is an additive `OPERATION_QUIESCE`** in `line_editor::proto`, which changes no
  existing opcode and which only a supervisor sends. The swap's drain step rides the served
  endpoint so its FIFO does the waiting, per §41 (the endpoint is the broker, and a device is
  revoked by taking it back), as `hot_swap_contract` does. Not a second, supervisor-only control
  endpoint, because that loses the FIFO property and makes the drain racy.
- **A parked reader is told `FLAG_RETRY`**, meaning "ask again". At an idle prompt `swish` is always
  parked in `OPERATION_READLINE`, and its one-shot reply capability lives in `line_editor`'s
  capability table, where no one can move it (notes/hung-component.md, question 3), so the
  incumbent answers it before it quiesces. `swish` re-issues the same `OPERATION_READLINE`; the
  replacement, having absorbed the edit line, redraws it; the person sees nothing. Not
  `FLAG_INTERRUPTED`, because `swish` would redraw its prompt as if the person had pressed `^C`,
  which lies about why. Not deferring the swap until no read is parked, because at an idle prompt
  that is never. The flag name is provisional.

What is built:

- `line_editor::component` declares what a supervisor routes to it, with one declaration per
  output-sink shape. `line_editor::handoff` is its blob: the edit line and cursor, the kill buffer,
  the history ring, the prompt, the queued lines, raw mode and its queue, and the `^C` count. A
  supervisor mints the run with `MemoryRegion::RETYPE`'s page count, and `line_editor` declares one
  page.
- `terminal_supervisor` (name provisional, calef's to name) is what `system_initializer` builds. It
  builds `line_editor` from the declaration and runs the §209 swap when asked on its swap endpoint.
  Instances are built in two reusable bays, because a replacement split straight from the budget
  sits above the incumbent and leaves a hole when it dies, so a second swap fails for want of
  budget.
- `kernel::user::terminal_swap_tests` swaps it twice under a typist and a parked reader on all
  three architectures: a line half typed before the swap finishes after it, and history typed
  before it is recalled after it. `script/swish-check`, plain and graphical, boots through it.
- The quiesce is bounded. A stand-in built from the supervisor's own image makes the `CALL` and
  signals the answer on a notification the supervisor's timer also signals. A `line_editor` that does not answer within
  `QUIESCE_BOUND_MS` is replaced without its state and its region destroyed, which answers its
  callers `Gone`. The second test in `terminal_swap_tests` hangs one behind a console that stops
  acknowledging. A spawner that passes a swap endpoint copies the supervisor's image and grants a
  stand-in's budget; one that passes none, which is every boot today, grants nothing extra.

What is left is the trigger. [§308 (the installer asks the terminal to swap)](../design/decisions/0308-the-installer-asks-the-terminal-to-swap.md)
rules it, and `design/roadmap/0694-the-installer-asks-the-terminal-to-swap.md` builds it.

## display_terminal: swapped live through the swap reserve

calef ruled its three questions on 2026-10-19 (UTC), and milestone 23's block records the rulings.
How it works:

- **The holder is its spawner.** `terminal_supervisor`, in display mode and handed
  `display_terminal`'s image, builds it before `line_editor` and holds it beside it. `SWAP`'s second
  word is `CHILD_LINE_EDITOR` or `CHILD_DISPLAY_TERMINAL`. Each child has its own fault endpoint,
  so a death collected is always the child's it was waited for.
- **The pages come from the swap reserve.** The supervisor's budget holds one `display_terminal`
  bay, not two. A swap is lent the machine's reserve (`supervisor::SWAP_RESERVE_PAGES`) with
  `LEND` first; without it the answer is `NEEDS_RESERVE`. The supervisor retypes the handoff run
  at the reserve's base, builds the replacement in the rest, and swaps into it. A swap is
  then two moves: into the reserve, and home to the bay with the same image, which reads its own
  blob. The supervisor deletes every capability it made from the reserve before it answers, and
  the lender's `DESTROY` revokes the run's mappings. A second swap while the reserve is lent finds
  nothing to split.
- **The handoff is everything.** `video_terminal::handoff` writes the live grid, the scrollback
  oldest first, the cursor, the rendition and the parser's state, three little-endian words a
  cell. Because the run exists only during a swap, it is sent rather than mapped at build:
  `CTL_HANDOFF`, a `SEND_CAP` on the control endpoint, answered `NOTE_WRITTEN` once the blob is in
  it. A replacement started `START_ABSORB` takes the same message before it paints, so its first
  picture is the incumbent's screen rendered from the absorbed state.

`system_tests::user::terminal_display_swap_tests` is the proof, on all three architectures.

## compositor: not before it has a boot path

It is the easiest of the three on paper: no state worth moving, and every surface a client owns is a
capability its supervisor routed, so a replacement wired from the same declaration gets the same
surfaces. But no boot runs it. Swapping it under the milestone 33 test harness alone would measure
the harness.

## Milestones

- Milestone 669 (swap `line_editor` live under `system_initializer`) is built, under a terminal
  supervisor rather than `system_initializer`, as ruled.
- Milestone 600 (the graphical terminal stack is built in userspace) is built, so
  `system_initializer` builds `display_terminal`.
- Milestone 932 (swap the compositor live) waits for a boot that runs the compositor.

## BUGS

`terminal_supervisor`'s, which its module doc lists:

- Nothing on a real boot asks it to swap. `system_initializer` passes no swap endpoint, so the
  supervisor builds the terminal and waits; only the kernel's test swaps. Milestone 694 (the
  installer asks the terminal to swap) builds the trigger, and §308's frame run, sent with `SWAP`,
  is how a new image arrives. Until then a replacement is the same image, and the swap reserve is
  held by the kernel's test.
- A `graphical_terminal` session builds its own `display_terminal` and hands the supervisor its
  endpoint, so a session's `display_terminal` is not supervised. Holding it would cost the session
  the image's 220 pages and buy nothing until a boot can ask for a swap.
- A `display_terminal` replacement that refuses its own blob on the move home leaves the terminal
  serving from the reserve, answered `IN_RESERVE`. The reserve stays lent until a later `SWAP`
  finishes the move. Same-image refusal is what the host round trip in `video_terminal::handoff`
  is there to rule out.
- A hung `line_editor`'s state is lost, and its callers are told `Gone`. Only the incumbent can
  write the blob, so the half-typed line, the history and any queued lines are gone. `swish` and
  `rmle` handle `FLAG_RETRY` but not `Gone`, and the terminal contract does not say which of its
  requests may be repeated, so on a real boot the shell would read a hang as a failed read. That is
  a contract question for an architect; only the kernel's test typist asks again today.
- A hang caused by the sink can answer the replacement late. In console mode a `line_editor`
  blocked in its sink's reply is hung by the console. If the console recovers, its ack for the dead
  incumbent's flush reaches the replacement's first `RECEIVE`, one flush early.
- A slow `line_editor` is convicted as a hung one. Nothing distinguishes an answer that arrives
  after the bound from none. The bound is about thirty times the slowest healthy answer measured.
- A `line_editor` that faults is not restarted. Its death is collected at the next swap, and
  until then the terminal is gone. Restarting it fresh is the hung path without the quiesce, and is
  not built because nothing has asked for it.

The note's own:

`line_editor`'s blob is smaller than `LineDisc`'s buffers, because it does not carry the
completed-line snapshot or the browse stash. Measured by `line_editor`'s host test on 2026-09-27: a
full ring of near-maximal history lines serializes to under 2 KiB, and with four queued lines the
whole blob is under 4 KiB, so it fits the one page it declares. `MemoryRegion::RETYPE` takes a page
count anyway, DECISIONS §233 (`MemoryRegion::RETYPE` takes a page count), because `redoxfs_server`
will not fit a page and `swapper`'s fixture uses two.
`display_terminal`'s instance was measured with `MemoryRegion::USAGE` on 2026-10-19 (UTC): 207 to
212 pages committed, most of it the 177 pages of grid, which are in `.data` because a blank cell
is not zero bytes.

## See also

- notes/state-handoff.md, notes/live-replacement.md, notes/terminal-contract.md
- design/roadmap/0177-graphical-interactive-boot.md for why the graphical stack is built kernel-side

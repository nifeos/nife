# Reading a board: the appendices to notes/board-console.md

[`notes/board-console.md`](../board-console.md) carries what the tool is for, the commands, the exit
statuses, the one byte it writes and when, what it recognizes, a summary of its design, EXAMPLES and
a one-line BUGS list. These files carry the detail its prose budget has no room for.

- [`markers.md`](markers.md): every marker with its source, the traps in the four outcomes, the
  extlinux failure, the degraded U-Boot environment, and how a job-mix sweep is judged.
- [`design.md`](design.md): partial lines, the reader thread, opening the port before configuring
  it, why there is no serial dependency, and how it is tested with no board.
- [`limitations.md`](limitations.md): the BUGS list in full.
- [`job-mix-examples.md`](job-mix-examples.md): watching a job-mix sweep under QEMU, its three
  endings as real runs, and why their numbers moved after milestone 168 (a multi-tasking workload
  benchmark).

*Name: provisional. The directory follows the siting default of §212 (a prose budget), `notes/X/`
beside `notes/X.md`. calef names directories.*

Each stem below is named for its content, and calef names files.

- `markers` Name: provisional.
- `design` Name: provisional.
- `limitations` Name: provisional, after `notes/soak/limitations.md` and
  `notes/frames/limitations.md`.
- `job-mix-examples` Name: provisional, for the parent's job-mix EXAMPLES.

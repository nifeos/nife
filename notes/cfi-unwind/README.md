# Call-frame information: the appendices to notes/cfi-unwind.md

[`notes/cfi-unwind.md`](../cfi-unwind.md) carries what CFI is, why the hand-written assembly lacked
it, where the section is placed now, what the directives describe on each architecture, what the
evidence showed, and BUGS. These files carry the evidence its prose budget has no room for.

- [`link-script-discovery.md`](link-script-discovery.md): the link-script line that discarded every
  CFI record, the three placements tried, and the measurement that the one kept costs the boot image
  nothing.
- [`gdb-transcripts.md`](gdb-transcripts.md): the before-and-after GDB backtraces on aarch64, for a
  brand-new thread and for a resumed one.

*Name: provisional. The directory follows the siting default of §212 (a prose budget), `notes/X/`
beside `notes/X.md`. calef names directories.*

Each stem below is named for its content, and calef names files.

- `link-script-discovery` Name: provisional. After the parent's own heading.
- `gdb-transcripts` Name: provisional.

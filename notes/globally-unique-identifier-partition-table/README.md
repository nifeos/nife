# The GUID Partition Table: the appendix to notes/globally-unique-identifier-partition-table.md

[`notes/globally-unique-identifier-partition-table.md`](../globally-unique-identifier-partition-table.md)
carries the format, the crate's design, the traps, what the fixtures taught, how it is counted and
proved in brief, what it does not do, and the write half. This file carries the evidence its prose
budget has no room for.

- [`the-proofs.md`](the-proofs.md): every Kani harness over the crate with what it quantifies over
  and its time, why the CRC harnesses are bounded, and why two harnesses took the shape they did.
- [`the-modules.md`](the-modules.md): the reasons behind the modules' doc comments (the two CRC
  implementations, the header's declared size, type and unique GUIDs, where to read, Miri and the
  sweeps), and the commands that regenerate the fixtures.

*Name: provisional. The directory follows §212 (a prose budget)'s siting default, `notes/X/`
beside `notes/X.md`. calef names directories.*

The stem here is named for its content, and calef names files.

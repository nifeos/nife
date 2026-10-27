---
status: PROPOSED
raised: 2026-10-27
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# The boot stick notes are read against their code

Raised by the 2026-10-27 documentation sweep
([its report](../../audit-reports/2026-10-27-the-fat-crate-read-from-the-outside.md), finding 9).
Name: provisional.

Reuse: the method of the two pilots, #2778 (`notes/system-initializer`) and #2782
(`notes/e1000e`), with its ledger and its review pass.

## The work

The sweep sorted `notes/boot-stick.md` and `notes/boot-stick/` (about 5,400 words, 82 passages) by
the method of the 2026-10-26 UTC ruling, which reads each note against its code, and fixed every
contradiction it found. It did not make the cuts and moves the sort asks for, because the pilots measured that a
third of first-pass cuts removed something a reader needs, and only a review pass caught them. That
pass is this milestone.

Seventeen passages narrate what a code doc already says. Among them are the flag table
(`stick_maker::cli`'s `USAGE`), the write-then-rename paragraph (`write.rs`), and "The halves" in
`the-program.md` (the crate's module docs). Others are "The payload is embedded at build time"
(`crates/stick_maker/build.rs`), the Linux sysfs facts (`linux.rs`) and the Windows paragraphs
(`windows.rs`).

Six passages belong beside their code:

- the rustc tier evidence and the relocation count, beside `xtask/src/portable_executable.rs`;
- "What has not been checked on Linux", into `linux.rs`'s `BUGS`;
- the GUID measurement (the MBR signature and the volume ids), beside `host/macos.rs`'s `erase`;
- the device tree's copy ceilings, as a pointer to each architecture's `ALLOCATION_CEILING`;
- the seal's mechanism, as a pointer to `sealed_pair::measured_entries`, keeping the 2026-09-19
  proof;
- the bench tables' quoted loader lines, once the stick's shared names are code (the sibling
  proposal `the-boot-sticks-shared-names-are-code.md`).

## Exit test

Each passage of the area has a row in a ledger (gone, moves, stays, contract) with the symbol it
rests on, and a review has read every gone and moves row against the code. The note's headings and
anchors still resolve from every file that cites them.

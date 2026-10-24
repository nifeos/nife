# The C seam: the appendices to notes/c-seam.md

[`notes/c-seam.md`](../c-seam.md) carries why C is the demonstration, the shape of the Rust shell,
the libc tiers, the toolchain and the confinement test in brief, what the supervisor had to hold,
and the caveats. These files carry the detail its prose budget has no room for.

- [`the-toolchain.md`](the-toolchain.md): how clang is found, why one compiler must carry both
  backends, and every flag with its reason.
- [`the-confinement-test.md`](the-confinement-test.md): the layout and its two witnesses, the five
  assertions and the control, and the fault transcripts from both ISAs.

*Name: provisional. The directory follows the siting default of §212 (a prose budget), `notes/X/`
beside `notes/X.md`. calef names directories.*

Each stem here is named for its content, and calef names files.

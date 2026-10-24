# ELF: the appendix to notes/elf.md

[`notes/elf.md`](../elf.md) carries the format, the header, the magic numbers, what QEMU does with
an ELF kernel, why ELF rather than a flat binary, how the kernel loads one, what the loader refuses,
the measured segment headers and the permissions rule. This directory carries the history its prose
budget has no room for.

- [`why-not-mach-o-or-pe.md`](why-not-mach-o-or-pe.md): why macOS and Windows use other formats,
  and the punchline that UEFI boots PE, with a correction dated 2026-10-11.
- [`loaded-image-check.md`](loaded-image-check.md): what a program's check of its own loaded image
  catches, and why it traps.
- [`parser.md`](parser.md): the design of `crates/elf`, what it refuses, which machine a file may
  be for, the fuzz seeds, and how a note is found.

*Name: provisional, minted 2026-10-11 (UTC) by lane/seventh-ten-longest-notes, which split the
parent under §212 (a prose budget). The directory follows §212's siting default, `notes/X/` beside
`notes/X.md`. calef names directories.*

- `why-not-mach-o-or-pe` Name: provisional, minted 2026-10-11 (UTC) by
  lane/seventh-ten-longest-notes, after the parent's "Why don't macOS and Windows use it?". calef
  names files.
- `loaded-image-check` Name: provisional, minted 2026-10-24 (UTC) by comments/929-crates-18, for
  the crate it describes. calef names files.

# Unsafe obligations: the appendices to notes/unsafe-obligations.md

[`notes/unsafe-obligations.md`](../unsafe-obligations.md) carries the two lints and how they
compose, the survey, the shape of the 33, its BUGS and how to re-run it. These files carry the three
milestones that followed it, each summarized there.

- [`kani-gate.md`](kani-gate.md): the clippy pass over `cfg(kani)` (milestone 113 (the proofs' own unsafe code is ungated)), the shim it
  compiles against, and what it found.
- [`caller-obligations.md`](caller-obligations.md): the SAFETY comments that bound nobody, decided
  (milestone 112 (the SAFETY comments that bind nobody)), and the `# Safety` check that shipped.
- [`census.md`](census.md): how much unsafe there is, measured over time (milestone 134 (the register of measures)).
- [`ceilings.md`](ceilings.md): what each census number is held to, and every move of each ceiling.

*Name: provisional, minted 2026-10-11 (UTC) by lane/next-ten-longest-notes, which split the parent
under §212 (a prose budget). The directory follows §212's siting default, `notes/X/` beside
`notes/X.md`. calef names directories.*

The stems here were minted the same day by the same lane. Each is named for its content, and calef
names files.

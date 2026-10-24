# Proving things about `user/`: the appendices to notes/user-proofs.md

[`notes/user-proofs.md`](../user-proofs.md) carries why the note exists, the defect it found, what
is proved, the stub boundary, how to add a harness, why `xtask` is refused, the examples and BUGS.
These files carry the measurements its prose budget has no room for.

- [`what-kani-could-not-do.md`](what-kani-could-not-do.md): the three properties written, run and
  abandoned, and why each shape fails.
- [`what-it-took.md`](what-it-took.md): the three errors that stopped `cargo kani` on an EL0
  program, the fix for each, and why the binaries are derived rather than listed.

*Name: provisional. The directory follows §212 (a prose budget)'s siting default, `notes/X/` beside
`notes/X.md`. calef names directories.*

Each stem here is named after the parent's own heading, and calef names files.

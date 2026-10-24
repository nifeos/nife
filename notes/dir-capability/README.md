# The directory capability: the appendices to notes/dir-capability.md

[`notes/dir-capability.md`](../dir-capability.md) carries the rights ladder, attenuation, the
refusal errnos, handle 0, the structural finding, `RENAME`'s wire and rights, a summary of the
tests, a one-line BUGS list and EXAMPLES. These files carry the detail its prose budget has no room
for.

- [`caretaker.md`](caretaker.md): why `fs_subtree_caretaker` checks nothing, the verb table that
  replaced its `match`, and the startup ordering one shared frame needed.
- [`rename.md`](rename.md): `RENAME`'s two atomicities, what it refuses and why, and the two
  checks that are ours rather than the engine's.
- [`tests-and-stack-usage.md`](tests-and-stack-usage.md): the three attacker runs, the host-side check of the
  image, and the FS server's stack before and after `RENAME`.
- [`limitations.md`](limitations.md): the BUGS list in full.

*Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes, which split the parent
under §212 (a prose budget). The directory follows §212's siting default, `notes/X/` beside
`notes/X.md`. calef names directories.*

The stems here were minted the same day by the same lane. Each is named for its content, and calef
names files.

# Memory ordering: the appendices to notes/memory-ordering.md

[`notes/memory-ordering.md`](../memory-ordering.md) carries where this kernel's ordering lives, every
fence adjudicated, the ordered atomics by protocol, the `PAIR:` gate, the loom queue and BUGS. These
files carry the evidence its prose budget has no room for.

- [`the-count.md`](the-count.md): the population as milestone 116 (the fences with no
  partner) counted it, and why a line-oriented grep and a careless test-code exclusion both get it wrong.
- [`the-atomics-in-detail.md`](the-atomics-in-detail.md): `HWID`'s decorative acquire, the
  compositor's corrected `publish` comment, the window list's write order, and the relaxed
  compare-exchanges.
- [`checking-pairing.md`](checking-pairing.md): the broad pairing check that was built, run and
  refused, and why the narrow `PAIR:` gate shipped instead.

*Name: provisional. The directory follows §212 (a prose budget)'s siting default, `notes/X/`
beside `notes/X.md`. calef names directories.*

Each stem here is named for its content, and calef names files.

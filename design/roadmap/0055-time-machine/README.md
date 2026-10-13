# Time Machine over SMB3: the appendices to milestone 55

[`design/roadmap/0055-time-machine.md`](../0055-time-machine.md) carries the retirement, the
discovery half that stays, what was decided and built, the one gate left open and the limitations.
This directory carries the detail its prose budget has no room for.

- [`what-landed.md`](what-landed.md): the SMB half as built, covering
  the `AAPL` context, the flush witness, the throughput measurements and the inherited identity.
- [`path-to-built.md`](path-to-built.md): the scoping pass, its hardware steps A to C,
  and the `ReplaceIfExists` finding.
- [`reference.md`](reference.md): the router's Samba configuration as the requirement list, xattrs
  and `posix_rename`, the users, mDNS, and what Time Machine requires.

*Name: provisional. The directory follows §212 (a prose budget)'s siting default,
`design/roadmap/X/` beside `design/roadmap/X.md`. calef names directories.*

- `what-landed` Name: provisional. For the block's dated record of the SMB half. calef names files.
- `path-to-built` Name: provisional. After the parent's "The path to BUILT, ordered and gated".
  calef names files.
- `reference` Name: provisional. After the parent's "The reference implementation is known". calef
  names files.

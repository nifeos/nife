# Partitioning and extended attributes: the appendices to milestone 57

[Milestone 57 (partitioning and formatting a real drive, and extended
attributes)](../0057-partitioning-and-xattrs.md) carries the status, what was decided and built, the
tools table, the `mkfs` finding, the capability shape, its Follow-on and its `BUGS`. This directory
carries the build accounts its prose budget has no room for.

- [`extended-attributes.md`](extended-attributes.md): the extended-attribute fork, the check that
  decided it, the layer as built and its recovery half.
- [`host-recovery.md`](host-recovery.md): reading the drive from a Mac or a Linux host, and the
  decision against filesystem-level encryption on the backup volume.

*Name: provisional. The directory follows §212 (a prose budget)'s siting default,
`design/roadmap/X/` beside `design/roadmap/X.md`. calef names directories.*

- `extended-attributes` Name: provisional. After the parent's "Extended attributes: decided in
  direction, open in mechanism". calef names files.
- `host-recovery` Name: provisional. For the parent's "Reading the drive from a MacBook or a Linux
  host" and "Decided: no filesystem-level encryption on the backup volume". calef names files.

# Host recovery: the appendices to notes/host-recovery.md

[`notes/host-recovery.md`](../host-recovery.md) carries why upstream did not solve recovery, what
the tool does, the operational rule about keeping the reader with the backup, how recovery is
proven, and what the attribute and partition work changed, in brief. These files carry the
detail its prose budget has no room for.

- [`extended-attributes.md`](extended-attributes.md): how attributes come back on the extracted
  files, what the tool is honest about, a transcript on a Mac, how it is proven, and its BUGS.
- [`devices-and-partitions.md`](devices-and-partitions.md): the partition selectors, the correction
  about the old tool on a partitioned disk, transcripts, how it is proven, and its BUGS.
- [`the-library.md`](the-library.md): the library's seams (errors, the partition window, the
  version message, import, attributes on the host) and how its tests are shaped.

*Name: provisional. The directory follows §212 (a prose budget)'s siting default, `notes/X/`
beside `notes/X.md`. calef names directories.*

Each stem here is named for its content, and calef names files.

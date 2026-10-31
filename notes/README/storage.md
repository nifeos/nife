# Notes index: Storage

Part of [the notes index](../README.md), which says how to add a line.

- [The RedoxFS filesystem server](../fs-server.md): RedoxFS confined behind a capability-shaped file contract. It is the launch store (§34); ext4 through `rsext4` is the next measurement for that role.
- [Rust filesystem crates on nife's own targets, 2026-10-07](../filesystem-crates-2026-10-07.md): which btrfs, FAT and ext4 crates build for the three nife targets and round-trip on foreign images; answers question 1 of §34 (RedoxFS is the primary filesystem, on three conditions). Name provisional.
- [The launch filesystem's candidates, surveyed license first](../launch-filesystem-candidates.md): the evidence behind RedoxFS as the launch store (§34), the four routes compared (RedoxFS, UFS2, ext2, ext4), and why ext4 through `rsext4` comes next while §34 refuses UFS2. Name provisional.
- [A frame per filesystem client channel](../a-frame-per-filesystem-client-channel.md): a window per file client, by badge.
- [RedoxFS std-footprint audit](../redoxfs-audit.md): costing the RedoxFS port to no_std by building it.
- [The directory capability](../dir-capability.md): a directory split into separable, attenuable rights.
- [Removal needs a directory](../rm.md): why `rm` gets a directory and `-r` widens it.
- [`touch`: create if absent](../touch.md): create-if-absent and mtime setting, with separate rights.
- [Extended attributes](../xattr.md): named byte strings on files, kept above RedoxFS.
- [UFS2's Rust crates on nife's targets, 2026-10-31](../ufs2-builds-2026-10-31.md): what stops `ufs-core` and `rufs` on the three targets, with and without `std`, and what the patches cost; the measurement milestone 981 (UFS2, read and write) made before §34 refused it. Name provisional.
- [Reading the backup from a MacBook or a Linux host](../host-recovery.md).
- [The GUID Partition Table](../globally-unique-identifier-partition-table.md): reading, writing and validating GPT, host-tested and proved.
- [nifefs](../nifefs.md): the boot archive format and its 32-byte names.
- [The filesystem test fixture](../filesystem-fixture.md): why each witness file, verdict word and benchmark shape is the one it is.

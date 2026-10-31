# Notes index: Storage

Part of [the notes index](../README.md), which says how to add a line.

- [The RedoxFS filesystem server](../fs-server.md): RedoxFS confined behind a capability-shaped file contract. It is the launch store until UFS2 meets its bar in milestone 981 (UFS2, read and write: a Rust server with FreeBSD's tools as the oracle). Milestone 982 (Retire RedoxFS once a launch store proves itself) then removes it.
- [Rust filesystem crates on nife's own targets, 2026-10-07](../filesystem-crates-2026-10-07.md): which btrfs, FAT and ext4 crates build for the three nife targets and round-trip on foreign images; answers question 1 of §34 (RedoxFS is the primary filesystem, on three conditions). Name provisional.
- [The launch filesystem's candidates, surveyed license first](../launch-filesystem-candidates.md): the evidence behind UFS2 as the launch filesystem (§34), with HAMMER2, btrfs, OpenZFS and ext4 refused; milestone 981 builds UFS2 and milestone 982 retires RedoxFS. Name provisional.
- [A frame per filesystem client channel](../a-frame-per-filesystem-client-channel.md): a window per file client, by badge.
- [RedoxFS std-footprint audit](../redoxfs-audit.md): costing the RedoxFS port to no_std by building it.
- [The directory capability](../dir-capability.md): a directory split into separable, attenuable rights.
- [Removal needs a directory](../rm.md): why `rm` gets a directory and `-r` widens it.
- [`touch`: create if absent](../touch.md): create-if-absent and mtime setting, with separate rights.
- [Extended attributes](../xattr.md): named byte strings on files, kept above RedoxFS.
- [Reading the backup from a MacBook or a Linux host](../host-recovery.md).
- [The GUID Partition Table](../globally-unique-identifier-partition-table.md): reading, writing and validating GPT, host-tested and proved.
- [nifefs](../nifefs.md): the boot archive format and its 32-byte names.
- [The filesystem test fixture](../filesystem-fixture.md): why each witness file, verdict word and benchmark shape is the one it is.

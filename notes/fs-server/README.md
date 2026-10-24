# The FS server: the appendices to notes/fs-server.md

[`notes/fs-server.md`](../fs-server.md) is the page to read: the three processes, the contract,
the write path, and one summary per measured property. The files here hold the evidence and the
history behind each summary.

- [`block-server.md`](block-server.md): whole-block requests, interrupt completion, and the disk
  order.
- [`stack-sizing.md`](stack-sizing.md): the stack overflow that cost a day, and the measurement.
- [`crash-consistency.md`](crash-consistency.md): the property, the injector, the controls and the
  device test.
- [`mkfs-on-target.md`](mkfs-on-target.md): creating a filesystem on the target, and the divergence.
- [`write-path-history.md`](write-path-history.md): the write that seemed to loop, and why it never
  did.
- [`verb-table.md`](verb-table.md): one row per opcode, and the row that carries a security property.
- [`statfs.md`](statfs.md): op 18 in full.
- [`file-protocol.md`](file-protocol.md): why each file and block verb is shaped as it is.
- [`throughput.md`](throughput.md): where the milliseconds go, the cache, the transfer unit and the
  throughput BUGS.
- [`kernel-wiring.md`](kernel-wiring.md): how `kernel/src/user/fs_service/` wires the servers, the
  caretakers and their clients, and why.
- [`the-core.md`](the-core.md): the `redoxfs_server` core's handle table, names and verbs, and where
  it departs from upstream RedoxFS.
- [`caches-and-io.md`](caches-and-io.md): the memo, the block cache, the wire shapes the dispatch
  decodes, and the EL0 binary's IO.

Name: provisional, minted 2026-10-11 (UTC) by `lane/ten-longest-notes`, which split the parent under
§212 (a prose budget), for the directory and every stem in it. The directory follows the
`notes/<stem>/` appendix convention §212 set, so its name is the parent's stem. Each file is named
for its content. calef names directories and files, and `script/names --unratified` lists each stem.

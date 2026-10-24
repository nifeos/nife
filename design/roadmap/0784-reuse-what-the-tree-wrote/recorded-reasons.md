# Recorded reasons for what stays

Appendix to [784. Reuse what the tree wrote](../0784-reuse-what-the-tree-wrote.md).

Each line is ready to paste as the component's `Reuse:` line, to be conformed to the amendment's
landed wording.

- `calendar`: Reuse: write. Proved (11 Kani over every day in range); the §46 worked example, and
  `time` and `chrono` cannot be restructured for CBMC. The proof argument has not changed.
- `elf`: Reuse: write. Kernel, and proved (8 Kani); `xmas-elf` shows the bug class (RUSTSEC-2025-0018). `goblin`, cole14's `elf` and `object` measured and refused (calef, 2026-10-22 UTC).
- `device_tree_blob`: Reuse: write. Kernel, proved (4 Kani); `fdt` is MPL-2.0.
- `globally_unique_identifier_partition_table`: Reuse: write. Proved (8 Kani, CRC restructured for
  CBMC per §46), which a dependency would lose; installer and loader must agree
  byte for byte.
- `measured_boot`'s SHA-256: Reuse: write. calef ruled "Keep our SHA-256." on 2026-10-22 (UTC):
  §46 (thin primitives or whole subsystems)'s rule 3 (the kernel writes) wins over rule 4 (crypto is taken), and `sha2` stays out of the
  measured-boot trusted base.
- `network_time_protocol`: Reuse: write. Proved (7 Kani, fixed-point multiply restructured per §46);
  `sntpc` exists but would lose the proof.
- `glob`, `timetable`, `capability_plan`, `subtree_scope`, `capability_manifest_note`: Reuse: write. Proved, and
  each encodes a capability rule no upstream has.
- `package_archive`: Reuse: write. Decided by §197 (a package is one archive file); `tar` refused.
- `nifefs`: Reuse: write. Kernel; a deliberate minimal format.
- `bitmap_font`: Reuse: taken. The glyphs were chosen by §100 (the terminal font).
- `line_editor`: Reuse: write. `noline` is MPL-2.0 with 37k downloads; `rustyline` needs `libc`
  termios, which the PAL does not bind. Revisit if termios is bound.
- `documentation`: Reuse: write, for the index only. The renderer's parser is candidate 9a.
- `file_allocation_table`: Reuse: write, for now. Writes only our own ESP; `system_installer` is `no_std`,
  and `fatfs` has no `no_std`
  release without `core_io`. Revisit if `fatfs` 0.4 releases or the installer moves to std.
- `swish`, `grant_plan`, `system_initializer`, the `*_protocol` crates, `system_log`, the
  `process-tools` programs: Reuse: write. The ABI or a capability (rule 3); nothing upstream reads nife's ABI.
- `redoxfs_server`, `tools/redoxfs_host`: Reuse: adapted. The adapter around vendored RedoxFS (§34).
- `virtio`, `gpu_driver`, `keyboard_driver` and `virtio_net_transport`: Reuse: write. calef ruled
  option D on 2026-10-24 (UTC), items (c1) to (c4). `virtio-drivers` assumes a driver lays out its
  own virtqueues; nife's kernel builds every ring itself, eight entries at fixed offsets,
  Kani-proved, so no driver can aim device DMA ([the ruling](the-virtio-drivers-stay-ours.md)).
- `coremark`: Reuse: taken. A port of the upstream benchmark.
- `stick_maker`'s `plist.rs`: Reuse: write. calef ruled "Keep ours." on 2026-10-20 (UTC). `plist`
  1.10.1 would bring nine crates new to the tree (`serde`, `quick-xml`, `time` and its five helpers
  and `base64`) to read trusted local `diskutil` output, in a host-only program.
- `wc`, `rm`, `printenv` and `date`: Reuse: write. calef ruled "A" on 2026-10-21 (UTC), item (h).
  uutils `uu_wc` and `uu_rm` do not build for nife (`rustix` needs a `libc` it lacks), and
  `uu_wc` would need a filesystem grant ours refuses. uutils is a basalt package candidate.

# Recorded reasons for what stays

Appendix to [784. Reuse what the tree wrote](../0784-reuse-what-the-tree-wrote.md).

Each line is ready to paste as the component's `Reuse:` line, to be conformed to the amendment's
landed wording.

- `calendar`: Reuse: write. Proved (11 Kani over every day in range); the §46 worked example, and
  `time` and `chrono` cannot be restructured for CBMC. The proof argument has not changed.
- `elf`: Reuse: write. Kernel, and proved (8 Kani); `xmas-elf` shows the bug class (RUSTSEC-2025-0018).
- `device_tree_blob`: Reuse: write. Kernel, proved (4 Kani); `fdt` is MPL-2.0.
- `globally_unique_identifier_partition_table`: Reuse: write. Proved (9 Kani, CRC restructured for
  CBMC per §46), which a dependency would lose; installer and loader must agree
  byte for byte.
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
- `coremark`: Reuse: taken. A port of the upstream benchmark.
- `stick_maker`'s `plist.rs`: Reuse: write. calef ruled "Keep ours." on 2026-10-20 (UTC). `plist`
  1.10.1 would bring nine crates new to the tree (`serde`, `quick-xml`, `time` and its five helpers
  and `base64`) to read trusted local `diskutil` output, in a host-only program.
- `wc`, `rm`, `printenv` and `date`: Reuse: write. calef ruled "A" on 2026-10-21 (UTC), item (h).
  uutils `uu_wc` and `uu_rm` do not build for nife (`rustix` needs a `libc` it lacks), and
  `uu_wc` would need a filesystem grant ours refuses. uutils is a basalt package candidate.

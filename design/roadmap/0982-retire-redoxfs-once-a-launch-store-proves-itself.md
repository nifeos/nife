---
status: REFUSED
promoted_from: retire-redoxfs-once-a-launch-store-proves-itself
raised: 2026-10-30
refused_by: §34
---
# 982. Retire RedoxFS once a launch store proves itself

Refused 2026-10-31 (UTC) on #2927, by §34 (RedoxFS is the primary filesystem, on three conditions):
RedoxFS stays the launch store. This block was the removal that would follow a UFS2 server
(milestone 981 (UFS2, read and write)), and §34 refuses it the same day. The inventory below stays because a
later removal would start from it.

## Revisit

- **Condition.** The ext4 measurement through `rsext4` (the proposal
  `ext4-through-rsext4-under-the-crash-injector`) passes the injector of milestone 37 (Prove RedoxFS's crash consistency), and an ext4 server
  then boots the composed image, takes `jig` installs and holds the installed-programs table on
  aarch64, riscv64 and x86_64. That removal would be a milestone of its own, minted from this
  inventory.

## What would leave the tree

The vendored engine and its server: `vendor/redoxfs/`, `vendor/redoxfs.pin`,
`vendor/redoxfs.divergence.patch`, `patches/redoxfs-no-std-vec-import.patch`,
`patches/redoxfs-no-std-create-uuid.patch` and the EL0 server crate `redoxfs_server/`.

The tooling: the host round-trip tool `tools/redoxfs_host/`, the package
`packages/redoxfs.package.toml`, the fuzz target `fuzz/fuzz_targets/redoxfs_server_session.rs` with
its falsification patch, and the workspace members in `Cargo.toml` that name `vendor/redoxfs` and
`tools/redoxfs_host`. The RedoxFS allowances and comments in `deny.toml`, `.cargo/config.toml` and
`clippy.toml` go too, and `deny.toml`'s header stops counting four workspaces.

The build and gate helpers in `xtask/src/disk.rs` and its siblings: `mkredoxfs`, `mkredoxfs_crash`,
`redoxfs_server_build`, `redoxfs_check_after_run` and `redoxfs_crash_check_after_run` bind the
image path, the crash harness and the disk checks to the RedoxFS engine. Each gets a twin for the replacing
store or goes, except the crash harness, which must get its twin, because it judges the replacing
store.

The prose: `notes/redoxfs-audit.md`, `notes/fs-server.md`, `notes/README/storage.md`,
`design/backup-server-ladder.md`, §176 (Offering the RedoxFS patches upstream) and §129 (Whether
`filesystem_proto::fs::RENAME` grows a `NOREPLACE` flag). Each one either moves to describe the replacing store
or its decision takes a status that says it no longer applies.

This list rests on a search, so it is a floor. The lane re-derives it with `git grep -i redoxfs` at
its own base, and that search, not this list, is the inventory.

## Index row

The removal of RedoxFS behind a proven replacement launch store. Refused 2026-10-31: RedoxFS stays
the launch store, and UFS2 is not built. It reopens if ext4 through `rsext4` passes milestone 37's
injector and then meets the launch store's bar; the inventory of what would leave stays here.

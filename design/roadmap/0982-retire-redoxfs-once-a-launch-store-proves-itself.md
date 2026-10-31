---
status: NOT-STARTED
promoted_from: retire-redoxfs-once-a-launch-store-proves-itself
raised: 2026-10-30
milestone_dependencies: 32, 37
decision_dependencies: 34
machine_requirements: none
specific_machine: none
needs_person: no
---
# 982. Retire RedoxFS once a launch store proves itself

Raised 2026-10-30 (UTC) by lane `lane/filesystem-candidates-survey`, from
[the launch filesystem's candidates](../../notes/launch-filesystem-candidates.md). UFS2 replaces
RedoxFS as the launch filesystem, and RedoxFS then leaves the tree rather than sitting beside it
(§34 (UFS2 is the launch filesystem, and RedoxFS stays primary until UFS2 meets the bar), ruled
2026-10-31 UTC). This milestone is the removal. It starts only when milestone 981 (UFS2, read and
write: a Rust server with FreeBSD's tools as the oracle) meets the bar below.

An exception, and a foot gun: `milestone_dependencies` does not name 981. The field resolves only
when 981 is `BUILT`, and this milestone must run while 981 is `PARTIAL`, after its phase 3 and
before its phase 4. So `script/roadmap --ready` offers this block before its bar holds. A lane that
takes it checks the bar below first, and stops if any line fails.

Reuse: nothing. This removes code and takes no dependency. Every piece it names already exists.

## The bar

RedoxFS leaves only when UFS2 does all three of these in the tree, on aarch64, riscv64 and x86_64:

- boots the composed image from a UFS2 launch volume,
- takes `jig` installs onto it, and
- holds the installed-programs table the progenitor reads.

These are milestone 981's removal bar, the end of its phase 3. The crash claim is not on this list
(ruled 2026-10-31 UTC). Milestone 981's phase 4 proves it with milestone 37 (Prove RedoxFS's crash
consistency)'s injector after RedoxFS is gone. Until all three hold, RedoxFS stays the launch store
and the lane deletes nothing below. RedoxFS is MIT and its upstream is slow rather than dead, so the
removal is about the launch decision and the maintenance a second store costs, not about a license.

## Scope

In scope: deleting RedoxFS and everything that exists only to build, run, check or describe it;
pointing the paths that use it at the UFS2 server; and setting the roadmap dispositions. Out of
scope: any change to `filesystem_contract`, the FS-server capability shape or the crash injector,
which carry to UFS2 unchanged.

## What leaves the tree

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
image path, the crash harness and the disk checks to the RedoxFS engine. Each gets a UFS2 twin from
milestone 981 or goes, except the crash harness, which must get its UFS2 twin: phase 4 of 981 runs
on it.

The prose: `notes/redoxfs-audit.md`, `notes/fs-server.md`, `notes/README/storage.md`,
`design/backup-server-ladder.md`, §176 (Offering the RedoxFS patches upstream) and §129 (Whether
`filesystem_proto::fs::RENAME` grows a `NOREPLACE` flag). Each one either moves to describe UFS2 or
its decision takes a status that says it no longer applies.

This list rests on a search, so it is a floor. The lane re-derives it with `git grep -i redoxfs` at
its own base, and that search, not this list, is the inventory.

## The roadmap this re-points

- Milestone 32 (A real filesystem: RedoxFS behind a capability FS server) becomes `REMOVED`: its
  server is the code this milestone deletes, and its contract survives under UFS2.
- Milestone 37 (Prove RedoxFS's crash consistency) keeps its injector and its numbers. Whether the
  block keeps its number or its harness gains a new home is this lane's call, made with the
  disposition vocabulary in `notes/roadmap.md`.
- Milestone 346 (RedoxFS first pin bump) and milestone 687 (a system log on RedoxFS) either re-point
  at UFS2 or close.
- Milestone 140 (Mount a drive this system did not create) names RedoxFS as the backup volume. This
  lane edits that row, not 140's ordering.

## What stays

The `filesystem_contract` crate and the FS-server capability shape carry to UFS2 unchanged; they are
what make the swap one component rather than a kernel change. Milestone 37's crash injector stays.
`nifefs` (the read-only initial-ramdisk filesystem) has nothing to do with this and stays, as do
`boot_slot`, the partition GUID work and `installed_programs`.

## The order

One lane, after milestone 981 meets the removal bar. It points the server path and the disk helpers
at UFS2, deletes the vendored tree and its tooling, sweeps the prose and sets the roadmap
dispositions. Each commit stands alone for a reviewer. The gate stays green at every commit, because
the UFS2 paths exist before the RedoxFS ones go.

## Done when

`git grep -i redoxfs` returns only dated records (bench logs, measurements, decisions that keep
their history under §298 (a document states what is in force now)) and the dispositions above, and
every gate passes on all three architectures with no RedoxFS code built.

## BUGS

- The inventory above rests on a search of the tree, not on a build, so it is a floor: the lane will
  find more.
- Deleting the vendored engine removes the only store with injector numbers, and UFS2 has none
  until milestone 981's phase 4. In that window the launch store's crash evidence is weaker than
  RedoxFS's was, and its floor is `fsck` on mount. That is an accepted risk (ruled 2026-10-31 UTC).
  Milestone 37's injector stays, ported to UFS2, so the window closes rather than becoming the
  standard. The tell is this block `BUILT` with 981 still `PARTIAL`.
- Whether milestone 37 keeps its number stays open on purpose.

## Index row

RedoxFS leaves the tree once UFS2 (milestone 981) boots the composed image, takes `jig` installs,
and holds the installed-programs table; the crash claim follows in 981's phase 4. One lane deletes
the vendored engine, its server, its host tool, package, fuzz target and disk helpers, sweeps the
prose, marks milestone 32 `REMOVED` and re-points 346, 687 and 140's backup row. `filesystem_contract`
and the crash injector carry over unchanged.

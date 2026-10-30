---
status: PROPOSED
raised: 2026-10-30
milestone_dependencies: 32, 37
decision_dependencies: 34
machine_requirements: none
specific_machine: none
needs_person: no
---
# Retire RedoxFS once a launch store proves itself

Raised 2026-10-30 (UTC) by lane `lane/filesystem-candidates-survey`. It names the work the survey
([the launch filesystem's candidates](../../../notes/launch-filesystem-candidates.md)) implies:
after a replacement store clears the bar, RedoxFS leaves the tree rather than sitting beside it.

Reuse: nothing. This removes code and takes no dependency. Every piece it names already exists, and
the replacement it waits on is the UFS2 arc
([UFS2, read and write](ufs2-read-and-write-against-freebsds-tools.md)).

## The bar

RedoxFS leaves only when a replacement store does all four of these in the tree:

- boots the composed image from it,
- takes `jig` installs onto it,
- holds the installed-programs table the progenitor reads, and
- matches RedoxFS's crash claim: 186 injected lying-device cases with 0 silently wrong (milestone 37
  (Prove RedoxFS's crash consistency)).

Until then RedoxFS stays the launch store, and DECISIONS §34 (RedoxFS is the primary filesystem, on
three conditions) stands. RedoxFS is MIT and its upstream is slow rather than dead, so the retirement
is about the launch decision and the maintenance it costs, not about a license.

## What leaves the tree

The handled pieces are the vendored engine and its pin (`vendor/redoxfs/`, `vendor/redoxfs.pin`,
`vendor/redoxfs.divergence.patch`), the two vendored patches (`patches/redoxfs-no-std-vec-import.patch`
and `patches/redoxfs-no-std-create-uuid.patch`) and the EL0 server crate (`redoxfs_server/`). The
rest are the host round-trip tool (`tools/redoxfs_host/`), the package
(`packages/redoxfs.package.toml`), the fuzz target (`fuzz/fuzz_targets/redoxfs_server_session.rs`)
with its falsification patch, and the workspace members that name `vendor/redoxfs` and
`tools/redoxfs_host` (`Cargo.toml`). The RedoxFS allowances and comments in `deny.toml`,
`.cargo/config.toml` and `clippy.toml` go too.

The build and gate pieces are `xtask/src/disk.rs` and its siblings. `mkredoxfs`, `mkredoxfs_crash`,
`redoxfs_server_build`, `redoxfs_check_after_run` and `redoxfs_crash_check_after_run` bind the image
path, the crash harness and the disk checks to the RedoxFS engine. Each needs either a replacement
store's twin or deletion.

The prose pieces are the notes and decisions that describe a RedoxFS-shaped tree:
`notes/redoxfs-audit.md`, `notes/fs-server.md`, `design/backup-server-ladder.md`, DECISIONS §176
(Offering the RedoxFS patches upstream) and §129 (Whether `filesystem_proto::fs::RENAME` grows a
`NOREPLACE` flag).
A retirement lane sweeps the rest with one search, because a search, not this list, is the honest
inventory.

## The roadmap this re-points

Milestone 32 (A real filesystem: RedoxFS behind a capability FS server) becomes `REMOVED`: the
server it built is the code this proposal deletes, and its contract survives under a new engine.
Milestone 37 (Prove RedoxFS's crash consistency) keeps its injector and its numbers; whether the
block keeps its number or its harness gains a new home is the retirement lane's call, made with the
disposition vocabulary. Milestone 346 (RedoxFS first pin bump) and milestone 687 (a system log on
RedoxFS) either re-point at the replacement store or close. Milestone 140 (Mount a drive this system
did not create) names RedoxFS as the backup volume and says its own milestone does not touch it, so
this retirement edits that row rather than 140's ordering.

## What stays

The `filesystem_contract` crate and the FS-server capability shape carry to the new engine unchanged;
they are what make the swap one component rather than a kernel change. Milestone 37's crash injector
stays. The GPL confinement lesson of DECISIONS §135 (Running GPL software is aggregation, the
capability boundary is what makes it so, and packages are how it arrives) stays, and it is what keeps
btrfs and OpenZFS reachable as confined programs. `nifefs` (the read-only initial-ramdisk
filesystem) is unrelated and stays, as do `boot_slot`, the partition GUID work and
`installed_programs`.

## The order, and the one decision

Replacement first, in its own lane and its own milestone: it boots the image, takes a `jig` install,
holds the table and passes the injector. Then one retirement lane re-points the server, deletes the
vendored tree and its tooling, replaces the RedoxFS disk helpers, sweeps the prose and sets the
roadmap dispositions. The one decision this needs is calef's re-aim of DECISIONS §34 (RedoxFS is the
primary filesystem, on three conditions), which the UFS2 proposal already asks for.

## BUGS

- The list above rests on a search of the tree, not on a build, so it is a floor: a retirement lane
  must re-derive it and will find more.
- Deleting the vendored engine removes the only store with crash-consistency evidence in the tree at
  the moment the replacement lands. A retirement lane must not delete RedoxFS before the replacement
  publishes its own three numbers, which is why the bar above names them.
- Whether milestone 37 keeps its number is a roadmap call this proposal leaves open on purpose.

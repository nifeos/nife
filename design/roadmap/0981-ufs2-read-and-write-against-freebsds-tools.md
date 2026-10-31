---
status: NOT-STARTED
promoted_from: ufs2-read-and-write-against-freebsds-tools
raised: 2026-10-30
milestone_dependencies: 37
decision_dependencies: 34
machine_requirements: none
specific_machine: none
needs_person: no
---
# 981. UFS2, read and write: a Rust server with FreeBSD's tools as the oracle

Raised 2026-10-30 (UTC) by lane `lane/filesystem-candidates-survey`, which wrote
[the launch filesystem's candidates](../../notes/launch-filesystem-candidates.md) on pull request
#2899. UFS2 is nife's launch filesystem (§34 (UFS2 is the launch filesystem, and RedoxFS stays primary
until UFS2 meets the bar), ruled 2026-10-31 UTC). This milestone builds the server that makes
the ruling true. Milestone 982 (Retire RedoxFS once a launch store proves itself) removes RedoxFS
once this one meets the bar below, and not before.

The shape follows milestone 190 (ext4, read and write: a Rust implementation with libext2fs as the
host-side oracle): a Rust implementation on the target, C on the host as the judge. Phase 1 needs
nothing that does not exist, and each fork sits at the front of the phase that carries it.

Reuse: the two Rust implementations are `ufs-core` 0.1.5 (Apache-2.0, a 2,778-line reader) and
`rufs` 0.6.0 (BSD-2-Clause, a 2,711-line read/write implementation over FUSE). The oracle is
FreeBSD's `newfs`, `makefs` and `fsck_ffs`, all BSD-2-Clause, all building on a host. The crash
injector comes from milestone 37 (Prove RedoxFS's crash consistency), which injects power cuts, torn
writes and a lying device and needs no change. The activation store reuses the table and its grammar
(`crates/installed_programs`) and the approval file and its check (`crates/approval_file`); UFS2
supplies the container. BSD-2-Clause sits on `deny.toml`'s allow-list, so every piece named here
passes the license gate.

## What the launch store has to do

The launch store is the writable filesystem the image boots into, that `jig` installs packages
onto, and that holds the installed-programs table the progenitor reads. It must:

1. Format, mount read-write and write files on aarch64, riscv64 and x86_64.
2. Survive a power cut without silently wrong data.
3. Carry permissions, symlinks, hard links and extended attributes.
4. Rest on a mature implementation that makes and judges images.
5. Ship under a license `deny.toml` accepts, and leave nife a server rather than a port.

UFS2 is FreeBSD's default root filesystem and the second on-disk generation of the Berkeley Fast File
System: 256-byte inodes, 64-bit block pointers, fragments, cylinder groups, extents, extended
attributes and snapshots. McKusick's paper and FreeBSD's `sys/ufs/ffs/fs.h` document the format. The
survey holds the comparison with HAMMER2, btrfs, OpenZFS and ext4, and why each lost.

## Scope

In scope: a UFS2 FS server bound to `filesystem_contract`, the same contract the RedoxFS server
implements; a UFS2 image writer for the composed image; the activation store on that image; and the
crash measurement. Out of scope: deleting RedoxFS (milestone 982), and mounting a UFS2 drive another
system made, which belongs to milestone 140 (Mount a drive this system did not create) if anyone
asks for it.

## The arc

### Phase 1: the builds

Build `ufs-core` and `rufs` for the three bare targets and record what stops each one.

Both stop today at `can't find crate for 'std'`. `ufs-core` needs `no_std` plus an `alloc` import;
its blockers are `thiserror` and `Vec`, not the format logic. `rufs` runs `std` by design because it
carries `libc` and FUSE. The fork here: does the server run as a `std` program, or does `rufs` gain a
`no_std` port? §34's 2026-09-20 amendment allows a userspace filesystem `std`, and nife's `std` is
single-threaded with `panic = abort`, so the lane measures against `targets/*-unknown-nife.json`.
Record both crates as shipped and with the patch, the way
[filesystem-crates-2026-10-07.md](../../notes/filesystem-crates-2026-10-07.md) recorded the btrfs and
ext4 crates. Each new dependency still goes to an architect under §46.

### Phase 2: make, judge and read

`makefs` and `newfs` on a host produce two images, one FreeBSD-made and one from the lane's own
writer. `rufs` reads both, and `fsck_ffs -n` judges each one.

The deliverable is a server that mounts a UFS2 image and reads files through it, bound to
`filesystem_contract`. No `CREATE` and no `TRUNCATE` yet, the same gap milestone 32 (A real
filesystem: RedoxFS behind a capability FS server) recorded for its first phase. The composed
image's launch volume is a UFS2 image from here on, and it carries the first installed-programs
table (see below).

### Phase 3: the write path

Create, unlink, rename, symlink, hard link, truncate and extended attributes. The format already
carries all of them, and `fsck_ffs` already checks all of them.

The fork is the directory and fragment allocator. `rufs` marks its own writes experimental, so the
lane either hardens `rufs`'s allocator behind the contract or writes one against `ufs-core`'s reader.
Either stays small, because the whole format fits in a few hundred lines of structs. `jig install`
onto the launch volume closes the phase.

### Phase 4: consistency, and the claim

The lane chooses the crash claim rather than assuming it. Soft updates order metadata writes and need
no journal. Soft Updates with Journaling (SUJ) adds an in-filesystem `.sujournal` and replays it
after a crash. Both carry BSD-2-Clause, and neither exists in a Rust implementation today. So phase 4
either adds one or records the honest alternative, `fsck` on mount, which repairs rather than
prevents.

Then the injector from milestone 37 runs against the server. RedoxFS's numbers are the standard: 186
lying-device cases, 112 recovered, 74 refused, 0 silently wrong. A launch store that cannot match the
zero is not done, whatever else it does.

## The oracle

`fsck_ffs` judges every image the server writes, in CI, the way `fsck_msdos` judges the FAT work.
A test that writes through the server and then reads back through the same server proves only that
the server agrees with itself. The oracle reads the bytes with FreeBSD's own code, and its exit
status is the verdict. The host-side tools build from FreeBSD's source on Linux and macOS; the lane
records the build recipe beside the gate that runs it.

## The activation store rides on this arc

The composed image's first installed-programs table lives on this volume as files under
`activation/` (ruled 2026-10-30 UTC, on #2894). Phase 2's image holds `activation/1` and
`activation/current`, and phase 3's write path is what writes them at build time and at install.

The table's grammar stays nife's. `installed_programs` owns the generations and the `current`
pointer, and `approval_file` owns the check. UFS2 owns the container only. No bespoke store crate and
no seed image exist: one store, one format and one writer.

The ordering cost: a composed image carries a table only once this arc reads and writes a UFS2
volume, so milestone 909's table half lands with phase 2. The check half of 909's item 3 does not wait
on this arc.

## Done when

UFS2 meets the bar that retires RedoxFS, on aarch64, riscv64 and x86_64 under QEMU:

- the composed image boots from a UFS2 launch volume,
- `jig install` writes a package onto it,
- it holds the installed-programs table the progenitor reads, and
- milestone 37's injector reports 0 silently wrong cases, and the note that records the run states
  the consistency grade (soft updates, SUJ, or `fsck` on mount).

`fsck_ffs` passes on every image the gate writes. Milestone 982 starts when every line above holds.

## What stays

The `filesystem_contract` crate, the capability model and the FS-server shape do not change. The
crash injector stays. The GPL confinement lesson of §135 (Running GPL software is aggregation, the
capability boundary is what makes it so, and packages are how it arrives) stays, and it keeps btrfs
and OpenZFS reachable as confined programs. Milestone 190 reads and writes ext4 for drives Linux made,
which is a different requirement from the launch store; this arc reorders neither 140 nor 190.

## BUGS

- No UFS2 server exists, and no image has gone through nife. The survey is desk work plus two
  failed builds.
- The `no_std` cost is one build error, not a measured patch. Nobody knows whether closing it takes a
  day or a week.
- No Rust implementation of soft updates or SUJ exists. Until phase 4 adds one, a crash claim on
  UFS2 is `fsck`-repaired rather than consistent, and the record must say which.
- HAMMER2's size and the FreeBSD port's completeness come from lookup, so the survey's second place
  rests on weaker evidence than its first.

## Index row

UFS2 is nife's launch filesystem (§34). A Rust FS server, built from `ufs-core` and `rufs` and judged
by FreeBSD's `newfs`, `makefs` and `fsck_ffs`, in four phases: builds, a read-only server and image
writer, the write path, then a crash claim measured by milestone 37's injector. The launch volume
carries the first installed-programs table as files under `activation/`. Done when UFS2 boots the
composed image, takes `jig` installs, holds the table and matches RedoxFS's zero silently wrong, which
is the bar that starts milestone 982.

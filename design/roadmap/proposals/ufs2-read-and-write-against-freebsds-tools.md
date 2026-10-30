---
status: PROPOSED
raised: 2026-10-30
milestone_dependencies: 37
decision_dependencies: 34, unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# UFS2, read and write: a Rust server with FreeBSD's tools as the oracle

Raised 2026-10-30 (UTC) by lane `lane/filesystem-candidates-survey`, which wrote
[the launch filesystem's candidates](../../../notes/launch-filesystem-candidates.md) and opened pull
request #2899. It answers calef's ruling of 2026-10-29 (UTC) on #2894:

> a survey lane runs first (the 0190 model, across the candidates), license first; C implementations
> are references to read, not engines to vendor; the Rust options for btrfs and UFS are the
> promising starts.

The survey ranked UFS2 first. This proposal turns that ranking into a four-phase arc. The shape
follows milestone 190 (ext4, read and write: a Rust implementation with libext2fs as the host-side
oracle). Phase 1 needs nothing that does not exist, and each fork sits at the front of the phase
that carries it.

Reuse: the two Rust implementations are `ufs-core` 0.1.5 (Apache-2.0, a 2,778-line reader) and
`rufs` 0.6.0 (BSD-2-Clause, a 2,711-line read/write implementation over FUSE). The oracle is
FreeBSD's `newfs`, `makefs` and `fsck_ffs`, all BSD-2-Clause, all building on a host. The crash
injector comes from milestone 37 (Prove RedoxFS's crash consistency), which injects power cuts, torn
writes and a lying device and needs no change.

## Why UFS2

UFS2 is FreeBSD's default root filesystem and the second on-disk generation of the Berkeley Fast File
System. The format stays small: 256-byte inodes, 64-bit block pointers, fragments, cylinder groups,
extents, extended attributes and snapshots. McKusick's paper and FreeBSD's `sys/ufs/ffs/fs.h`
document it.

A launch store must do five things here. It formats, mounts read-write and writes files on the three
targets. It carries permissions, symlinks, hard links and extended attributes. It rests on a mature
implementation that makes and judges images. It ships under a license the tree accepts. And it
leaves nife a server rather than a port. UFS2 passes each one.

The license is the first refusal, and UFS2 nearly clears it already. `deny.toml` allows MIT,
Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-3-Clause, 0BSD and ISC. BSD-2-Clause is BSD-3-Clause
minus the no-endorsement clause, so it is strictly easier than a clause the list already holds.
Adding it takes one line beside the ISC entry and one ruling from calef.

The survey holds the full comparison. btrfs and OpenZFS survive only as read-only libraries or
confined GPL programs. Their licenses bar a linked write path (DECISIONS §135 (Running GPL software
is aggregation, the capability boundary is what makes it so, and packages are how it arrives)).
HAMMER2 offers no implementation to reuse and no extended attributes. RedoxFS keeps the role until
one of these proves itself (DECISIONS §34 (RedoxFS is the primary filesystem, on three conditions)).

## The arc

### Phase 1: the license and the builds

The BSD-2-Clause ruling goes to calef first, because it gates the FreeBSD half. In parallel, build
`ufs-core` and `rufs` for the three bare targets and record what stops each one.

Both stop today at `can't find crate for 'std'`. `ufs-core` needs `no_std` plus an `alloc` import;
its blockers are `thiserror` and `Vec`, not the format logic. `rufs` runs `std` by design because it
carries `libc` and FUSE, so the fork here is whether the server is a `std` program or `rufs` gains a
`no_std` port. Record both the way the 2026-10-07 measurement recorded the btrfs and ext4 crates: as
shipped, and with the patch.

### Phase 2: make, judge, and read

A UFS2 mkfs and a read-only server, with FreeBSD as the judge. `makefs` and `newfs` on a host produce
two images, `rufs` reads both, and then `fsck_ffs` judges what nife writes.

The deliverable is a server that mounts an existing UFS2 image and reads files through it, bound to
the `filesystem_contract` the RedoxFS server uses today. No `CREATE` and no `TRUNCATE` yet, the same
honest gap milestone 32 (A real filesystem: RedoxFS behind a capability FS server) recorded for its
first phase.

### Phase 3: the write path

Create, unlink, rename, symlink, hard link and extended attributes. UFS2 earns its keep here, because
the format already carries all of them and `fsck_ffs` already checks all of them.

The fork is the directory and fragment allocator. `rufs` marks its own writes experimental, so the
lane either hardens `rufs`'s allocator behind the contract or writes an allocator against
`ufs-core`'s reader. Both stay small: the whole format fits in a few hundred lines of structs.

### Phase 4: consistency, and the claim

A crash claim the lane chooses rather than assumes. Soft updates order metadata writes and need no
journal. Soft Updates with Journaling (SUJ) adds an in-filesystem `.sujournal` and replays it after a
crash. Both carry BSD-2-Clause. Neither exists in a Rust implementation today, so phase 4 either adds
one or records the honest alternative: `fsck` on mount, which repairs rather than prevents.

Then run the injector from milestone 37 (Prove RedoxFS's crash consistency) against the server and
publish the three numbers RedoxFS carries: 186 lying-device cases, 112 recovered, 74 refused, 0
silently wrong. A launch store that cannot match the zero is not done, whatever else it does.

## The decision asked

Two of them, for calef:

1. Add BSD-2-Clause to `deny.toml`'s allow-list, or refuse UFS2's FreeBSD half and keep only the
   Apache-2.0 reader.
2. Re-aim DECISIONS §34 (RedoxFS is the primary filesystem, on three conditions) from RedoxFS to
   UFS2 once phases 2 through 4 pass, which is what moves RedoxFS to a retirement lane of its own.

## What stays

The `filesystem_contract` crate, the capability model and the FS-server shape do not change. The
crash injector stays. The GPL confinement lesson from §135 (Running GPL software is aggregation, the
capability boundary is what makes it so, and packages are how it arrives) stays, and it keeps btrfs
and OpenZFS reachable as confined programs.

## What this does not decide

Milestone 140 (Mount a drive this system did not create) still orders FAT32, then ext2, then
milestone 190 (ext4, read and write: a Rust implementation with libext2fs as the host-side oracle).
This arc is the launch store, which is a different requirement from mounting a drive nife did not
create, and it reorders neither. Milestone 190 does not touch RedoxFS, and this arc does not either.

## BUGS

- No UFS2 server exists, and no image went through nife. The survey is desk work plus two failed
  builds.
- The `no_std` cost is one build error, not a measured patch. Nobody knows whether closing it takes a
  day or a week.
- HAMMER2's size and the FreeBSD port's completeness come from lookup, so the second-place ranking
  rests on weaker evidence than the first.

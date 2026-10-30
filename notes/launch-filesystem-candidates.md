# The launch filesystem's candidates, surveyed license first

*Name: provisional, minted 2026-10-29 (UTC) by lane/filesystem-candidates-survey. Surveyed at base
`ae00da31b`. It answers the question calef asked on 2026-10-29: which filesystem should nife launch
on, given how little RedoxFS is progressing, and whether UFS, OpenZFS or btrfs is the better
candidate.*

## Why the question is open again

§34 (RedoxFS is the primary filesystem, on three conditions) chose RedoxFS for four reasons: a real
system already roots on it, copy-on-write with checksums, Rust with no `std` on both bare targets,
and maintenance upstream. The fourth reason is the one that has moved.

`vendor/upstream-status.md` records what upstream has done since the pin. RedoxFS 0.9.1 shipped
2026-07-01. Since then the project has taken 43 non-merge commits and cut no release. Most
of those commits are Redox OS integration: `redox-rings`, a writer pipe, `libredox` updates, and
`DiskRing`. None adds a filesystem feature this tree would use, and the two files the tree's
divergence patch touches, `Cargo.toml` and `src/transaction.rs`, are both edited upstream, so the
next bump conflicts.

The first three reasons still hold. This note does not reopen them. It asks whether another
filesystem is a better bet for the *launch* store: the writable filesystem the image boots into,
that `jig` installs packages onto, and that holds the installed-programs table.

## The question, and the order the survey answers it

A launch filesystem must do five things here:

1. Format, mount read-write, and write files, on aarch64, riscv64 and x86_64.
2. Survive a power cut without silently wrong data.
3. Carry permissions, symlinks, hard links and extended attributes, because packages and users need
   them.
4. Rest on a mature implementation that makes and judges it, so a young nife server has an oracle.
5. Ship under a license the tree can accept.

The survey checks those in the reverse order. License is first because it is the cheapest refusal:
a candidate that cannot pass `deny.toml` as linked code never earns the days a build, image and
crash test would cost. Then the oracle, then the writer, then crash consistency.

## The license pass

`deny.toml` allows MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-3-Clause, 0BSD and ISC. The
list is an allow-list on purpose: the failure it catches is a license nobody read.

| candidate | implementation and license | on the allow-list? | verdict |
|---|---|---|---|
| RedoxFS | the vendored engine, MIT | yes | the incumbent |
| UFS2 (FFS) | FreeBSD `sys/ufs`, BSD-2-Clause; `rufs` 0.6.0, BSD-2-Clause; `ufs-core` 0.1.5, Apache-2.0; `go-filesystems/ufs`, BSD-3-Clause | `ufs-core` yes; BSD-2 no, but strictly easier than the BSD-3 already allowed | one ruling from calef |
| HAMMER2 | DragonFly's `sys/vfs/hammer2`, BSD-3-Clause | yes | allowed, and measured below |
| btrfs | Linux's `fs/btrfs` and `btrfs-progs`, GPL-2.0; `lambutter`, `btrfs-core`, `rust-fs-btrfs`, `btrfs-transaction`, MIT or Apache-2.0 | the Rust readers yes; the kernel and progs only as a confined program under §135 (running GPL software is aggregation, and the capability boundary is what makes it so) | read-only Rust survives |
| OpenZFS | `openzfs`, CDDL-1.0 file by file | no | needs a copyleft reading |
| ext4 | Linux's `fs/ext4`, GPL-2.0; `e2fsprogs`, LGPL-2.1; `ext4-view`, MIT or Apache-2.0; `ext4_rs`, MIT | the Rust readers yes; the rest only confined | read-only Rust survives |
| F2FS, XFS, bcachefs, nilfs2 | their kernels and tools, GPL-2.0 | no | no Rust writer, and no confined program exists |

Two findings decide the shape of the rest of the survey.

**UFS2 is the only candidate whose license is both permissive and nearly allowed.** BSD-2-Clause is
not on the list, but it is BSD-3-Clause minus the no-endorsement clause, so it is strictly easier.
Adding it is one line and one ruling, the way ISC entered the list on 2026-09-20. `ufs-core` is already
Apache-2.0. Nothing about UFS2 needs a copyleft reading.

**OpenZFS and btrfs both survive only as confined programs or read-only libraries.** Their licenses
put the write path out of reach as linked code: CDDL for OpenZFS, GPL-2.0 for btrfs. §135 lets a
GPL program run as a separate confined program, which is how the 2026-10-07 note considered LKL. It
does not let a Rust FS server link the kernel's btrfs.

## What each candidate is, and what it costs

### UFS2

The Berkeley Fast File System's second on-disk generation, FreeBSD's default root filesystem. 256
byte inodes, 64-bit block pointers, fragments, cylinder groups, extents, extended attributes,
snapshots. McKusick's UFS2 paper and FreeBSD's `sys/ufs/ffs/fs.h` document the on-disk format, which
is small next to btrfs or ZFS.

Two Rust implementations exist and both are small. `ufs-core` is a 2,778-line reader, `#![forbid(unsafe_code)]`,
bounds-checked, fuzzed, Apache-2.0. `rufs` is a 2,711-line read/write implementation behind a FUSE
driver, BSD-2-Clause, with writes marked experimental and soft updates still planned. Neither is
`no_std` as published. A first build for the three bare targets stops at `can't find crate for
'std'`; `ufs-core`'s blockers are `thiserror` and `Vec`, not the format logic, and `rufs` is `std`
by design because it carries `libc` and FUSE.

The oracle is the strongest of any candidate. FreeBSD's `newfs`, `makefs` and `fsck_ffs` are
BSD-2-Clause, build on a host, and are the same programs that make and repair every FreeBSD root
volume. `fsck_ffs` can judge a nife UFS2 server on every image it writes, the way `fsck_msdos`
judges the FAT work.

Crash consistency comes in three grades. Soft updates order metadata writes and need no journal;
they are a runtime mechanism, not an on-disk feature, so a simple writer that ignores them can still
produce an image `fsck_ffs` accepts. Soft Updates with Journaling (`SUJ`) adds an in-filesystem
`.sujournal` and replays it after a crash. Both are BSD-2-Clause. Neither is in any Rust
implementation today, so the first nife server would rely on `fsck` after a crash rather than on a
journal, unless the lane takes soft updates on as its own piece.

### HAMMER2

The feature match, and the largest project. DragonFly's default filesystem: block copy-on-write,
checksums that recurse to the volume header, instant snapshots, compression, deduplication, and no
`fsck` because mount chooses the newest valid volume root. BSD-3-Clause, so already allowed.

It has no implementation outside DragonFly's C and a FreeBSD port. There is no Rust reader and no
Rust writer. It also lacks extended attributes, which rule 3 above requires. Adopting it means
porting or hosting a filesystem of its own, which is the objection §34 already made to writing one:
the thesis is the kernel confining the filesystem, not the filesystem.

### btrfs

Already measured on 2026-10-07 in [filesystem-crates-2026-10-07.md](filesystem-crates-2026-10-07.md).
Every maintained Rust reader builds for all three targets, three with no change. No Rust
implementation writes btrfs: `rust-fs-btrfs` refuses ordinary writes and read-write mounts with a
block-group tree, and `btrfs-transaction` is a tree-mutation library with no inode or directory
allocator. The kernel and `btrfs-progs` are GPL-2.0. The 2026-10-07 ruling (fork 3) already refused
btrfs as a RedoxFS replacement for that reason, and nothing since has moved the writer.

### OpenZFS

Also measured on 2026-10-07. `lamzfs` reads ZFS on all three targets and writes nothing. `libzpool`
is the kernel module compiled for user space: about 18 MB of C, threads from `libspl`, an ARC that
expects gigabytes, and CDDL-1.0 on every file. Hosting it needs real threads in one address space,
which is milestone 812 (`std::thread::spawn` runs real threads in one address space), and a
person's CDDL reading against §135. Neither exists yet.

### ext4, and the rest

ext4's Rust readers are permissive and read-only; no Rust writer handles `metadata_csum`, so a write
would produce metadata Linux flags as corrupt. F2FS, XFS, bcachefs and nilfs2 are GPL-2.0 with no
Rust implementation. None is a launch candidate.

## The ranking the license pass leaves

1. **UFS2.** Permissive, nearly allowed, documented, small, two Rust implementations, the best
   oracle, and a journaling design that is BSD-2-Clause. It is the only candidate that can be a
   nife server rather than a port of a kernel subsystem.
2. **HAMMER2.** The closest feature match and already allowed, but no implementation to reuse and no
   extended attributes. A project of its own, not a launch filesystem this year.
3. **RedoxFS.** Keep it until a replacement proves itself. Its crash-consistency measurement (186
   lying-device cases, 112 recovered, 74 refused, 0 silently wrong) is the standard any replacement
   must meet, and its upstream is slow rather than dead.
4. **btrfs and OpenZFS.** Refused for the launch store on the 2026-10-07 measurements and the
   licenses. btrfs stays a read-only candidate for milestone 140 (Mount a drive this system did not
   create); OpenZFS stays a candidate for after 812 and a CDDL ruling.

## What to measure next

The license pass is the first increment. The next one is:

1. The BSD-2-Clause ruling. Add it to `deny.toml`'s allow-list, or refuse UFS2's FreeBSD half and
   keep only the Apache-2.0 reader.
2. Build the two Rust implementations for the three targets. `ufs-core` needs `no_std` and an
   `alloc` import; `rufs` needs a `no_std` port or a decision that the server is a `std` program.
   Record both as shipped and with the patch, the way the 2026-10-07 note does.
3. Make and judge images. `makefs` and `newfs` on a host, read and written by `rufs`, then
   `fsck_ffs` on the result. Two images: one FreeBSD-made, one nife-made.
4. Run the crash injector. The harness of milestone 37 (Prove RedoxFS's crash consistency) already
   injects power cuts, torn writes and a lying device. Run it against a UFS2 server and record the
   same three numbers RedoxFS has.
5. Decide the consistency grade. Soft updates, SUJ, or `fsck` on mount. The first nife server
   can ship without either ordering mechanism, but then its crash claim is `fsck`-repaired rather
   than consistent, and the note that records it must say so.

## BUGS

- No UFS2 server exists, and no image went through nife for this note. The license pass is desk
  work plus two failed builds.
- The `no_std` cost is one build error, not a measured patch. `ufs-core`'s `thiserror` and `Vec`
  uses are the named blockers; nobody has measured whether the patch takes a day or a week.
- HAMMER2's size, and the FreeBSD port's completeness, come from lookup rather than measurement.
- A person qualified to give OpenZFS's CDDL reading owes it. This note does not give it.
- The launch store's exact requirement is not settled here. #2894 asks where the first
  installed-programs table lives, and the answer depends on the filesystem this survey recommends.

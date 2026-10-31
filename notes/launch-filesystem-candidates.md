# The launch filesystem's candidates, surveyed license first

*Name: provisional, minted 2026-10-29 (UTC) by lane/filesystem-candidates-survey. Surveyed at base
`ae00da31b`. It answers the question calef asked on 2026-10-29: which filesystem should nife launch
on, given how little RedoxFS is progressing, and whether UFS, OpenZFS or btrfs is the better
candidate.*

## What this survey decided

RedoxFS stays nife's launch store (§34 (RedoxFS is the primary filesystem, on three conditions),
ruled 2026-10-31 UTC on #2927): the writable filesystem the image boots into, that `jig` installs
packages onto, and that holds the installed-programs table. nife does not build a UFS2 server
(§34 refuses milestone 981, and milestone 982, the removal that would have followed, with it). The
next measurement is ext4 through `rsext4` under milestone 37's injector, carried by the proposal
`ext4-through-rsext4-under-the-crash-injector`. If it passes, one ext4 server may become both the
launch store and milestone 190's reader of Linux drives; if it fails, RedoxFS stays. The rest of
this note is the evidence: the license pass, each candidate, and the four routes compared.

## Why the question opened

§34 chose RedoxFS for four reasons: a real
system already roots on it, copy-on-write with checksums, Rust with no `std` on both bare targets,
and maintenance upstream. The fourth reason is the one that has moved.

`vendor/upstream-status.md` records what upstream has done since the pin. RedoxFS 0.9.1 shipped
2026-07-01. Since then the project has taken 43 non-merge commits and cut no release. Most
of those commits are Redox OS integration: `redox-rings`, a writer pipe, `libredox` updates, and
`DiskRing`. None adds a filesystem feature this tree would use. The bump itself is cheap. Measured on
2026-10-31 (UTC), nife's whole divergence from 0.9.1 (+120 −17 lines in seven source files) applies
to upstream's head (`db847d8`, 2026-10-29) without a conflict. That head needs one more import
(`Box` in `src/disk/mod.rs`) to build `no_std`, and its 39 library tests pass.

The first three reasons still hold, and this note does not reopen them. It asks whether another
filesystem is a better bet for the *launch* store: the writable filesystem the image boots into,
that `jig` installs packages onto, and that holds the installed-programs table.

## The question, and the order the survey answers it

A launch filesystem must do five things here:

1. Format, mount read-write, and write files, on aarch64, riscv64 and x86_64.
2. Survive a power cut without silently wrong data.
3. Rename one name over another atomically (the installer flips `activation/current` that way),
   keep a modification time that only advances, and keep names as written. Extended attributes are
   in `filesystem_contract` with no production caller yet. Permissions, symlinks and hard links have
   no verb in the contract, since access is a right on a directory capability, so they are habit
   today and become real with the first ported POSIX package that needs one.
4. Rest on a mature implementation that makes and judges it, so a young nife server has an oracle.
5. Ship under a license the tree can accept.

The survey checks those in the reverse order. License is first because it is the cheapest refusal:
a candidate that cannot pass `deny.toml` as linked code never earns the days a build, image and
crash test would cost. Then the oracle, then the writer, then crash consistency.

## The license pass

`deny.toml` allows MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-3-Clause, 0BSD, ISC,
BSD-2-Clause and Zlib. The list is an allow-list on purpose: the failure it catches is a license
nobody read.

| candidate | implementation and license | on the allow-list? | verdict |
|---|---|---|---|
| RedoxFS | the vendored engine, MIT | yes | the incumbent |
| UFS2 (FFS) | FreeBSD `sys/ufs`, BSD-2-Clause; `rufs` 0.6.0, BSD-2-Clause; `ufs-core` 0.1.5, Apache-2.0; `go-filesystems/ufs`, BSD-3-Clause | yes; BSD-2-Clause is on the list | allowed; refused for the launch store on its other costs |
| HAMMER2 | DragonFly's `sys/vfs/hammer2`, BSD-3-Clause | yes | allowed, and measured below |
| btrfs | Linux's `fs/btrfs` and `btrfs-progs`, GPL-2.0; `lambutter`, `btrfs-core`, `rust-fs-btrfs`, `btrfs-transaction`, MIT or Apache-2.0 | the Rust readers yes; the kernel and progs only as a confined program under §135 (running GPL software is aggregation, and the capability boundary is what makes it so) | read-only Rust survives |
| OpenZFS | `openzfs`, CDDL-1.0 file by file | no | needs a copyleft reading |
| ext4 | Linux's `fs/ext4`, GPL-2.0; `e2fsprogs`, LGPL-2.1; `ext4-view`, MIT or Apache-2.0; `rsext4`, Apache-2.0; `rust-fs-ext4` and `ext4_rs`, MIT | the Rust implementations yes; the rest only as an oracle or confined | a permissive Rust writer survives |
| F2FS, XFS, bcachefs, nilfs2 | their kernels and tools, GPL-2.0 | no | no Rust writer, and no confined program exists |

Two findings decide the shape of the rest of the survey.

**UFS2's license needs no copyleft reading.** BSD-2-Clause is BSD-3-Clause minus the
no-endorsement clause, so it is strictly easier than an entry the list already held, and it sits
on the list. `ufs-core` is Apache-2.0.

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
implementation today, so a first nife server would rely on `fsck` after a crash rather than on a
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

ext4 has a permissive Rust writer. On 2026-10-31 (UTC), `rsext4` 0.9.2 (Starry-OS, Apache-2.0,
`no_std`) built unchanged for the three nife targets. Every write it made on `mkfs.ext4` images
with `metadata_csum` on passed `e2fsck -fn`: a file, a directory, a symlink, a hard link, a rename
and an extended attribute. `rust-fs-ext4` 0.8.0 passed on ext4 too, and `ext4_rs` 1.3.3 failed. The
kernel's ext4 and e2fsprogs stay GPL-2.0 and LGPL-2.1, so they serve as the oracle under §136. F2FS,
XFS, bcachefs and nilfs2 are GPL-2.0 with no Rust implementation, and none is a launch candidate.

## The four routes, compared

Compared on #2927, 2026-10-31 (UTC). Line counts are non-blank, non-comment lines; a range is an
estimate.

| | RedoxFS, vendored | nife-owned UFS2 | nife-owned ext2 | ext4 |
|---|---|---|---|---|
| Rust to own | none new; the engine is 4,590 lines if upstream stops | reader about 1,200, writer 3,000 to 4,000, an `fsck` about 5,000 | about 1,500, 3,000 and 5,000 | `rsext4` (30,456) as a dependency, or 8,000 to 15,000 of our own |
| Crash mechanism | copy-on-write, a header ring, a checksum on every pointer | `fsck` on mount; soft updates exist in no Rust | `fsck` only | a jbd2 journal, which `rsext4` carries, untested |
| Oracle that judges our writes | none independent | `fsck_ffs` | `e2fsck` and a Linux mount | `e2fsck`, and Linux's journal replay |
| Milestone 37's injector | 0 silently wrong of 186 | no data checksum, so a lying device cannot reach 0 | the same | the same; metadata checksums turn most metadata damage into a refusal |
| Other systems' drives (milestone 140) | none | few | some, and 140's second stratum | most Linux drives |

FAT fails the launch store before any of this. It has no atomic rename over a name and no crash
mechanism, it stores times to two seconds between 1980 and 2107, and it folds case.

## The ranking

1. **RedoxFS**, the launch store. It meets every requirement above today, and it is the only
   candidate measured against milestone 37's injector (186 lying-device cases, 112 recovered, 74
   refused, 0 silently wrong). Its weaknesses: nothing but RedoxFS reads it, so no independent
   oracle exists, and its `Disk` trait has no flush.
2. **ext4 through `rsext4`**, the next measurement. One server could hold the launch store and read
   Linux drives for milestone 190.
3. **UFS2.** Refused for the launch store (milestone 981). It has the best oracle, and it would need
   a writer, an `fsck` and a crash mechanism of nife's own, for little interop.
4. **HAMMER2.** No implementation to reuse and no extended attributes.
5. **btrfs and OpenZFS.** Refused for the launch store on the 2026-10-07 measurements and the
   licenses. btrfs stays a read-only candidate for milestone 140 (Mount a drive this system did not
   create); OpenZFS stays a candidate for after 812 and a CDDL ruling.

## What comes next

The proposal `ext4-through-rsext4-under-the-crash-injector` puts `rsext4` behind milestone 37's
write log and runs its three sweeps, judged by `e2fsck -fn` after the journal replays. The launch
volume keeps the first installed-programs table as files under `activation/` (ruled 2026-10-30
UTC, on #2894), on RedoxFS.

## BUGS

- No image went through nife for this note. The ext4 and UFS2 builds are link-only, and their
  writes ran on the host.
- [ufs2-builds-2026-10-31.md](ufs2-builds-2026-10-31.md) keeps the UFS2 builds: 6 lines for
  `ufs-core`, 49 for `rufs` as a nife `std` program.
- The ext4 probes live outside the tree so far, not beside
  [filesystem-crates-2026-10-07.md](filesystem-crates-2026-10-07.md), and the estimates in the
  table above are judgments.
- HAMMER2's size, and the FreeBSD port's completeness, come from lookup rather than measurement.
- A person qualified to give OpenZFS's CDDL reading owes it. This note does not give it.

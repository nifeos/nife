---
status: PARTIAL
raised: 2026-08-30
milestone_dependencies: none
decision_dependencies: 46, unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 190. ext4, read and write: a Rust implementation with libext2fs as the host-side oracle

calef's ext4 drives hold the family's borg repositories. They were formatted by Linux with
`mke2fs` defaults, `metadata_csum` among them, and they will not be reformatted. So nife reads and
writes ext4 as Linux wrote it. The requirement is standing and has no deadline.

The implementation is the Rust crate `rsext4`. We bring it up to the bar of §46 (thin primitives
or whole subsystems; we write everything in between) by contributing to it upstream. We test it here
against libext2fs and against Linux itself, under §136 (a mature foreign implementation earns its
place as an oracle, not as a dependency). This is the ext4 row of milestone 140 (mount a drive this
system did not create). It does not change 140's order for FAT32 or ext2, and it does not touch
RedoxFS.

Reuse: `rsext4` 0.9.2 on crates.io (Apache-2.0, `no_std` with `alloc`, depending on `bitflags` and
`thiserror`), developed in `rcore-os/tgoskits` under `fs/rsext4`. The `Starry-OS/rsext4`
repository that its manifest names has not moved since 2026-04-02 and is not where work happens.
e2fsprogs 1.47 and a Linux kernel are the host-side oracles. The other candidates and the
reason each lost are in [the survey](0190-ext4-read-write/survey.md).

## Why rsext4

It is the one Rust implementation that writes ext4 with `metadata_csum` and writes a jbd2 journal,
and it has a maintained upstream. Taking it is §46's 2026-10-04 default: a file format is the
exposure case, and code other systems have run carries evidence that code written here cannot.
What it lacks is what our bar asks for, and the rule says to contribute that rather than write
around it.

## What is measured

Measured 2026-11-14 (UTC) on a Linux host with e2fsprogs 1.47, against `rsext4` 0.9.2 through its
public API:

- Read. On a 640 MiB image `mke2fs` made with its defaults (3,025 entries, an extent tree of
  depth 2, an htree directory), every file read back byte-identical to what Linux wrote. Images
  with 1 KiB and 2 KiB blocks failed to mount with an `Io` error. The test device reported
  4,096-byte sectors, which may be the cause; the crate's own round-trip tests at those block sizes
  pass.
- Write, bug 1. When a directory grows past its inline extents into a level-1 extent leaf, the
  directory's `i_blocks` comes out 16 sectors short. It reproduces at every size tried from 300 to
  3,000 entries. `e2fsck -fn` exits 4 ("i_blocks is 488, should be 504"). The directory's contents
  are correct.
- Write, bug 2. `truncate_inode` on a file whose extent tree has depth 2 returns
  `Corrupted { op: "extent:empty_index" }` in 2 of 8 seeds. It is not atomic: when it fails, the
  bytes from the new end of file to the end of that block are already zeroed.
- Journal. It writes descriptor, revoke and commit blocks with journal checksum v3, and Linux's
  `e2fsck` replays them. Across two images, 158 power-cut points all recovered to a clean volume.
  It commits only on `sync()` or unmount. In 2 of 79 cuts on one image the `needs_recovery` flag was
  cleared before the journal's start was reset; both recovered, and the order is still wrong.
- `unsafe`. Eleven uses. Nine are the hardware CRC32C paths for aarch64 and x86_64. Two are in
  `blockgroup_description/table.rs`, which casts a `&[u8]` to `*const Ext4GroupDesc` and to
  `*mut Ext4GroupDesc` with no `SAFETY` comment and no proof of alignment.
- Fuzzing and proofs. The crate has no fuzz targets and no Kani harnesses.
- Targets. It builds unchanged for nife's three targets, measured by milestone 997 (ext4
  through rsext4, under milestone 37's crash injector). Nothing has run it under nife.

Not yet tested: a mount by the kernel rather than `e2fsck`, writes reordered between flushes,
extended attribute writes, volumes over 1 GiB, concurrent callers, `orphan_file`, hostile images,
and performance.

## The bar, and where rsext4 stands

§46, as amended, lets a third-party crate be taken when it meets four conditions; a crate short of
them is one we contribute to upstream until it meets them.

| condition | `rsext4` 0.9.2 | what closes it |
|---|---|---|
| a safety argument on every use of `unsafe` | none on the two casts; the CRC32C paths are not yet read | the review below, landed upstream |
| proofs that match ours | none | the harnesses below, upstream where they fit, here otherwise |
| aarch64, riscv64 and x86_64 | builds for all three; hardware CRC32C on two, a software path on riscv64 | the oracle suite passing on each |
| maintained | active in `tgoskits`; at least 24 commits by six people from 2026-08-04 to 2026-11-08 | a crates.io release that carries our fixes |

## What we contribute upstream

Every contribution goes to `rcore-os/tgoskits`, against `fs/rsext4`. That repository carries its
own `AGENTS.md` and `CLAUDE.md`, so its maintainers work with agents themselves. Its policy on
agent-written submissions is read, and followed, before the first one.

1. The `i_blocks` fix, with its reproducer as a test in the crate: grow a directory to 300
   entries, unmount, and require `e2fsck -fn` to exit 0.
2. The truncate fix, with its reproducer: the failing seeds as fixed inputs, a check that
   truncation of a depth-2 tree succeeds, and a check that a failed truncation leaves the file's
   bytes as they were. Atomicity is the larger half of this fix.
3. The recovery-flag order. The journal's start is reset before `needs_recovery` is cleared,
   with the power-cut point that showed it as a test.
4. Fuzz targets: mount and walk an arbitrary image; parse a superblock, a group descriptor
   table, an extent tree, a directory block and an htree index; and replay an arbitrary journal.
   Each target runs under `cargo fuzz` with a seed corpus of small images `mke2fs` made.
5. The `unsafe` review. A `SAFETY` comment on each of the nine CRC32C uses, stating the CPU
   feature the call needs and where it is detected. For the two group-descriptor casts, a sound
   replacement: decode each field from the byte slice with `from_le_bytes`, or use a
   zero-copy derive that checks size and alignment, so the casts and their `unsafe` go. The
   replacement keeps the 32-byte and 64-byte descriptor sizes and is tested against both.
6. Kani harnesses that match ours. The claims nife proves of its own on-disk parsers, written
   for `rsext4`'s. Decoding a superblock, a group descriptor, an extent header or entry, or a
   directory entry reads only inside the slice it was given, for every input up to a stated bound.
   A directory block walk ends and stays inside its block. The software CRC32C equals the bitwise
   definition. Harnesses that upstream does not want are kept here, against the crate's public
   API.

**Every issue and pull request text is shown to the architect before it is filed.** An upstream
submission is an outward step, and the review is of the exact text, not of the idea. A fuzz finding
in `rsext4` is reported the same way. If upstream declines a change the bar needs, this milestone
stops and takes the choice to the architect.

No contribution is consumed from a branch. §305 (git dependencies are allowed on `nifeos/*` only,
pinned to a landed commit) rules out a git dependency on `tgoskits`, so nife takes a fix when a
crates.io release carries it.

## What we test on our side

The four phases keep their intent, each independently useful. The measurement suite above becomes
the oracle that every phase is judged by.

| phase | what | judged by |
|---|---|---|
| 1 | Read-only: one confined `ext4_server` holding one block-device capability, with `rsext4` mounted read-only | the oracle, read half, over the corpus |
| 2 | The differential oracle: `rsext4` against libext2fs and against Linux, in `tools/`, on the host | itself, in CI, before any write is trusted |
| 3 | Write, `metadata_csum` on every write | `e2fsck -fn` exits 0 after each operation; Linux reads back what nife wrote |
| 4 | jbd2: `rsext4`'s journal, which Linux replays | power cut, then Linux replay, then `e2fsck -fn` |

The oracle. A host harness in `tools/`, its own workspace, `std`, never in the shipping graph,
the pattern `tools/redoxfs_host` set. For reads, it extracts every file and directory listing
through `rsext4` and through `debugfs`, and requires them equal. For writes, it applies an operation
sequence through `rsext4`, then judges the image with `e2fsck -fn` and reads it back through
`debugfs`. Bugs 1 and 2 are its first two fixed cases.

The corpus is images `mke2fs` makes with its defaults and with 1 KiB, 2 KiB and 4 KiB blocks, on
devices with 512-byte and 4,096-byte sectors. It adds extent depths 0 to 2, htree directories,
`orphan_file`, and a volume over 1 GiB. calef's own drives join it read-only, after a `dumpe2fs -h`
on each shows the feature set phase 1 must support. Their borg repositories verify themselves, so
`borg check` after a round trip is an end-to-end result no synthetic image gives.

Write reordering. A block device between `rsext4` and the image records every write and every
flush. For each interval between two flushes, the sweep persists the interval's writes in other
orders and cuts power after each prefix of each order, then lets Linux replay and runs
`e2fsck -fn`. A device may reorder writes between barriers, so a volume that recovers only from
the order it was written in is wrong.

Power cuts and lying devices. Milestone 997 puts `rsext4` behind the injector of milestone 37
(Prove RedoxFS's crash consistency), with power cuts, torn writes and dropped writes. Phase 4
reuses that glue and judges with Linux replay. It adds what 997 does not: a mount on a real Linux
kernel in a virtual machine as well as `e2fsck` replay.

Hostile images. nife's fuzz rotation runs the upstream targets over this corpus, under the
runner of milestone 42 (supply chain and fuzzing in CI), and also drives `ext4_server` over the
block-device seam with mutated images. A drive nife did not write is untrusted input, and the
server holding it is confined so that a parser bug costs one server.

## The adoption ruling it still needs

Each dependency is an architect's ruling, and `rsext4` is not yet ruled. The proposal is its own
pull request, which carries `needs-architect` and shows, for the version it pins:

- each condition of the bar, with the commit or test that meets it;
- the crate's `unsafe`, counted and located, with what checks each use;
- its dependency graph from `cargo tree -e normal` for each target, which at `tgoskits`' head adds
  `spinning_top`;
- the oracle's results on that version, phase by phase;
- that it stays upgradable, under §81 (a dependency stays upgradable; we suppress churn, never the
  upgrade), and what an upgrade reruns.

Phase 1 may start against the oracle on the host before the ruling. Nothing links `rsext4` into a
nife image until it is given.

## Done when

- Bugs 1 and 2 and the recovery-flag order are fixed in a crates.io release of `rsext4`, each with
  its reproducer as a test.
- Every use of `unsafe` in that release carries a safety argument, and the group-descriptor casts
  are gone.
- The Kani harnesses above pass, upstream or here, and `script/verify` runs the ones held here.
- Fuzz targets are upstream, and nife's rotation runs them and `ext4_server`'s with no open finding.
- The oracle passes in CI on aarch64, riscv64 and x86_64, every image in the corpus included.
- The write-reordering and power-cut sweeps recover every point to a clean volume under Linux
  replay.
- `ext4_server` reads and writes a Linux-formatted drive on each architecture under QEMU, and
  `e2fsck -fn` and `borg check` pass afterwards.
- The adoption ruling is given.

## What this milestone does not decide

- Whether nife hosts borg repositories or makes borg archives. Hosting needs only this milestone.
  Making them needs a borg client, and no Rust implementation of the repository format exists. That
  is a separate and much larger block, minted only if calef wants nife to make backups.
- Whether ext4 replaces RedoxFS as the launch store. Milestone 997 measures that, and its result
  does not change this milestone's plan.
- FAT32 and ext2, which keep milestone 140's order.

## BUGS

- Between commits, the journal protects nothing. `rsext4` commits only on `sync()` or unmount, so a
  power cut loses every write since the last of them. For a borg repository this is mostly survivable,
  because borg discards a segment tail with no commit tag and `borg check` verifies every chunk. That
  is a mitigation, not an equivalence, and the server must say so where a reader meets the feature.
  When `ext4_server` commits is a design question for phase 4.
- ext4 checksums metadata, not file data, so a write the device drops reads back as stale bytes with
  no error.
- `e2fsck` has no Rust equivalent here. A drive damaged on nife is repaired by plugging it into
  Linux.
- The `metadata_csum` claim about calef's drives is inferred from `mke2fs`'s default until the
  `dumpe2fs -h` survey measures it.
- The 1 KiB and 2 KiB mount failures are not yet explained.
- No phase is priced. The honest estimate comes after the corpus survey and the first upstream
  round.

## Follow-on

- **Done.** The plan: this block and its survey appendix name the crate, the measurements, the
  upstream work, our own tests and the conditions above.
- **Outstanding.** The upstream round: the two bug fixes, the recovery-flag order, the safety
  arguments, the cast replacement, the fuzz targets and the Kani harnesses. Each text goes to the
  architect before anyone files it.
- **Outstanding.** Our side: the oracle, the image corpus, the write-reordering and power-cut sweeps,
  hostile-image fuzzing, and `ext4_server` itself.
- **Outstanding.** The adoption ruling under §46 (dependency rule), which no lane can give.

## Index row

calef's ext4 drives hold the family's borg repositories, so nife reads and writes the format Linux
wrote. The implementation is `rsext4`, brought to the bar of §46 by contributing upstream: two
write-path bug fixes with reproducers, fuzz targets, safety arguments and a sound replacement for
two unchecked casts, and Kani harnesses. Here, a differential oracle against libext2fs and Linux
judges four phases: read-only, the oracle itself, write, and jbd2 with Linux replay, plus write
reordering and hostile-image fuzzing. Every upstream text is shown to the architect before it is
filed, and adopting the crate still needs an architect's ruling.

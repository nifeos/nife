# ext4, read and write: the write oracle

The write half of [milestone 190](../0190-ext4-read-write.md)'s oracle: what `rsext4` 0.9.2 writes,
judged by `e2fsck -fn` and read back through `debugfs`. The read half is in
[the read-oracle appendix](read-oracle.md).

## How it judges a write

`tools/ext4_oracle/write_compare.py` builds the read oracle's corpus, then runs each case in
`tools/ext4_oracle/write-cases.txt` on a fresh copy of three of its images:

| base | block size | sector size |
|---|---|---|
| `default-4k` | 4,096 | 4,096 |
| `default-4k` | 4,096 | 512 |
| `default-1k` | 1,024 | 512 |

`ext4_oracle_write` (`tools/ext4_oracle/src/bin/ext4_oracle_write.rs`) mounts the copy read-write
through `rsext4`, applies the case's operations, and unmounts. A failed operation stops the case
there, and the writer still unmounts the volume, so the oracle judges what the failure left too. The data each
write puts down is a xorshift64 stream of a seed that the case names, which the driver computes
itself; nothing on the oracle side asks `rsext4` what it wrote.

The oracle judges each run three ways:

- apply: whether every operation returned success, or the first that failed and its error;
- fsck: what `e2fsck -fn` 1.47 prints, with the clock set to the corpus's own time;
- readback: the `debugfs` manifest of the written image against a model. The model is the base
  image's `debugfs` manifest with the same operations applied in Python, up to the operation that
  failed and not including it. Path, kind, permission bits, owner, group, link count, size (files
  and links), content and extended attributes must be equal. Times, inode numbers and extents are
  the writer's to choose and are not compared.

The cases cover these operations:

- creating files and directories, and writing to them: small, one block, 1 MiB, past a hole;
- overwriting and appending, to files the case made and to files Linux wrote;
- truncating down and up, and truncating a depth-2 tree Linux wrote;
- renaming a file, a directory, and over an existing name;
- unlinking the last link and one of three, and `mkdir` and `rmdir`;
- hard links, and fast and slow symbolic links;
- growing a directory into an htree of 3,000 entries, and the corpus's own htree by 500;
- setting, replacing and removing extended attributes, in the inode and in a block, on new and
  Linux-written inodes, in the `user`, `trusted` and `security` namespaces;
- `sync` between operations. Each of the 16 cases runs on the three
bases, so a run has 48 outcomes, held to `tools/ext4_oracle/write-expected.txt`. A change in either
direction fails the run, so a fix updates this page, the milestone's `BUGS` and the expected file
together.

## What it measured

Measured 2026-11-22 (UTC) on a Linux host with e2fsprogs 1.47.2, against `rsext4` 0.9.2. A run takes
100 seconds on a developer machine, about five minutes of processor time.

Every operation applies, and the readback equals the model, in every case but bug 2's. `e2fsck`
calls 35 of the 48 images clean. The other 13 are three bugs, two already known and one the oracle
found:

| case | 4 KiB on 4,096 | 4 KiB on 512 | 1 KiB on 512 |
|---|---|---|---|
| `directory-past-inline-extents` (bug 1) | exit 4, `i_blocks` 72 for 88 | exit 4, 72 for 88 | exit 4, 58 for 62, padding |
| `truncate-depth-2-to-zero` (bug 2) | fails, readback equal | fails, equal | fails, equal |
| `truncate-depth-2-inside-a-block` (bug 2) | fails, content differs | fails, differs | fails, differs |
| `inode-bitmap-padding` (bug 3) | clean | clean | exit 4, padding |
| `create`, `mkdir-rmdir`, `htree-growth` | clean | clean | exit 4, padding |

- Bug 1. A directory that grows while files' data takes the blocks after it fragments, so it passes
  the four extents its inode holds into a level-1 leaf. Its `i_blocks` then comes out two blocks
  short: 16 sectors at 4 KiB blocks, 4 at 1 KiB. `e2fsck` also offers to narrow the tree. A
  directory grown with empty files stays contiguous, keeps its extents in the inode and is clean, at
  3,000 entries as at 500. The directory's entries read back correct.
- Bug 2. A file written as 1,400 separate blocks has an extent tree of depth 2 at both block sizes.
  `truncate_inode` to 0 or to 100 bytes returns `Corrupted { op: "extent:empty_index" }` every
  time. To 0 it leaves the file's bytes as they were. To 100 it has already zeroed bytes 100 to
  4,095 of the first block, which the readback shows as a content difference: the failure is not
  atomic. In both, `e2fsck` exits 0 but offers to narrow the tree. A depth-2 file `mke2fs` wrote
  (`/extents/depth-2`) truncates to 0 cleanly.
- Bug 3, found here. At 1 KiB blocks a group's 1,024 inodes take 1,024 bits of an 8,192-bit bitmap
  block, and `e2fsck` requires the bits past them set. The first inode `rsext4` allocates in a group `mke2fs`
  left `INODE_UNINIT` gets a bitmap whose padding is clear, and `e2fsck` exits 4 ("Padding at end of
  inode bitmap is not set"). In the corpus image `mke2fs` initialized groups 0 to 2, which hold three
  free inodes, so the fourth new inode is the first to reach group 3; any case that makes four inodes at
  1 KiB shows it. The 4 KiB image has one group and never reaches the path.

## What it does not cover

- `inline_data`, which `rsext4` cannot mount;
- images without `metadata_csum` or `orphan_file`, and 2 KiB blocks;
- writes reordered between flushes, and power cuts;
- owners other than root and permissions other than 0644 and 0755, which the operation language
  does not set;
- a readback through the Linux kernel rather than libext2fs.

*Name: provisional. For the measurement the parent's phase 3 cites. calef names files.*

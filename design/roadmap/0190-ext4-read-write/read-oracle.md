# The read oracle: what `rsext4` reads, against `debugfs`

The measurement behind phase 1 of [milestone 190 (ext4, read and write: a Rust implementation
with libext2fs as the host-side oracle)](../0190-ext4-read-write.md). The reader is the `rsext4`
that `tools/ext4_oracle/Cargo.toml` pins: nifeos/rsext4, which is upstream's 0.9.2 with the fixes
its `CHANGES.md` lists. Measured on a Linux host with e2fsprogs 1.47.2, against 0.9.2 on 2026-11-16
(UTC) and against the fork on 2026-11-23. Every image was generated there; no real drive was read.

## How it compares

`nifeos/nife:tools/ext4_oracle/corpus/make-corpus.sh`
builds each image with `mke2fs -d` from the tree
`nifeos/nife:tools/ext4_oracle/corpus/populate.py` writes. It then sets three files'
times with `debugfs`, indexes the large directory with `e2fsck -fyD`, and requires `e2fsck -fn` to
exit 0.
Two runs give byte-identical images, since the script fixes the UUID, the hash seed and every
time.

Each side prints one line per directory entry, sorted by path, and the two must be equal line for
line. The fields:

- the path, the inode number, the kind and the permission bits;
- the owner, the group, the link count and the size;
- the access, change, modification and birth times;
- the content: length and FNV-1a-64 of a file, the target of a symbolic link;
- every extended attribute's name, length and FNV-1a-64;
- the leaf extents: logical block, length, physical block, and whether each is unwritten.

`nifeos/nife:tools/ext4_oracle/debugfs_manifest.py`
prints the `debugfs` side. The `rsext4` side mounts read-only with journal replay off, walks
`read_directory` from the root, and reads each file, link, attribute list and extent map through
the crate's public API; it is `nifeos/nife:tools/ext4_oracle/src/main.rs`.
`nifeos/nife:tools/ext4_oracle/compare.py` builds the corpus, runs both
sides on each image at both sector sizes, and holds the outcomes to
`nifeos/nife:tools/ext4_oracle/expected.txt`. The CI job
`ext4-read-oracle` runs it in every merge group, and on a pull request that touches the oracle.

The tree holds 3,060 entries: 3,031 regular files, 23 directories, 5 symbolic links and a FIFO.
They put each shape the reader must handle in front of it:

- files at and across the block size;
- sparse files whose extent trees reach depth 0, 1 and 2 at the image's block size;
- a directory of 3,000 entries that `e2fsck -D` turns into an htree;
- fast and slow symbolic links, and a dangling one;
- a file with three hard links;
- extended attributes in the inode, and one of 900 bytes in its own block;
- long and non-ASCII names;
- three files whose times need the extra fields: nanoseconds, a time before 1970 and a time past
  2106.

## The corpus

| image | block | features beyond the 1.47 defaults |
|---|---|---|
| `default-4k` | 4 KiB | none: `metadata_csum`, `metadata_csum_seed`, `orphan_file`, `64bit`, `flex_bg`, `huge_file`, `dir_nlink`, `extra_isize`, `dir_index`, `ext_attr`, `has_journal` |
| `default-2k` | 2 KiB | none |
| `default-1k` | 1 KiB | none |
| `no-metadata-csum-4k` | 4 KiB | `^metadata_csum`, `uninit_bg` in its place, so group descriptors carry the older CRC16 |
| `no-metadata-csum-1k` | 1 KiB | the same |
| `inline-data-4k` | 4 KiB | `inline_data` |
| `no-orphan-file-4k` | 4 KiB | `^orphan_file`, as a drive formatted before e2fsprogs 1.47.0 would be |

e2fsprogs 1.47.0 added `metadata_csum_seed` and `orphan_file` to `mke2fs`'s defaults (its release
notes), so which of these a drive carries depends on the e2fsprogs that formatted it.

## Results

The reader saw each image twice: on a device reporting 512-byte logical sectors and on one
reporting 4,096-byte sectors.

| image | 512-byte sectors | 4,096-byte sectors |
|---|---|---|
| `default-4k` | all 3,060 equal | all 3,060 equal |
| `default-2k` | all 3,060 equal | refused: `BadSuperblock`, `device:filesystem_geometry` |
| `default-1k` | all 3,060 equal | refused: the same |
| `no-metadata-csum-4k` | all 3,060 equal | all 3,060 equal |
| `no-metadata-csum-1k` | all 3,060 equal | refused: the same |
| `inline-data-4k` | refused: `UnsupportedFeature`, incompatible bit `0x8000` | the same |
| `no-orphan-file-4k` | all 3,060 equal | all 3,060 equal |

On every image that mounts, every field of every entry agrees with `debugfs`. That covers the
directory walk, every byte of every file, every link target, every extended attribute and every
extent, the depth-2 extent trees and the htree directory included, and the four times with their
nanoseconds and epoch bits.

Upstream's 0.9.2 differs in two ways, and the fork fixes both. It reports the three refusals as a
bare `Io` (upstream item 7, below). And three entries differ, the three in `/timestamps`, in the
fields below (upstream item 8).

| file | field | on disk (seconds field, extra field) | `debugfs`, decoded as Linux does | `rsext4` |
|---|---|---|---|---|
| `nanoseconds` | modification | `0x5f5e1000`, `0x1d6f3454` | 1600000000.123456789 | 1600000000 |
| `nanoseconds` | access | `0x5f5e1000`, `0xee6b27fc` | 1600000000.999999999 | 1600000000 |
| `nanoseconds` | birth | `0x5f5e1000`, `0x00000004` | 1600000000.000000001 | 1600000000 |
| `before-1970` | modification | `0xffffff9c`, `0` | -100 | 4294967196 |
| `before-1970` | access | `0x80000000`, `0` | -2147483648 | 2147483648 |
| `after-2106` | modification | `0x00000010`, `0x00000001` | 4294967312 | 16 |
| `after-2106` | birth | `0x00000020`, `0x00000002` | 8589934624 | 32 |

In 0.9.2, `InodeInfo`'s four times are the raw 32-bit seconds fields, so the API drops the nanoseconds, the
sign and the epoch bits. The crate decodes them correctly inside (`Ext4Inode`'s
`decode_extra_time` matches Linux's `ext4_decode_extra_time`), and its write path takes a full
`Ext4Timestamp`; only the read side drops them. `debugfs` 1.47.2 itself prints the before-1970
time as a date in 2106, since it shows the seconds field unsigned; the manifest decodes the raw
fields the way Linux does.

## Why 1 KiB and 2 KiB blocks are refused on 4,096-byte sectors

`rsext4` addresses the device in its logical sectors, and a 1 KiB or 2 KiB block is not a whole
number of 4,096-byte sectors, so it cannot read one. The refusal is correct, and Linux refuses the
same mount for the same reason (recalled, not read). It is in `read_superblock`
(`src/ext4/mkfs.rs`), which sets the filesystem block size on the device; the device's
`validate_filesystem_geometry` (`src/blockdev/cached_device.rs`) returns `BadSuperblock` with the
operation `device:filesystem_geometry`. `mount_with_services` (`src/ext4/mount.rs`) then maps every
error from `read_superblock` to a bare `Ext4Error::io()`, so the caller sees the error a failed read
gives, with no context. Two other call sites do the same: `device_has_error_state` and
`reload_after_journal_replay`.

The fork removes the mapping, and the same mount reports
`BadSuperblock, Operation { op: "device:filesystem_geometry" }`. The crate's own
`tests/linux_image_repro.rs` opens its images with 4,096-byte sectors, which is how the 2026-11-14
measurement met it. `nifeos/nife:tools/ext4_oracle/tests/sector_size_refusal.rs`
requires that report.

## What this does not measure

- A Linux kernel as the reader. The host gave no way to mount a loop device, so the oracle here is
  `debugfs`, which is libext2fs and not the kernel.
- A volume over 1 GiB, hostile images, and any image not made by `mke2fs -d`.
- calef's drives. Which of the profiles above they match waits on their `dumpe2fs -h`.

*Name: provisional. calef names files.*

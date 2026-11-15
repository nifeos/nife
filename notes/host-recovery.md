# Reading the backup from a MacBook or a Linux host (milestone 57)

The question that makes a backup credible rather than merely functional: the board is dead, can I
get my data? calef asked it about the RedoxFS volume the backup server writes, and the honest
first answer was "we would have to write something". This note is what got written, what upstream
already had, and the operational rule that has to hold for any of it to matter in a year. The
evidence it has no room for is in [`host-recovery/`](host-recovery/README.md).

## Upstream did not already solve this, and it is worth being precise about why

`vendor/redoxfs/src/bin/` has five binaries and it is easy to assume one of them extracts. None
does:

| Upstream binary | Direction | Use to recover? |
|---|---|---|
| `redoxfs-ar` | host directory **into** a new image | No. It is an *archiver*: it creates the filesystem as it writes. The name suggests `tar`, which reads too; this one only writes |
| `redoxfs-clone` | image **to** image | No. It copies a filesystem to another filesystem, not to host files |
| `redoxfs-mkfs` | makes an empty one | No |
| `redoxfs-resize` | grows one | No |
| `redoxfs` (`mount.rs`) | serves an image over FUSE | Yes, and that is the path we deliberately do not take. See below |

So the read side upstream ships is FUSE, and `fuse` is a feature `tools/redoxfs_host` excludes
(`default-features = false, features = ["std"]`). On Linux enabling it is nearly free. On macOS it
means macFUSE, a third-party system extension, and reduced security mode on Apple Silicon. **A
recovery story that begins "first, reboot into recovery mode and lower your Mac's security
settings" is not a recovery story.** The library, though, links here already with `std`, so the
read paths cost a tool instead: `ls`, `cat` and `extract` on top of the same engine, no FUSE, no
kernel extension, no root, identical on macOS and Linux.

## What the tool does

```
redoxfs_host partitions DEVICE            # the GUID partition table: number, size, type, label
redoxfs_host ls         VOLUME [PATH]     # a directory listing: kind, attributes, size, name
redoxfs_host cat        VOLUME PATH       # one file's bytes to stdout
redoxfs_host xattr      VOLUME PATH       # what extended attributes are on it
redoxfs_host xattr      VOLUME PATH NAME  # one attribute's bytes to stdout
redoxfs_host extract    VOLUME PATH DEST  # a whole subtree onto the host filesystem, attributes and all
```

Plus the pre-existing write side (`mkfs`, `put`) and one addition, `import IMAGE HOST_DIR`, which
copies a host directory into an image using upstream's own `redoxfs::archive`.

A `VOLUME` is an image file, or a device plus a partition selector (milestone 110, below).

`PATH` is always relative to the filesystem root, and `..` is refused, which is the same rule the FS
server enforces on the wire (notes/fs-server.md). `DEST` becomes the thing extracted: a
directory in the image lands as a directory at `DEST`, a file lands as the file `DEST`. That is
`cp -R SRC DEST` with a `DEST` that does not exist yet, and it avoids the "into or as?" ambiguity
that makes people run a recovery twice.

Files, directories and symlinks are extracted, with permission bits masked to `0o777`. Setuid,
setgid and sticky are dropped on purpose: an image is data being recovered, not something that gets
to hand out authority on the machine doing the recovery. Entry names are checked before use, so a
damaged or hostile image cannot walk out of the destination directory.

### The read paths do not write to the image

This is not a detail. The image you are extracting from may be the last copy of the data, and may
be on a failing disk or read-only media. So `ls`, `cat` and `extract` open the file without write
permission and pass `cleanup: false`, and there are two traps in doing that with this engine:

- `FileSystem::open(.., cleanup: true)`, which is what the mount path uses, tidies allocations
  and therefore writes. It is not needed to read correctly: `open` picks the newest *valid* header
  out of the ring either way, which is the crash-consistency property itself (notes/fs-server.md).
  Upstream's own `redoxfs-clone` reads its source disk exactly this way.
- `Transaction::read_node` updates atime, but only when the last read was more than an hour
  ago. That is the worst possible shape for a bug: every test on a freshly made image passes, and
  the first read of a real backup dirties a node and the header ring. `read_node_inner` is the same
  read without the timestamp, so the recovery paths use it. The round-trip test hashes the whole
  image file across a read and compares, which is what pins this down.

## The operational rule: keep the reader, or its exact pin, with the backup

We are pinned at RedoxFS 0.9.1, on-disk format version 8, and a reader must match the format
version it reads. `Header::valid` checks the version before it checks anything else, so an image
written by a different RedoxFS presents as *no valid header anywhere in the ring*, and the engine
reports that as ENOENT. Being told "no such file or directory" about a disk you are holding is the
wrong thing to be told while recovering a backup. So when an open fails, the tool reads the
signature and version straight off the disk and says which version it found and which one it reads.
There is a test that forges the mismatch and asserts the message.

The rule that follows, and it is the whole point of this note:

> **A backup readable only by software you no longer have is not a backup.** Store the recovery
> tool, or the exact source it is built from, *with* the backup.

Concretely, that means the backup's off-site copy carries one of:

1. a built `redoxfs_host` binary for the host you would recover on (fine, but binaries rot across
   OS versions and architectures), or
2. the nife commit hash plus `vendor/redoxfs` at 0.9.1 and `patches/`, which is enough to
   rebuild the tool with nothing but a Rust toolchain, or
3. both, which costs a few megabytes.

Option 2 is the one that survives. It is also why `vendor/` is checked in rather than fetched: the
recovery path must not depend on a registry, a git host, or a company still existing.

The same discipline applies to any future pin bump. Bumping `vendor/redoxfs` to a version with a
different on-disk format silently strands every existing backup, so a bump is a migration, not an
upgrade, and it belongs in `design/decisions/` with a plan for the images already written.

## No filesystem-level encryption, so no key handling

Decided by calef on 2026-07-30 (roadmap, milestone 57): "If I'm struggling to get the data off, I'm
not all that worried about somebody else getting it." RedoxFS does support encryption
(`vendor/redoxfs/src/key.rs`), and this volume does not use it. Encryption belongs at the Time
Machine layer instead, where the Mac encrypts before anything is sent, so the server never holds
plaintext and recovery uses the client's key rather than the server's.

For this tool the consequence is a real simplification: every `FileSystem::open` passes `None` for
the password, and there is no key handling anywhere in the recovery path. Nothing to lose,
nothing to store, nothing to get wrong at 2am.

The caveat belongs here too. If Time Machine encryption *is* switched on, recovery then depends on
that password, which relocates the "can I get my data" risk rather than removing it. That password
belongs wherever the family's other credentials live, not only in one Keychain.

## The same-engine objection, answered once so nobody relitigates it

The reader shares any bug the writer has, because it is the same code. True, and true of every
filesystem: `e2fsprogs` shares lineage with the Linux ext4 driver. The risk that actually strands
data is an *undocumented* format, and RedoxFS is open source with upstream tooling. What the shared
engine does not give you is an independent check of the format, which is why the round-trip test
below writes with upstream's archiver rather than with our own `put`.

## How it is proven

`tools/redoxfs_host/tests/recovery.rs`, and the shape of the test is the argument:

- Every step is a separate invocation of the built binary. Nothing is shared between the write
  and the read but the bytes in the image file: no cached `FileSystem`, no warm allocator, no
  in-process state that could make a reader agree with a writer for the wrong reason. A test that
  writes and reads in one process proves the two halves of one program agree; this proves the
  format is on the disk.
- The write side is upstream's archiver, through `import`. If our reader only ever read images
  our writer made, a shared misunderstanding of the format would pass.
- **The tree is not flat and not small**: a 300 KiB file spanning several 128 KiB records, a
  three-deep directory chain, an empty file, an empty directory, a symlink, and an executable whose
  mode has to survive. Single-block flat cases hide exactly the bugs worth catching.
- The extracted tree is compared against the tree that went in, entry by entry, including file
  length, a content fingerprint, permission bits, and whether a symlink came back as a symlink or
  was flattened into a copy.
- Refusals are asserted, not assumed: `..`, `cat` of a directory, `ls` of a file, a missing name.
- The image is hashed before and after a read to prove the read wrote nothing.

## Extended attributes come back on the files (milestone 57)

An image written by nife carries a directory in its root called `.extended-attributes`, holding one small
file per node that has extended attributes. `ls` shows it and `extract` copies it out. That the
store is visible here is a decision and not a leak: no client of the FS server can reach it, and a
recovery host is not a client of the contract; it holds the image file. The record format is
written down in `filesystem_contract::xattr::store` so a person holding a damaged image can read it.

`extract` also puts the attributes back on the extracted files and directories (`setxattr` on
macOS, `lsetxattr` on Linux, never following a symlink), because Time Machine's sparsebundle keeps
Apple's metadata in exactly these attributes. `ls` marks an entry that carries attributes with `@`,
and `xattr` renders or dumps them without extracting anything. Three things it is honest about:

- An attribute's `u32` type code cannot come along, because no host filesystem has a field for
  one. Each dropped code is named on stderr and counted, and the raw store still comes out beside
  the tree, which is where the codes live.
- Nothing about attributes can fail an extraction. A damaged blob, a name a host refuses, or a
  destination that holds no attributes is reported, counted and walked past.
- The counts are printed even when they are zero. "0 attributes reattached" on a backup you know
  carried some is the line that tells you the destination cannot hold them.

[`extended-attributes.md`](host-recovery/extended-attributes.md) has a real transcript on a Mac,
how the reattachment is proven, and its BUGS: Linux refuses a name without a `user.` prefix and
any attribute on a symlink, and FreeBSD is not wired up.

## A device and a partition, not only an image file (milestone 110)

A real disk has a partition table where an image has a filesystem, so the tool takes a device plus
a partition selector and does the offset itself. The selector is `--partition N`, 1-based as in
`/dev/sda3` or `disk4s3`, or `--partition-type GUID` for a program. Both flag names are provisional.
`partitions DEVICE` prints the table first. The selector is refused on `mkfs`, `put` and `import`,
because opening a device read-write by accident is worse than opening an image that way.

```console
$ redoxfs_host partitions target/nifefs-blank.img
$ redoxfs_host extract target/nifefs-blank.img / /tmp/recovered --partition 3
```

The work corrected its own premise. The old tool did not fail on a partitioned disk: the engine's
header scan finds whichever filesystem lies in the first 256 MiB and opens that. That can read a
partition nobody named, miss one further out, and size the allocator from the whole device. So
reading a device whole is now a warning that names the flag, not a refusal.
[`devices-and-partitions.md`](host-recovery/devices-and-partitions.md) has the selectors, the
correction, transcripts, how it is proven, and its BUGS. The largest: it has never been run
against a real raw device, and the GPT backup table is not consulted when the primary is damaged.

## What this does not do, and where it goes next

- **It does not write to an image it is recovering**, by design. `put` and `import` exist for
  building fixtures and open read-write; the recovery verbs never do.
- No repair. If no header in the ring is valid, the tool says so and stops. A format-aware
  salvage tool (walk the tree from an older generation, recover what parses) is a real thing to
  want and is not this.
- The Linux FUSE mount stays available as a feature flag if it is ever wanted for convenience.
  It is not the recovery story, and turning it on would put `fuser` into the one tool that
  currently has no platform dependency at all.

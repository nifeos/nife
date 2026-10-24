# The `redoxfs_host` library: its boundaries and its tests

*An appendix to [`notes/host-recovery.md`](../host-recovery.md). It holds the reasons behind the
seams in `tools/redoxfs_host/src/` and the shape of its tests, which the code's doc comments state
briefly and link here.*

## Errors become strings at the boundary

The engine's error type is `syscall::error::Error`, redox_syscall's errno. The library maps it to
`String` once, at its boundary, so the redox_syscall type never reaches a caller. That is the rule
the roadmap sets for the FS server's ABI edge: map the error type once, at the boundary.

The library never constructs an engine error. That is why the `_tx` helpers return
`Result<T, String>` and the `fs.tx` closures hand that back through `Ok`. A `Transaction` can only
be made by `FileSystem::tx`, whose closure must fail in the engine's own vocabulary. The read paths
write nothing, so committing a walk that failed commits nothing.

## The window onto a partition

`PartitionDisk` is a `Disk` that is a window onto a file. It keeps the engine inside a partition by
construction rather than by the engine's good behavior. Every address the engine computes is
relative to block zero, and `size` reports the partition's length, which is what its allocator
sizes itself from. An address past the end is `EIO`, never a read of the next partition.

It mirrors the `PartitionDisk` in `redoxfs_server/src/bin/mkfs.rs`, with a file underneath instead
of the block-service IPC. The two stay separate: that one is `no_std` inside a `[[bin]]` on the
board, and this one wraps `DiskFile`.

A whole-file volume is the same type with `first_block: 0` and no length, which delegates `size` to
the file. That case must not be turned into a block count. A raw device on macOS reports a length
of zero, and computing a size from it would hand the engine a zero-block disk.

`Volume::window` refuses three partitions, each where a reader meets it:

- A partition whose first byte is not on a 4096-byte boundary. That filesystem would read back
  correctly only through a disk offset by the same fraction, which is the worst failure available.
  The board's `mkfs` refuses to create one for the same reason.
- A partition that runs past the end of a regular file. That is a truncated dump, and half-reading
  one produces a corrupt recovery that looks like a real one. The bound applies only to a regular
  file: a raw device reports a length of zero on macOS, so the check would refuse every real disk.
- A partition shorter than one filesystem block, which holds nothing to open.

The length is floored to whole blocks rather than refused, unlike creation. A partition whose
length is not a multiple of 4096 holds a filesystem that reads perfectly, plus a tail the engine
never addresses.

`warn_if_partitioned` warns rather than refuses when a device is opened whole, because the bytes do
come back. [devices-and-partitions.md](devices-and-partitions.md) has the three things wrong with
letting that pass silently.

## Naming the version when an open fails

`Header::valid` checks the version before anything else. An image written by a different RedoxFS
therefore shows no valid header anywhere in the ring, and the engine's error for that is ENOENT.
"No such file or directory" for a file that is plainly there is the wrong thing to read at 2am. So
when the signature is on the disk, `open_failed` reads the version field beside it and names both
versions. That is the message that sends a person to find the pin.

It reads the version at `at`, where the filesystem starts in the file: zero for an image, the
partition's first byte for a device. Reading at zero would answer "not a RedoxFS image" for every
partitioned disk, which is the message the function exists to avoid.

## Import uses upstream's archiver

`import` is `redoxfs::archive`, the engine half of `redoxfs-ar`, rather than a directory walk of
ours. It is the write side of the format, written by the people who defined the format, so an
`extract` proven against it is proven against something other than our own writer. `redoxfs-ar`
itself cannot be used, because it creates the filesystem as it archives, and `import` fills an image
`mkfs` already made. A failure part-way leaves what was written in place, committed and consistent.
The import is incomplete, and the tool says so and exits non-zero.

## Attributes, read and put back

`.nife-attrs` holds one file per node with extended attributes, named for the node's `TreePtr` id
in hex, in the records `filesystem_contract::xattr::store` defines. The FS server writes it and
nothing else does. The tool reads the layout through `filesystem_contract`, a dependency rather
than a comment describing it (CLAUDE.md rule 7). Nothing in the read path may fail an extraction.
A recovery that abandoned a hundred thousand files over one damaged blob would be worse than
useless, so every problem is a skip that the caller counts and walks past.

`host_xattr` puts an attribute back on a host file. `std` wraps none of it. The call is `setxattr`
on macOS and `lsetxattr` on Linux with different argument lists, and FreeBSD spells it
`extattr_set_link`. So the module is three implementations behind one signature, plus an arm that
refuses on a platform nobody has taught it. Three properties are decisions:

- Links are not followed. macOS passes `XATTR_NOFOLLOW`; Linux uses the `l`-prefixed call. An
  extract recreates a symlink as a symlink, so following it would hang the link's attributes on
  whatever the host has at the target path today, which is somebody else's file. Linux refuses
  `user.*` attributes on a symlink outright, and that refusal is reported, not hidden.
- A failure is an `std::io::Error` carrying the platform's errno, because the errno decides what a
  recovering person should do. `ENOTSUP` means the destination holds no attributes, so extract
  somewhere else. `EPERM` on Linux means the name has no `user.` prefix. `E2BIG` means the value is
  over that filesystem's ceiling.
- Names are bytes. The store refuses only NUL and over-length names
  (`filesystem_contract::xattr::is_valid_name`), and both platform calls take a NUL-terminated C
  string, so the only conversion is the terminator. Nothing invents a namespace prefix: rewriting
  `foo` to `user.foo` would hand back a file whose metadata does not say what the backup said.

## How the tests are shaped

- `tests/recovery.rs` runs every step as a separate invocation of the built binary. Nothing is
  shared between the write and the read but the image's bytes: no cached `FileSystem`, no warm
  allocator. Its tree is neither flat nor small. A 300 KiB file spans 37.5 of the 8 KiB records
  milestone 138 (close the read gap) moved to, and dozens of 4 KiB blocks. A three-deep chain exercises the path walk.
  An empty file and an empty directory are where a "did you write anything?" bug slips through. A
  symlink is a node kind that is neither, and an executable proves the mode survives.
- `tests/partition.rs` builds the filesystem as an image and then places it in partition 3, so the
  bytes came from a writer that knew nothing of partitions. Its table is built with the parser the
  reader uses; xtask's `blank_check_after_run` reads a table the guest wrote, which covers a table
  both halves agree on wrongly. Partition 1, an empty EFI system partition, proves a selector picks
  a partition and a wrong pick fails cleanly. Partition 2 starts on an odd LBA and must be refused.
- `tests/attributes.rs` takes its fixture from `redoxfs_server::Server`, the sans-IO core the board
  runs, over a `DiskFile`. A reader and a writer in one crate can share a misunderstanding of the
  format and agree perfectly. A destination that holds no attributes is a case, not a skip: the test
  asserts the tool said so, with a count, as DECISIONS §42 (a filesystem declares what it offers and must be truthful) requires.
- `tests/roundtrip.rs` reopens the image for every operation, so a read goes through the filesystem
  rather than a cache. It is the pin's witness: the same engine the FS server serves images with.

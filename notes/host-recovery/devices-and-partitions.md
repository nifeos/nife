# A device and a partition, not only an image file (milestone 110)

*Name: provisional. After the parent's own heading.*

How the recovery tool [`notes/host-recovery.md`](../host-recovery.md) describes reads a filesystem inside a partition of a real disk, the
correction the work forced, a transcript, how it is proven, and its BUGS. It was built as
milestone 110 (the recovery tool takes a device and a partition).

A real disk has a partition table where an image has a filesystem, so the tool now takes a device
plus a partition selector and does the offset itself. `crates/globally_unique_identifier_partition_table` reads the table, the
partition's first LBA becomes block zero of a `PartitionDisk`, and nothing above the disk layer
knows a partition was involved. That is the same shape the board's own `mkfs` uses
(`redoxfs_server/src/bin/mkfs.rs`), which is why the host reads the partition by the same rules the board
wrote it by rather than by a second implementation that could disagree.

The gap had a witness, and closing it was the acceptance test. Milestone 57's post-run check
(`blank_check_after_run` in xtask) needed to read a filesystem the guest created inside a partition,
so it parsed the table and sliced the partition out into its own file before handing that file
to the tool. Twenty lines, on the host, in a build script: the join, written in the wrong place.
Those lines are gone and the check passes unchanged, which is a stronger claim than a new test would
have been: it shows the capability *moved* rather than being written twice.

## Two selectors, and why not a third

| Selector | Spelling | For |
|---|---|---|
| Partition number | `--partition 3` | A person with a listing in front of them. 1-based, so it is the number in `/dev/sda3`, in macOS's `disk4s3`, and in `sgdisk -i 3` |
| Type GUID | `--partition-type EC5CC08B-...` | A program. It names what the partition is, so it survives a disk being re-partitioned, and it is how the guest finds its own data partition (DECISIONS §45) |

Not the partition name. GPT labels are cosmetic and frequently absent: macOS writes none at all
(notes/globally-unique-identifier-partition-table.md), so a selector keyed on one would fail on exactly the disk the recovery story is
about. Both flag names are provisional (CLAUDE.md: names are an architect's call).

The selector is refused on `mkfs`, `put` and `import`. Those verbs open read-write, recovery
never does, and opening a *device* read-write by accident is a considerably worse mistake than
opening an image read-write.

`partitions DEVICE` prints the table, because `--partition 3` is only obvious once something has
printed the three.

## The correction: the old tool did not fail on a partitioned disk, which is worse

The milestone's premise was that the tool "reads an image file, not a raw device", and the machine
overruled it during the work. `FileSystem::open` scans blocks 0..65536 for a valid header
(`vendor/redoxfs/src/filesystem.rs`), so handing it a partitioned device does not fail. It finds
whichever filesystem lies in the first 256 MiB and opens that, and the test written to assert a
clean refusal failed by *succeeding*.

That is not the join, and it is worse than not working, for three reasons:

- It reads a partition nobody named. On a disk with two RedoxFS volumes it takes the lower one,
  and a recovery that read the wrong partition looks exactly like a recovery that worked.
- It stops at 256 MiB. A data partition further out is invisible, and a boot partition and a
  root partition ahead of the data is the ordinary layout rather than a contrived one.
- It sizes the engine from the whole device, so the allocator believes it owns bytes that belong
  to other partitions. Harmless while every path is read-only, and a corruption waiting for the day
  one is not.

So reading a device whole is now a warning, not a refusal: the bytes do come back, and a tool
that refused to read a disk it can read would be the wrong answer at 2am. The line says what
actually happened and names the flag that removes the guess.

## EXAMPLES

A real transcript, against `target/nifefs-blank.img` after a test run. Nothing on that disk was
written by the host: the guest partitioned it and made the filesystem, and this is a Mac reading
it back with no `dd` in front of it.

```console
$ redoxfs_host partitions target/nifefs-blank.img
512-byte logical blocks, disk GUID 8784494D-E9F9-48FB-AFC9-F57D43A563B8
  #   first LBA    last LBA          size  type
  1        2048       10239       4.0 MiB  EFI system  nife boot
  2       10240       30719      10.0 MiB  Linux filesystem  nife root
  3       30720      129023      48.0 MiB  nife data  nife data

$ redoxfs_host ls target/nifefs-blank.img / --partition 3
file          63  made-on-target

$ redoxfs_host extract target/nifefs-blank.img / /tmp/recovered --partition 3
extracted / to /tmp/recovered: 1 files, 1 directories, 0 symlinks, 63 bytes, 0 attributes reattached
```

And what it says if you forget the selector, which is the correction above made audible:

```console
$ redoxfs_host cat target/nifefs-blank.img made-on-target
redoxfs_host: target/nifefs-blank.img has a GUID partition table (512-byte blocks, 3
partitions), so it is a device rather than an image. Read whole, the engine takes whichever
filesystem its scan meets first and sizes itself from the whole device. Name the partition with
--partition N (`redoxfs_host partitions` prints them).
CRK57: this filesystem was created by nife on the target
```

## How it is proven

`tools/redoxfs_host/tests/partition.rs`, and the fixture is the argument again. The device is a file
with a real table at the front, and a filesystem built as an ordinary image and then placed in
the third partition. So the bytes in the partition were produced by a writer that knew nothing about
partitions, and an offset wrong by one block in either direction opens nothing. Three partitions, so a
selector has to *pick* rather than find the only filesystem there is, and one of them starts on an
odd LBA so the alignment refusal is exercised rather than asserted.

The test that matters most is `a_partition_past_the_engines_scan_is_reachable_only_by_name`: a
320 MiB device with its data partition at 272 MiB, which the engine's own header scan cannot reach.
Read whole it fails; named, it reads. That is the line between the capability being new and being
cosmetic.

What none of this can catch is a table this project's parser and this project's writer agree is
wrong. `blank_check_after_run` closes that, because the table it reads was written by the *guest*.

## BUGS

- It has never been run against a real raw device. Everything here is proven against files, and
  a device brings its own rules. macOS wants `/dev/rdiskN` for unbuffered access and may want root.
  A raw device reports a length of zero through `metadata`, which is why the
  "partition runs past the end of the file" check applies only to regular files. The arithmetic is
  the same either way; the plumbing around it is not proven.
- A partition whose first byte is not on a 4096-byte boundary is refused, not rounded in, which
  is the same rule the board's `mkfs` applies when creating one. A filesystem placed there reads
  back correctly only through a disk offset by the same fraction.
- The logical block size is guessed, 512 first and then 4096, because nothing in a GPT records
  it and nothing on the blk wire reports it (`disk_surveyor`'s BUGS). A wrong guess fails on the
  signature rather than reading a plausible wrong table, but a disk with some other block size is
  not read at all.
- A hybrid MBR is refused by `crates/globally_unique_identifier_partition_table`, so a disk that Boot Camp or an old macOS installer
  touched will not parse here even though its GPT is fine. That refusal is deliberate (the same disk
  described twice, by two tables that can disagree) and it is inherited, not decided here.
- `--partition` on the write verbs is refused rather than implemented. Formatting one partition
  of a device from the host is a real thing to want and is not this: it would be the first path in
  this tool that can destroy a partition it was not aimed at.
- The backup table is not consulted. If the primary header is damaged the tool says the disk has
  no table, while `GloballyUniqueIdentifierPartitionTable::check_backup` exists and the backup copy
  is probably intact. Reading the backup when the primary fails is the obvious next thing and is not
  built.

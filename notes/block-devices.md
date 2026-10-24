# Block devices: what is attached, and what holding one means

Milestone 57's block-device lane. `crates/globally_unique_identifier_partition_table` had been able to read a partition table since
2026-07-30 and was wired to nothing; notes/globally-unique-identifier-partition-table.md said so plainly, under "what this crate does not
do": *"No I/O, by design. Somebody still has to read LBA 1. That is the block-device lane of
milestone 57, and it is separate on purpose."* This is that lane.

Two things came out of it, and the second is the one worth arguing about.

## 1. The machine reads a table it did not write

`disk_surveyor` holds a block-service endpoint, reads LBA 0 through 33 of the
disk behind it, hands the bytes to `crates/globally_unique_identifier_partition_table`, and reports what is there. Then it reads the 33
blocks at the far end and checks the backup table against the primary. The kernel never sees a
partition table; every byte of judgment happens in a userspace crate whose tests run on the host.

The provenance is the point. The test image is built by `xtask::mkgptdisk` from
`crates/globally_unique_identifier_partition_table/tests/fixtures/sgdisk-64m.{head,tail}`. Those are
the first 34 and last 33 blocks of a 64 MiB disk that `sgdisk` 1.0.10 (gptfdisk, C++) partitioned.
The 64 MiB of nothing between them is left out and put back as zeros. So the guest parses a table
written by people who have never heard of this project. A reader tested against its own writer
proves almost nothing, because every mistake it makes going in it makes symmetrically coming out.
That argument is why the crate's host tests use these fixtures, and it does not stop applying at the
QEMU boundary.

Nothing is *in* the partitions. What is under test is finding them.

### The block-size mismatch, which is the only real arithmetic here

A GPT counts in logical blocks, 512 bytes on every disk this project has met. The block service
a program holds moves one filesystem block per request, 4096 bytes, because that is
`redoxfs::BLOCK_SIZE` and what keeps a mount's device round trips affordable (notes/fs-server.md).
So "read LBA 1" is "read transfer block 0, take bytes 512..1024", and the backup table's 33 logical
blocks start partway into a transfer block at an offset that depends on the disk's size.

`globally_unique_identifier_partition_table::span::Span::covering(byte_offset, len, transfer)` is that arithmetic, in one host-tested
place, because three open-coded divisions in a driver is how you get an off-by-one that reads a
plausible-looking buffer full of the wrong bytes and then blames the CRC. It does not break the
crate's no-I/O rule: it computes *where* to read and the caller reads.

On the 64 MiB test disk the three reads work out as:

| what | logical | transfer blocks | offset |
|---|---|---|---|
| protective MBR | LBA 0 | block 0 | 0 |
| primary header + entry array | LBA 1..33 | blocks 0..4 | 512 |
| backup array + backup header | LBA 131039..131071 | blocks 16379..16383 | 3584 |

### The 512 assumption, stated where a reader meets it

The block protocol (`filesystem_contract::blk`) carries no logical block size. There is nowhere for the
surveyor to read the device's from, so it assumes 512. `crates/globally_unique_identifier_partition_table` handles 4096 and has a test at
it; a 4Kn disk would be read here as though its LBA 1 were at byte 512, which is a wrong answer
rather than an error. The fix is a field on the wire, not a change in the program.

## 2. Listing the drives and holding one are different authorities

This is the design claim, and milestone 57's entry has been making it since 2026-07-30:

> Partitioning and `mkfs` are destructive and need authority over a *whole block device*. So the
> tool holds one device capability and can destroy exactly that device and nothing else. Compare
> `parted /dev/sda` as root, where a typo reaches any disk in the machine, and calef's own
> instructions carry a "confirm the target device path before proceeding" warning precisely because
> the tool cannot enforce it. **Here the warning is structural**: the tool was handed one disk.

`crates/block_roster` is the listing half. One page, written by the kernel at wiring time from its
own mmio and PCIe scans (virtio-blk on either transport, and NVMe controllers by class code),
mapped **read-only** into whichever program was granted it. A program without the mapping has
nowhere to look, and the refusal is "you hold no such capability" rather than a permission check.

**It is deliberately not a `*_proto`.** Ten crates in `crates/` carry that suffix and describe
messages two programs exchange; this one describes bytes at an address, written once and thereafter
only read. The distinction is the authority claim restated rather than a filing convention: a
protocol has a server, and a server is something that can be asked, confused, or persuaded, whereas
there is nothing here to ask. It is also why this page needs no seqlock where the clock page does.

The precedent it follows is the compositor's window enumeration (DECISIONS §33): a **read-only
mapping, not a verb**. There is no request to forge, no server to confuse, and nothing to authorize
at read time, because the authorization happened when the mapping was made.

### The roster carries no capacity, on purpose

An entry is an ordinal and a transport. That is all. Two reasons, and the first is the interesting
one:

- A size is a fact about a device you hold. You learn it by asking (`filesystem_contract::blk::SIZE`),
  which takes the endpoint. An enumerator that answered "how big" would be answering a question only
  a holder should be able to ask, which quietly makes the listing the more powerful of the two
  authorities.
- Finding out would have side effects. Reading a virtio-blk capacity off a PCIe function means
  sizing and assigning BARs and enabling memory decoding (`kernel/src/pci.rs::bring_up`), which is
  not something a *listing* has any business causing, and which would disturb whichever driver
  already owns the function. So `pci::count_block_devices` reads config space and stops.

The ordinals are the same numbers `virtio::find_block_device_n` counts by, which is a promise rather
than a coincidence. The roster and the wiring walk the bus the same way, so a listing and a wiring
cannot disagree about which disk is which.

### What an NVMe entry says

An NVMe entry carries exactly what a virtio one does, an ordinal and a transport (§193 (what a
block-roster entry calls an NVMe disk)). Its ordinal counts NVMe controllers. That identifies a disk
only while each controller has one namespace, which is every controller this tree drives. A
controller with two namespaces is the measurement that would make an entry need to say which.

### The negative control

`disk_surveyor`'s second role announces the roster's address and writes to it. The mapping is
read-only, so the write faults and the kernel kills the process; the test asserts the fault landed at
exactly that address. Knowing where the roster is buys nothing, and there is no address at which the
write succeeds, because the boundary is a page permission rather than a check the program could be
argued out of.

A test that only showed the listing working would be a description. This is what makes it a claim.

## The wiring, in one picture

```text
                      ┌─────────────────────────────────────────┐
   the kernel ────────│ writes ONE roster page, read-only        │
   scans the buses    └──────────────────┬──────────────────────┘
                                         │  RO mapping = "you may LIST"
   disk ──virtio──►┌──────────────┐      ▼
                   │ block server │──blk IPC──►  disk_surveyor
                   └──────────────┘   endpoint = "you may READ AND WRITE
                     owns the DMA,     this one device's blocks"
                     one device
```

The block server is `fs_service::spawn_block_server`, unchanged and now `pub(super)`: the FS server
is no longer the only thing that wants "a block device, served over IPC, by a process that owns the
DMA and nothing else".

## The test disk is the fourth one, and command-line order is reversed

QEMU's `virt` assigns virtio-mmio devices to slots in reverse command-line order, so the GPT
image goes first on the line to land at slot 3, leaving nifefs at 0, RedoxFS at 1 and the
crash image at 2. Both runner scripts explain this where they do it; getting it backwards silently
hands a test the wrong disk. Its own image, for the same reason milestone 37's crash test has one:
a test that shares a fixture couples its result to whether some other test ran first.

## The write half, and the disk it has to itself

There is a fifth mmio disk, at slot 4, and it is blank on purpose.

- **`disk_partitioner`** (provisional name) writes the table `disk_surveyor` reads, drawing its
  unique GUIDs from the entropy service. notes/globally-unique-identifier-partition-table.md has the details, including why every write is
  a read-modify-write.
- **`mkfs`** (provisional name) creates a RedoxFS filesystem inside the nife data
  partition of that table. notes/fs-server.md has the details, including the vendor divergence it
  needed and why the first attempt at that divergence did not work.

The claim both are built to make is about the pair. A disk endpoint and an entropy endpoint are
jointly sufficient to partition and format a drive, and separately neither is. The kernel test
withholds each in turn from the same binary, with the same budget, the same stack and the same
shared page. Then it *reads the disk* to show that a refused run wrote nothing.

`mkfs`'s wiring needed `grant_at` rather than `run`'s fill-in-order grants, and the reason
generalizes: when the missing capability is not the last slot, withholding it has to leave a
hole. A shorter grant list renumbers everything above the gap, so a program that was meant to find
slot 1 empty would instead find its report endpoint there and write a verdict into a block server.

The blank disk is regenerated every run and shared with nothing, for milestone 37's reason
(DECISIONS §27 (the filesystem service)). The riscv `virt` machine uses seven of its eight mmio transports (five disks, a
NIC, an RNG). That is worth knowing before anything wires an eighth: QEMU drops a virtio-mmio device
past the last transport silently, and the symptom is a test *skipping*.

## The kernel's wiring

`kernel/src/user/disk_service.rs` hands every program on this path its pages as `PageFrame`
capabilities, which the program maps itself with `PageFrame::MAP` out of a budget it also holds.
Four properties follow. The rights are attenuable: a roster held `READ` is refused a writable
mapping before any page table is touched. The frame can be delegated onward. The mapping is
recorded, so `PageFrame::REVOKE` can pull it back, and
`disk_tests::the_roster_can_be_revoked_out_from_under_its_holder` is that claim. And the
authority is legible: the `grant_at` calls are the whole of what a program can reach.

So the negative control has two rungs. It cannot obtain a writable window at all, and the fault on
the read-only one is the second line of defense. Only the stack is still wired at spawn, because a
program needs a stack before it can make the syscall that would map one.

Every role of one binary shares one slot layout, and a role that lacks a capability leaves its slot
empty. A shorter grant list would renumber the slots above the gap, and that would be a different
program.

The write half's numbers are reservations, and on a 128 MiB test boot they add up. `mkfs` gets 48
stack pages: a full file-server workload peaks at 135,824 bytes (34 pages) on the deeper ISA, and
creating one filesystem walks less of the tree. The server's own 96 would cost 1.5 MiB more across
the five `mkfs` runs, whose stacks are never freed. Each run's 384-page heap is four times a real
mount's 352 KiB high-water, and `start_maker` destroys the previous run's region before taking
another. When reservations pile up, the symptom is an unrelated test failing to get its budget
several tests later.

## What this lane deliberately did not do

- No hot plug. The roster is written once and never again. A hot-plug story would change
  `block_roster` (a published-page discipline, the way `clock_contract` has one) rather than its
  readers.
- No partition-aware mount. Nothing yet opens a filesystem *at* a partition's offset. The blk
  wire protocol has no base-block field, so a partition capability (an endpoint bound to a window of
  a disk, the way a directory capability is bound to a subtree) has nowhere to live yet. That is the
  natural next rung and it is a protocol change, not a program.

## Three things the build found that the plan did not predict

**`user/link.ld` had a latent bug that only a bss-only program could hit.** `.bss` had no
`ALIGN(4096)`. A program whose `.data` is empty gets `.data` collapsed by the linker, so the `data`
PT_LOAD began wherever `.rodata` happened to end, the two segments shared a page, and the loader
refused the program with `SegmentsOverlap`. Every earlier program happened to have some initialized
data. The surveyor's 44 KiB of table buffers are all `.bss`, so it was the first to meet it.

One stack page is not enough, and the symptom is not a stack overflow. A debug-build
`GloballyUniqueIdentifierPartitionTable::parse` walking 128 entries (an `Entry` is 128 bytes by
value) plus a second `Header::decode` for the backup overran the single mapped stack page by about
200 bytes. That presents as a data abort on the program's own `sp` and then, thirty seconds later,
as the lost-wakeup watchdog, because the test was still waiting on a report from a process that had
died. `spawn_fs_client` records exactly this twice; it is now three times.

**`MAX_DEVICES` again**, the sixth bump. Every milestone that wires one more confined device costs a
table slot forever, because a transport is never unregistered. The constant's comment now says the
fix is an unregister on process death rather than counting to seven.

And then the write half counted to seven, the same day, for the blank disk's block server. The
comment records that it was told not to and why it did anyway: the unregister has to decide what a
`Virtio` capability *is* once its holder is dead. It must also decide whether a transport may be
handed to a second driver after the first programmed the device. That is a lifetime decision about a
kernel object rather than a bookkeeping change. Worth flagging as a real piece of work now: two
bumps in one day is a different signal from one every few milestones.

## The disk tree: a whole disk as one file, with no cache

`crates/disk_tree` is the pure half of `components/src/disk_tree.rs`, built for milestone 834 (ioping
on nife and Linux). It serves a namespace of two names, a directory `nvme0` and a file `n1` in it,
whose bytes are the disk's from block 0.

It exists so that a program run at the prompt can measure the device rather than a file server's
memory. `redoxfs_server` keeps whole small files and recently read blocks in memory. Linux answers
the same need with `/dev/nvme0n1` opened with `O_DIRECT`. Every read the disk tree answers becomes
exactly one device read, and its tests hold that.

It is read-only twice over. No mutating verb is answered (each gets `EROFS`), and the server's block
endpoint is badged with a window the NVMe server never writes from
(`non_volatile_memory_express::can_write`).

The shell mounts it as its second tree (calef, 2026-10-18 21:36 UTC, "Yes, read-only at prompt"),
so a measurement names the file inside the mounted directory:

```text
cd /disk/nvme0
ioping -D -c 1000 -i 0 -s 4k -S 64m -o 1m n1
```

One file is the whole namespace, whatever partitions are on the disk. A partition is a byte range of
`n1`, given to ioping with `-o` and `-S`. Naming partitions would need the table read in the server.
A read longer than the server's transfer window comes back short, at most `blk::TRANSFER_BLOCKS`
blocks from the first one touched. POSIX allows that, and every client in this tree loops on a short
read.

## See also

- [The GUID Partition Table](globally-unique-identifier-partition-table.md) for the format, the fixtures and the proofs.
- [The RedoxFS filesystem server](fs-server.md) for the block server and the blk protocol.
- [Reading the backup from a MacBook or a Linux host](host-recovery.md) for the other half of "the
  board is dead, can I get my data".
- [DMA](dma.md) for what "the block server owns the transport" is actually buying.

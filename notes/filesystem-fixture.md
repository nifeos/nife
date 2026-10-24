# The filesystem test fixture

*Name: provisional, minted 2026-10-24 (UTC) by comments/929-crates-16, for the crate it describes.*

`crates/filesystem_fixture` holds the names and contents of the files the test image ships, the
witnesses' verdict words, and the shapes of the `walk` and `throughput` benchmarks. Three programs
read it and must agree: the host build that writes the RedoxFS image, the client that reads and
writes through the FS server, and the kernel test that asserts the client's report. This page
holds the reasons behind each shape, which the crate's doc comments link.

## Where the line sits

Milestone 886 (the filesystem test fixture moves to its own crate) ruled where the line between
this crate and `filesystem_contract` sits. The protocol words shipping programs send each other
(`READY`, `DESCENT_REFUSED`, and `rm`'s status convention) stay in `filesystem_contract`.
Everything here is test data or a test hook, and nothing that boots depends on this crate. It is
`no_std` and depends on `filesystem_contract` alone.

## The shared-frame witness

Milestone 599 (a frame per filesystem client channel) is the defect it witnesses. Two files sit at
the image root with names of the same length and distinct contents. A substitution then shows up
as the wrong body rather than as a length mismatch.

The witness has the victim intend `fs-witness-victim` and the attacker overwrite the staged name
with `fs-witness-usurpr` before the victim calls. The name lengths must be equal because the
length a client sends travels in a register, which the caller chooses. The name travels in the
shared page, which any holder of the frame can rewrite. The victim sends its own name's length,
so the attacker's name must fill exactly that many bytes for the server to resolve it.

## The durability witness

The witness asks `filesystem_contract::fs::SYNC` twice and reads the count each answer carries.
Two independent things are checked, which is why one verdict is not enough:

1. The count was already non-zero when the witness first asked. Nothing in this process had
   synced yet, so only another process answering a flush can have moved it. That is the
   end-to-end leg, and it cannot be faked from inside the guest.
2. The count strictly increased between the two calls. That separates a real device round trip
   from a server that answers a constant.

The verdict classes are deliberately far apart. "This machine's device cannot flush" is a hardware
fact and "the flush never left the server" is a wiring bug, and a run that confused them would send
somebody chasing the wrong half of the stack.

Nothing produces these verdicts. The only role that ran the witness was `fs_test_client`'s SMB
verify role, which went with the SMB implementation (notes/smb.md). Its end-to-end leg needed a
network client to cause the first flush. `SYNC` and the block server's `VIRTIO_BLK_T_FLUSH` are
still wired and still correct, and nothing gates them. The vocabulary is kept because the
classification is the useful half. Re-homing the witness onto a role that needs no network is a
small piece of work with no design question in it.

## The throughput shape

The transfer unit is `filesystem_contract::fs::TRANSFER_MAX`, so the phase measures the protocol's
own ceiling. A comparison against a system whose client may pass a differently sized buffer
compares two different things, however large the unit is ([benchmarks.md](benchmarks.md)).

A phase moves 1 MiB whatever the transfer size. The bytes are pinned rather than the transfer
count: with the count pinned, a 64 KiB transfer would move sixteen times the data a 4 KiB one does.
The size is bounded by the 16 MiB fixture image, not chosen for statistics.

### The record read

`RECORD_READ` reads the first page of a record. The prediction was that this would be much cheaper
than a read in the middle: `read_node_inner` asks for the record at
`BlockLevel::for_bytes(offset_within_record + len)`, which is level 0 at the start and level 5 at
the end. It measures the same as the other two read phases, to within 3%.

The reason is one level down. `read_record` reads the block the pointer stores, and only then
checks whether that is as large as the level asked for. A fully written record is stored at its
node's record level, so a read of an ordinary file fetches the whole record, whatever offset it
asked for. That is what made the arithmetic in [benchmarks.md](benchmarks.md) close. At record
level 1 a 4 KiB read is two block reads.

`RECORD` is 128 KiB because each RedoxFS record size from level 0 to level 5 divides it. A value
that tracked the record size would have made figures from different record levels incomparable.

### The payload fill

RedoxFS compresses a record with lz4 and skips a write whose record is byte-identical to what is
already there. A benchmark that sends the same page twice measures that short circuit, and one that
sends 32 identical pages into one 128 KiB record measures lz4. The payload is therefore fresh and
incompressible per write, which costs 512 stores. `PAYLOAD_FILL` reports that cost so it can be
subtracted from the write phases.

## The directory trees

Each tree is a sibling of the others in the image root, so a capability to one is provably not a
capability to another. `tree::OTHER` is a real directory that the process one hop up the chain
really can open, for the same reason the per-file attacker's neighbor is a real file: "it cannot
reach a sibling" must be a claim about something that exists. `deeper` gives a second descent
somewhere to go, so `filesystem_contract::dir::DESCEND` has something to withhold.

The walk tree that milestone 121 (`ripgrep` on nife: enumeration as a capability) prices has one
region per cost:

```text
  /walk/chain/f  chain/c/f  chain/c/c/f ... chain/c^8/f   one component per level
  /walk/wide/n000 .. n127                                 one entry per name
  /walk/narrow/n000                                       the one-entry control
  /walk/sizes/s4k  s64k  s256k                            one byte per byte
```

`walk_pricing` stages it on a host and walks it on both sides, so the image builder, the priced
program and a host comparison all read one shape.

## The blank disk

The blank disk carries three partitions:

```text
  LBA      2048..10239    an EFI system partition (nothing formats it; it is here so the
                          table has more than one entry and more than one unique GUID)
  LBA     10240..30719    a Linux filesystem partition, likewise
  LBA     30720..129023   the nife data partition: 48 MiB, 4096-aligned at both ends,
                          and the one `mkfs` formats
```

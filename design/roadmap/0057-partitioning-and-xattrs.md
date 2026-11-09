---
status: BUILT
raised: 2026-07-31
built: 2026-08-03
---
# 57. Partitioning and formatting a real drive, and extended attributes

In brief. calef's router setup is `parted` then `mkfs.ext4` then three mounted partitions. Plus the
xattr gap milestone 55 (Time Machine) surfaced. Nearly all of this is testable in QEMU against
virtio-blk with no board, so it is schedulable before 2026-08-21 rather than waiting on hardware.

Status, taken from the tree on 2026-08-03 rather than from this entry. The reading half is done
end to end. `crates/globally_unique_identifier_partition_table` parses and writes tables, and `disk_surveyor` reads a real one off a virtio-blk
device on both ISAs. `crates/block_roster_contract` answers "what drives are attached" as a read-only mapping.
The extended-attribute layer and its host recovery half are built, and `tools/redoxfs_host` extracts.
Writing on the target was gated on randomness, and both halves were built in commit `4db2fd74`,
each holding an entropy endpoint (see the Follow-on). See the corrected table below, which used to
say "the tools, none of which exist".

## Extended attributes: decided and built

calef decided 2026-07-30: extended attributes, not AppleDouble sidecars, on the grounds that we
will want them anyway. RedoxFS has none, so the fork was between extending its on-disk format and
layering attributes in the FS server. It closed on 2026-07-31 in favor of the layer, by an
amendment to §34 (RedoxFS is the primary filesystem, on three conditions). A small check had shown
RedoxFS groups a file write and an attribute write into one transaction. The crash sweep of
milestone 37 (prove RedoxFS's crash consistency) then measured that rather than inheriting it.

Built 2026-07-31 on both ISAs (notes/xattr.md): four verbs in `fs_proto`, and a store the FS
server keeps in a reserved directory of the image. It holds one blob per node, keyed on the
`TreePtr` id. Built
2026-08-01: the recovery side, with `redoxfs_host extract` putting attributes back on the extracted
files. Milestone 61 (the caretakers) closed the caretakers' gap on the four verbs.

The full account is in
[extended-attributes.md](0057-partitioning-and-xattrs/extended-attributes.md). It holds the
direction, the two ways it could have gone and the check that decided it. It also holds the layer's
limits, the three ways to get it wrong, and what the recovery build found.


## The tools

This table was written 2026-07-30 saying "none of which exist" and was wrong within a day. It
is corrected here on 2026-08-03 from the tree rather than from the plan, and the correction is
itself the finding: three of its four rows had landed and the entry still read as though nothing
had. Take a status from the merged tree.

| Need | Status | Note |
|---|---|---|
| GPT parsing | Built 2026-07-30 | `crates/globally_unique_identifier_partition_table`, proved against tables `sgdisk` and macOS `diskutil` wrote. Mandatory even if we never write one: you cannot find a partition on a real disk without reading the table |
| GPT writing | Built 2026-07-30 | `Gpt::create`, `write_primary_header`, `write_backup_header`, `mbr::write`. Re-emitting `sgdisk`'s table reproduces its bytes exactly. What it will not do is invent a unique GUID, which is the row below |
| Reading a table on the target | Built 2026-08-03 | `disk_surveyor` over the block service, both ISAs, against an image built from the `sgdisk` fixture. notes/block-devices.md |
| Block device enumeration | Built 2026-08-03 | `crates/block_roster_contract`: a read-only page the kernel writes, listing what is attached and deliberately not how big it is. Listing and holding are different authorities, and the negative control writes to the roster and dies |
| Partitioning on the target | Built in commit `4db2fd74` | `components/src/disk_partitioner.rs` draws a unique GUID per partition from the entropy service and lays out the table with `Gpt::create`. No pin divergence |
| `mkfs` on the target | Built in commit `4db2fd74` | `redoxfs_server/src/bin/mkfs.rs`, with `Header::new_with_uuid` taken as divergence 4 (`vendor/README.md`). Never blocked on `std`; see the finding below |

The write half was the same wall twice: an identifier that must be unique needs randomness, and
neither `crates/globally_unique_identifier_partition_table` nor a `no_std` RedoxFS has any. The
difference between the two was that partitioning needed only plumbing (an entropy endpoint into the
program that does it) while `mkfs` also needed a change inside `vendor/redoxfs`, which is a
decision. Both were built in commit `4db2fd74`.

## Finding 2026-08-01: `mkfs` on the target is blocked on entropy, not on `std`

Investigated and measured, because "the FS server is `no_std` and the creation APIs are std-gated"
reads like a dead end and is not the real constraint.

`FileSystem::create` and `create_reserved` carry `#[cfg(feature = "std")]`, and so do the imports
they need and `Header::new`. Un-gating them is mechanical for all but one call:
`Header::new` stamps a fresh v4 UUID into the header with `uuid::Uuid::new_v4()`, which is
`getrandom`, which is the std path. The encryption branch wants randomness too (`Salt::new`,
`Key::new`), and that one does not matter here because this volume is deliberately unencrypted.

So the blocker is that a filesystem needs a unique identifier and the engine has no source of
randomness in a `no_std` build. nife does: the entropy service of milestone 56 (secrets,
credentials, and the entropy to make them safe). The shape of the fix is therefore small and
upstreamable, and it is the shape upstream already uses one line away: `create` takes `ctime` and
`ctime_nsec` as *parameters* precisely because a `no_std` engine has no clock. A
`Header::new_with_uuid(size, uuid: [u8; 16])` does for randomness exactly what those parameters do
for time, and the caller (which has an entropy capability) supplies it.

The same problem appears twice in this milestone, and has the same answer both times.
notes/globally-unique-identifier-partition-table.md already records that `crates/globally_unique_identifier_partition_table` will not invent a partition GUID, for the identical
reason: "a GUID that is not random is not unique, this crate has no randomness, and inventing one
from a counter would be worse than refusing." Partitioning and formatting on the target are both
gated on plumbing the entropy service to the program that does them, and neither is gated on `std`.

This is a decision for calef, because the fix is a divergence from the pin. `patches/README.md` records the
patch and how to submit it, which is the mitigation, and §46's rule is that taking one is a decision rather than a convenience. It is
also worth weighing against the pragmatic alternative. `redoxfs_host` on a Mac can partition and
format the drive today, which is what actually gets a disk ready for the board on 2026-08-21. The
target-side version is then a capability demonstration rather than a prerequisite.

GPT is a good crate to write. Pure computation, well specified, so it is host-tested with tests in
milliseconds, and it has real Kani targets: CRC round-trip, primary and backup headers agreeing,
entry-array bounds, and refusing a table whose entries overlap.

Built 2026-07-30: `crates/globally_unique_identifier_partition_table`, the parsing and writing halves both. Parse, validate (four CRC-32s,
the geometry, overlapping partitions, the protective MBR, the backup against the primary) and create,
with no I/O at all: the caller supplies blocks and receives blocks, so the whole thing is host-tested.
Seven Kani harnesses in `script/verify`. The claim that makes it credible is that it is tested against
two real tables this project did not write, from `sgdisk` and from macOS `diskutil`, committed as
fixtures. Re-emitting `sgdisk`'s table reproduces its bytes exactly, and so does rebuilding it from
scratch. Two findings landed in notes/globally-unique-identifier-partition-table.md: macOS writes no GPT partition names at all, so
nothing may identify a partition by its label, and the two tools disagree about the protective MBR's
CHS fields, which is why those are not validated. The nife partition type GUID is DECISIONS §45.
That sentence used to end "what remains on this milestone is unchanged: the transaction check for
xattrs, `mkfs` on the target, block-device enumeration, and the host extraction tool", and by
2026-08-03 three of those four were done and the fourth had turned out to be a decision. The list
now lives in the table above, corrected from the tree.

## The capability shape is the demonstration

Partitioning and `mkfs` are destructive and need authority over a *whole block device*. So the
tool holds one device capability and can destroy exactly that device and nothing else. Compare
`parted /dev/sda` as root, where a typo reaches any disk in the machine, and calef's own instructions
carry a "confirm the target device path before proceeding" warning precisely because the tool cannot
enforce it. Here the warning is structural: the tool was handed one disk.

That also makes it a natural place for milestone 47's `enumerate` right to earn itself: listing
attached devices and holding one of them are different authorities.

Built 2026-08-03, and it did not become an `enumerate` *right* (notes/block-devices.md). The
prediction that it would was reasonable and the tree said otherwise: `dir::ENUMERATE` is a bit in a
capability a server checks, and a device listing has no server to check it. So the listing is a
read-only mapping instead, `crates/block_roster_contract`, which is the compositor's window-enumeration
shape (DECISIONS §33) rather than the filesystem's. There is nothing to authorize at read time
because the authorization happened when the mapping was made, and a program that holds no mapping
has nowhere to look rather than a request that gets refused.

Two consequences worth recording. The roster carries no capacity, because a size is a fact about
a device you hold and you get it from `blk::SIZE`, which takes the endpoint. Answering it in the
listing would quietly make the listing the more powerful of the two authorities, and it would mean
bringing a PCIe function up on behalf of a `ls`. And the negative control is what turns this from a
description into a claim: the same binary, given the roster and no disk, writes to the roster's exact
address and dies.

## Reading the drive from a host, and no encryption

Built 2026-07-30, so that "the board is dead, can I get my data?" has an answer: `tools/redoxfs_host`
reads a nife image on macOS or Linux with `ls`, `cat` and `extract`, plus `import`, `mkfs`, `put`
and (2026-08-01) `xattr`, with no FUSE and no root. Upstream's `fuse` feature stays off by our own
choice. The operational rule that follows: keep the recovery tool, or its exact source pin, with
the backup (notes/host-recovery.md).

Decided by calef on 2026-07-30: no filesystem-level encryption on the backup volume. Encryption
belongs at the Time Machine layer, where the Mac encrypts before anything is sent.

The full account is in [host-recovery.md](0057-partitioning-and-xattrs/host-recovery.md). It holds
why upstream did not already solve this and what the build found that the plan did not predict. It
also compares the three recovery paths, answers the same-engine objection, and gives the reasoning
and consequences of the encryption decision.


## Sequencing, and what was left

Rewritten 2026-08-03 because everything it sequenced has happened. The GPT crate (2026-07-30), the
transaction check (2026-07-31, answered yes), and the host extraction tool (2026-07-30, the cheapest
credibility win here) are done. So are the extended-attribute layer and its recovery half
(2026-07-31 and 2026-08-01), and the block-device path with a real table read on the target
(2026-08-03). Real drives arrive with milestone 53 (the board's own peripherals); the board arrives around
2026-08-21.

The write half followed in commit `4db2fd74`. Partitioning needed only an entropy endpoint in the
program that does it. `mkfs` also needed `Header::new_with_uuid` inside `vendor/redoxfs`, taken as
divergence 4 (`vendor/README.md`) and carried for upstream as
`patches/redoxfs-no-std-create-uuid.patch`.

Effort: not estimated. The GPT crate turned out to be about one lane on the history-calibrated
scale, and so did the block-device lane.

## Follow-on

- **Milestone 61.** The caretakers (`fs_file_caretaker`, `fs_subtree_caretaker`,
  `fs_nameset_caretaker`) answered `EOPNOTSUPP` to all four xattr verbs instead of forwarding, so a
  program behind a per-file grant could not reach its own file's attributes. 61 closed it and found
  the general defect underneath: nothing made a caretaker and the contract agree.
- **Milestone 110.** `tools/redoxfs_host` reads a whole-device image and a real drive has a
  partition table at offset zero. `crates/globally_unique_identifier_partition_table` and the recovery verbs both existed here and nothing
  joined them; 110 is that join, and the index calls it this milestone's residual.
- **Refused.** Extending the RedoxFS on-disk format for extended attributes, in favour of layering
  them in the FS server. Normally the layer is dismissible because on Linux anything can open the
  file directly and bypass it; here nothing can, since all access goes through `fs_proto`. The
  format extension would also be a materially larger divergence from the 0.9.1 pin that every future
  bump pays for.
  This refusal is milestone 485 (design/roadmap/0485-xattrs-in-the-redoxfs-on-disk-format.md), which
  carries it with the condition that would change it.
- **Refused.** Filesystem-level encryption on the backup volume. calef, 2026-07-30: "If I'm
  struggling to get the data off, I'm not all that worried about somebody else getting it."
  Encryption belongs at the Time Machine layer, where the Mac encrypts before anything is sent, so
  the server never holds plaintext and the recovery tool needs no key handling at all.
- **Refused.** Re-enabling upstream's `fuse` feature for mounting. On Linux it is a feature flag and
  nearly free, but the recovery story is `ls`/`cat`/`extract`/`xattr` in `tools/redoxfs_host`, which
  is identical on macOS and Linux and needs no root. macOS mounting additionally wants macFUSE, a
  third-party system extension plus reduced security mode on Apple Silicon, which makes it an
  optional convenience rather than the answer to "can I get my data".
- **Refused.** An indexed, typed attribute store in the BFS shape. Only opaque blobs are needed
  today, and every attribute already carries a `u32` type code the layer stores, returns and never
  interprets. So an indexed store later is a change of implementation rather than a format migration
  plus a wire break.
  This refusal is milestone 486 (design/roadmap/0486-an-indexed-typed-attribute-store.md), which
  carries it with the condition that would change it.
- **Recorded.** `notes/host-recovery.md` says an attribute's type code cannot survive extraction,
  because no host filesystem has a per-attribute type word. Each dropped code is named and counted
  and the raw `.nife-attrs` store still comes out beside the tree as its only home, which is also
  why a recovered tree carries one directory the user did not put there.
- **Recorded.** `notes/host-recovery.md` draws the consequence for pin bumps: a reader must match
  the on-disk format version, so a bump to a different format strands every image already written
  and is a migration rather than an upgrade. The operational rule that follows is to keep the
  recovery tool, or its exact source pin, with the backup.
- **Recorded.** `notes/globally-unique-identifier-partition-table.md` records that `crates/globally_unique_identifier_partition_table` will not invent a partition GUID, because a
  GUID that is not random is not unique, the crate has no randomness, and inventing one from a
  counter would be worse than refusing. The same note carries the two fixture findings: macOS writes
  no GPT partition names, and the two tools disagree about the protective MBR's CHS fields.
- **Done.** Partitioning on the target is built: `components/src/disk_partitioner.rs`, landed in commit
  `4db2fd74` ("disk: partition and format a disk on the target, with the two capabilities that takes",
  2026-08-03). It holds exactly a block-service endpoint for one disk and an entropy endpoint. It
  draws four version-4 GUIDs, lays out three partitions through `Gpt::create` and writes both copies
  of the table. Its `ROLE_VERIFY` role reads the result back from a process holding no entropy at
  all. The negative control (the same binary with no entropy endpoint) writes nothing. The bullet above
  was written before that landed.
- **Done.** `mkfs` on the target is built: `redoxfs_server/src/bin/mkfs.rs`, in the same commit
  `4db2fd74`. The divergence it needed was taken and documented rather than left implicit:
  `Header::new_with_uuid` and `FileSystem::create_reserved_with_uuid` are in `vendor/redoxfs`,
  recorded as divergence 4 in `vendor/README.md`, and carried for upstream submission as
  `patches/redoxfs-no-std-create-uuid.patch`. The engine takes the uuid as an argument the same way
  `create` already takes `ctime`, so no randomness enters vendored code. The bullet above was
  written before that landed.

## Index row

you cannot find a partition without reading the table, and all of it is testable in QEMU before
the board lands. Built: the host recovery tool (`ls`/`cat`/`extract`/`xattr`), `crates/globally_unique_identifier_partition_table`, the **extended-attribute layer**, and (2026-08-03) **reading a real table on the target** plus **block-device enumeration**, which is a read-only roster page. The **write**
half followed in commit `4db2fd74`: partitioning and on-target `mkfs`, each drawing its
identifiers from an entropy endpoint, with `mkfs` taking a fourth divergence from the RedoxFS pin

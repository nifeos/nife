# Prior art for the block_roster_contract crate

An appendix to [the milestone](../1059-block-roster-contract-outside-review.md). The reviewer
recalled these from general knowledge unless a row says read, and read the crates.io figures on
2026-11-09.

## How other systems list block devices

| system | how it answers "which block devices exist" | what this crate takes or refuses |
|---|---|---|
| NVMe, the Identify Active Namespace ID list | a 4 KiB page of up to 1024 `u32` namespace ids in ascending order, zero-filled after the last | the closest shape, a fixed page of fixed-width ids. NVMe ends the list with a zero, so an unwritten page reads as an empty list; the roster carries a magic and a count, so a zeroed frame is refused |
| virtio-blk | no list; each device announces itself on its transport (a PCI function, an MMIO node), and its config space carries capacity and block size | the roster is the list virtio lacks, and carries no capacity on purpose: a program asks the device, through the endpoint, for its size |
| Linux `gendisk`, `/sys/block`, `lsblk` | an ambient, mutable, hot-plugged namespace with persistent names (`nvme0n1`), major and minor numbers, size, and events on change | the roster is a read-only snapshot granted to a program, with no names, no persistent identity and no hot plug. A user coming from Linux will miss an identity (a serial or a GUID) most; P2-6 says so |
| UEFI Block I/O protocol | one handle per device, found with `LocateHandleBuffer`; the handle is also the access path | the roster separates listing from access by design: holding an entry grants nothing |

## Rust crates nearby

| crate | what it gives | how it relates |
|---|---|---|
| `embedded-storage` 0.3.2 (9.9 M downloads), `embedded-sdmmc` 0.10.0, `block_device` 0.1.3 (read) | device access traits (`ReadStorage`, `BlockDevice::read`); none enumerates devices | the roster is not an access trait, and its README should say so, so a searcher for one moves on. A roster entry plus one of these traits is the pairing a driver author would build |
| `virtio-drivers` 0.13.0 (read) | guest drivers for virtio devices; enumeration is the caller's | a consumer could map an entry with `TRANSPORT_VIRTIO_PCI` to the PCI transport of that crate; no glue exists and none is proposed |

## Error types for a refused parse

P1-2 follows the common shape of `core` and the ecosystem. `core::str::from_utf8` returns a
`Utf8Error` that implements `Display` and `core::error::Error`. Parsers such as `httparse` return
one small error enum, a variant per refusal, rather than `Option`. `ReadError` takes the same
shape, with one variant per refusal, which is what lets the reader of the page say "the magic is
wrong" in place of "no roster".

Unique to this crate: Kani proves both the reader and the writer of the page total. The magic
carries the version, so an old reader refuses a new page. And the page has the capability shape: a
program holds a listing someone granted it, never a lever.

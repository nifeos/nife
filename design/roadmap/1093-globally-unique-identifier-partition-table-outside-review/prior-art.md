# Prior art

An appendix to [the milestone](../1093-globally-unique-identifier-partition-table-outside-review.md).

How the crate sits against what a crates.io user and a kernel reader already know. Recent downloads
are the last 90 days at the time of the review.

| where | what it does | what a reader coming from it will miss here |
|---|---|---|
| `gpt` 4.1 (601k recent / 2.36M all time) | `std`, `GptDisk` over `Read + Seek`, reads and writes both tables, uses `crc` and `uuid` | opening a disk whose primary header is bad from its backup; an `std::io` convenience layer; `std::error::Error` on its errors |
| `gpt_disk_types` 0.16 (19.8k / 124k) and `gpt_disk_io` 0.17 (11.9k / 108k), google/gpt-disk-rs | `no_std`, no allocation; typed `Lba`, `LbaLe`, `BlockSize`, `U64Le`; `GptHeader` with `calculate_header_crc32` and `update_header_crc32`; `MasterBootRecord`; a `BlockIo` trait with `read_primary_gpt_header` and `read_secondary_gpt_header`; re-exports `crc`, `ucs2` and `uguid` | a typed block address, so no caller can pass a byte offset as one; reading the secondary header on its own; re-exports of the dependency types, so a caller adds no second dependency |
| `uguid` 2.2 (705k / 2.40M), google | a `no_std` GUID stored in the on-disk mixed-endian order, so reading sixteen bytes is a copy; used by `gpt_disk_types` and the `uefi` crate | nothing: this crate's `uuid` stays in RFC 9562 order and proves the two swaps instead (decision 2 below) |
| `uuid` 1.29 (253M / 978M) | the identifier type this crate exposes, with default features off | nothing to miss; the figure is why keeping it is the recommendation |
| `mbrman` 0.7 (118k / 582k) | reading and managing MBR tables | hybrid MBRs, which this crate refuses by design (the package's baseline) |
| Linux `block/partitions/efi.c` | `is_gpt_valid` checks the signature, the header size against the block, the header CRC, `my_lba` against the block that held it, `first_usable_lba` and `last_usable_lba` against the device's last block, and refuses any `sizeof_partition_entry` but 128; `compare_gpts` warns on every field the two headers must share; `find_valid_gpt` uses the alternate header when the primary is bad (with the `gpt` boot option) and warns "Alternate GPT header not at the end of the disk" | a check against the device's size (item 4); a way in through the backup (item 3); the crate is more lenient on entry size (any multiple of 8 from 128), which the specification allows |
| FreeBSD `sys/geom/part/g_part_gpt.c` | each of the four elements (two headers, two arrays) carries a state, `MISSING`, `CORRUPT`, `INVALID`, `UNSUPPORTED` or `OK`; a header is `INVALID` when `hdr_lba_alt > last`, when `hdr_lba_table` lands inside the usable range or when `hdr_entsz < 128` or is not a multiple of 8; the scheme reads whichever header is `OK` | the per-element state, which is what lets a tool say "the backup is fine, the primary is not" (item 3) |
| `gdisk` (gptfdisk) | the tool that wrote one fixture; on a bad primary it offers the backup; writes the backup array on the blocks before the backup header | the offer (item 3) |

Where this crate differs: it does no I/O at all, so it compiles for the host and runs its tests
against the two tools' bytes. It is `no_std` with no allocation and no `unsafe`. It takes the block
size from the slice the caller hands in, so a 4K-native disk needs no parameter. It has eight
machine-checked proofs, each with a patch that turns it red. It has two real fixtures and three
exhaustive single-byte sweeps (one ignored for time). It refuses a hybrid MBR on purpose. What it
costs: a caller must read the right blocks itself, and today nothing tells it where (item 5) or
whether the header's idea of the disk matches the device (item 4). The crate cannot open a disk whose primary table has
damage at all (item 3). The concrete example: `uefi_loader/src/chooser.rs` reads
blocks 0 to 33, hands `head[1024..]` to `parse`, and holds `media.last_block` without comparing it
with `table.block_count()`. Linux's `is_gpt_valid` would have compared.

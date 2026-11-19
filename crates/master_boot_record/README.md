# master_boot_record

The master boot record (MBR): sector 0 of a disk, four primary partition entries and a two-byte
signature. This crate parses it and names the sectors that belong to no partition. It is `no_std`,
allocates nothing, has no `unsafe` and no dependency.

## The contract

- `MasterBootRecord::parse(sector)` reads a 512-byte sector 0, or returns `None` without the
  `0x55 0xaa` signature. Each entry is its type byte and two little-endian words (first sector and
  length), read from that entry's own offsets.
- `PartitionEntry::is_used` and `contains`: an entry is used when its type and length are both
  non-zero, and it contains a sector only inside its own extent, computed in 64 bits.
- `MasterBootRecord::scratch_sector_range(want)`: up to `want` sectors ending just before the
  first used partition and starting no lower than sector 1, as `(first, count)`. It is `None` for a
  protective MBR (a `GPT` disk keeps its header in that gap), a table with no partitions, a first
  partition at sector 0 or 1, or a `want` of 0. A caller that gets `None` must refuse to write.

## EXAMPLES

```rust
use master_boot_record::MasterBootRecord;

let mut sector = [0u8; 512];
sector[446 + 4] = 0x0b; // a FAT32 partition
sector[446 + 8..446 + 12].copy_from_slice(&2048u32.to_le_bytes());
sector[446 + 12..446 + 16].copy_from_slice(&61_000_000u32.to_le_bytes());
sector[510] = 0x55;
sector[511] = 0xaa;
let table = MasterBootRecord::parse(&sector).unwrap();
// Eight sectors ending just before the partition.
assert_eq!(table.scratch_sector_range(8), Some((2040, 8)));
```

## What is proven

Two Kani harnesses (`src/verification.rs`), each with a falsification patch that turns it red. No
partition table makes the scratch range touch a used partition or sector 0, and the parse accepts
exactly the sectors that end in the signature and reads each entry from its own bytes.

## License

MIT OR Apache-2.0, at your option.

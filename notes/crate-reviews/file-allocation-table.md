# Review package: `file_allocation_table`

*Name: provisional, minted by lane milestone/813-review-package. The file is the
`file_allocation_table` package of [crate-reviews.md](../crate-reviews.md), kept in its own file
because the signature list alone is longer than the other packages together.*

This is what the architect reads to clear `crates/file_allocation_table` for its move to its own
repository and its first publication ([journey 5](../../design/journeys/05-crates-released-on-their-own.md),
step 2; [milestone 813 (a proven FAT crate, released on its own)](../../design/roadmap/0813-a-proven-fat-crate-released-on-its-own.md)).
It covers the crate as pull request #3233 leaves it, the change that removes every reference to
nife from the crate. The open questions are at the end, each with its background first and a
recommendation.

## In one paragraph

FAT12, FAT16 and FAT32 as pure computation, `no_std` with no allocator, no `unsafe` and no run-time
dependency. It reads (boot sector, table, directories, long names, files, the MBR), writes (create,
write, extend, truncate, rename, remove, directories, attributes, the label, the clean-shutdown bit)
and formats (any size, and one fixed EFI-system-partition shape). 31 Kani harnesses prove the
parsers and the table, allocation and write invariants, each with a falsification patch; 300 tests
pass, most of them against the rust-fatfs oracle and `fsck.fat`. The README alone took a scratch
copy of the crate to a green `cargo test` on Rust 1.88 and on stable, and to `cargo kani` with
stock Kani 0.68.0. The one gate it fails is a provisional name. The decisions it needs are about
how much API to publish, not about whether the code is right.

## Identity

| item | value |
|---|---|
| crate | `file_allocation_table`, ratified on #1806 (FAT spelled out). crates.io answers 404 for it, so it is free. |
| version | `0.1.0` in the manifest |
| tag to cut | `v0.1.0`, on the commit that creates the repository, the way `nifeos/jh7110_entropy` was tagged |
| repository | `nifeos/file_allocation_table`, the crate's own name, as `nifeos/jh7110_entropy` is. GitHub answers 404 for it. |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0`, with `LICENSE-MIT` and `LICENSE-APACHE` at the crate root |
| `publish` | `false`; `crates/publish-allowlist.txt` lists no crate |
| size | 7,679 lines of library source, 1,516 of Kani harnesses, 4,982 of unit tests, 3,771 of integration tests |
| package | `cargo package` packs 119 files, 734 KiB (186 KiB compressed): the source, 31 falsification patches, the tests and their fixtures |

## The public API

Generated from rustdoc JSON by `script/crate-review-packet file_allocation_table` (`cargo public-api`
is not installed here); generic parameters and trait items are filled in from the source. Every item
is re-exported at the crate root except the three `efi_system_partition` constants. Unless the list
says otherwise, every type derives `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq` and `Hash`.
`FormatOptions`, `FormatPlan`, `VolumeLayout`, `LayoutError` and `NamedEntry` derive neither `Hash`
nor more; the device-holding types (`Volume`, `WritableVolume`, `Directory`, `NamedEntries`,
`Chain`, `FileReader`, `FileWriter`) derive `Debug` only. No error type implements `Display` or
`core::error::Error`, and no enum is `#[non_exhaustive]`.

### The caller's side

```rust
pub trait SectorDevice {
    type Error;
    fn sector_size(&self) -> SectorSize;
    fn read_sector(&mut self, index: u64, buffer: &mut [u8]) -> Result<(), Self::Error>;
}
pub trait WritableSectorDevice: SectorDevice {
    fn write_sector(&mut self, index: u64, buffer: &[u8]) -> Result<(), Self::Error>;
}
impl<D: SectorDevice + ?Sized> SectorDevice for &mut D;
impl<D: WritableSectorDevice + ?Sized> WritableSectorDevice for &mut D;

pub enum SectorSize { Bytes512, Bytes1024, Bytes2048, Bytes4096 }
impl SectorSize {
    pub const MAX: usize;                                   // 4096
    pub const fn from_bytes(bytes: u16) -> Option<SectorSize>;
    pub const fn bytes(self) -> usize;
    pub const fn shift(self) -> u32;
}

pub trait Clock { fn now(&mut self) -> Option<CivilDateTime>; }
impl<C: Clock + ?Sized> Clock for &mut C;
pub struct NoClock;                                         // also Default; now() is None
pub struct CivilDateTime { pub year: u16, pub month: u8, pub day: u8,
                           pub hour: u8, pub minute: u8, pub second: u8 }
```

### Reading

```rust
pub struct Volume<D>;
impl<D: SectorDevice> Volume<D> {
    pub fn open(device: D) -> Result<Volume<D>, ReadError<D::Error>>;
    pub const fn boot_sector(&self) -> &BootSector;
    pub fn into_device(self) -> D;
    pub fn table_entry(&mut self, cluster: u32) -> Result<TableEntry, ChainError<D::Error>>;
    pub fn chain(&mut self, first: u32) -> Chain<'_, D>;
    pub fn root_directory(&mut self) -> Directory<'_, D>;
    pub fn directory(&mut self, first: u32) -> Directory<'_, D>;
    pub fn root_named_entries(&mut self) -> NamedEntries<'_, D>;
    pub fn named_entries(&mut self, first: u32) -> NamedEntries<'_, D>;
    pub fn find_in_directory(&mut self, directory: u32, name: &str)
        -> Result<Option<NamedEntry>, ChainError<D::Error>>;
    pub fn lookup(&mut self, path: &str) -> Result<NamedEntry, LookupError<D::Error>>;
    pub fn open_file_reader(&mut self, entry: &NamedEntry)
        -> Result<FileReader<'_, D>, FileOpenError>;
    pub fn volume_label(&mut self) -> Result<Option<VolumeLabel>, ChainError<D::Error>>;
    pub fn is_clean(&mut self) -> Result<Option<bool>, ChainError<D::Error>>;
    pub fn count_free_clusters(&mut self) -> Result<u32, ChainError<D::Error>>;
    pub fn file_system_information(&mut self)
        -> Result<Option<FileSystemInformation>, ChainError<D::Error>>;
}
pub struct Chain<'v, D>;           // Iterator<Item = Result<u32, ChainError<D::Error>>>
pub struct Directory<'v, D>;       // Iterator over raw slots
pub struct NamedEntries<'v, D>;    // Iterator over entries with their long names

pub struct BootSector;
impl BootSector {
    pub fn parse(sector: &[u8]) -> Result<BootSector, ParseError>;
    pub fn read<D: SectorDevice>(device: &mut D) -> Result<BootSector, ReadError<D::Error>>;
    pub fn sector_size(&self) -> SectorSize;
    pub fn sectors_per_cluster(&self) -> u32;
    pub fn bytes_per_cluster(&self) -> u32;
    pub fn file_allocation_table_type(&self) -> FileAllocationTableType;
    pub fn clusters(&self) -> u32;
    pub fn total_sectors(&self) -> u32;
    pub fn reserved_sectors(&self) -> u16;
    pub fn table_count(&self) -> u8;
    pub fn table_sectors(&self) -> u32;
    pub fn table_first_sector(&self, index: u8) -> Option<u32>;
    pub fn active_table(&self) -> Option<u8>;
    pub fn first_data_sector(&self) -> u32;
    pub fn root_directory(&self) -> RootDirectory;
    pub fn media(&self) -> u8;
    pub fn hidden_sectors(&self) -> u32;
    pub fn file_system_information_sector(&self) -> Option<u16>;
    pub fn backup_boot_sector(&self) -> Option<u16>;
    pub fn volume_id(&self) -> Option<u32>;
    pub fn cluster_first_sector(&self, cluster: u32) -> Option<u32>;
}
pub enum FileAllocationTableType { Fat12, Fat16, Fat32 }
impl FileAllocationTableType {
    pub fn from_clusters(clusters: u32) -> FileAllocationTableType;
    pub fn entry_bits(self) -> u32;
}
pub enum RootDirectory { Region { first_sector: u32, sectors: u32, entries: u16 }, Cluster(u32) }
pub enum TableEntry { Free, Reserved, Next(u32), Bad, EndOfChain }
impl TableEntry {
    pub fn decode(file_allocation_table_type: FileAllocationTableType, raw: u32) -> TableEntry;
    pub fn encode(self, file_allocation_table_type: FileAllocationTableType) -> u32;
}

pub const SLOT_BYTES: usize;                                // 32
pub enum Slot { End, Free, Used(DirectoryEntry) }
impl Slot { pub fn parse(slot: &[u8; 32], file_allocation_table_type: FileAllocationTableType) -> Slot; }
pub enum DirectoryEntry { Short(ShortEntry), LongName(LongNameEntry) }
pub struct ShortEntry;
impl ShortEntry {
    pub fn name(&self) -> [u8; 11];
    pub fn attributes(&self) -> u8;
    pub fn is_directory(&self) -> bool;
    pub fn is_volume_label(&self) -> bool;
    pub fn first_cluster(&self) -> u32;
    pub fn size_bytes(&self) -> u32;
}
pub struct LongNameEntry;
impl LongNameEntry {
    pub fn ordinal(&self) -> u8;
    pub fn is_last(&self) -> bool;
    pub fn checksum(&self) -> u8;
    pub fn code_units(&self) -> [u16; 13];
}

pub struct NamedEntry;                                      // Debug, Clone, Copy, PartialEq, Eq
impl NamedEntry {
    pub const fn short_entry(&self) -> &ShortEntry;
    pub const fn short_name(&self) -> [u8; 11];
    pub const fn long_name(&self) -> Option<&LongFileName>;
    pub const fn long_name_refusal(&self) -> Option<LongFileNameError>;
    pub const fn attributes(&self) -> u8;
    pub fn user_attributes(&self) -> Attributes;
    pub const fn first_cluster(&self) -> u32;
    pub const fn size_bytes(&self) -> u32;
    pub const fn is_directory(&self) -> bool;
    pub const fn is_volume_label(&self) -> bool;
    pub fn is_dot_entry(&self) -> bool;
    pub fn is_called(&self, name: &str) -> bool;
}
pub struct LongFileName;
impl LongFileName {
    pub fn code_units(&self) -> &[u16];
    pub fn characters(&self) -> impl Iterator<Item = Result<char, DecodeUtf16Error>>;
}

pub struct FileReader<'v, D>;
impl<'v, D: SectorDevice> FileReader<'v, D> {
    pub const fn size_bytes(&self) -> u32;
    pub const fn position(&self) -> u32;
    pub fn set_position(&mut self, position: u32);
    pub fn read(&mut self, buffer: &mut [u8]) -> Result<usize, FileReadError<D::Error>>;
}

pub struct FileSystemInformation;
impl FileSystemInformation {
    pub fn parse(sector: &[u8], clusters: u32)
        -> Result<FileSystemInformation, FileSystemInformationError>;
    pub fn free_cluster_count(&self) -> Option<u32>;
    pub fn next_free_cluster(&self) -> Option<u32>;
}
pub struct VolumeLabel([u8; 11]);                           // private field
impl VolumeLabel { pub const fn as_bytes(&self) -> &[u8; 11]; pub fn trimmed(&self) -> &[u8]; }

pub const PARTITION_COUNT: usize;                           // 4
pub struct MasterBootRecord;
impl MasterBootRecord {
    pub fn parse(sector: &[u8]) -> Result<MasterBootRecord, MasterBootRecordError>;
    pub fn read<D: SectorDevice>(device: &mut D)
        -> Result<MasterBootRecord, MasterBootRecordReadError<D::Error>>;
    pub fn partition(&self, index: usize) -> Option<Partition>;
    pub fn partitions(&self) -> impl Iterator<Item = Partition>;
}
pub struct Partition;
impl Partition {
    pub fn is_bootable(&self) -> bool;
    pub fn partition_type(&self) -> u8;
    pub fn first_sector(&self) -> u32;
    pub fn sector_count(&self) -> u32;
    pub fn has_file_allocation_table_type(&self) -> bool;
    pub fn restrict_device<D>(self, device: D) -> PartitionDevice<D>;
}
pub struct PartitionDevice<D>;                              // implements SectorDevice
impl<D> PartitionDevice<D> { pub fn into_device(self) -> D; }
```

### Writing

```rust
pub struct WritableVolume<D, C>;                            // Deref and DerefMut to Volume<D>
impl<D: WritableSectorDevice, C: Clock> WritableVolume<D, C> {
    pub fn open(device: D, clock: C) -> Result<WritableVolume<D, C>, ReadError<D::Error>>;
    pub fn into_device(self) -> D;
    pub fn into_volume(self) -> Volume<D>;
    pub fn create_file(&mut self, directory: u32, name: &str)
        -> Result<NamedEntry, WriteError<D::Error>>;
    pub fn create_directory(&mut self, directory: u32, name: &str)
        -> Result<NamedEntry, WriteError<D::Error>>;
    pub fn rename(&mut self, entry: &NamedEntry, directory: u32, new_name: &str)
        -> Result<NamedEntry, WriteError<D::Error>>;
    pub fn remove_file(&mut self, entry: &NamedEntry) -> Result<(), WriteError<D::Error>>;
    pub fn remove_directory(&mut self, entry: &NamedEntry) -> Result<(), WriteError<D::Error>>;
    pub fn set_attributes(&mut self, entry: &NamedEntry, attributes: Attributes)
        -> Result<NamedEntry, WriteError<D::Error>>;
    pub fn set_volume_label(&mut self, label: &str) -> Result<(), WriteError<D::Error>>;
    pub fn open_file_writer(&mut self, entry: &NamedEntry)
        -> Result<FileWriter<'_, D, C>, FileOpenError>;
    pub fn mark_clean(&mut self) -> Result<(), ChainError<D::Error>>;
    pub fn allocate_cluster(&mut self, extending: Option<u32>)
        -> Result<u32, AllocationError<D::Error>>;
    pub fn deallocate_chain(&mut self, first: u32) -> Result<u32, AllocationError<D::Error>>;
    pub fn truncate_chain(&mut self, first: u32, keep: u32)
        -> Result<u32, AllocationError<D::Error>>;
}
pub struct FileWriter<'v, D, C>;
impl<'v, D: WritableSectorDevice, C: Clock> FileWriter<'v, D, C> {
    pub const fn size_bytes(&self) -> u32;
    pub const fn position(&self) -> u32;
    pub fn set_position(&mut self, position: u32);
    pub fn write(&mut self, data: &[u8]) -> Result<usize, WriteError<D::Error>>;
    pub fn set_length(&mut self, length: u32) -> Result<(), WriteError<D::Error>>;
}
pub struct Attributes(u8);                                  // private field; also Default
impl Attributes {
    pub const EMPTY: Attributes;  pub const READ_ONLY: Attributes;  pub const HIDDEN: Attributes;
    pub const SYSTEM: Attributes; pub const ARCHIVE: Attributes;
    pub const fn union(self, other: Attributes) -> Attributes;
    pub const fn difference(self, other: Attributes) -> Attributes;
    pub const fn contains(self, other: Attributes) -> bool;
    pub const fn bits(self) -> u8;
}
```

### Formatting

```rust
pub struct FormatOptions;                                   // Debug, Clone, Copy, PartialEq, Eq
impl FormatOptions {
    pub fn new(sector_size: SectorSize, total_sectors: u32) -> FormatOptions;
    pub fn with_file_allocation_table_type(self, kind: FileAllocationTableType) -> FormatOptions;
    pub fn with_sectors_per_cluster(self, sectors: u32) -> FormatOptions;
    pub fn with_volume_id(self, volume_id: u32) -> FormatOptions;
    pub fn with_hidden_sectors(self, hidden_sectors: u32) -> FormatOptions;
    pub fn with_label(self, label: &str) -> Result<FormatOptions, FormatError>;
}
pub struct FormatPlan;                                      // Debug, Clone, Copy, PartialEq, Eq
impl FormatPlan {
    pub fn new(options: FormatOptions, clock: &mut impl Clock) -> Result<FormatPlan, FormatError>;
    pub fn file_allocation_table_type(&self) -> FileAllocationTableType;
    pub fn clusters(&self) -> u32;
    pub fn sectors_per_cluster(&self) -> u32;
    pub fn table_sectors(&self) -> u32;
    pub fn metadata_sectors(&self) -> u32;
    pub fn fill_sector(&self, index: u32, out: &mut [u8]) -> bool;
    pub fn write_to<D: WritableSectorDevice>(&self, device: &mut D) -> Result<(), D::Error>;
}

// The EFI system partition writer: one FAT32 shape, 512-byte sectors, 4 KiB clusters.
pub struct VolumeLayout;                                    // Debug, Clone, Copy, PartialEq, Eq
impl VolumeLayout {
    pub fn new(total_sectors: u64, hidden_sectors: u64, file_len: u64, volume_id: u32,
               label: &str, file_name: &str, clock: &mut impl Clock)
        -> Result<VolumeLayout, LayoutError>;
    pub const fn clusters(&self) -> u32;
    pub const fn table_sectors(&self) -> u32;
    pub const fn first_data_sector(&self) -> u64;
    pub const fn cluster_first_sector(&self, cluster: u32) -> u64;
    pub const fn metadata_sectors(&self) -> u64;
    pub const fn file_first_sector(&self) -> u64;
    pub const fn file_sectors(&self) -> u64;
    pub fn fill_sector(&self, index: u64, out: &mut [u8]) -> bool;
}
pub const SECTOR_BYTES: usize;              // 512
pub const CLUSTER_SECTORS: u32;             // 8
pub const CLUSTER_BYTES: usize;             // 4096
pub const RESERVED_SECTORS: u32;            // 32
pub const TABLE_COUNT: u32;                 // 2
pub const ROOT_CLUSTER: u32;                // 2
pub const FAT32_MINIMUM_CLUSTERS: u32;      // 65525
pub mod efi_system_partition {
    pub const EFI_CLUSTER: u32;             // 3
    pub const BOOT_CLUSTER: u32;            // 4
    pub const FILE_CLUSTER: u32;            // 5
}
```

### Errors

| type | variants |
|---|---|
| `ParseError` | `TooShort`, `NoSignature`, `NoJump`, `SectorSize`, `ClusterSize`, `NoReservedSectors`, `NoTables`, `Media`, `NoSectors`, `NoTableSectors`, `MetadataPastEnd`, `NoClusters`, `LayoutContradictsClusterCount`, `TooManyClusters`, `TableTooSmall`, `RootCluster`, `Version`, `ActiveTable` |
| `ReadError<E>` | `Device(E)`, `Refused(ParseError)`, `SectorSizeMismatch { device, volume }` |
| `ChainError<E>` | `Device(E)`, `NotADataCluster(u32)`, `Broken { cluster, entry }`, `TooLong` |
| `LookupError<E>` | `Chain(ChainError<E>)`, `NotFound`, `NotADirectory`, `RootHasNoEntry` |
| `FileOpenError` | a unit struct |
| `FileReadError<E>` | `Chain(ChainError<E>)`, `ShorterThanItsSize`, `Failed` |
| `LongFileNameError` | `BadStart`, `OutOfOrder`, `PiecesDisagree`, `Incomplete`, `ChecksumMismatch`, `BadPadding` |
| `FileSystemInformationError` | `TooShort`, `NoSignature` |
| `MasterBootRecordError` | `TooShort`, `NoSignature`, `BadStatus { index }`, `EmptyPartition { index }`, `PartitionOverflows { index }` |
| `MasterBootRecordReadError<E>` | `Device(E)`, `Parse(MasterBootRecordError)` |
| `PartitionError<E>` | `Device(E)`, `OutsidePartition(u64)` |
| `AllocationError<E>` | `Chain(ChainError<E>)`, `VolumeFull`, `NotTheEndOfAChain(u32)` |
| `WriteError<E>` | `Chain(ChainError<E>)`, `VolumeFull`, `FileTooLarge`, `ShorterThanItsSize`, `EntryChanged`, `Failed`, `DirectoryFull`, `NameRefused`, `LabelRefused`, `AlreadyExists`, `NotAFile`, `NotADirectory`, `ReadOnly`, `TimeOutOfRange`, `DirectoryNotEmpty`, `MovedIntoItself` |
| `FormatError` | `TooSmall`, `NoSuchGeometry`, `LabelRefused`, `TimeOutOfRange`, `ClusterSizeRefused` |
| `LayoutError` | `NotFat32`, `TooLarge`, `NameRefused`, `LabelRefused`, `TimeOutOfRange`, `FileTooBig` |

`From` conversions: `ChainError<E>` into `AllocationError<E>` and `WriteError<E>`, `LookupError<E>`
into `WriteError<E>`, and the two `ChainError` and `LookupError` conversions the packet lists.

### What is surprising in it

- **Fourteen error enums are exhaustive.** Adding any variant is a breaking change, so a new refusal
  costs a minor version in 0.x and a major one after 1.0. #3233 added `LayoutError::LabelRefused`,
  which would have been one. Open question 3.
- **No error implements `Display` or `core::error::Error`.** A caller cannot `?` one into a boxed
  error. Adding both later is not breaking, so this need not hold the release.
- **`VolumeLayout` brings a second vocabulary to the crate root.** `SECTOR_BYTES = 512`,
  `CLUSTER_SECTORS`, `CLUSTER_BYTES`, `RESERVED_SECTORS`, `TABLE_COUNT` and `ROOT_CLUSTER` describe
  the one EFI shape, yet sit at the root of a crate whose reader and general format take any sector
  size from 512 to 4096. A stranger reads `SECTOR_BYTES` as the crate's sector size. Of these and of
  `VolumeLayout`'s eight methods, nife calls `new`, `metadata_sectors`, `fill_sector`,
  `file_first_sector` and `SECTOR_BYTES`, and nothing else does. Open question 2.
- **`VolumeLayout::cluster_first_sector` underflows below cluster 2.** Its documentation says so: a
  panic in a debug build and a wrong sector in release. A scratch program confirmed both. Every
  other public function in the crate returns an error or `None` for a bad input. Open question 2.
- **The allocation primitives are public.** `WritableVolume::allocate_cluster`, `deallocate_chain`
  and `truncate_chain` change the table directly, so a caller can leak clusters or free a file's
  chain under its directory entry. They are public because the oracle tests in
  `tests/the_writer_agrees.rs` drive them. The read-side primitives (`TableEntry`, `Slot`,
  `Volume::chain`, `Volume::table_entry`, `Volume::directory`) are harmless and useful to a repair
  tool. Open question 4.
- **A directory is a `u32` cluster number, and 0 means the root.** `create_file(0, ...)` creates in
  the root; any other number is taken as a directory's first cluster. A wrong number is refused by
  the chain walk, not by the type system.
- **Reads cannot see timestamps.** The writer stamps creation, write and access times, but neither
  `NamedEntry` nor `ShortEntry` has a method that returns them. Adding one later is not breaking.
- **`CivilDateTime` has public fields and stops at seconds.** FAT's creation time carries
  hundredths. Adding a field later would be breaking, and making the struct non-exhaustive would
  stop callers from building one. Seconds match every other field FAT stores.
- **Reads take `&mut self`**, because a read moves the device. A `Volume` is not shared between
  readers.
- **Two shipped texts still read as this tree's.** The crate doc opens "growing in stages. Three
  parts exist today", and the `BUGS` list calls the EFI writer "the `mkfs`". `VolumeLayout::new`'s
  doc says a caller "with an entropy capability" should draw the serial number. These are small
  edits for the move lane, listed under what the move still needs.

## Names

`python3 helpers/name_release.py --draft file_allocation_table` lists 368 names: the crate, 58
modules and 309 items. Every name a stranger meets has a `Name:` block, and the move commit strips
them all.

| source | names |
|---|---|
| #1806 (the crate) | 1 |
| #2720 (the first names of the reader and the EFI writer) | 94 |
| #2760 (directories and long names) | 55 |
| #2996, #2999 and #3001 (the stage 6 to 9 names, the clock, and the class rule on `tests` and `verification`) | 217 |
| provisional | 1: `LayoutError::LabelRefused`, open question 1 |

## Proofs

31 Kani harnesses, each with a falsification patch at
`falsifications/<module.path>.<harness>.patch`. `script/crate-standalone` proved every harness in a
copy of the crate and saw every patch fail its harness; the stranger copy proved them with stock
Kani 0.68.0, the version the shared template pins.

| area | harnesses | what they prove |
|---|---|---|
| boot sector and BPB | 4 | any 512-byte sector parses or is refused; what is accepted lies inside the volume; the type is the cluster count's; the table holds an entry per cluster; only data clusters have sectors |
| table | 4 | every entry lies inside the table; decoding by markers; a stored entry reads back and disturbs nothing else; encoding inverts decoding |
| chain walk | 1 | no walk outlasts the volume's cluster count (volumes of one to eight clusters) |
| directories | 3 | every slot parses as its bytes say; a directory reads only its own sectors; the run finder takes the first run of free slots |
| long names | 5 | the checksum is the specification's loop; every written run is well formed; every alias is a legal short name and carries its number; the reader accepts only a well-formed run |
| file read | 1 | a read stays inside the file's size and clusters |
| allocation | 3 | allocation takes one free cluster and nothing else; a release frees only its own chain and never a bad cluster; a cut keeps exactly the head |
| write cursor | 2 | a partial sector write keeps the head and zeroes the slack; a write extends the chain to the size and stays inside it |
| `FSInfo` | 2 | any sector parses to hints inside the volume; a store changes only the two hints |
| format | 1 | a computed geometry parses back as itself (clusters of 1 to 8 sectors) |
| partition table | 2 | any sector parses to a sound table or an error; a partition device reads only its partition |
| attributes, label, time | 3 | setting attributes changes the four user bits only; an accepted label is legal bytes then spaces; any time packs to legal fields |

What the proofs do not cover, and what covers it instead:

- **Whole images.** Kani proves per sector and over model volumes of a few clusters. Whole images
  are covered by the oracle tests and by two fuzz targets that live in nife, not in the crate
  (`fuzz/fuzz_targets/file_allocation_table_boot_sector.rs` and `..._writer.rs`). Open question 6.
- **The operations that compose the proven steps.** Create, rename, remove, path lookup, name
  comparison, directory growth and `VolumeLayout` have no harness; tests and the fuzz writer target
  cover them.
- **Crash consistency.** FAT has none. The crate orders its writes so a crash leaks a cluster and
  never shares one, and that ordering is tested, not proven.
- **The caller's device.** The crate assumes `read_sector` fills the buffer it is given and that the
  sector size does not change.

## Tests

300 pass and one is ignored, on Rust 1.88 and on stable: 234 unit tests, 57 integration tests in
13 files, and 9 doctests. `cargo clippy --all-targets` and `cargo doc` print no warning.

- **The oracle.** `lamfat` (rust-fatfs's main line) reads every tree this crate writes on FAT12,
  FAT16 and FAT32, and this crate reads every tree `lamfat` writes, name for name and byte for byte.
- **Other implementations.** Where `fsck.fat` or `fsck_msdos` is installed every image must pass
  `fsck -n`; where `mkfs.fat` is installed its FAT12 and FAT16 volumes take this crate's writer.
  Both were installed for this run. Two FAT32 images a macOS host formatted and wrote are read to
  their recorded hashes.
- **Pinned bytes.** `the_zero_timestamp_images_do_not_change` pins the formatted and EFI volumes'
  digests, so any change to what the writer emits is seen.
- **The README example** compiles and runs as `tests/the_readme_example.rs`.
- **Ignored.** `a_third_party_driver_reads_it` attaches an image for the host's own driver, on
  macOS. It did not run.
- **Mutation.** nife's ledger (`notes/project-metrics/mutation-triage.csv`) holds 40 rulings for
  this crate: 21 equivalent, 16 killed, 3 gaps. The gaps are a directory's 65,536-entry ceiling,
  which no test reaches, and one `slot_location` mutant that outruns `cargo-mutants`' time bound.
  The ledger stays in nife; the crate repository's own mutation run starts from nothing.

## Dependencies, `unsafe`, `no_std`

- **Run time: none.** `[dependencies]` is empty.
- **Development only**, both from crates.io, pinned with `=`: `lamfat 0.4.3` (MIT), the oracle,
  and `ruzstd 0.9.1` (MIT), which decompresses the two macOS images. nife's `deny.toml` allows
  `lamfat` to this crate alone; the new repository's `deny.toml` needs the same exception.
- **`unsafe`: none.** No site in the crate.
- **`no_std`**, with no `alloc`. The library builds on Rust 1.88 for
  `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and `x86_64-unknown-none`. No
  public signature names a type from another crate.
- **Panics.** Library code has no `unwrap`, `expect` or `panic!`. The `assert!` sites are in
  constant evaluation, and one `debug_assert!` bounds a loop. The exception is the
  `cluster_first_sector` underflow above.

## License and notices

`MIT OR Apache-2.0`, nife's own pair, with both texts at the crate root; `LICENSE-MIT` names the
copyright holder. The crate copies no code: it follows Microsoft's "FAT: General Overview of
On-Disk Format" version 1.03, and milestone 1019's list of crates that carry facts from their
sources does not name it. rust-fatfs, hadris-fat, embedded-sdmmc and simple-fatfs were read for
behavior only. Microsoft's long-name patents have all expired (milestone 813's patent table).

One notice question remains. The six boot-sector fixtures in `tests/fixtures/*.boot` are the first
512 bytes of volumes `mkfs.fat` made, so they carry dosfstools' boot stub and its "This is not a
bootable disk" message, and `cargo package` ships them in the `.crate`. dosfstools is
GPL-3.0-or-later. Open question 5.

## The README, from a stranger's chair

The crate's tracked files were copied alone into an empty directory, and only the README's commands
were run. This was not a true stranger: the reader had already seen the crate. A recorded stranger
run in the new repository is soak condition 4.

| step | result |
|---|---|
| `cargo test` on stable | 300 pass, 1 ignored |
| `cargo +1.88 test` (the stated minimum) | 300 pass, 1 ignored |
| `cargo kani`, Kani 0.68.0 as the README names | see the result below |
| a falsification patch, `patch -p3` from the crate root | all 31 apply |
| `cargo package` | packs; warns that the manifest has no `repository`, `homepage` or `documentation` |

What a stranger would trip on: nothing that stopped the run. The manifest needs a `repository` key
once the repository exists. The README does not say how to depend on the crate, which crates.io
answers once it publishes.

KANI_RESULT

## The `VolumeLayout` label, as #3233 leaves it

**Background.** `VolumeLayout` writes the one volume a bootloader installer needs: FAT32, 512-byte
sectors, 4 KiB clusters, `\EFI\BOOT\<file>` holding one file whose bytes the caller writes
contiguously after the metadata. nife's system installer and its cloud-image builder use it.
Until #3233 it wrote the fixed label `NIFE` into the boot sector and the root directory. That was
the only project policy in an otherwise general writer.

**What #3233 did.** `VolumeLayout::new` now takes `label: &str`, between `volume_id` and
`file_name`, with no default. It is checked exactly as `FormatOptions::with_label` checks one (one
to eleven bytes a short name allows), and a refusal is the new `LayoutError::LabelRefused`. nife's
two callers pass `"NIFE"`, and their bytes did not change. The alternative #3233 rejected was to move
the writer out of the crate into nife.

**The trade-off.**

- For: the crate keeps one tested EFI writer that any installer can use, and the policy stays with
  the caller. Validation is shared with the general format, so the two cannot disagree.
- Against: `new` now takes seven positional arguments, three `u64` and two `&str` side by side. A
  scratch program swapped the two strings: `"BOOTX64.EFI"` is refused as a label because a label
  cannot hold a dot, so the likely swap fails loudly. Swapping the three sizes compiles.
- Against: the general format makes a label optional (`with_label`), and `VolumeLayout` requires
  one. An empty label is refused; a caller who wants none passes `"NO NAME"`, the specification's
  placeholder, which nothing in the documentation says.
- Against: it is a second format path with its own constants, which the general `FormatPlan` could
  express as options (FAT32, cluster size 8) plus the three directories. Folding it in is more work
  than this release needs.

Open question 2 asks how much of this shape to publish.

## Semantic versioning

The first release has no baseline. `cargo semver-checks` builds the crate and runs its 202 checks
against a copy of itself without complaint, so the tool works on it. The tag `v0.1.0` on the move
commit becomes the baseline that every later change is checked against with `--baseline-rev`, as
the pilot's tags are. Soak condition 5 then measures a period with no change the tool calls
breaking; the period is not yet set.

## What the move still needs

| item | owner |
|---|---|
| Ratify `LayoutError::LabelRefused`, or rename it (open question 1). `script/crate-standalone` fails at its names step until then. | architect |
| The API changes the open questions settle, made in nife before the move so the move commit carries them. | a lane |
| The crate doc's "growing in stages", the `BUGS` list's "the `mkfs`", and "an entropy capability" in `VolumeLayout::new`. | the lane of open question 2, or the move lane |
| A `repository = "https://github.com/nifeos/file_allocation_table"` key in the manifest. | move lane |
| The `lamfat` exception in the new repository's `deny.toml`. | move lane |
| nife's three consumers (`components`, `xtask`, `fuzz`) move from the path dependency to `git` and `rev` with `version = "0.1.0"`. | move lane |

## Result of `script/crate-standalone`

STANDALONE_RESULT

## Open questions

Each is a decision about what the first release publishes. A published API is a one-way door: once
a stranger depends on it, taking something away is a breaking change. Before publication every one
of these is still cheap.

1. **The name `LayoutError::LabelRefused`.**
   Background: #3233 added the variant when the label became the caller's. Its siblings
   `FormatError::LabelRefused` and `WriteError::LabelRefused` are ratified, and so is
   `LayoutError::NameRefused` beside it. It is the crate's one unratified name, and
   `script/crate-standalone` fails until it is ratified.
   Recommendation: ratify it as it stands. It says the same thing in the same words as its three
   neighbors.

2. **How much of the EFI-system-partition writer to publish.**
   Background: `VolumeLayout` is a fixed FAT32 shape for installers, now with the caller's label.
   Its public surface is `new`, seven more methods, seven crate-root constants and the three
   `efi_system_partition` constants. nife, its only user, calls four of those items. The crate-root
   constants describe the EFI shape alone, but read as the crate's (a `SECTOR_BYTES` of 512 in a
   crate that handles 4096-byte sectors), and `cluster_first_sector` underflows below cluster 2.
   Options: (a) publish it as it stands; (b) publish `VolumeLayout` with `new`, `metadata_sectors`,
   `fill_sector`, `file_first_sector` and `file_sectors`, move the constants into
   `efi_system_partition` and make the rest private; (c) leave `VolumeLayout` out of the first
   release.
   Recommendation: (b). It keeps the writer #3233 chose to keep, takes nothing nife uses, and
   publishes nothing a stranger could misread or call into a panic. Anything hidden can be made
   public in a later minor release without breaking anyone; the reverse breaks.

3. **`#[non_exhaustive]` on the fourteen error enums.**
   Background: an exhaustive enum lets a caller match every variant with no wildcard arm, so adding
   a variant breaks that caller. Marking an enum `#[non_exhaustive]` makes every caller write a
   wildcard arm, and then new variants are not breaking. Adding the attribute after publication is
   itself breaking; removing it is not. The data enums (`TableEntry`, `Slot`, `SectorSize`,
   `FileAllocationTableType`) are complete by the specification and stay as they are.
   Recommendation: mark the fourteen error enums `#[non_exhaustive]` before 0.1.0, the stricter
   choice that can be relaxed later.

4. **The allocation primitives on `WritableVolume`.**
   Background: `allocate_cluster`, `deallocate_chain` and `truncate_chain` edit the table directly.
   Kani proves each correct on its own, but a caller who uses them outside a file operation can leak
   clusters or free a live file's chain. They are public so that the integration tests can check
   the table against the oracle.
   Recommendation: make the three crate-private before 0.1.0 and move their oracle checks into the
   crate's unit tests. Keep the read-side primitives public.

5. **The `mkfs.fat` boot stub in six test fixtures.**
   Background: the fixtures are 512-byte boot sectors that `mkfs.fat` (dosfstools, GPL-3.0-or-later)
   wrote, and the bytes after the BPB are dosfstools' boot code and message. The parser never reads
   them. They ship inside the `.crate`, whose license is MIT or Apache-2.0. Program output is usually
   not the program's to license, but this output embeds a fragment of the program, and a stranger
   auditing licenses would ask.
   Recommendation: zero the boot-code region of the six fixtures and say so in
   `make-boot-sectors.sh`. Every test keeps its meaning and the question disappears.

6. **Where the two fuzz targets live after the move.**
   Background: `file_allocation_table_boot_sector` and `file_allocation_table_writer` are in nife's
   `fuzz/`, which the fuzz host runs and whose findings it files as private advisories on
   nifeos/nife. The shared crate template has no fuzz job. After the move the targets would fuzz
   the crate through nife's `git` and `rev` dependency, and a finding would be filed against nife
   for code that lives in another repository.
   Recommendation: keep both targets in nife for the soak, since the pinned dependency is exactly
   what nife ships, and decide their home when the template gains a fuzz job. This does not hold
   the move.

7. **The version, tag and repository.**
   Background: the manifest says `0.1.0`; the pilot was tagged `v0.1.0` at its move and nife pins
   it by `rev` with the version beside it. The repository name follows the pilot's, the crate's own
   name with underscores.
   Recommendation: `0.1.0`, tag `v0.1.0` on the move commit once the answers above have landed, and
   `nifeos/file_allocation_table`.

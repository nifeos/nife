# The public API of `file_allocation_table`

*Name: provisional, minted by lane milestone/813-review-package. An appendix of
[the review package](file-allocation-table.md), which judges this list; this page only states
it.*

`script/crate-review-packet file_allocation_table` generated the item list from rustdoc JSON
(`cargo public-api` is not installed here), and this page fills in generic parameters and trait
items from the source. The crate root re-exports every item except the ten constants of the public
`efi_system_partition` module. Unless the list says otherwise, every type derives `Debug`,
`Clone`, `Copy`, `PartialEq`, `Eq` and `Hash`. `FormatOptions`, `FormatPlan`, `VolumeLayout`,
`LayoutError` and `NamedEntry` lack `Hash`; the types that hold a device (`Volume`,
`WritableVolume`, `Directory`, `NamedEntries`, `Chain`, `FileReader`, `FileWriter`) derive `Debug`
alone. No error type implements `Display` or `core::error::Error`. Every error enum is
`#[non_exhaustive]`; the data enums (`SectorSize`, `FileAllocationTableType`, `RootDirectory`,
`TableEntry`, `Slot`, `DirectoryEntry`) are not.

## The caller's side

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
    pub const MAXIMUM_BYTES: usize;                         // 4096
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

## Reading

```rust
pub struct Volume<D>;
impl<D: SectorDevice> Volume<D> {
    pub fn open(device: D) -> Result<Volume<D>, ReadError<D::Error>>;
    pub const fn boot_sector(&self) -> &BootSector;
    pub fn into_device(self) -> D;
    pub fn table_entry(&mut self, cluster: u32) -> Result<TableEntry, ChainError<D::Error>>;
    pub fn chain(&mut self, first_cluster: u32) -> Chain<'_, D>;
    pub fn root_directory(&mut self) -> Directory<'_, D>;
    pub fn directory(&mut self, first_cluster: u32) -> Directory<'_, D>;
    pub fn root_named_entries(&mut self) -> NamedEntries<'_, D>;
    pub fn named_entries(&mut self, first_cluster: u32) -> NamedEntries<'_, D>;
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
    pub fn chars(&self) -> impl Iterator<Item = char> + '_;
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

## Writing

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
}
pub struct FileWriter<'v, D, C>;
impl<'v, D: WritableSectorDevice, C: Clock> FileWriter<'v, D, C> {
    pub const fn size_bytes(&self) -> u32;
    pub const fn position(&self) -> u32;
    pub fn set_position(&mut self, position: u32);
    pub fn write(&mut self, data: &[u8]) -> Result<usize, WriteError<D::Error>>;
    pub fn set_len(&mut self, size_bytes: u32) -> Result<(), WriteError<D::Error>>;
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

## Formatting

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
    pub fn new(total_sectors: u64, hidden_sectors: u64, file_size_bytes: u64, volume_id: u32,
               label: &str, file_name: &str, clock: &mut impl Clock)
        -> Result<VolumeLayout, LayoutError>;
    pub const fn metadata_sectors(&self) -> u64;
    pub const fn file_first_sector(&self) -> u64;
    pub const fn file_sectors(&self) -> u64;
    pub fn fill_sector(&self, index: u64, out: &mut [u8]) -> bool;
}
pub mod efi_system_partition {
    pub const SECTOR_BYTES: usize;          // 512
    pub const CLUSTER_SECTORS: u32;         // 8
    pub const CLUSTER_BYTES: usize;         // 4096
    pub const RESERVED_SECTORS: u32;        // 32
    pub const TABLE_COUNT: u32;             // 2
    pub const ROOT_CLUSTER: u32;            // 2
    pub const MINIMUM_FAT32_CLUSTERS: u32;  // 65525
    pub const EFI_CLUSTER: u32;             // 3
    pub const BOOT_CLUSTER: u32;            // 4
    pub const FILE_CLUSTER: u32;            // 5
}
```

## Errors

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
| `WriteError<E>` | `Chain(ChainError<E>)`, `VolumeFull`, `FileTooLarge`, `ShorterThanItsSize`, `EntryChanged`, `Failed`, `DirectoryFull`, `NameRefused`, `LabelRefused`, `AlreadyExists`, `NotAFile`, `NotADirectory`, `ReadOnly`, `TimeOutOfRange`, `DirectoryNotEmpty`, `MovedIntoItself` |
| `FormatError` | `TooSmall`, `NoSuchGeometry`, `LabelRefused`, `TimeOutOfRange`, `ClusterSizeRefused` |
| `LayoutError` | `NotFat32`, `TooLarge`, `NameRefused`, `LabelRefused`, `TimeOutOfRange`, `FileTooBig` |

Two `From` conversions let `?` carry an error up: `ChainError<E>` and `LookupError<E>`, each into
`WriteError<E>`. Each enum in the table is `#[non_exhaustive]`.


#![no_std]
//! **A FAT12, FAT16 and FAT32 filesystem, sans I/O, growing in stages.** Three parts exist today.
//!
//! The reader opens a volume through [`SectorDevice`], a trait the caller implements over any
//! device that reads whole sectors. [`BootSector::read`] reads and checks the boot sector and its
//! BPB, and types the volume by its cluster count as the specification does (see
//! `boot_sector.rs`). Every field is treated as hostile: a parse never panics and never reads out
//! of bounds, and what it accepts has its regions in order inside the volume.
//!
//! [`Volume`] holds the device and the boot sector and reads the rest. It decodes the file
//! allocation table entry by entry ([`TableEntry`]), walks a cluster chain with [`Volume::chain`],
//! and reads a directory slot by slot ([`Volume::root_directory`], [`Volume::directory`]). A chain
//! walk yields at most as many clusters as the volume holds, so no table contents can make it
//! loop, and every read it makes lies inside the volume the boot sector describes.
//! [`Volume::root_entries`] and [`Volume::entries`] read a directory as files: each short entry
//! comes with its long name ([`LongFileName`]), assembled from the pieces ahead of it and accepted
//! only when their ordinals, padding and checksum are the specification's.
//!
//! [`Volume::lookup`] finds an entry by path, without regard to case, by long name or short name.
//! [`Volume::open_file`] opens one for reading as a [`FileReader`], which reads through the chain
//! and never returns a byte past the entry's size. A whole-disk image opens through
//! [`MasterBootRecord`]: [`Partition::device`] shows one partition as a device of its own, and
//! [`Volume::open`] opens the volume inside it.
//!
//! The table is writable through [`WritableSectorDevice`]. [`Volume::allocate_cluster`],
//! [`Volume::truncate_chain`] and [`Volume::free_chain`] take and give back clusters, each writing
//! every live copy of the table so a crash leaks a cluster and never shares one. The FAT32 `FSInfo`
//! hints ([`FsInfo`]) follow in memory and [`Volume::sync`] writes them. The first change clears the
//! clean-shutdown bit (FAT16 and FAT32) and the sync sets it back ([`Volume::is_clean`]).
//!
//! [`Volume::create_file`] makes an empty file (a name that is not 8.3 gets long-name slots and a
//! numbered alias), [`Volume::open_file_writer`] opens a [`FileWriter`] that writes, extends and
//! sets a length, and [`Volume::remove_file`] deletes. [`Volume::make_directory`],
//! [`Volume::remove_directory`], [`Volume::rename`], [`Volume::set_attributes`] and
//! [`Volume::set_volume_label`] change the tree. A full directory grows by a
//! cluster, except the fixed root of FAT12 and FAT16. Each returns a [`WriteError`].
//!
//! [`Format`] lays out an empty volume of any size, as sectors a caller writes. [`VolumeLayout`]
//! lays out one shape through the same sizing and boot sector: a FAT32 EFI system partition holding
//! one boot file at `\EFI\BOOT\<name>`. A volume below [`FAT32_MINIMUM_CLUSTERS`] is refused.

//! # BUGS
//!
//! - The `mkfs` numbers its alias `~1`, right only while the volume holds one file.
//! - Every timestamp is zero: a `mkfs` with no clock capability writes 1970 rather than inventing one.
//! - The `mkfs` writes the `FSInfo` free-cluster count once. After that only [`Volume::sync`] keeps
//!   it; a volume dropped without a sync leaves a stale hint, which the specification lets a driver
//!   tolerate and `fsck.fat` reports.
//! - The backup `FSInfo` copy in the backup boot area is never updated, as with Linux's driver.
//! - A newly allocated cluster holds whatever the device held; a caller that needs zeros writes them.
//! - A write has no barrier: the crate orders its writes and the device decides when each reaches the
//!   medium. A failure between the writes of one table entry's copies leaves the copies unequal.
//! - A name with a character outside the Basic Multilingual Plane is refused, and a lower-case 8.3
//!   name is stored as a long name, since the crate writes no lower-case flag.
//! - A directory slot is found by walking from the directory's first cluster each time, so creating
//!   in a directory of n clusters costs n chain steps per slot read.
//! - Timestamps are zero and a writer sets only the archive attribute.
//! - The 65,536-entry ceiling of a directory (`grow_directory`'s limit) is untested: reaching it
//!   needs a volume and a test of that size.
//! - A file's bytes are read by cluster chain only; there is no cache above the sector and a
//!   backwards seek walks the chain again from its start.
//! - Names compare by simple Unicode upper-casing, not the volume's code page or upcase table, and
//!   a short name's bytes above 0x7F (an OEM code page) match only themselves.
//! - The MBR reader does not follow extended partitions, and reads no GUID partition table.
//! - Partition sectors are in the device's own sectors, which is 512 bytes for the tools that wrote
//!   the tables this was tested against.
//! - A long name is compared by its UTF-16 code units as stored; a lone surrogate is not an error.
//! - A directory read follows only the table the reader picks (FAT32's live table, or the first),
//!   and never compares the mirrors.
//! - A loop in a hostile table stops a chain walk only at the volume's cluster count: on a FAT32
//!   volume claiming 268 million clusters, that many table reads, nearly all from the cached sector.
//! - A boot sector is checked against itself, never against the device's length, which the
//!   [`SectorDevice`] trait does not report; a read past the end is the device's error to return.
//! - A FAT32 layout under 65525 clusters (which `mkfs.fat -F 32` makes with a warning) is refused,
//!   because Linux reads it as FAT32 and the specification as FAT16.
//! - Not crash-atomic and no journal, which is FAT.
//!
//! Name: ratified 2026-10-07 (#1806). FAT spelled out, as the acronym test asks; permanent once
//! published.

mod allocation;
mod attributes;
mod boot_sector;
mod directory;
mod directory_write;
#[doc(alias = "extensible firmware interface system partition")]
pub mod efi_system_partition;
mod file;
mod file_write;
mod file_writer;
mod format;
mod fs_info;
mod label;
mod long_name;
mod long_name_assembly;
mod lookup;
mod master_boot_record;
mod named_entry;
mod reader;
mod sector;
mod table;
mod time;
mod volume;
mod writable_volume;

pub use allocation::AllocationError;
pub use attributes::Attributes;
pub use boot_sector::{BootSector, FileAllocationTableType, ParseError, ReadError, RootDirectory};
pub use directory::{Directory, DirectoryEntry, LongNameEntry, SLOT_BYTES, ShortEntry, Slot};
pub use file::{FileReadError, FileReader, NotAFile};
pub use file_write::WriteError;
pub use file_writer::FileWriter;
pub use format::{Format, FormatError, FormatOptions};
pub use fs_info::{FsInfo, FsInfoError};
pub use label::VolumeLabel;
pub use long_name_assembly::{LongFileName, LongNameError};
pub use lookup::LookupError;
pub use master_boot_record::{
    MasterBootRecord, MasterBootRecordError, MasterBootRecordReadError, PARTITION_COUNT, Partition,
    PartitionDevice, PartitionError,
};
pub use named_entry::{Entries, NamedEntry};
pub use reader::{Chain, ChainError, Volume};
pub use sector::{SectorDevice, SectorSize, WritableSectorDevice};
pub use table::TableEntry;
pub use time::{Clock, NoClock, Timestamp};
pub use volume::{
    CLUSTER_BYTES, CLUSTER_SECTORS, FAT32_MINIMUM_CLUSTERS, LayoutError, RESERVED_SECTORS,
    ROOT_CLUSTER, SECTOR_BYTES, TABLE_COUNT, VolumeLayout,
};
pub use writable_volume::WritableVolume;

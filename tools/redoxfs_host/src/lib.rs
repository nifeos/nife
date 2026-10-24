//! Host-side RedoxFS image operations (milestone 32, extended for recovery in milestone 57).
//!
//! The library half of the `redoxfs_host` tool: create an image, walk it, read a file back, and
//! extract a subtree, against the vendored 0.9.1 pin (vendor/redoxfs), the engine the nife FS
//! server serves it with. Every read verb takes a [`Volume`], an image or one GPT partition of a
//! device (milestone 110 (the recovery tool takes a device and a partition)), its offset in the
//! [`PartitionDisk`] the engine is handed. Reads write nothing: `read_node_inner`, never
//! `read_node`, whose atime update dirties the header ring. No key handling: the volume is not
//! encrypted (milestone 57 (partitioning and formatting a real drive, and extended attributes),
//! decided 2026-07-30). Engine errors become `String` here (notes/host-recovery/the-library.md).

use std::fs::File;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use filesystem_contract::xattr;
use globally_unique_identifier_partition_table::GloballyUniqueIdentifierPartitionTable;
use redoxfs::{BLOCK_SIZE, Disk, DiskFile, FileSystem, Node, Transaction, TreeData, TreePtr};
use syscall::error::{EIO, Error};
use uuid::Uuid;

/// Public so the recovery test can read an attribute back off the host file the extract wrote. The
/// round trip is the claim this feature makes, and closing it needs both halves in one place.
pub mod host_xattr;

mod attributes;
mod extract;
mod listing;
mod partition_table;
mod volume;
mod walk;
mod write;

use attributes::*;
pub use attributes::{Attr, attrs};
pub use extract::{ExtractStats, extract};
use listing::*;
pub use listing::{Kind, LsEntry, cat, ls};
use partition_table::*;
pub use partition_table::{PartitionInfo, TableInfo, partitions};
use volume::*;
pub use volume::{PartitionDisk, PartitionSelector, Volume};
use walk::*;
pub use write::{import, mkfs, put};

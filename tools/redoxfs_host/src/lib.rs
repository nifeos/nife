//! Host-side RedoxFS image operations (milestone 32, extended for recovery in milestone 57).
//!
//! The library half of the `redoxfs_host` tool: create an image, walk it, read a file back, and
//! extract a whole subtree to a host directory, all against the **vendored 0.9.1 pin**
//! (vendor/redoxfs), which is the same engine the nife FS server serves it with. That
//! identity is the point twice over. It is why an image built here is proven against exactly the
//! code that will mount it on nife, and it is why the answer to "the board is dead, can I
//! get my data?" is a tool rather than a kernel driver: the engine already links here with `std`,
//! so `ls`/`cat`/`extract` cost a few hundred lines and run identically on macOS and Linux with no
//! FUSE, no kernel extension, and no root. See notes/host-recovery.md.
//!
//! **The read paths open the image read-only and write nothing to it**, which matters when the
//! image is the last copy of the data. That rules out `FileSystem::open`'s `cleanup` pass and
//! rules out `Transaction::read_node`, whose atime update dirties a node (and so the header ring)
//! on any file last read more than an hour ago. `read_node_inner` is the same read without the
//! timestamp, so recovery uses it.
//!
//! **A device and a partition, not only an image file** (milestone 110). Every read verb takes a
//! [`Volume`], which is either the whole file (the bytes at offset zero are the filesystem, which is
//! what an image is) or one partition inside it, found by reading the GUID partition table with
//! `crates/globally_unique_identifier_partition_table`. The offset then lives in the
//! [`PartitionDisk`] the engine is handed, so block zero of the filesystem is the partition's first
//! block and nothing above the disk layer knows a partition was involved. That is the same shape
//! the board's own `mkfs` uses, and the point is that a disk pulled out of the machine at 2am can
//! be read where it lies instead of being `dd`ed into an image first, on a laptop that may not have
//! room for it.
//!
//! **No key handling, deliberately** (roadmap milestone 57, decided 2026-07-30). RedoxFS supports
//! encryption and this volume does not use it: encryption belongs at the Time Machine layer, where
//! the Mac encrypts before anything is sent. So every `FileSystem::open` here passes `None` for the
//! password and a reader needs no secret at all.
//!
//! Errors are rendered to `String` at this boundary. The engine's error type is
//! `syscall::error::Error` (redox_syscall's errno); mapping it once here keeps the redox_syscall
//! type out of our callers, the same "map the error type once, at the boundary" rule the roadmap
//! sets for the FS server's ABI edge. We never *construct* one, which is why the `_tx` helpers
//! return `Result<T, String>` and the `fs.tx` closures hand that back through `Ok`: a
//! `Transaction` can only be made by `FileSystem::tx`, whose closure must fail in the engine's own
//! vocabulary. Read paths write nothing, so committing a walk that failed commits nothing.

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

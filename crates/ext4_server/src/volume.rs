//! **What the server asks of an ext4 implementation**: the [`Volume`] trait, and the nodes,
//! entries and statistics it answers with.
//!
//! Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.

use alloc::vec::Vec;

use crate::ErrorNumber;

/// A node on the volume: an inode number, for ext4.
///
/// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
pub type NodeId = u64;

/// What a node is, as far as the contract can tell the difference.
///
/// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// A directory.
    Directory,
    /// A regular file.
    RegularFile,
    /// A symbolic link, which is listed and never followed.
    SymbolicLink,
    /// A character or block device, a named pipe or a socket.
    Special,
}

/// One name in a directory, as the volume lists it.
///
/// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectoryEntry {
    /// The name's bytes, without a `/` or a zero byte.
    pub name: Vec<u8>,
    /// The node it names.
    pub node: NodeId,
    /// What that node is.
    pub kind: NodeKind,
}

/// The facts about one node the contract carries.
///
/// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeStatus {
    /// What the node is.
    pub kind: NodeKind,
    /// Its size in bytes.
    pub size_bytes: u64,
    /// Its modification time in whole Unix seconds, negative before 1970.
    pub modification_seconds: i64,
}

/// The volume's size, as `GET_VOLUME_STATISTICS` reports it.
///
/// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VolumeStatistics {
    /// One block of the filesystem, in bytes.
    pub block_bytes: u64,
    /// How many blocks the volume has.
    pub total_blocks: u64,
    /// How many of them are free.
    pub free_blocks: u64,
}

/// **An ext4 volume, mounted read-only**: what the server asks of an implementation. It has no
/// method that writes, so a server over it cannot write whatever a request says.
///
/// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
pub trait Volume {
    /// The root directory's node.
    fn root(&self) -> NodeId;

    /// The node `name` names in `directory`, with its kind, or `None` when there is no such name.
    /// `name` is one component, already checked by the server.
    fn lookup(
        &mut self,
        directory: NodeId,
        name: &[u8],
    ) -> Result<Option<(NodeId, NodeKind)>, ErrorNumber>;

    /// Every entry of `directory`, `.` and `..` included or not, in any order. The server sorts
    /// them and drops the two.
    fn list(&mut self, directory: NodeId) -> Result<Vec<DirectoryEntry>, ErrorNumber>;

    /// The node's status.
    fn status(&mut self, node: NodeId) -> Result<NodeStatus, ErrorNumber>;

    /// Read the file's bytes at `offset` into `out`, returning how many were read; 0 at or past
    /// the end.
    fn read(&mut self, node: NodeId, offset: u64, out: &mut [u8]) -> Result<usize, ErrorNumber>;

    /// The full name of every extended attribute on the node, its namespace prefix included
    /// (`user.comment`, `security.selinux`, `system.posix_acl_access`), in any order.
    fn extended_attribute_names(&mut self, node: NodeId) -> Result<Vec<Vec<u8>>, ErrorNumber>;

    /// The value of the attribute with the full name `name`, or `None` when it is not set.
    fn extended_attribute(
        &mut self,
        node: NodeId,
        name: &[u8],
    ) -> Result<Option<Vec<u8>>, ErrorNumber>;

    /// The volume's size.
    fn statistics(&mut self) -> VolumeStatistics;
}

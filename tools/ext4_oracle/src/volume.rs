//! **An `rsext4` mount as `ext4_server`'s [`Volume`]**, over any block device the server can read.
//! The server path (`bin/ext4_oracle_server.rs`) serves an image file through it, and the ext4
//! fuzz targets (`tools/ext4_oracle_fuzz`) serve the generated corpus through it, so the two judge
//! the same adapter.
//!
//! The device is read as the block capability serves one, in whole 4,096-byte blocks, through
//! [`Sectors`] at [`SECTOR_BYTES`], and mounted read-only with journal replay off. Every write
//! `rsext4` asks for is refused before it reaches the sectors.
//!
//! This module is the one place `rsext4` meets `ext4_server`: the crate links no ext4 code, and
//! `rsext4` stays host-only in this tool (ruled on pull request #3639, 2026-11-16 UTC).
//!
//! Name: provisional, minted 2026-11-26 (UTC) by lane/ext4-fuzz-rotation. calef names files.
use ext4_server::{
    BlockDevice, DirectoryEntry, ErrorNumber, NodeId, NodeKind, NodeStatus, SECTOR_BYTES, Sectors,
    Volume, VolumeStatistics,
};
use file_system_contract::directory;
use rsext4::error::Ext4Result;
use rsext4::*;

use crate::namespace;

/// **Mount `blocks` read-only, with journal replay off**, as the server's volume, or the error
/// `rsext4` refuses the mount with.
///
/// Name: provisional, minted 2026-11-26 (UTC) by lane/ext4-fuzz-rotation.
pub fn mount<B: BlockDevice>(blocks: B) -> Result<Ext4Volume<B>, Ext4Error> {
    let sectors =
        Sectors::new(blocks, SECTOR_BYTES).expect("SECTOR_BYTES is a sector size Sectors accepts");
    let services = MountServices::new(FixedClock, (), NoopObserver);
    Ext4::mount(
        Device(sectors),
        services,
        MountOptions::read_only_no_journal_replay(),
    )
    .map(Ext4Volume)
}

/// `rsext4`'s device: the sectors the server adapts from the block capability. Every write is
/// refused before it reaches them.
///
/// Name: provisional, minted 2026-11-26 (UTC) by lane/ext4-fuzz-rotation.
pub struct Device<B>(Sectors<B>);

impl<B: BlockDevice> BlockIo for Device<B> {
    fn read(&mut self, buffer: &mut [u8], sector: SectorId, count: u32) -> Ext4Result<()> {
        self.0
            .read(sector.raw(), count, buffer)
            .map_err(|_| Ext4Error::io())
    }
    fn write(&mut self, _: &[u8], _: SectorId, _: u32) -> Ext4Result<()> {
        Err(Ext4Error::read_only())
    }
    fn flush(&mut self) -> Ext4Result<()> {
        Ok(())
    }
    fn geometry(&self) -> DeviceGeometry {
        DeviceGeometry::new(self.0.sector_bytes(), self.0.sectors())
    }
    fn capabilities(&self) -> DeviceCapabilities {
        DeviceCapabilities {
            read_only: true,
            ..DeviceCapabilities::default()
        }
    }
}

/// The clock a read-only mount is given: one fixed time, since nothing it does writes one.
struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> Ext4Result<Ext4Timestamp> {
        Ok(Ext4Timestamp::new(1_600_000_000, 0))
    }
}

type Fs<B> = Ext4<Device<B>, MountedServices<(), NoopObserver, ()>>;

/// An `rsext4` mount as the server's [`Volume`].
///
/// Name: provisional, minted 2026-11-26 (UTC) by lane/ext4-fuzz-rotation.
pub struct Ext4Volume<B: BlockDevice>(Fs<B>);

/// The errno a client sees for an `rsext4` error: the three the contract tells apart, and `EIO`
/// for every other, corruption and checksum failures included.
fn errno(error: Ext4Error) -> ErrorNumber {
    match error.kind() {
        Ext4ErrorKind::NotFound => 2,
        Ext4ErrorKind::NotDirectory => directory::ENOTDIR,
        Ext4ErrorKind::IsDirectory => directory::EISDIR,
        _ => ext4_server::INPUT_OUTPUT_ERROR,
    }
}

fn kind_of(file_type: DirectoryEntryType) -> NodeKind {
    match file_type {
        DirectoryEntryType::Directory => NodeKind::Directory,
        DirectoryEntryType::RegularFile => NodeKind::RegularFile,
        DirectoryEntryType::Symlink => NodeKind::SymbolicLink,
        _ => NodeKind::Special,
    }
}

fn inode_number(node: NodeId) -> Result<InodeNumber, ErrorNumber> {
    InodeNumber::from_u64(node).map_err(errno)
}

impl<B: BlockDevice> Ext4Volume<B> {
    /// The namespace and the name within it of a full attribute name, as `rsext4` stores it.
    fn split_attribute(name: &[u8]) -> Option<(XattrNamespace, &[u8])> {
        let spaces = [
            XattrNamespace::PosixAclAccess,
            XattrNamespace::PosixAclDefault,
            XattrNamespace::User,
            XattrNamespace::Trusted,
            XattrNamespace::Security,
            XattrNamespace::System,
        ];
        spaces.into_iter().find_map(|space| {
            let prefix = namespace(space);
            let rest = name.strip_prefix(prefix.as_bytes())?;
            let whole = !prefix.ends_with('.');
            (whole == rest.is_empty()).then_some((space, rest))
        })
    }
}

impl<B: BlockDevice> Volume for Ext4Volume<B> {
    fn root(&self) -> NodeId {
        u64::from(self.0.root_inode().raw())
    }

    fn lookup(
        &mut self,
        dir: NodeId,
        name: &[u8],
    ) -> Result<Option<(NodeId, NodeKind)>, ErrorNumber> {
        let name = FileName::new(name).map_err(|_| directory::EINVAL)?;
        let found = self
            .0
            .lookup_child(inode_number(dir)?, name)
            .map_err(errno)?;
        Ok(found.map(|info| (u64::from(info.number.raw()), kind_of(info.file_type()))))
    }

    fn list(&mut self, dir: NodeId) -> Result<Vec<DirectoryEntry>, ErrorNumber> {
        let dir = inode_number(dir)?;
        let mut entries = Vec::new();
        let mut cursor = DirectoryCursor::Start;
        loop {
            let batch = self.0.read_directory(dir, cursor, 256).map_err(errno)?;
            let Some(last) = batch.last() else { break };
            cursor = last.next_cursor;
            for entry in batch {
                let kind = match entry.file_type {
                    DirectoryEntryType::Unknown => {
                        kind_of(self.0.inode(entry.inode).map_err(errno)?.file_type())
                    }
                    known => kind_of(known),
                };
                entries.push(DirectoryEntry {
                    name: entry.name,
                    node: u64::from(entry.inode.raw()),
                    kind,
                });
            }
            if cursor == DirectoryCursor::End {
                break;
            }
        }
        Ok(entries)
    }

    fn status(&mut self, node: NodeId) -> Result<NodeStatus, ErrorNumber> {
        let info = self.0.inode(inode_number(node)?).map_err(errno)?;
        Ok(NodeStatus {
            kind: kind_of(info.file_type()),
            size_bytes: info.size,
            modification_seconds: info.mtime.sec,
        })
    }

    fn read(&mut self, node: NodeId, offset: u64, out: &mut [u8]) -> Result<usize, ErrorNumber> {
        self.0
            .read_inode(inode_number(node)?, offset, out)
            .map_err(errno)
    }

    fn extended_attribute_names(&mut self, node: NodeId) -> Result<Vec<Vec<u8>>, ErrorNumber> {
        let names = self.0.list_xattrs(inode_number(node)?).map_err(errno)?;
        Ok(names
            .into_iter()
            .map(|n| [namespace(n.namespace).into_bytes(), n.name].concat())
            .collect())
    }

    fn extended_attribute(
        &mut self,
        node: NodeId,
        name: &[u8],
    ) -> Result<Option<Vec<u8>>, ErrorNumber> {
        if !self
            .extended_attribute_names(node)?
            .iter()
            .any(|n| n == name)
        {
            return Ok(None);
        }
        let Some((space, within)) = Self::split_attribute(name) else {
            return Ok(None);
        };
        self.0
            .get_xattr(inode_number(node)?, space, within)
            .map(Some)
            .map_err(errno)
    }

    fn statistics(&mut self) -> VolumeStatistics {
        let s = self.0.statfs();
        VolumeStatistics {
            block_bytes: s.block_size,
            total_blocks: s.total_blocks,
            free_blocks: s.free_blocks,
        }
    }
}

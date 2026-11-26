//! Print the manifest of an ext4 image as a client of `ext4_server` reads it: the server path of
//! the ext4 read oracle.
//!
//! Usage: `ext4_oracle_server <image>`
//!
//! The image is read as the block capability serves a device, in whole 4,096-byte blocks, through
//! `ext4_server::Sectors` at `ext4_server::SECTOR_BYTES`, and mounted read-only by `rsext4` with
//! journal replay off. An `ext4_server::Server` serves the mount, and everything below reaches the
//! volume only through file-service requests a client could send: `OPENDIR`, `READDIR`, `OPEN`,
//! `FSTAT`, `READ`, `LISTXATTR`, `GETXATTR`, `GET_MODIFICATION_TIME_UNIX_SECONDS` and `CLOSE`.
//!
//! The output is one line per entry, sorted by path, in the fields a client can see: path, kind,
//! size, content, attributes and modification time. `compare.py --server` projects the `debugfs`
//! manifest onto the same fields. A kind is `Directory` or `RegularFile`, or the error `OPEN`
//! answers for anything else (`ELOOP` for a symbolic link, `ENXIO` for a device, a named pipe or a
//! socket). A mount the crate refuses prints one `MOUNT-ERROR` line with the error and exits 2.
//!
//! This file is the one place `rsext4` meets `ext4_server`: the crate links no ext4 code, and
//! `rsext4` stays host-only in this tool (ruled on pull request #3639, 2026-11-16 UTC).
//!
//! It computes CRC32C on the path `ext4_oracle_manifest::crc32c_path::select` takes, and
//! `EXT4_ORACLE_CRC32C` can require one.
//!
//! See nifeos/record:design/roadmap/0190-ext4-read-write/read-oracle.md.
//!
//! Name: provisional. calef names files.
use std::fs::File;
use std::io::Write;
use std::os::unix::fs::FileExt;

use ext4_oracle_manifest::{fnv, hex_or_text, namespace};
use ext4_server::{
    BlockDevice, DirectoryEntry, ErrorNumber, NodeId, NodeKind, NodeStatus, SECTOR_BYTES, Sectors,
    Server, Volume, VolumeStatistics,
};
use file_system_contract::directory::{self, rights};
use file_system_contract::{block, directory_entry, extended_attribute, file_system};
use rsext4::error::Ext4Result;
use rsext4::*;

/// The image file as a block device: whole blocks, the tail of the last one zero.
struct FileBlocks {
    file: File,
    size: u64,
}

impl BlockDevice for FileBlocks {
    fn read_block(
        &mut self,
        index: u64,
        out: &mut [u8; block::BLOCK_BYTES],
    ) -> Result<(), ErrorNumber> {
        let at = index * block::BLOCK_BYTES as u64;
        if at >= self.size {
            return Err(ext4_server::INPUT_OUTPUT_ERROR);
        }
        let take = (self.size - at).min(block::BLOCK_BYTES as u64) as usize;
        out[take..].fill(0);
        self.file
            .read_exact_at(&mut out[..take], at)
            .map_err(|_| ext4_server::INPUT_OUTPUT_ERROR)
    }

    fn size_bytes(&self) -> u64 {
        self.size
    }
}

/// `rsext4`'s device: the sectors the server adapts from the block capability. Every write is
/// refused before it reaches them.
struct Device(Sectors<FileBlocks>);

impl BlockIo for Device {
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

struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> Ext4Result<Ext4Timestamp> {
        Ok(Ext4Timestamp::new(1_600_000_000, 0))
    }
}

type Fs = Ext4<Device, MountedServices<(), NoopObserver, ()>>;

/// An `rsext4` mount as the server's [`Volume`].
struct Ext4Volume(Fs);

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

impl Ext4Volume {
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

impl Volume for Ext4Volume {
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

/// A client of the server: its one window, and requests sent through it.
struct Client {
    server: Server<Ext4Volume>,
    window: Vec<u8>,
}

/// A reply word as a result: an errno when it is negative.
fn result(r0: i64) -> Result<u64, ErrorNumber> {
    if r0 < 0 {
        Err((-r0) as ErrorNumber)
    } else {
        Ok(r0 as u64)
    }
}

/// An errno as the manifest prints it.
fn errno_text(e: ErrorNumber) -> String {
    match e {
        ext4_server::TOO_MANY_SYMBOLIC_LINKS => "ELOOP".into(),
        ext4_server::NO_SUCH_DEVICE => "ENXIO".into(),
        ext4_server::VALUE_TOO_LARGE => "EOVERFLOW".into(),
        extended_attribute::ERANGE => "ERANGE".into(),
        extended_attribute::E2BIG => "E2BIG".into(),
        other => format!("E{other}"),
    }
}

impl Client {
    fn call(
        &mut self,
        op: u64,
        handle: u64,
        len: usize,
        w1: u64,
    ) -> (Result<u64, ErrorNumber>, u64) {
        let w0 = file_system::request(op, handle, len as u64);
        let (r0, r1) = self.server.handle(w0, w1, &mut self.window);
        (result(r0), r1)
    }

    fn named(
        &mut self,
        op: u64,
        handle: u64,
        name: &[u8],
        w1: u64,
    ) -> (Result<u64, ErrorNumber>, u64) {
        self.window[..name.len()].copy_from_slice(name);
        self.call(op, handle, name.len(), w1)
    }

    fn close(&mut self, handle: u64) {
        self.call(file_system::CLOSE, handle, 0, 0)
            .0
            .expect("a handle this client opened closes");
    }

    /// Every entry of a directory handle, as `READDIR` pages them.
    fn entries(&mut self, handle: u64) -> Result<Vec<(Vec<u8>, bool)>, ErrorNumber> {
        let mut all = Vec::new();
        loop {
            let filled = self
                .call(file_system::READDIR, handle, 0, all.len() as u64)
                .0? as usize;
            if filled == 0 {
                return Ok(all);
            }
            all.extend(directory_entry::iter(&self.window[..filled]).map(|(n, d)| (n.to_vec(), d)));
        }
    }

    /// Every byte of a file handle, as `READ` delivers them.
    fn content(&mut self, handle: u64) -> Result<Vec<u8>, ErrorNumber> {
        let mut bytes = Vec::new();
        loop {
            let at = bytes.len() as u64;
            let n = self
                .call(
                    file_system::READ,
                    handle,
                    file_system::MAXIMUM_TRANSFER_BYTES,
                    at,
                )
                .0? as usize;
            if n == 0 {
                return Ok(bytes);
            }
            bytes.extend_from_slice(&self.window[..n]);
        }
    }

    /// Every attribute of what a handle names, as `name=length:FNV-1a-64`, sorted, or `none`.
    fn attributes(&mut self, handle: u64) -> String {
        let filled = match self.call(file_system::LISTXATTR, handle, 0, 0).0 {
            Ok(n) => n as usize,
            Err(e) => return errno_text(e),
        };
        let names: Vec<Vec<u8>> = extended_attribute::list::iter(&self.window[..filled])
            .map(<[u8]>::to_vec)
            .collect();
        let mut parts = Vec::new();
        for name in names {
            let text = String::from_utf8_lossy(&name).into_owned();
            match self.named(file_system::GETXATTR, handle, &name, 0).0 {
                Ok(r0) => {
                    let len = extended_attribute::reply_value_len(r0 as i64);
                    let value = &self.window[..len];
                    parts.push(format!("{text}={len}:{:016x}", fnv(value)));
                }
                Err(e) => parts.push(format!("{text}={}", errno_text(e))),
            }
        }
        parts.sort();
        if parts.is_empty() {
            "none".into()
        } else {
            parts.join(",")
        }
    }

    /// One manifest line per entry under the root, unsorted.
    fn walk(&mut self) -> Vec<String> {
        let mut lines = Vec::new();
        let mut pending: Vec<Vec<u8>> = vec![Vec::new()];
        while let Some(path) = pending.pop() {
            // Each directory is opened afresh from the root by its path, so the handles held at
            // once stay two whatever the tree's width.
            let handle = if path.is_empty() {
                file_system::ROOT
            } else {
                match self
                    .named(
                        file_system::OPENDIR,
                        file_system::ROOT,
                        &path[1..],
                        rights::PARENT_RIGHTS,
                    )
                    .0
                {
                    Ok(h) => h,
                    Err(e) => {
                        lines.push(format!("{}\tOPENDIR-{}", hex_or_text(&path), errno_text(e)));
                        continue;
                    }
                }
            };
            match self.entries(handle) {
                Ok(entries) => {
                    for (name, is_directory) in entries {
                        let mut child = path.clone();
                        child.push(b'/');
                        child.extend_from_slice(&name);
                        lines.push(self.entry_line(handle, &child, &name, is_directory));
                        if is_directory {
                            pending.push(child);
                        }
                    }
                }
                Err(e) => lines.push(format!("{}\tREADDIR-{}", hex_or_text(&path), errno_text(e))),
            }
            if handle != file_system::ROOT {
                self.close(handle);
            }
        }
        lines
    }

    /// The line of one entry: path, kind, size, content, attributes and modification time.
    fn entry_line(&mut self, parent: u64, path: &[u8], name: &[u8], is_directory: bool) -> String {
        let modification = match self
            .named(
                file_system::GET_MODIFICATION_TIME_UNIX_SECONDS,
                parent,
                name,
                0,
            )
            .0
        {
            Ok(seconds) => seconds.to_string(),
            Err(e) => errno_text(e),
        };
        let (kind, size, content, attributes) = if is_directory {
            match self
                .named(file_system::OPENDIR, parent, name, rights::PARENT_RIGHTS)
                .0
            {
                Ok(h) => {
                    let attributes = self.attributes(h);
                    self.close(h);
                    ("Directory".into(), "-".into(), "-".into(), attributes)
                }
                Err(e) => (
                    format!("OPENDIR-{}", errno_text(e)),
                    "-".into(),
                    "-".into(),
                    "-".into(),
                ),
            }
        } else {
            match self.named(file_system::OPEN, parent, name, rights::READ) {
                (Ok(h), opened_size) => {
                    let size = match self.call(file_system::FSTAT, h, 0, 0).0 {
                        Ok(s) if s == opened_size => s.to_string(),
                        Ok(s) => format!("{s}-OPEN-SAID-{opened_size}"),
                        Err(e) => errno_text(e),
                    };
                    let content = match self.content(h) {
                        Ok(b) => format!("{}:{:016x}", b.len(), fnv(&b)),
                        Err(e) => errno_text(e),
                    };
                    let attributes = self.attributes(h);
                    self.close(h);
                    ("RegularFile".into(), size, content, attributes)
                }
                (Err(e), _) => (errno_text(e), "-".into(), "-".into(), "-".into()),
            }
        };
        format!(
            "{}\t{kind}\t{size}\t{content}\t{attributes}\t{modification}",
            hex_or_text(path)
        )
    }
}

fn main() {
    ext4_oracle_manifest::crc32c_path::select();
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("usage: ext4_oracle_server <image>");
        std::process::exit(64);
    }
    let file = File::open(&args[1]).expect("the image opens for reading");
    let size = file.metadata().expect("the image has a length").len();
    let sectors = Sectors::new(FileBlocks { file, size }, SECTOR_BYTES)
        .expect("SECTOR_BYTES is a sector size Sectors accepts");
    let services = MountServices::new(FixedClock, (), NoopObserver);
    let fs = match Ext4::mount(
        Device(sectors),
        services,
        MountOptions::read_only_no_journal_replay(),
    ) {
        Ok(fs) => fs,
        Err(e) => {
            println!("MOUNT-ERROR\t{e:?}");
            std::process::exit(2);
        }
    };
    let mut client = Client {
        server: Server::new(Ext4Volume(fs)),
        window: vec![0; file_system::MAXIMUM_TRANSFER_BYTES],
    };
    let mut lines = client.walk();
    lines.sort();
    let mut out = std::io::stdout().lock();
    for l in lines {
        writeln!(out, "{l}").unwrap();
    }
}

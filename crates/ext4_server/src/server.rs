//! **One file-service request, decoded and answered** over a read-only [`Volume`].
//!
//! Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use file_system_contract::directory::{self, Rights, rights};
use file_system_contract::{
    MAXIMUM_ERRNO, block, directory_entry, error, extended_attribute, file_system, operation, verb,
    volume_statistics,
};
use subtree_scope::Refusal;

use crate::{
    BAD_HANDLE, ErrorNumber, INPUT_OUTPUT_ERROR, NO_SUCH_DEVICE, NO_SUCH_ENTRY, NodeId, NodeKind,
    TOO_MANY_OPEN_HANDLES, VALUE_TOO_LARGE, Volume,
};

/// One page, the most a reply whose length the server chooses may fill.
const PAGE: usize = block::BLOCK_BYTES;

/// The rights the endpoint's bound directory carries: list, read and descend, and nothing that
/// writes, so no handle this server mints carries a write right either.
const ROOT_RIGHTS: u64 = rights::ENUMERATE | rights::READ | rights::DESCEND;

/// The most handles open at once, [`file_system::ROOT`] included. Provisional, restrictive first.
///
/// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
pub const MAXIMUM_OPEN_HANDLES: usize = 256;

/// What a handle names.
enum Open {
    /// A directory, its rights, and its listing once `READDIR` has read it. The volume is
    /// read-only, so a listing never goes stale.
    Directory {
        node: NodeId,
        rights: Rights,
        listing: Option<Vec<(Vec<u8>, bool)>>,
    },
    /// A regular file and the rights it was opened with.
    File { node: NodeId, rights: Rights },
}

impl Open {
    fn node(&self) -> NodeId {
        match self {
            Open::Directory { node, .. } | Open::File { node, .. } => *node,
        }
    }

    fn rights(&self) -> Rights {
        match self {
            Open::Directory { rights, .. } | Open::File { rights, .. } => *rights,
        }
    }
}

/// A walk's failure: an errno, or a `subtree_scope` refusal turned into one.
struct Failure(ErrorNumber);

impl From<Refusal> for Failure {
    fn from(refusal: Refusal) -> Self {
        Failure(refusal.errno())
    }
}

/// **The server**: a volume and the handles open on it.
///
/// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
pub struct Server<V> {
    volume: V,
    handles: BTreeMap<u64, Open>,
}

impl<V: Volume> Server<V> {
    /// Serve `volume`, with [`file_system::ROOT`] bound to its root directory.
    ///
    /// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
    pub fn new(volume: V) -> Self {
        let root = Open::Directory {
            node: volume.root(),
            rights: Rights::root(ROOT_RIGHTS),
            listing: None,
        };
        let mut handles = BTreeMap::new();
        handles.insert(file_system::ROOT, root);
        Server { volume, handles }
    }

    /// **Answer one request**: `w0` and `w1` as the client sent them, and `window` the client's
    /// channel, which names and bytes are read from and replies written into. Returns the two
    /// reply words.
    ///
    /// A verb the contract marks as mutating answers `EROFS` before anything else is read. The
    /// binding verbs and an unknown code answer `EINVAL`. This is the one place an [`ErrorNumber`](crate::ErrorNumber)
    /// becomes a reply word.
    ///
    /// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
    pub fn handle(&mut self, w0: u64, w1: u64, window: &mut [u8]) -> (i64, u64) {
        let code = operation(w0);
        if verb::of(code).is_some_and(|v| v.mutates()) {
            return (error::word(directory::EROFS), 0);
        }
        let handle = file_system::request_handle(w0);
        let requested = file_system::request_length_bytes(w0);
        let len = requested.min(PAGE).min(window.len());
        let bulk = requested
            .min(file_system::MAXIMUM_TRANSFER_BYTES)
            .min(window.len());
        let page = PAGE.min(window.len());
        // Every name is copied out of the window before it is judged: a client's second thread
        // can rewrite the window while the request is served.
        let name = window[..len].to_vec();
        let result = match code {
            file_system::OPEN => self.open(handle, &name, w1),
            file_system::READ => self.read(handle, w1, &mut window[..bulk]).map(with_zero),
            file_system::CLOSE => self.close(handle).map(with_zero),
            file_system::FSTAT => self.size(handle).map(with_zero),
            file_system::OPENDIR => self.open_directory(handle, &name, w1).map(with_zero),
            file_system::READDIR => self
                .read_directory(handle, w1, &mut window[..page])
                .map(with_zero),
            file_system::GETXATTR => self
                .extended_attribute(handle, &name, &mut window[..page])
                .map(with_zero),
            file_system::LISTXATTR => self
                .extended_attribute_names(handle, &mut window[..page])
                .map(with_zero),
            file_system::GET_VOLUME_STATISTICS => {
                self.statistics(handle, &mut window[..page]).map(with_zero)
            }
            file_system::GET_MODIFICATION_TIME_UNIX_SECONDS => {
                self.modification_time(handle, &name).map(with_zero)
            }
            file_system::NARROW => self.narrow(handle, w1).map(with_zero),
            _ => Err(directory::EINVAL),
        };
        match result {
            Ok(reply) => reply,
            Err(e) if (1..=MAXIMUM_ERRNO).contains(&e) => (error::word(e), 0),
            Err(_) => (error::word(INPUT_OUTPUT_ERROR), 0),
        }
    }

    /// The directory a handle names, with its rights: `EBADF` for no such handle, `ENOTDIR`
    /// for a file.
    fn directory(&self, handle: u64) -> Result<(NodeId, Rights), ErrorNumber> {
        match self.handles.get(&handle) {
            Some(Open::Directory { node, rights, .. }) => Ok((*node, *rights)),
            Some(Open::File { .. }) => Err(directory::ENOTDIR),
            None => Err(BAD_HANDLE),
        }
    }

    /// Install `open` under the lowest free handle.
    fn mint(&mut self, open: Open) -> Result<u64, ErrorNumber> {
        if self.handles.len() >= MAXIMUM_OPEN_HANDLES {
            return Err(TOO_MANY_OPEN_HANDLES);
        }
        let free = (1..=file_system::MAXIMUM_HANDLE)
            .find(|h| !self.handles.contains_key(h))
            .ok_or(TOO_MANY_OPEN_HANDLES)?;
        self.handles.insert(free, open);
        Ok(free)
    }

    /// Walk every component of `path` but the last from `start`, as `subtree_scope::walk` does,
    /// refusing the attribute store's reserved name anywhere in it.
    fn walk<'p>(
        &mut self,
        start: NodeId,
        rights: Rights,
        path: &'p [u8],
        requested: u64,
    ) -> Result<(NodeId, Rights, &'p [u8]), ErrorNumber> {
        if path
            .split(|&b| b == b'/')
            .any(|c| c == extended_attribute::STORE_DIRECTORY.as_bytes())
        {
            return Err(directory::EINVAL);
        }
        let volume = &mut self.volume;
        subtree_scope::walk(start, rights, path, requested, |node, step| {
            match volume.lookup(node, step).map_err(Failure)? {
                Some((child, kind)) => Ok((child, scope_kind(kind))),
                None => Err(Failure(NO_SUCH_ENTRY)),
            }
        })
        .map_err(|Failure(e)| e)
    }

    /// `OPEN`: a regular file under a directory handle, by a relative path. Replies with the
    /// handle and the file's size.
    fn open(
        &mut self,
        parent: u64,
        path: &[u8],
        step_rights: u64,
    ) -> Result<(i64, u64), ErrorNumber> {
        let (start, rights) = self.directory(parent)?;
        let (directory_node, rights, last) = self.walk(start, rights, path, step_rights)?;
        if rights.denies_all(rights::READ | rights::WRITE) {
            return Err(NO_SUCH_ENTRY);
        }
        let (node, kind) = self
            .volume
            .lookup(directory_node, last)?
            .ok_or(NO_SUCH_ENTRY)?;
        if kind == NodeKind::Special {
            return Err(NO_SUCH_DEVICE);
        }
        subtree_scope::check_landing(scope_kind(kind), false).map_err(Refusal::errno)?;
        let size = self.volume.status(node)?.size_bytes;
        let rights = rights.attenuate(rights::READ | rights::WRITE);
        let handle = self.mint(Open::File { node, rights })?;
        Ok((handle as i64, size))
    }

    /// `OPENDIR`: a directory under a directory handle, by a relative path, carrying the
    /// parent's rights narrowed to `requested`.
    fn open_directory(
        &mut self,
        parent: u64,
        path: &[u8],
        requested: u64,
    ) -> Result<i64, ErrorNumber> {
        let (start, rights) = self.directory(parent)?;
        let requested = rights.substitute_parent_rights(requested);
        let (directory_node, rights, last) = self.walk(start, rights, path, requested)?;
        if !rights.allows(rights::DESCEND) {
            return Err(NO_SUCH_ENTRY);
        }
        let (node, kind) = self
            .volume
            .lookup(directory_node, last)?
            .ok_or(NO_SUCH_ENTRY)?;
        subtree_scope::check_landing(scope_kind(kind), true).map_err(Refusal::errno)?;
        let child = rights.attenuate(requested);
        if child != Rights::root(requested) {
            return Err(directory::EPERM);
        }
        self.mint(Open::Directory {
            node,
            rights: child,
            listing: None,
        })
        .map(|h| h as i64)
    }

    /// `NARROW`: a second handle on a directory already held, with fewer rights.
    fn narrow(&mut self, handle: u64, requested: u64) -> Result<i64, ErrorNumber> {
        let (node, rights) = self.directory(handle)?;
        let requested = rights.substitute_parent_rights(requested);
        let child = rights.attenuate(requested);
        if child != Rights::root(requested) {
            return Err(directory::EPERM);
        }
        self.mint(Open::Directory {
            node,
            rights: child,
            listing: None,
        })
        .map(|h| h as i64)
    }

    /// `READ`: bytes of a file at `offset`.
    fn read(&mut self, handle: u64, offset: u64, out: &mut [u8]) -> Result<i64, ErrorNumber> {
        match self.handles.get(&handle) {
            Some(Open::File { node, rights }) if rights.allows(rights::READ) => {
                let node = *node;
                self.volume.read(node, offset, out).map(|n| n as i64)
            }
            Some(Open::File { .. }) | None => Err(BAD_HANDLE),
            Some(Open::Directory { .. }) => Err(directory::EISDIR),
        }
    }

    /// `CLOSE`: any handle but [`file_system::ROOT`], which stays bound while the server runs.
    fn close(&mut self, handle: u64) -> Result<i64, ErrorNumber> {
        if handle == file_system::ROOT {
            return Err(directory::EINVAL);
        }
        self.handles.remove(&handle).map(|_| 0).ok_or(BAD_HANDLE)
    }

    /// `FSTAT`: the size in bytes of what the handle names.
    fn size(&mut self, handle: u64) -> Result<i64, ErrorNumber> {
        let node = self.handles.get(&handle).ok_or(BAD_HANDLE)?.node();
        let size = self.volume.status(node)?.size_bytes;
        i64::try_from(size).map_err(|_| VALUE_TOO_LARGE)
    }

    /// `READDIR`: `directory_entry` records from entry `cursor` on, sorted by name, without
    /// `.`, `..` or the attribute store's reserved name.
    fn read_directory(
        &mut self,
        handle: u64,
        cursor: u64,
        out: &mut [u8],
    ) -> Result<i64, ErrorNumber> {
        let (node, rights) = self.directory(handle)?;
        if !rights.allows(rights::ENUMERATE) {
            return Err(directory::EPERM);
        }
        let listing = match self.handles.get(&handle) {
            Some(Open::Directory {
                listing: Some(listing),
                ..
            }) => listing,
            _ => {
                let fresh = listing_of(self.volume.list(node)?);
                if let Some(Open::Directory { listing, .. }) = self.handles.get_mut(&handle) {
                    *listing = Some(fresh);
                }
                match self.handles.get(&handle) {
                    Some(Open::Directory {
                        listing: Some(listing),
                        ..
                    }) => listing,
                    _ => return Err(INPUT_OUTPUT_ERROR),
                }
            }
        };
        let first = usize::try_from(cursor).unwrap_or(usize::MAX);
        let mut filled = 0;
        for (name, is_directory) in listing.iter().skip(first) {
            match directory_entry::encode(&mut out[filled..], name, *is_directory) {
                Some(n) => filled += n,
                None => break,
            }
        }
        Ok(filled as i64)
    }

    /// `GETXATTR`: one attribute's value into `out`, the reply packing its kind and length.
    fn extended_attribute(
        &mut self,
        handle: u64,
        name: &[u8],
        out: &mut [u8],
    ) -> Result<i64, ErrorNumber> {
        let node = self.readable(handle)?;
        if !extended_attribute::is_valid_name(name) {
            return Err(extended_attribute::ERANGE);
        }
        let value = self
            .volume
            .extended_attribute(node, name)?
            .ok_or(extended_attribute::ENODATA)?;
        if value.len() > extended_attribute::MAXIMUM_VALUE_BYTES || value.len() > out.len() {
            return Err(extended_attribute::E2BIG);
        }
        out[..value.len()].copy_from_slice(&value);
        Ok(extended_attribute::reply(
            extended_attribute::RAW,
            value.len(),
        ))
    }

    /// `LISTXATTR`: every attribute name on the node, sorted, in one reply or `ERANGE`.
    fn extended_attribute_names(
        &mut self,
        handle: u64,
        out: &mut [u8],
    ) -> Result<i64, ErrorNumber> {
        let node = self.readable(handle)?;
        let mut names = self.volume.extended_attribute_names(node)?;
        if names.len() > extended_attribute::MAXIMUM_ATTRIBUTES {
            return Err(extended_attribute::ERANGE);
        }
        names.sort();
        let mut filled = 0;
        for name in &names {
            filled += extended_attribute::list::encode(&mut out[filled..], name)
                .ok_or(extended_attribute::ERANGE)?;
        }
        Ok(filled as i64)
    }

    /// The node behind a handle that carries `READ`, for the attribute verbs: `EBADF` otherwise.
    fn readable(&self, handle: u64) -> Result<NodeId, ErrorNumber> {
        match self.handles.get(&handle) {
            Some(open) if open.rights().allows(rights::READ) => Ok(open.node()),
            _ => Err(BAD_HANDLE),
        }
    }

    /// `GET_VOLUME_STATISTICS`: the volume's size, for any handle this server minted.
    fn statistics(&mut self, handle: u64, out: &mut [u8]) -> Result<i64, ErrorNumber> {
        if !self.handles.contains_key(&handle) {
            return Err(BAD_HANDLE);
        }
        let s = self.volume.statistics();
        volume_statistics::encode(out, s.block_bytes, s.total_blocks, s.free_blocks)
            .map(|n| n as i64)
            .ok_or(INPUT_OUTPUT_ERROR)
    }

    /// `GET_MODIFICATION_TIME_UNIX_SECONDS`: the modification time of a name directly under a
    /// directory handle.
    fn modification_time(&mut self, handle: u64, name: &[u8]) -> Result<i64, ErrorNumber> {
        let (node, rights) = self.directory(handle)?;
        if !rights.allows(rights::READ) {
            return Err(NO_SUCH_ENTRY);
        }
        if !subtree_scope::is_name(name) || name == extended_attribute::STORE_DIRECTORY.as_bytes() {
            return Err(directory::EINVAL);
        }
        let (child, _) = self.volume.lookup(node, name)?.ok_or(NO_SUCH_ENTRY)?;
        let seconds = self.volume.status(child)?.modification_seconds;
        if seconds < 0 {
            return Err(VALUE_TOO_LARGE);
        }
        Ok(seconds)
    }
}

/// A reply whose second word is 0.
fn with_zero(r0: i64) -> (i64, u64) {
    (r0, 0)
}

/// The listing `READDIR` serves: sorted by name, without `.`, `..` or the attribute store's
/// name, and ending before the first name a record cannot carry (empty, or past 255 bytes).
fn listing_of(entries: Vec<crate::DirectoryEntry>) -> Vec<(Vec<u8>, bool)> {
    let store = extended_attribute::STORE_DIRECTORY.as_bytes();
    let mut listing: Vec<(Vec<u8>, bool)> = entries
        .into_iter()
        .filter(|e| e.name != b"." && e.name != b".." && e.name != store)
        .map(|e| (e.name, e.kind == NodeKind::Directory))
        .collect();
    listing.sort();
    if let Some(bad) = listing
        .iter()
        .position(|(name, _)| name.is_empty() || name.len() > usize::from(u8::MAX))
    {
        listing.truncate(bad);
    }
    listing
}

/// What `subtree_scope` makes of a node: a special file is stepped through as a file would be,
/// which refuses it with `ENOTDIR`.
fn scope_kind(kind: NodeKind) -> subtree_scope::NodeKind {
    match kind {
        NodeKind::Directory => subtree_scope::NodeKind::Directory,
        NodeKind::RegularFile | NodeKind::Special => subtree_scope::NodeKind::File,
        NodeKind::SymbolicLink => subtree_scope::NodeKind::Symlink,
    }
}

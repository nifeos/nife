//! **The disk tree: a read-only view of a whole disk as one file, served without a cache**
//! (milestone 834 (ioping on nife and Linux)). The pure half of `components/src/disk_tree.rs`.
//!
//! A `file_system_contract::file_system` namespace of exactly two names: [`DIRECTORY`] under the root, and
//! [`FILE`] in it, whose bytes are the disk's bytes from block 0 and whose size is the disk's. Every
//! [`file_system::READ`] this answers becomes exactly one device read ([`Answer::Read`]), with no buffer to
//! answer from, and no mutating verb is answered (each is [`directory::EROFS`]). The shell mounts it as
//! its second tree. See `partsfoundry/record:notes/block-devices.md`, "The disk tree: a whole disk as one file, with no
//! cache".

//! # BUGS
//!
//! - One file is the whole namespace, whatever partitions are on it; a partition is a byte range.
//! - A read longer than the server's transfer window (`block::TRANSFER_BLOCKS`) comes back short.
//!
//! Name: provisional (lane `lane/ioping-direct-io`, 2026-10-18 UTC), for the crate, the program,
//! [`DIRECTORY`] and [`FILE`]. The mount path is `grant_plan::MOUNTED_TREE_PATH`'s.

#![no_std]

use file_system_contract::{
    block, directory, directory_entry, error, extended_attribute, file_system, operation, verb,
};

/// The one directory under the root: the controller, as Linux's `nvme0`.
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/ioping-direct-io.
pub const DIRECTORY: &[u8] = b"nvme0";
/// The one file in [`DIRECTORY`]: namespace 1, as Linux's `nvme0n1`.
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/ioping-direct-io.
pub const FILE: &[u8] = b"n1";

/// What the root carries: list it, walk into it, read what is in it. Nothing that writes, so no
/// handle this server mints can carry a right to write either ([`directory::Rights::attenuate`] cannot
/// widen).
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/ioping-direct-io.
pub const ROOT_RIGHTS: u64 =
    directory::rights::ENUMERATE | directory::rights::READ | directory::rights::DESCEND;

/// Open handles across every client, the root included. The table is per server, as
/// `redoxfs_server`'s is, because a caretaker translates its client's numbers into these.
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/ioping-direct-io.
pub const HANDLES: usize = 32;

const ENOENT: i32 = 2;
const EBADF: i32 = 9;
const EMFILE: i32 = 24;
const EOPNOTSUPP: i32 = 95;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Node {
    Root,
    Directory,
    Namespace,
}

impl Node {
    fn child(self, name: &[u8]) -> Option<Node> {
        match self {
            Node::Root if name == DIRECTORY => Some(Node::Directory),
            Node::Directory if name == FILE => Some(Node::Namespace),
            _ => None,
        }
    }

    fn entries(self) -> &'static [(&'static [u8], bool)] {
        match self {
            Node::Root => &[(DIRECTORY, true)],
            Node::Directory => &[(FILE, false)],
            Node::Namespace => &[],
        }
    }
}

/// **One device read, and what to do with it.** The server reads `blocks` blocks starting at
/// `block` into its transfer window, copies `len` bytes starting `skip` bytes in to the start of
/// the client's page, and replies `len`.
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/ioping-direct-io.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Read {
    /// The first device block, in [`block::BLOCK_BYTES`] units.
    pub block: u64,
    /// How many blocks, `1..=block::TRANSFER_BLOCKS`.
    pub blocks: usize,
    /// Where in the first block the client's bytes start.
    pub skip: usize,
    /// How many bytes the client gets, never 0.
    pub len: usize,
}

/// What a request comes to.
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/ioping-direct-io.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// Reply with these two words; the page already holds whatever the reply describes.
    Reply(i64, u64),
    /// Go to the device first ([`Read`]).
    Read(Read),
}

fn err(errno: i32) -> Answer {
    Answer::Reply(error::word(errno), 0)
}

/// The namespace and its handle table.
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/ioping-direct-io.
pub struct Tree {
    size: u64,
    handles: [Option<(Node, directory::Rights)>; HANDLES],
}

impl Tree {
    /// A tree over a disk of `size` bytes, with the root installed at [`file_system::ROOT`].
    ///
    /// Name: provisional, minted 2026-10-18 (UTC) by lane/ioping-direct-io.
    pub fn new(size: u64) -> Self {
        let mut handles = [None; HANDLES];
        handles[file_system::ROOT as usize] =
            Some((Node::Root, directory::Rights::root(ROOT_RIGHTS)));
        Tree { size, handles }
    }

    fn get(&self, handle: u64) -> Option<(Node, directory::Rights)> {
        self.handles.get(handle as usize).copied().flatten()
    }

    fn install(&mut self, entry: (Node, directory::Rights)) -> Answer {
        match self.handles.iter().position(Option::is_none) {
            Some(h) => {
                self.handles[h] = Some(entry);
                Answer::Reply(h as i64, 0)
            }
            None => err(EMFILE),
        }
    }

    /// **Answer one request.** `w0` and `w1` are the request's two words; `page` is the client's
    /// shared page, holding the name for a verb that takes one and receiving a listing for
    /// [`file_system::READDIR`].
    ///
    /// Name: provisional, minted 2026-10-18 (UTC) by lane/ioping-direct-io.
    pub fn handle(&mut self, w0: u64, w1: u64, page: &mut [u8]) -> Answer {
        let code = operation(w0);
        let Some(v) = verb::of(code) else {
            return err(directory::EINVAL);
        };
        let Some((node, rights)) = self.get(file_system::request_handle(w0)) else {
            return err(EBADF);
        };
        // A copy, because a listing is written into the page the name arrived in.
        let mut name = [0u8; 256];
        let n = file_system::request_length_bytes(w0);
        if v.carries_len() && code != file_system::READ && code != file_system::WRITE {
            if n > name.len() || n > page.len() {
                return err(directory::EINVAL);
            }
            name[..n].copy_from_slice(&page[..n]);
        }
        let name = &name[..n.min(name.len())];
        match code {
            file_system::OPENDIR => self.open_directory(node, rights, name, w1),
            file_system::OPEN => self.open_file(node, rights, name, w1),
            file_system::READ => self.read(node, n.min(page.len()), w1),
            file_system::FSTAT => match node {
                Node::Namespace => Answer::Reply(self.size as i64, 0),
                _ => Answer::Reply(0, 0),
            },
            file_system::CLOSE if file_system::request_handle(w0) == file_system::ROOT => {
                err(directory::EINVAL)
            }
            file_system::CLOSE => {
                self.handles[file_system::request_handle(w0) as usize] = None;
                Answer::Reply(0, 0)
            }
            file_system::READDIR => list(node, rights, w1, page),
            // Nothing here was ever written, so there is nothing to make durable.
            file_system::SYNC => Answer::Reply(0, 0),
            file_system::LISTXATTR => Answer::Reply(0, 0),
            file_system::GETXATTR => err(extended_attribute::ENODATA),
            _ if v.mutates() => err(directory::EROFS),
            // GET_MODIFICATION_TIME_UNIX_SECONDS, GET_VOLUME_STATISTICS and the binding verbs: a disk has no modification time or free
            // count this server could truthfully give, and it enforces no bound subtree.
            _ => err(EOPNOTSUPP),
        }
    }

    /// Walk `name` from `(node, rights)`: every component but the last is a descent asking
    /// `DESCEND | step`, as [`file_system::OPEN`] and [`file_system::OPENDIR`] both define a path.
    fn walk(
        node: Node,
        rights: directory::Rights,
        name: &[u8],
        step: u64,
    ) -> Result<(Node, directory::Rights, &[u8]), i32> {
        let mut parts = name.split(|&b| b == b'/');
        let mut last = parts.next().unwrap_or_default();
        let (mut node, mut rights) = (node, rights);
        for next in parts {
            (node, rights) = descend(node, rights, last, directory::rights::DESCEND | step)?;
            last = next;
        }
        if last.is_empty() || last == b"." || last == b".." {
            return Err(directory::EINVAL);
        }
        Ok((node, rights, last))
    }

    fn open_directory(
        &mut self,
        node: Node,
        rights: directory::Rights,
        name: &[u8],
        asked: u64,
    ) -> Answer {
        let found = Self::walk(node, rights, name, asked)
            .and_then(|(node, rights, last)| descend(node, rights, last, asked));
        match found {
            Ok(entry) => self.install(entry),
            Err(errno) => err(errno),
        }
    }

    fn open_file(
        &mut self,
        node: Node,
        rights: directory::Rights,
        name: &[u8],
        step: u64,
    ) -> Answer {
        let (parent, rights, last) = match Self::walk(node, rights, name, step) {
            Ok(found) => found,
            Err(errno) => return err(errno),
        };
        // The contract asks for `READ` or `WRITE`; no handle here carries `WRITE`
        // ([`ROOT_RIGHTS`]), so `READ` is the whole test, and every file handle can read.
        if !rights.allows(directory::rights::READ) {
            return err(ENOENT);
        }
        match parent.child(last) {
            None => err(ENOENT),
            Some(Node::Namespace) => match self.install((Node::Namespace, rights)) {
                Answer::Reply(h, _) if h >= 0 => Answer::Reply(h, self.size),
                refused => refused,
            },
            Some(_) => err(directory::EISDIR),
        }
    }

    fn read(&self, node: Node, asked: usize, offset: u64) -> Answer {
        if node != Node::Namespace {
            return err(directory::EISDIR);
        }
        if offset >= self.size || asked == 0 {
            return Answer::Reply(0, 0);
        }
        let bs = block::BLOCK_BYTES;
        let skip = (offset % bs as u64) as usize;
        let room = block::TRANSFER_BLOCKS * bs - skip;
        let len = asked
            .min(room)
            .min((self.size - offset).min(usize::MAX as u64) as usize);
        Answer::Read(Read {
            block: offset / bs as u64,
            blocks: (skip + len).div_ceil(bs),
            skip,
            len,
        })
    }
}

/// One step down, as [`file_system::OPENDIR`] defines it: `DESCEND` on the parent or the name is not there,
/// a directory or `ENOTDIR`, and exactly the rights asked for or `EPERM`.
fn descend(
    node: Node,
    rights: directory::Rights,
    name: &[u8],
    asked: u64,
) -> Result<(Node, directory::Rights), i32> {
    if name.is_empty() || name == b"." || name == b".." {
        return Err(directory::EINVAL);
    }
    if !rights.allows(directory::rights::DESCEND) {
        return Err(ENOENT);
    }
    match node.child(name) {
        None => Err(ENOENT),
        Some(Node::Namespace) => Err(directory::ENOTDIR),
        Some(child) => {
            let narrowed = rights.attenuate(asked);
            if narrowed.bits() != asked {
                return Err(directory::EPERM);
            }
            Ok((child, narrowed))
        }
    }
}

fn list(node: Node, rights: directory::Rights, cursor: u64, page: &mut [u8]) -> Answer {
    if node == Node::Namespace {
        return err(directory::ENOTDIR);
    }
    if !rights.allows(directory::rights::ENUMERATE) {
        return err(directory::EPERM);
    }
    let mut at = 0;
    for &(name, is_dir) in node.entries().iter().skip(cursor as usize) {
        match directory_entry::encode(&mut page[at..], name, is_dir) {
            Some(n) => at += n,
            None => break,
        }
    }
    Answer::Reply(at as i64, 0)
}

#[cfg(test)]
mod tests;

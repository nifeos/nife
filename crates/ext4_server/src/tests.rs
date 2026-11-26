use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;

use file_system_contract::directory::{self, rights};
use file_system_contract::{
    block, directory_entry, error, extended_attribute, file_system, verb, volume_statistics,
};

use crate::server::MAXIMUM_OPEN_HANDLES;
use crate::{
    BAD_HANDLE, BlockDevice, DirectoryEntry, ErrorNumber, INPUT_OUTPUT_ERROR, NO_SUCH_DEVICE,
    NO_SUCH_ENTRY, NodeId, NodeKind, NodeStatus, Sectors, Server, TOO_MANY_OPEN_HANDLES,
    TOO_MANY_SYMBOLIC_LINKS, VALUE_TOO_LARGE, Volume, VolumeStatistics,
};

/// One node of the fake volume.
struct Node {
    kind: NodeKind,
    bytes: Vec<u8>,
    children: Vec<(Vec<u8>, NodeId)>,
    attributes: Vec<(Vec<u8>, Vec<u8>)>,
    seconds: i64,
}

/// **An in-memory volume**: root 2 (as ext4's is) holding `docs/` with `a.txt` and `b.txt`, a
/// symbolic link `link`, a named pipe `pipe`, a file `old` dated before 1970, and the attribute store's
/// reserved name, which the server must never show.
struct Fake {
    nodes: BTreeMap<NodeId, Node>,
}

fn node(kind: NodeKind, bytes: &[u8], seconds: i64) -> Node {
    Node {
        kind,
        bytes: bytes.to_vec(),
        children: Vec::new(),
        attributes: Vec::new(),
        seconds,
    }
}

impl Fake {
    fn new() -> Self {
        let mut nodes = BTreeMap::new();
        let mut root = node(NodeKind::Directory, b"", 100);
        root.children = vec![
            (b".".to_vec(), 2),
            (b"..".to_vec(), 2),
            (b"pipe".to_vec(), 6),
            (b"docs".to_vec(), 3),
            (b"link".to_vec(), 5),
            (b"old".to_vec(), 7),
            (extended_attribute::STORE_DIRECTORY.as_bytes().to_vec(), 8),
        ];
        let mut docs = node(NodeKind::Directory, b"", 200);
        docs.children = vec![(b"b.txt".to_vec(), 9), (b"a.txt".to_vec(), 4)];
        let mut a = node(NodeKind::RegularFile, b"hello, ext4", 300);
        a.attributes = vec![
            (b"user.comment".to_vec(), b"first".to_vec()),
            (b"security.selinux".to_vec(), b"label".to_vec()),
        ];
        nodes.insert(2, root);
        nodes.insert(3, docs);
        nodes.insert(4, a);
        nodes.insert(5, node(NodeKind::SymbolicLink, b"docs", 0));
        nodes.insert(6, node(NodeKind::Special, b"", 0));
        nodes.insert(7, node(NodeKind::RegularFile, b"", -1));
        nodes.insert(8, node(NodeKind::Directory, b"", 0));
        nodes.insert(9, node(NodeKind::RegularFile, b"b", 400));
        Fake { nodes }
    }

    fn get(&self, n: NodeId) -> Result<&Node, ErrorNumber> {
        self.nodes.get(&n).ok_or(INPUT_OUTPUT_ERROR)
    }
}

impl Volume for Fake {
    fn root(&self) -> NodeId {
        2
    }

    fn lookup(
        &mut self,
        d: NodeId,
        name: &[u8],
    ) -> Result<Option<(NodeId, NodeKind)>, ErrorNumber> {
        let parent = self.get(d)?;
        if parent.kind != NodeKind::Directory {
            return Err(directory::ENOTDIR);
        }
        Ok(parent
            .children
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, c)| (*c, self.nodes[c].kind)))
    }

    fn list(&mut self, d: NodeId) -> Result<Vec<DirectoryEntry>, ErrorNumber> {
        Ok(self
            .get(d)?
            .children
            .iter()
            .map(|(name, c)| DirectoryEntry {
                name: name.clone(),
                node: *c,
                kind: self.nodes[c].kind,
            })
            .collect())
    }

    fn status(&mut self, n: NodeId) -> Result<NodeStatus, ErrorNumber> {
        let node = self.get(n)?;
        Ok(NodeStatus {
            kind: node.kind,
            size_bytes: node.bytes.len() as u64,
            modification_seconds: node.seconds,
        })
    }

    fn read(&mut self, n: NodeId, offset: u64, out: &mut [u8]) -> Result<usize, ErrorNumber> {
        let bytes = &self.get(n)?.bytes;
        let start = (offset as usize).min(bytes.len());
        let take = (bytes.len() - start).min(out.len());
        out[..take].copy_from_slice(&bytes[start..start + take]);
        Ok(take)
    }

    fn extended_attribute_names(&mut self, n: NodeId) -> Result<Vec<Vec<u8>>, ErrorNumber> {
        Ok(self
            .get(n)?
            .attributes
            .iter()
            .map(|(k, _)| k.clone())
            .collect())
    }

    fn extended_attribute(
        &mut self,
        n: NodeId,
        name: &[u8],
    ) -> Result<Option<Vec<u8>>, ErrorNumber> {
        Ok(self
            .get(n)?
            .attributes
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone()))
    }

    fn statistics(&mut self) -> VolumeStatistics {
        VolumeStatistics {
            block_bytes: 4096,
            total_blocks: 1000,
            free_blocks: 250,
        }
    }
}

/// A client: one window, and requests sent through it.
struct Client {
    server: Server<Fake>,
    window: Vec<u8>,
}

impl Client {
    fn new() -> Self {
        Client {
            server: Server::new(Fake::new()),
            window: vec![0; file_system::MAXIMUM_TRANSFER_BYTES],
        }
    }

    fn call(&mut self, operation: u64, handle: u64, len: u64, w1: u64) -> (i64, u64) {
        let w0 = file_system::request(operation, handle, len);
        self.server.handle(w0, w1, &mut self.window)
    }

    fn named(&mut self, operation: u64, handle: u64, name: &[u8], w1: u64) -> (i64, u64) {
        self.window[..name.len()].copy_from_slice(name);
        self.call(operation, handle, name.len() as u64, w1)
    }

    fn open_directory(&mut self, handle: u64, path: &[u8]) -> i64 {
        self.named(file_system::OPENDIR, handle, path, rights::PARENT_RIGHTS)
            .0
    }
}

fn error_word(e: ErrorNumber) -> i64 {
    error::word(e)
}

#[test]
fn every_mutating_verb_is_refused_read_only() {
    let mutating: Vec<u64> = (verb::FIRST..=verb::LAST)
        .filter(|&operation| verb::of(operation).is_some_and(|v| v.mutates()))
        .collect();
    assert_eq!(
        mutating,
        [
            file_system::WRITE,
            file_system::CREATE,
            file_system::TRUNCATE,
            file_system::MKDIR,
            file_system::RENAME,
            file_system::UNLINK,
            file_system::RMDIR,
            file_system::SETXATTR,
            file_system::REMOVEXATTR,
            file_system::SYNC,
            file_system::SET_MODIFICATION_TIME,
            file_system::SET_MODIFICATION_TIME_AT_UNIX_SECONDS,
        ]
    );
    let mut c = Client::new();
    for operation in mutating {
        // On the root, on a handle that does not exist, and with a name: always EROFS.
        assert_eq!(
            c.named(operation, file_system::ROOT, b"docs", 0).0,
            error_word(directory::EROFS)
        );
        assert_eq!(c.call(operation, 999, 0, 0).0, error_word(directory::EROFS));
    }
}

#[test]
fn binding_verbs_and_unknown_codes_are_invalid() {
    let mut c = Client::new();
    for operation in [
        file_system::BIND,
        file_system::UNBIND,
        file_system::BIND_SYNC,
        40,
        200,
    ] {
        assert_eq!(
            c.call(operation, file_system::ROOT, 0, 0).0,
            error_word(directory::EINVAL)
        );
    }
}

#[test]
fn open_reads_a_file_by_path() {
    let mut c = Client::new();
    let (h, size) = c.named(
        file_system::OPEN,
        file_system::ROOT,
        b"docs/a.txt",
        rights::READ,
    );
    assert!(h > 0);
    assert_eq!(size, 11);
    assert_eq!(c.call(file_system::FSTAT, h as u64, 0, 0).0, 11);
    let n = c.call(file_system::READ, h as u64, 5, 7).0;
    assert_eq!(&c.window[..n as usize], b"ext4");
    assert_eq!(c.call(file_system::READ, h as u64, 100, 11).0, 0);
    assert_eq!(c.call(file_system::CLOSE, h as u64, 0, 0).0, 0);
    assert_eq!(
        c.call(file_system::READ, h as u64, 5, 0).0,
        error_word(BAD_HANDLE)
    );
    assert_eq!(
        c.call(file_system::CLOSE, h as u64, 0, 0).0,
        error_word(BAD_HANDLE)
    );
    // A path's steps carry only DESCEND and what w1 asks for, so a walk asking nothing more
    // lands where READ is gone.
    let bare = c
        .named(file_system::OPEN, file_system::ROOT, b"docs/a.txt", 0)
        .0;
    assert_eq!(bare, error_word(NO_SUCH_ENTRY));
}

#[test]
fn open_refuses_what_is_not_a_regular_file() {
    let mut c = Client::new();
    let root = file_system::ROOT;
    assert_eq!(
        c.named(file_system::OPEN, root, b"docs", 0).0,
        error_word(directory::EISDIR)
    );
    assert_eq!(
        c.named(file_system::OPEN, root, b"link", 0).0,
        error_word(TOO_MANY_SYMBOLIC_LINKS)
    );
    assert_eq!(
        c.named(file_system::OPEN, root, b"link/a.txt", rights::READ)
            .0,
        error_word(TOO_MANY_SYMBOLIC_LINKS)
    );
    assert_eq!(
        c.named(file_system::OPEN, root, b"pipe", 0).0,
        error_word(NO_SUCH_DEVICE)
    );
    assert_eq!(
        c.named(file_system::OPEN, root, b"pipe/x", 0).0,
        error_word(directory::ENOTDIR)
    );
    assert_eq!(
        c.named(file_system::OPEN, root, b"missing", 0).0,
        error_word(NO_SUCH_ENTRY)
    );
    assert_eq!(
        c.named(file_system::OPEN, root, b"../docs", 0).0,
        error_word(directory::EINVAL)
    );
    assert_eq!(
        c.named(file_system::OPEN, root, b"", 0).0,
        error_word(directory::EINVAL)
    );
    let store = extended_attribute::STORE_DIRECTORY.as_bytes();
    assert_eq!(
        c.named(file_system::OPEN, root, store, 0).0,
        error_word(directory::EINVAL)
    );
}

#[test]
fn a_path_cannot_ask_for_a_write_right_the_root_lacks() {
    let mut c = Client::new();
    let asked = c.named(
        file_system::OPEN,
        file_system::ROOT,
        b"docs/a.txt",
        rights::WRITE,
    );
    assert_eq!(asked.0, error_word(directory::EPERM));
    let asked = c.named(
        file_system::OPENDIR,
        file_system::ROOT,
        b"docs",
        rights::ALL,
    );
    assert_eq!(asked.0, error_word(directory::EPERM));
}

#[test]
fn read_directory_is_sorted_and_hides_dot_entries_and_the_store() {
    let mut c = Client::new();
    let n = c.call(file_system::READDIR, file_system::ROOT, 0, 0).0;
    let names: Vec<(Vec<u8>, bool)> = directory_entry::iter(&c.window[..n as usize])
        .map(|(name, d)| (name.to_vec(), d))
        .collect();
    assert_eq!(
        names,
        [
            (b"docs".to_vec(), true),
            (b"link".to_vec(), false),
            (b"old".to_vec(), false),
            (b"pipe".to_vec(), false),
        ]
    );
    // The cursor counts entries: from 2 on is the last two.
    let n = c.call(file_system::READDIR, file_system::ROOT, 0, 2).0;
    let rest: Vec<&[u8]> = directory_entry::iter(&c.window[..n as usize])
        .map(|(n, _)| n)
        .collect();
    assert_eq!(rest, [&b"old"[..], b"pipe"]);
    assert_eq!(c.call(file_system::READDIR, file_system::ROOT, 0, 99).0, 0);
}

#[test]
fn a_directory_handle_carries_no_more_than_it_was_narrowed_to() {
    let mut c = Client::new();
    let docs = c.open_directory(file_system::ROOT, b"docs");
    assert!(docs > 0);
    let blind = c
        .call(file_system::NARROW, docs as u64, 0, rights::DESCEND)
        .0;
    assert!(blind > 0);
    assert_eq!(
        c.call(file_system::READDIR, blind as u64, 0, 0).0,
        error_word(directory::EPERM)
    );
    assert_eq!(
        c.named(file_system::OPEN, blind as u64, b"a.txt", rights::READ)
            .0,
        error_word(NO_SUCH_ENTRY)
    );
    let widen = c.call(file_system::NARROW, blind as u64, 0, rights::READ).0;
    assert_eq!(widen, error_word(directory::EPERM));
    let (file, _) = c.named(file_system::OPEN, docs as u64, b"a.txt", rights::READ);
    assert_eq!(
        c.call(file_system::NARROW, file as u64, 0, 0).0,
        error_word(directory::ENOTDIR)
    );
    assert_eq!(
        c.call(file_system::READ, docs as u64, 4, 0).0,
        error_word(directory::EISDIR)
    );
    assert_eq!(
        c.call(file_system::CLOSE, file_system::ROOT, 0, 0).0,
        error_word(directory::EINVAL)
    );
    assert_eq!(
        c.open_directory(file_system::ROOT, b"old"),
        error_word(directory::ENOTDIR)
    );
    assert_eq!(
        c.open_directory(file_system::ROOT, b"link"),
        error_word(TOO_MANY_SYMBOLIC_LINKS)
    );
}

#[test]
fn extended_attributes_are_listed_sorted_and_read() {
    let mut c = Client::new();
    let (h, _) = c.named(
        file_system::OPEN,
        file_system::ROOT,
        b"docs/a.txt",
        rights::READ,
    );
    let n = c.call(file_system::LISTXATTR, h as u64, 0, 0).0;
    let names: Vec<&[u8]> = extended_attribute::list::iter(&c.window[..n as usize]).collect();
    assert_eq!(names, [&b"security.selinux"[..], b"user.comment"]);
    let r = c
        .named(file_system::GETXATTR, h as u64, b"user.comment", 0)
        .0;
    assert_eq!(extended_attribute::reply_kind(r), extended_attribute::RAW);
    let len = extended_attribute::reply_value_len(r);
    assert_eq!(&c.window[..len], b"first");
    let missing = c.named(file_system::GETXATTR, h as u64, b"user.none", 0).0;
    assert_eq!(missing, error_word(extended_attribute::ENODATA));
    let empty = c.call(file_system::GETXATTR, h as u64, 0, 0).0;
    assert_eq!(empty, error_word(extended_attribute::ERANGE));
    // A directory handle without READ cannot read attributes.
    let blind = c
        .call(file_system::NARROW, file_system::ROOT, 0, rights::DESCEND)
        .0;
    assert_eq!(
        c.call(file_system::LISTXATTR, blind as u64, 0, 0).0,
        error_word(BAD_HANDLE)
    );
}

#[test]
fn volume_statistics_and_modification_times() {
    let mut c = Client::new();
    let n = c
        .call(file_system::GET_VOLUME_STATISTICS, file_system::ROOT, 0, 0)
        .0;
    assert_eq!(n as usize, volume_statistics::RECORD_BYTES);
    assert_eq!(
        volume_statistics::decode(&c.window),
        Some((4096, 1000, 250))
    );
    let time = |c: &mut Client, name: &[u8]| {
        c.named(
            file_system::GET_MODIFICATION_TIME_UNIX_SECONDS,
            file_system::ROOT,
            name,
            0,
        )
        .0
    };
    assert_eq!(time(&mut c, b"docs"), 200);
    assert_eq!(time(&mut c, b"link"), 0);
    assert_eq!(time(&mut c, b"old"), error_word(VALUE_TOO_LARGE));
    assert_eq!(time(&mut c, b"missing"), error_word(NO_SUCH_ENTRY));
    assert_eq!(time(&mut c, b"docs/a.txt"), error_word(directory::EINVAL));
}

#[test]
fn handles_run_out_at_the_limit() {
    let mut c = Client::new();
    for _ in 1..MAXIMUM_OPEN_HANDLES {
        assert!(c.open_directory(file_system::ROOT, b"docs") > 0);
    }
    assert_eq!(
        c.open_directory(file_system::ROOT, b"docs"),
        error_word(TOO_MANY_OPEN_HANDLES)
    );
}

#[test]
fn a_short_window_is_never_read_past() {
    let mut server = Server::new(Fake::new());
    let mut window = *b"docs/a.txt";
    let w0 = file_system::request(file_system::OPEN, file_system::ROOT, 4096);
    // The length asks for a page; the window is 10 bytes, so the name is those 10.
    assert_eq!(server.handle(w0, rights::READ, &mut window).1, 11);
}

/// A device of `blocks` blocks, block `i` filled with byte `i`, counting its reads.
struct Device {
    blocks: u64,
    reads: usize,
}

impl BlockDevice for Device {
    fn read_block(
        &mut self,
        index: u64,
        out: &mut [u8; block::BLOCK_BYTES],
    ) -> Result<(), ErrorNumber> {
        if index >= self.blocks {
            return Err(INPUT_OUTPUT_ERROR);
        }
        self.reads += 1;
        out.fill(index as u8);
        Ok(())
    }

    fn size_bytes(&self) -> u64 {
        self.blocks * block::BLOCK_BYTES as u64
    }
}

#[test]
fn sectors_of_512_bytes_span_blocks() {
    let device = Device {
        blocks: 4,
        reads: 0,
    };
    let mut sectors = Sectors::new(device, 512).unwrap();
    assert_eq!(sectors.sectors(), 32);
    let mut out = [0u8; 1024];
    // Sectors 7 and 8: the last of block 0 and the first of block 1.
    sectors.read(7, 2, &mut out).unwrap();
    assert!(out[..512].iter().all(|&b| b == 0));
    assert!(out[512..].iter().all(|&b| b == 1));
    assert_eq!(sectors.read(31, 2, &mut out), Err(INPUT_OUTPUT_ERROR));
    assert_eq!(sectors.read(0, 3, &mut out), Err(INPUT_OUTPUT_ERROR));
    assert_eq!(sectors.write(0, 1, &out), Err(directory::EROFS));
}

#[test]
fn sectors_of_4096_bytes_are_blocks_and_repeats_are_cached() {
    let device = Device {
        blocks: 2,
        reads: 0,
    };
    let mut sectors = Sectors::new(device, 4096).unwrap();
    let mut out = [0u8; 4096];
    sectors.read(1, 1, &mut out).unwrap();
    sectors.read(1, 1, &mut out).unwrap();
    assert!(out.iter().all(|&b| b == 1));
    assert_eq!(sectors.device_reads(), 1);
}

#[test]
fn sector_sizes_are_checked() {
    for bad in [0, 256, 768, 8192] {
        assert!(
            Sectors::new(
                Device {
                    blocks: 1,
                    reads: 0
                },
                bad
            )
            .is_none()
        );
    }
    for good in [512, 1024, 2048, 4096] {
        assert!(
            Sectors::new(
                Device {
                    blocks: 1,
                    reads: 0
                },
                good
            )
            .is_some()
        );
    }
}

impl Sectors<Device> {
    fn device_reads(&self) -> usize {
        self.device().reads
    }
}

//! Fuzz the RedoxFS file server's **request dispatch** with whole client sessions, and check the
//! subtree rule after every request (proposal #1592 part a, rank 1).
//!
//! `redoxfs_server::Server::handle`, which the EL0 binary's serve loop calls per request, runs over
//! a host-built RedoxFS image. A session is a sequence of `(badge, w0, w1, window bytes)` requests as
//! the kernel would deliver them. The rules are confinement claims, whose breach answers with
//! success and crashes nothing, so a crash-only fuzzer would miss both. The image is formatted once
//! and every input reopens it through a copy-on-write disk ([`Overlay`]);
//! `notes/fuzzing-the-services.md` records what this layout runs.

//! # The rules it checks, beyond "no panic"
//!
//! 1. *Admission.* A revoked badge, and any nonzero badge the server has no window for (milestone
//!    726 (an unknown badge fails closed in subtree_scope)), succeeds at nothing. A bound badge
//!    succeeds only through `ROOT` or a handle it minted, never binds or unbinds, and never closes
//!    `ROOT`. The model of who holds what is this file's own, kept from the replies.
//! 2. *Content.* A badge bound by the prologue never reads a file outside its grant: every file
//!    holds a marker naming where it sits, which no session write can spell. The rule stands down
//!    after an open badge's first successful `RENAME`, which can carry outside bytes in.
//!
//! Name: provisional (`lane/fuzz-service-handlers`, 2026-10-04 UTC); an architect names things.
//! Falsification: replayable `fuzz/falsifications/redoxfs_server_session.unknown_badge_fails_open.patch`

#![no_main]

use std::collections::HashMap;
use std::sync::OnceLock;

use filesystem_contract::{dir, fs, xattr};
use libfuzzer_sys::arbitrary::{Result as ArbResult, Unstructured};
use libfuzzer_sys::fuzz_target;
use redoxfs::{BLOCK_SIZE, Disk, FileSystem, Node, TreePtr};
use redoxfs_server::engine_error::{EIO, Error, Result};
use redoxfs_server::{ServeEdges, Server};

const BLOCK: usize = BLOCK_SIZE as usize;
/// The image size. RedoxFS's own minimum is well under this; it matches the proposal's measurement.
const IMAGE: usize = 4 * 1024 * 1024;
/// Requests per session after the prologue. Long enough to bind, mint, close and reuse a slot.
const MAX_REQUESTS: usize = 48;

/// **The largest end a session lets a file reach, below the server's own refusal.** A recorded
/// limitation, not a rule: shrinking a file costs time linear in its logical size (the `BUGS`
/// section of `Server::truncate`), so a session that grew a file to terabytes would time out on
/// every run and hide everything after it. A `WRITE` end or `TRUNCATE` size between this and
/// `MAX_FILE_END` is folded below it; past `MAX_FILE_END` it is sent as drawn, so the `EFBIG`
/// refusals stay exercised. Drop this when the proposal that `BUGS` entry names is built.
const SPARSE_LIMIT: u64 = 64 << 20;

fn sparse(code: u64, len: usize, w1: u64) -> u64 {
    let end = match code {
        fs::WRITE => w1.saturating_add(len as u64),
        fs::TRUNCATE => w1,
        _ => return w1,
    };
    if end > SPARSE_LIMIT && end <= redoxfs_server::MAX_FILE_END {
        w1 % SPARSE_LIMIT
    } else {
        w1
    }
}

/// Markers, each filled through a whole file. Eight bytes, so any read of fifteen or more bytes of
/// the file contains one whole, and a session's writes (a single repeated byte) cannot spell one.
const MARK_ROOT: &[u8] = b"ROOTONLY";
const MARK_D: &[u8] = b"DIRDONLY";
const MARK_E: &[u8] = b"DIREONLY";

/// The format, once: `f` at the root, `d/g`, and `d/e/h`, each holding its marker.
fn template() -> &'static [u8] {
    static IMAGE_BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    IMAGE_BYTES.get_or_init(|| {
        let mut fs = FileSystem::create(VecDisk(vec![0; IMAGE]), None, 0, 0).expect("create");
        fs.tx(|tx| {
            let file = |tx: &mut redoxfs::Transaction<VecDisk>, parent, name: &str, mark: &[u8]| {
                let body: Vec<u8> = mark.iter().copied().cycle().take(mark.len() * 64).collect();
                let ptr = tx
                    .create_node(parent, name, Node::MODE_FILE | 0o644, 0, 0)?
                    .ptr();
                tx.write_node(ptr, 0, &body, 0, 0).map(|_| ())
            };
            file(tx, TreePtr::root(), "f", MARK_ROOT)?;
            let d = tx
                .create_node(TreePtr::root(), "d", Node::MODE_DIR | 0o755, 0, 0)?
                .ptr();
            file(tx, d, "g", MARK_D)?;
            let e = tx.create_node(d, "e", Node::MODE_DIR | 0o755, 0, 0)?.ptr();
            file(tx, e, "h", MARK_E)?;
            Ok(())
        })
        .expect("populate");
        fs.disk.0
    })
}

/// A plain disk over a `Vec`, for the one format.
struct VecDisk(Vec<u8>);

impl Disk for VecDisk {
    unsafe fn read_at(&mut self, block: u64, buffer: &mut [u8]) -> Result<usize> {
        let off = block as usize * BLOCK;
        let src = self.0.get(off..off + buffer.len()).ok_or(Error::new(EIO))?;
        buffer.copy_from_slice(src);
        Ok(buffer.len())
    }
    unsafe fn write_at(&mut self, block: u64, buffer: &[u8]) -> Result<usize> {
        let off = block as usize * BLOCK;
        let dst = self
            .0
            .get_mut(off..off + buffer.len())
            .ok_or(Error::new(EIO))?;
        dst.copy_from_slice(buffer);
        Ok(buffer.len())
    }
    fn size(&mut self) -> Result<u64> {
        Ok(self.0.len() as u64)
    }
}

/// **The template, copy-on-write.** Reads come from the shared format unless this input wrote the
/// block; writes land in `dirty`. An input pays for the blocks it touched, not for the image.
struct Overlay {
    base: &'static [u8],
    dirty: HashMap<usize, Box<[u8; BLOCK]>>,
}

impl Overlay {
    fn block(&self, b: usize) -> Option<&[u8]> {
        match self.dirty.get(&b) {
            Some(d) => Some(&d[..]),
            None => self.base.get(b * BLOCK..(b + 1) * BLOCK),
        }
    }
}

impl Disk for Overlay {
    unsafe fn read_at(&mut self, block: u64, buffer: &mut [u8]) -> Result<usize> {
        for (i, chunk) in buffer.chunks_mut(BLOCK).enumerate() {
            let src = self.block(block as usize + i).ok_or(Error::new(EIO))?;
            chunk.copy_from_slice(&src[..chunk.len()]);
        }
        Ok(buffer.len())
    }
    unsafe fn write_at(&mut self, block: u64, buffer: &[u8]) -> Result<usize> {
        for (i, chunk) in buffer.chunks(BLOCK).enumerate() {
            let b = block as usize + i;
            let base = self
                .base
                .get(b * BLOCK..(b + 1) * BLOCK)
                .ok_or(Error::new(EIO))?;
            let slot = self.dirty.entry(b).or_insert_with(|| {
                let mut copy = Box::new([0u8; BLOCK]);
                copy.copy_from_slice(base);
                copy
            });
            slot[..chunk.len()].copy_from_slice(chunk);
        }
        Ok(buffer.len())
    }
    fn size(&mut self) -> Result<u64> {
        Ok(self.base.len() as u64)
    }
}

/// The host's IO edges: no crash injector, and a flush that always succeeds.
struct HostEdges;
impl ServeEdges for HostEdges {
    fn sync(&mut self) -> i64 {
        1
    }
}

/// What the model believes one badge is. Kept from the replies, never read from the server.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Model {
    Open,
    Bound,
    Revoked,
}

struct Session {
    server: Server<Overlay>,
    window: Vec<u8>,
    /// Badges 1..CLIENT_WINDOWS; badge 0 is always open and any other badge is unknown.
    badges: [Model; fs::CLIENT_WINDOWS],
    /// Which badge minted each live handle (0 for an open badge's).
    owners: HashMap<u64, u64>,
    /// The markers each prologue-bound badge must never read. Cleared when its grant changes.
    forbidden: [&'static [&'static [u8]]; fs::CLIENT_WINDOWS],
    /// Whether the content rule still holds (see the module comment).
    content_rule: bool,
}

/// The names a session uses: the template's own, the walk's refusals, and a little noise.
const NAMES: &[&[u8]] = &[
    b"f", b"g", b"h", b"d", b"e", b"d/g", b"d/e", b"d/e/h", b"n", b"d/n", b"..", b".", b"d/..",
    b"../f", b"/f", b"e/../g", b"", b"n\0", b"user.k",
];

const VERBS: &[u64] = &[
    fs::OPEN,
    fs::READ,
    fs::WRITE,
    fs::CLOSE,
    fs::FSTAT,
    fs::CREATE,
    fs::TRUNCATE,
    fs::OPENDIR,
    fs::READDIR,
    fs::MKDIR,
    fs::RENAME,
    fs::UNLINK,
    fs::RMDIR,
    fs::GETXATTR,
    fs::SETXATTR,
    fs::LISTXATTR,
    fs::REMOVEXATTR,
    fs::STATFS,
    fs::SYNC,
    fs::GETMTIME,
    fs::SETMTIME,
    fs::SETMTIME_AT,
    fs::BIND,
    fs::UNBIND,
];

impl Session {
    fn model(&self, badge: u64) -> Model {
        match badge {
            0 => Model::Open,
            b if b < fs::CLIENT_WINDOWS as u64 => self.badges[b as usize],
            _ => Model::Revoked,
        }
    }

    /// Lay a name into the window and return its length.
    fn name(&mut self, u: &mut Unstructured, at: usize) -> ArbResult<usize> {
        let bytes: Vec<u8> = if u.ratio(1, 8)? {
            let n = u.int_in_range(0..=32)?;
            u.bytes(n)?.to_vec()
        } else {
            NAMES[u.choose_index(NAMES.len())?].to_vec()
        };
        self.window[at..at + bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }

    fn badge(u: &mut Unstructured) -> ArbResult<u64> {
        Ok(match u.int_in_range(0u8..=10)? {
            9 => u64::MAX,
            10 => 0,
            b => b as u64,
        })
    }

    fn rights(u: &mut Unstructured) -> ArbResult<u64> {
        Ok(if u.ratio(3, 4)? {
            dir::ALL
        } else {
            u.int_in_range(0..=dir::ALL)?
        })
    }

    /// Draw one request, lay its payload into the window, and return `(badge, w0, w1)`.
    fn draw(&mut self, u: &mut Unstructured) -> ArbResult<(u64, u64, u64)> {
        let badge = Self::badge(u)?;
        let code = if u.ratio(1, 32)? {
            u.int_in_range(0..=255)?
        } else {
            VERBS[u.choose_index(VERBS.len())?]
        };
        let handle = u.int_in_range(0u64..=11)?;
        let small = u.int_in_range(0u64..=9000)?;
        let (len, w1) = match code {
            fs::OPEN | fs::OPENDIR | fs::MKDIR => (self.name(u, 0)?, Self::rights(u)?),
            fs::CREATE | fs::UNLINK | fs::RMDIR | fs::GETMTIME | fs::SETMTIME => {
                (self.name(u, 0)?, 0)
            }
            fs::SETMTIME_AT => (self.name(u, 0)?, small),
            fs::READ => (small as usize, u.int_in_range(0..=600)?),
            fs::WRITE => {
                let fill = u.arbitrary::<u8>()?;
                self.window[..small as usize].fill(fill);
                (small as usize, u.int_in_range(0..=600)?)
            }
            fs::RENAME => {
                let src = self.name(u, 0)?;
                let dst = self.name(u, src)?;
                (src, fs::rename_dst(u.int_in_range(0..=11)?, dst as u64))
            }
            fs::GETXATTR | fs::REMOVEXATTR => (self.name(u, 0)?, 0),
            fs::SETXATTR => {
                let name = self.name(u, 0)?;
                let value = u.int_in_range(0usize..=40)?;
                let kind = u.int_in_range(0u32..=3)?;
                (name, xattr::spec(kind, value as u64))
            }
            fs::BIND | fs::UNBIND => (0, Self::badge(u)?),
            _ => (u.int_in_range(0..=64)?, small),
        };
        let w1 = if u.ratio(1, 64)? { u.arbitrary()? } else { w1 };
        Ok((
            badge,
            fs::req(code, handle, len as u64),
            sparse(code, len, w1),
        ))
    }

    /// Send one request through the dispatch, check the rules, and update the model.
    fn send(&mut self, badge: u64, w0: u64, w1: u64) -> i64 {
        let (r0, _) = self
            .server
            .handle(badge, w0, w1, &mut self.window, &mut HostEdges);
        let code = filesystem_contract::operation(w0);
        let raw = fs::req_handle(w0);
        let ok = r0 >= 0;
        let model = self.model(badge);

        // Rule 1, admission.
        let mine = |h: u64| h == fs::ROOT || self.owners.get(&h) == Some(&badge);
        let must_fail = match model {
            Model::Open => false,
            Model::Revoked => true,
            Model::Bound => {
                code == fs::BIND
                    || code == fs::UNBIND
                    || (code == fs::CLOSE && raw == fs::ROOT)
                    || !mine(raw)
                    || (code == fs::RENAME && !mine(fs::dst_handle(w1)))
            }
        };
        assert!(
            !(ok && must_fail),
            "badge {badge} ({model:?}) succeeded at op {code} on handle {raw} (w1 {w1:#x}) -> {r0}"
        );

        // Rule 2, content.
        if ok && code == fs::READ && self.content_rule && (badge as usize) < fs::CLIENT_WINDOWS {
            let read = &self.window[..r0 as usize];
            for mark in self.forbidden[badge as usize] {
                assert!(
                    !read.windows(mark.len()).any(|w| w == *mark),
                    "badge {badge} read {} from outside its grant",
                    String::from_utf8_lossy(mark)
                );
            }
        }

        if !ok {
            return r0;
        }
        match code {
            fs::OPEN | fs::OPENDIR | fs::CREATE | fs::MKDIR => {
                assert_ne!(r0 as u64, fs::ROOT, "a minted handle shadowed ROOT");
                let owner = if model == Model::Bound { badge } else { 0 };
                self.owners.insert(r0 as u64, owner);
            }
            fs::CLOSE => {
                self.owners.remove(&raw);
            }
            fs::RENAME if model == Model::Open => self.content_rule = false,
            fs::BIND => {
                self.badges[w1 as usize] = Model::Bound;
                self.forbidden[w1 as usize] = &[];
            }
            fs::UNBIND => {
                self.badges[w1 as usize] = Model::Revoked;
                self.forbidden[w1 as usize] = &[];
                self.owners.retain(|_, b| *b != w1);
            }
            _ => {}
        }
        r0
    }
}

fn session(u: &mut Unstructured) -> ArbResult<()> {
    let disk = Overlay {
        base: template(),
        dirty: HashMap::new(),
    };
    let mut s = Session {
        server: Server::open(disk).expect("the template opens"),
        window: vec![0; fs::TRANSFER_MAX],
        badges: [Model::Open; fs::CLIENT_WINDOWS],
        owners: HashMap::new(),
        forbidden: [&[]; fs::CLIENT_WINDOWS],
        content_rule: true,
    };

    // The prologue: the caretaker (badge 0) opens `d` and `d/e` and binds badges 1 and 2 to them,
    // so every session starts with two confined clients whose grants the content rule knows.
    for (path, badge, forbidden) in [
        (&b"d"[..], 1u64, &[MARK_ROOT][..]),
        (&b"d/e"[..], 2, &[MARK_ROOT, MARK_D][..]),
    ] {
        s.window[..path.len()].copy_from_slice(path);
        let h = s.send(
            0,
            fs::req(fs::OPENDIR, fs::ROOT, path.len() as u64),
            dir::ALL,
        );
        assert!(h > 0, "the prologue's OPENDIR failed: {h}");
        assert_eq!(
            s.send(0, fs::req(fs::BIND, h as u64, 0), badge),
            0,
            "the prologue's BIND failed"
        );
        s.forbidden[badge as usize] = forbidden;
    }

    for _ in 0..MAX_REQUESTS {
        if u.is_empty() {
            break;
        }
        let (badge, w0, w1) = s.draw(u)?;
        s.send(badge, w0, w1);
    }
    Ok(())
}

fuzz_target!(|data: &[u8]| {
    let _ = session(&mut Unstructured::new(data));
});

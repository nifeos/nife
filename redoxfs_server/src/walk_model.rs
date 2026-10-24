//! A directory walk, replayed against the server core on the host (milestone 606 (a directory
//! walk costs what it does on Linux), provisional number).
//!
//! The request sequence `std`'s nife PAL sends for `walk_pricing::walk`, issued straight at
//! [`Server`] over a [`Counting`] disk. It answers what the on-target figure cannot separate: how
//! many device reads a walk costs, exactly (`tests/walk_cost.rs`: a warm walk reads nothing from
//! the device), and what the server's own work costs (`examples/walk_replay.rs`). See
//! notes/walk-cost.md.

//! # BUGS
//!
//! - The request sequence is a model of the PAL, written from
//!   `patches/std-nife/overlay/std/src/sys/fs/nife.rs` (`walk`, `dir_at`, `readdir`, `File::open`,
//!   `read_to_end`), and nothing checks that the two agree. The on-target `walk split` line is the
//!   check on it.
//! - Staged through the server, not by `redoxfs_host`, so the block layout differs from the
//!   on-target image's: counts of requests are the same, counts of device reads can differ.
//!
//! Name: provisional 2026-09-26 (milestone 606's lane); calef names modules.

use alloc::vec::Vec;

use filesystem_contract::dir;
use filesystem_fixture::walk as tree;
use redoxfs::Disk;
use syscall::error::Result;

use crate::{BLOCK, NameCopy, Server};

/// The model's names are its own constants and joined paths, copied into the [`NameCopy`] every
/// verb takes, exactly as the dispatch copies a client's.
fn copy(name: &str) -> Result<NameCopy> {
    NameCopy::copy_in(name.as_bytes())
}

/// A [`Disk`] that counts what reaches it: the calls, and the blocks they moved. Put it
/// *under* [`crate::CachedDisk`] and it counts the device reads a cache did not answer.
pub struct Counting<D> {
    inner: D,
    /// `read_at` calls that reached this disk.
    pub reads: u64,
    /// Blocks those calls read.
    pub blocks_read: u64,
}

impl<D> Counting<D> {
    /// Wrap `inner` with both counters at zero.
    pub fn new(inner: D) -> Self {
        Self {
            inner,
            reads: 0,
            blocks_read: 0,
        }
    }
}

impl<D: Disk> Disk for Counting<D> {
    unsafe fn read_at(&mut self, block: u64, buffer: &mut [u8]) -> Result<usize> {
        self.reads += 1;
        self.blocks_read += buffer.len().div_ceil(BLOCK) as u64;
        // SAFETY: forwarded from this method's own caller, on the same buffer.
        unsafe { self.inner.read_at(block, buffer) }
    }

    unsafe fn write_at(&mut self, block: u64, buffer: &[u8]) -> Result<usize> {
        // SAFETY: as above.
        unsafe { self.inner.write_at(block, buffer) }
    }

    fn size(&mut self) -> Result<u64> {
        self.inner.size()
    }
}

/// The requests one walk sent, by verb. Their sum is the walk's round trips through the FS
/// server, and on nife each is two IPC round trips (client to caretaker, caretaker to server).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Requests {
    pub opendir: u64,
    pub readdir: u64,
    pub open: u64,
    pub fstat: u64,
    pub read: u64,
    pub close: u64,
}

impl Requests {
    /// Every request, summed.
    pub fn total(&self) -> u64 {
        self.opendir + self.readdir + self.open + self.fstat + self.read + self.close
    }
}

/// What a replayed walk found: the same counts `walk_pricing::Totals` reports on target, so the
/// two can be checked against each other and against the fixture.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Walked {
    pub entries: usize,
    pub files: usize,
    pub bytes: usize,
    pub components: usize,
    pub requests: Requests,
}

/// **Build [`tree`] under `parent`**, named [`tree::ROOT`], through the server's own verbs, and
/// return nothing: a walk opens it the way the caretaker does, by name.
pub fn stage<D: Disk>(srv: &mut Server<D>, parent: u32) -> Result<()> {
    let root = srv.make_dir(parent, &copy(tree::ROOT)?, dir::ALL)?;
    let file = |srv: &mut Server<D>, at: u32, name: &str, body: &[u8]| -> Result<()> {
        let h = srv.create_file_at(at, &copy(name)?)?;
        let mut off = 0;
        while off < body.len() {
            let end = (off + BLOCK).min(body.len());
            srv.write(h, off as u64, &body[off..end])?;
            off = end;
        }
        srv.close(h)
    };
    let chain = srv.make_dir(root, &copy(tree::CHAIN)?, dir::ALL)?;
    let mut level = chain;
    for i in 0..=tree::DEPTH {
        if i > 0 {
            let next = srv.make_dir(level, &copy(tree::LEVEL)?, dir::ALL)?;
            if level != chain {
                srv.close(level)?;
            }
            level = next;
        }
        file(srv, level, tree::LEVEL_FILE, tree::SMALL_BODY)?;
    }
    srv.close(level)?;
    if level != chain {
        srv.close(chain)?;
    }
    let wide = srv.make_dir(root, &copy(tree::WIDE)?, dir::ALL)?;
    for i in 0..tree::WIDE_COUNT {
        let name = tree::wide_name(i);
        file(
            srv,
            wide,
            core::str::from_utf8(&name).unwrap_or("?"),
            tree::SMALL_BODY,
        )?;
    }
    srv.close(wide)?;
    let narrow = srv.make_dir(root, &copy(tree::NARROW)?, dir::ALL)?;
    let first = tree::wide_name(0);
    file(
        srv,
        narrow,
        core::str::from_utf8(&first).unwrap_or("?"),
        tree::SMALL_BODY,
    )?;
    srv.close(narrow)?;
    let sizes = srv.make_dir(root, &copy(tree::SIZES)?, dir::ALL)?;
    for (name, len) in tree::SIZE_FILES {
        let body: Vec<u8> = (0..len).map(tree::sized_byte).collect();
        file(srv, sizes, name, &body)?;
    }
    srv.close(sizes)?;
    srv.close(root)
}

/// The rights the PAL asks for: `ENUMERATE` on a directory it lists, `READ` in an `OPEN`'s second
/// word. The server adds `DESCEND` for every step on the way (ruling A).
const LIST: u64 = dir::ENUMERATE;
const OPEN: u64 = dir::READ;

/// **Replay `walk_pricing::walk` over the tree at `root`**, a directory handle holding at least
/// `ENUMERATE | READ | DESCEND`, as the PAL would send it through a caretaker bound there.
pub fn walk<D: Disk>(srv: &mut Server<D>, root: u32) -> Result<Walked> {
    let mut w = Walked::default();
    let mut path: Vec<Vec<u8>> = Vec::new();
    walk_from(srv, root, &mut path, &mut w)?;
    Ok(w)
}

/// `dir_at` in the PAL: one `OPENDIR` per component asking for `want`, closing each hop as the
/// next one lands, so at most two handles are open. Returns the handle, or `root` for the grant.
fn descend<D: Disk>(
    srv: &mut Server<D>,
    root: u32,
    path: &[Vec<u8>],
    want: u64,
    r: &mut Requests,
) -> Result<u32> {
    // Since milestone 606's ruling A the PAL sends the whole path in one `OPENDIR`.
    if path.is_empty() {
        return Ok(root);
    }
    r.opendir += 1;
    srv.open_dir(root, &copy(&joined(path))?, want)
}

/// The names joined by `/`, as the PAL sends a path.
fn joined(path: &[Vec<u8>]) -> alloc::string::String {
    let mut out = alloc::string::String::new();
    for (i, name) in path.iter().enumerate() {
        if i > 0 {
            out.push('/');
        }
        out.push_str(core::str::from_utf8(name).unwrap_or("?"));
    }
    out
}

fn walk_from<D: Disk>(
    srv: &mut Server<D>,
    root: u32,
    path: &mut Vec<Vec<u8>>,
    w: &mut Walked,
) -> Result<()> {
    // `readdir`: the whole listing drained up front, then the handle closed.
    let at = descend(srv, root, path, LIST, &mut w.requests)?;
    let mut entries: Vec<(Vec<u8>, bool)> = Vec::new();
    let mut page = [0u8; filesystem_contract::PAGE];
    loop {
        w.requests.readdir += 1;
        let n = srv.read_dir(at, entries.len() as u32, &mut page)?;
        if n == 0 {
            break;
        }
        for (name, is_dir) in filesystem_contract::dirent::iter(&page[..n]) {
            entries.push((name.to_vec(), is_dir));
        }
    }
    if at != root {
        w.requests.close += 1;
        srv.close(at)?;
    }

    for (name, is_dir) in entries {
        w.entries += 1;
        w.components += path.len() + 1;
        path.push(name);
        if is_dir {
            walk_from(srv, root, path, w)?;
        } else {
            // `File::open`: one `OPEN` with the whole path and the rights each step asks for
            // (ruling A). The server answers with the size in the reply's second word (ruling B),
            // which is server work but no request, and `read_to_end` sizes its buffer from it.
            w.requests.open += 1;
            let h = srv.open_file_path(root, &copy(&joined(path))?, OPEN)?;
            let _size = srv.fstat(h)?;
            let mut off = 0u64;
            loop {
                w.requests.read += 1;
                let got = srv.read(h, off, &mut page)?;
                if got == 0 {
                    break;
                }
                off += got as u64;
            }
            w.requests.close += 1;
            srv.close(h)?;
            w.bytes += off as usize;
            w.files += 1;
        }
        path.pop();
    }
    Ok(())
}

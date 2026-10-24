//! **Paths in one request (milestone 606, ruling A)**, with the hop-by-hop walk for a server that
//! predates it ([`PATHS`]).
//!
//! A seam of `sys/fs/nife.rs` (§282 (when a file is split, the parent becomes an index)):
//! `pub(super)` here reaches `nife` and its children.
//!
//! Name: provisional (lane maintainer/near-ceiling-splits, 2026-10-12 UTC), for the module and the
//! file.

use super::*;

/// Whether this server resolves a path in one request: 0 = not yet known, 1 = paths work, 2 = this
/// server walks hop by hop.
///
/// A server since milestone 606 (a directory walk costs what it does on Linux), ruling A, resolves
/// a relative path in one `OPEN` or `OPENDIR` with exactly the rights the hop-by-hop walk would get
/// (`filesystem_contract::fs::OPEN` has the rule); an older one answers `EINVAL` to any name with a
/// `/`. So the PAL sends the path and on `EINVAL` walks it instead. If the walk succeeds, the
/// `EINVAL` was the server's age and every later request walks without asking; an `EINVAL` the
/// walk repeats was the name's, and says nothing about the server.
pub(super) static PATHS: AtomicU8 = AtomicU8::new(0);

pub(super) fn paths_offered() -> bool {
    PATHS.load(Ordering::Relaxed) != 2
}

pub(super) fn paths_seen() {
    PATHS.store(1, Ordering::Relaxed);
}

pub(super) fn paths_refused() {
    if PATHS.load(Ordering::Relaxed) == 0 {
        PATHS.store(2, Ordering::Relaxed);
    }
}

/// Whether an error to a one-request path could be an older server's refusal of the `/`.
pub(super) fn refused_as_a_path(e: &io::Error) -> bool {
    PATHS.load(Ordering::Relaxed) == 0 && e.kind() == io::ErrorKind::InvalidInput
}

/// The names in `path` joined by `/`: what one request carries. `count_names` has already refused
/// anything that is not a plain name.
pub(super) fn joined(path: &Path) -> String {
    let mut out = String::new();
    for (i, name) in names(path).enumerate() {
        if i > 0 {
            out.push('/');
        }
        out.push_str(name);
    }
    out
}

/// The [`dir`] rights an `OPEN` needs on the directory holding the name.
///
/// `filesystem_contract::verb::TABLE` records `OPEN` as the contract's one "any of" row (`READ` *or* `WRITE`
/// will do), so this is the one place a caller has to say which, and it says it from the options it
/// was handed. Asking for both when the caller only reads would refuse a read through a read-only
/// grant, which is the over-asking trap one level down from the one [`descend`] describes.
///
/// [`dir`]: fsproto::dir
pub(super) fn open_rights(opts: &OpenOptions) -> u64 {
    let mut want = 0;
    if opts.read {
        want |= fsproto::dir::READ;
    }
    if opts.write || opts.append || opts.truncate {
        want |= fsproto::dir::WRITE;
    }
    want
}

/// `OPEN` the last name in `path`. Returns the server's handle and the size its reply carried (0
/// when it carried none; `filesystem_contract::fs::OPEN` says why 0 means "not given").
///
/// Since ruling A of milestone 606 (a directory walk costs what it does on Linux) a multi-name
/// path is one `OPEN` with the rights each step asks for in the second word, and a server that
/// refuses it gets the hop-by-hop walk ([`PATHS`] says how).
pub(super) fn open_handle(path: &Path, opts: &OpenOptions) -> io::Result<(u64, u64)> {
    if !is_reachable() {
        return Err(unsupported_err());
    }
    let mut p = page();
    let want = open_rights(opts);
    if count_names(path)? > 1 && paths_offered() {
        let joined = joined(path);
        p.put(joined.as_bytes());
        match open_reply(proto::req(proto::OPEN, proto::ROOT, joined.len() as u64), want) {
            Err(e) if refused_as_a_path(&e) => {}
            done => return done.inspect(|_| paths_seen()),
        }
        let (at, name) = walk(&mut p, proto::ROOT, path, want)?;
        p.put(name.as_bytes());
        let opened = open_reply(proto::req(proto::OPEN, at.0, name.len() as u64), want)?;
        paths_refused();
        return Ok(opened);
    }
    let (at, name) = walk(&mut p, proto::ROOT, path, want)?;
    p.put(name.as_bytes());
    open_reply(proto::req(proto::OPEN, at.0, name.len() as u64), want)
}

/// One `OPEN`, keeping both reply words: the handle, and the size ruling B put in the second.
pub(super) fn open_reply(w0: u64, want: u64) -> io::Result<(u64, u64)> {
    let (r0, r1) = rt::call(FS, w0, want);
    Ok((reply(r0)?, r1))
}

/// Create the last name in `path` and return the handle. [`open_handle`]'s twin: the wire shape is
/// identical and only the verb and the walk's rights differ, which is why `CREATE` matches `OPEN`.
///
/// The walk asks for `CREATE` *and* what the caller will do with the file, because a file handle
/// carries `parent & (READ | WRITE)` (`redoxfs_server`'s `create_file_at`): a descent that asked
/// only for `CREATE` would mint a file nobody can write (notes/std/fs-descent.md). `EEXIST` here
/// means somebody created the name after the open reported `NotFound`; it surfaces as
/// `AlreadyExists` rather than being retried, so a lost race never becomes a caller writing over a
/// file it believes it just made.
pub(super) fn create_handle(path: &Path, opts: &OpenOptions) -> io::Result<u64> {
    if !is_reachable() {
        return Err(unsupported_err());
    }
    let mut p = page();
    let (at, name) = walk(
        &mut p,
        proto::ROOT,
        path,
        fsproto::dir::CREATE | open_rights(opts),
    )?;
    p.put(name.as_bytes());
    request(proto::req(proto::CREATE, at.0, name.len() as u64), 0)
}

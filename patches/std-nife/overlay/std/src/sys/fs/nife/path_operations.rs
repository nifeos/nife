//! **Path-level operations**.
//!
//! A seam of `sys/fs/nife.rs` (§282 (when a file is split, the parent becomes an index)):
//! `pub(super)` here reaches `nife` and its children.
//!
//! Name: provisional (lane maintainer/near-ceiling-splits, 2026-10-12 UTC), for the module and the
//! file.

use super::*;

/// A name's metadata: open, `FSTAT`, close, then `GETMTIME`. Four messages instead of one, because
/// the contract has no stat-by-name verb; the authority is identical (the name still resolves only
/// under the granted directory, and all four need the `dir::READ` the walk asks for).
///
/// **A directory answers too** (milestone 64): `OPEN`'s `EISDIR` *is* the answer to "what kind of
/// thing is this name", so `Path::is_dir()` and `create_dir_all`'s recovery from `AlreadyExists`
/// work. `GETMTIME` goes last, so every error this returns comes from the walk, `OPEN` or `FSTAT`;
/// a refused one surfaces from `modified()` (see [`Mtime`]). The four are not atomic: a name
/// replaced between `OPEN` and `GETMTIME` reports the new node's time with the old node's size, the
/// window a Unix program calling `stat` twice has. A directory's size is a placeholder 0.
pub fn stat(path: &Path) -> io::Result<FileAttr> {
    if !is_reachable() {
        return Err(unsupported_err());
    }
    // ``, `.` and `/` name the granted directory, which no `OPEN` can reach: it is the directory
    // the endpoint is bound to rather than a name inside it, and it has no name to ask an mtime
    // by. `count_names` is the same test `readdir` uses.
    if count_names(path)? == 0 {
        return Ok(FileAttr { size: 0, dir: true, mtime: Mtime::Unnamed });
    }
    stat_under(proto::ROOT, path)
}

/// The name-shaped stat against whichever directory handle the caller holds: the root endpoint for
/// the free functions above, a [`Dir`]'s handle for the dirfd-shaped ones below. One mechanism,
/// because upstream's `fstatat(dirfd, ...)` and `stat(path)` differ in exactly the starting handle
/// and nothing else, and two copies of the four-message dance would be one too many places to
/// learn that `EISDIR` is an answer.
pub(super) fn stat_under(at: u64, path: &Path) -> io::Result<FileAttr> {
    let mut p = page();
    let (at, name) = walk(&mut p, at, path, fsproto::dir::READ)?;
    p.put(name.as_bytes());
    let (size, dir) = match request(proto::req(proto::OPEN, at.0, name.len() as u64), 0) {
        Ok(handle) => {
            let size = request(proto::req(proto::FSTAT, handle, 0), 0);
            // Closed before `size?` can return, so an `FSTAT` failure does not leak the handle.
            let _ = request(proto::req(proto::CLOSE, handle, 0), 0);
            (size?, false)
        }
        Err(e) if e.kind() == io::ErrorKind::IsADirectory => (0, true),
        Err(e) => return Err(e),
    };
    // The name again: nothing promises `OPEN`'s reply left the page as it found it.
    p.put(name.as_bytes());
    let (r0, _) = rt::call(FS, proto::req(proto::GETMTIME, at.0, name.len() as u64), 0);
    let mtime = match fsproto::reply_errno(r0 as i64) {
        None => Mtime::Seconds(r0),
        Some(_) => Mtime::Refused(r0),
    };
    Ok(FileAttr { size, dir, mtime })
}

/// No symlinks cross this contract, so following one and not following one are the same thing.
pub fn lstat(path: &Path) -> io::Result<FileAttr> {
    stat(path)
}

pub fn exists(path: &Path) -> io::Result<bool> {
    match stat(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// **List a directory** (`OPENDIR` + `READDIR`, milestone 64), drained here rather than streamed
/// ([`ReadDir`] says why).
///
/// - `read_dir(".")` (or `read_dir("")`) lists **the granted directory itself**, which is handle
///   `fs::ROOT` and costs no `OPENDIR`.
/// - `read_dir("sub")` asks the server for a capability to `sub` first, lists through it, and
///   closes it. That is descend-then-enumerate, and it is the only way to reach `sub`: this
///   process holds no name for it other than through its parent.
pub fn readdir(p: &Path) -> io::Result<ReadDir> {
    if !is_reachable() {
        return Err(unsupported_err());
    }
    // One guard for the whole exchange, because `page()` is not reentrant: every OPENDIR name,
    // every READDIR page, and the CLOSE all run under it. That also makes the listing a snapshot
    // with no other operation of ours interleaved.
    let mut page = page();

    // `ENUMERATE` and nothing more, for the reason `DirBuilder::mkdir` spells out: over-asking is
    // `EPERM`, not attenuation, so a PAL that asked for `dir::ALL` here would work through the
    // test's full-rights grant and fail through every narrowed one. Every hop on the way to a
    // nested path asks for `DESCEND | ENUMERATE`, which is the same minimum one level up.
    let at = dir_at(&mut page, p, fsproto::dir::ENUMERATE)?;
    let handle = at.0;

    let mut entries: Vec<(OsString, bool)> = Vec::new();
    let mut buf = [0u8; PAGE];
    let mut cursor: u64 = 0;
    let result = loop {
        // `req_len` is ignored by READDIR; the cursor rides in the second word.
        let filled = match request(proto::req(proto::READDIR, handle, 0), cursor) {
            Ok(n) => n as usize,
            Err(e) => break Err(e),
        };
        // Zero bytes means the cursor is past the end, which is how the contract tells "the
        // directory ended" apart from "the page was full" without the client guessing.
        if filled == 0 {
            break Ok(());
        }
        let filled = filled.min(PAGE);
        page.get(&mut buf[..filled]);
        let mut this_page = 0u64;
        for (name, dir) in fsproto::dirent::iter(&buf[..filled]) {
            // A name is bytes on the wire and an `OsString` in std, and on this target an
            // `OsString` is UTF-8. A name the FS server holds that is not UTF-8 cannot be
            // represented, so it is skipped rather than lossily renamed: handing back a name that
            // does not open is worse than not listing it.
            if let Ok(s) = core::str::from_utf8(name) {
                entries.push((OsString::from(s), dir));
            }
            this_page += 1;
        }
        if this_page == 0 {
            // A non-empty reply that decoded to no records is a malformed page, and looping on it
            // would spin forever against a server that keeps sending it.
            break Err(io::const_error!(
                io::ErrorKind::InvalidData,
                "the FS server returned a directory page with no readable entries"
            ));
        }
        cursor += this_page;
    };

    // `at` closes what it minted, and never closes `ROOT`.
    drop(at);
    result?;

    Ok(ReadDir { root: p.to_path_buf(), entries: entries.into_iter() })
}

/// **Remove a name** (`UNLINK`, milestone 64). Not revocation: a handle already open on the file
/// keeps reading, exactly as POSIX promises, and `filesystem_contract::fs::UNLINK` records why the contract
/// draws that line. A directory is refused (`IsADirectory`); [`rmdir`] is its verb.
pub fn unlink(p: &Path) -> io::Result<()> {
    if !is_reachable() {
        return Err(unsupported_err());
    }
    let mut page = page();
    let (at, name) = walk(&mut page, proto::ROOT, p, fsproto::dir::REMOVE)?;
    page.put(name.as_bytes());
    request(proto::req(proto::UNLINK, at.0, name.len() as u64), 0)?;
    Ok(())
}

/// **Remove an empty directory** (`RMDIR`, milestone 64). Empty-only is the safety property, not a
/// missing feature: `DirectoryNotEmpty` is the answer for anything else, and the recursion that
/// Unix's `rm -r` does belongs in userspace where each step can be checked against the capability
/// for that level. A file is refused (`NotADirectory`), the mirror of [`unlink`]'s refusal.
pub fn rmdir(p: &Path) -> io::Result<()> {
    if !is_reachable() {
        return Err(unsupported_err());
    }
    let mut page = page();
    let (at, name) = walk(&mut page, proto::ROOT, p, fsproto::dir::REMOVE)?;
    page.put(name.as_bytes());
    request(proto::req(proto::RMDIR, at.0, name.len() as u64), 0)?;
    Ok(())
}

/// **Move a name** (`RENAME`, milestone 64): write-a-temp-then-rename is how a careful program
/// replaces a file, and `filesystem_contract::fs::RENAME` is concurrency-atomic and crash-atomic.
///
/// Each side is walked separately (milestone 122) and both handles go in one request, so
/// `rename("a/tmp", "b/final")` is a single atomic message. The source directory loses a name
/// (`REMOVE`) and the destination gains one (`CREATE`), which is the contract's rule. The two names
/// travel in the shared page back to back, source first, because one request word cannot carry two
/// lengths.
pub fn rename(old: &Path, new: &Path) -> io::Result<()> {
    if !is_reachable() {
        return Err(unsupported_err());
    }
    let mut page = page();
    let (src_at, src) = walk(&mut page, proto::ROOT, old, fsproto::dir::REMOVE)?;
    let (dst_at, dst) = walk(&mut page, proto::ROOT, new, fsproto::dir::CREATE)?;
    if src.len() + dst.len() > PAGE {
        return Err(io::const_error!(
            io::ErrorKind::InvalidFilename,
            "the two names together are longer than the page shared with the FS server"
        ));
    }
    page.put(src.as_bytes());
    page.put_at(src.len(), dst.as_bytes());
    request(
        proto::req(proto::RENAME, src_at.0, src.len() as u64),
        proto::rename_dst(dst_at.0, dst.len() as u64),
    )?;
    Ok(())
}

/// **Copy a file's contents** (milestone 64, rank 26): an open, a read/write loop, and two closes.
/// No verb backs it and none needs to: unlike `rename` it need not be atomic, and doing it here
/// costs the messages the caller would spend. Both names resolve under the granted directory, so
/// it cannot copy across a capability boundary (a program holding two directories needs the
/// namespace milestone 47 owns). It copies no permissions, because §27 has none to copy.
pub fn copy(from: &Path, to: &Path) -> io::Result<u64> {
    let mut reader = OpenOptions::new();
    reader.read(true);
    let src = File::open(from, &reader)?;

    // Refuse a directory before creating anything, so a mistaken `copy("dir", "x")` does not leave
    // an empty `x` behind. `File::open` already answers `EISDIR` for a directory source, so reaching
    // here means the source is a file.
    let mut writer = OpenOptions::new();
    writer.write(true);
    writer.create(true);
    writer.truncate(true);
    let dst = File::open(to, &writer)?;

    let mut buf = [0u8; PAGE];
    let mut copied: u64 = 0;
    loop {
        let n = src.read(&mut buf)?;
        if n == 0 {
            break;
        }
        let mut written = 0;
        while written < n {
            // A short write is not an error and not a loop-exit: the contract's `WRITE` is capped at
            // one page and may return less, so the only correct read of `Ok(0)` here is a full disk,
            // which would otherwise spin.
            match dst.write(&buf[written..n])? {
                0 => {
                    return Err(io::const_error!(
                        io::ErrorKind::WriteZero,
                        "the FS server accepted no bytes; the destination cannot grow"
                    ));
                }
                k => written += k,
            }
        }
        copied += n as u64;
    }
    Ok(copied)
}

pub fn set_perm(_p: &Path, _perm: FilePermissions) -> io::Result<()> {
    unsupported()
}

/// The `nofollow` twin of [`set_perm`], added upstream in the 2026-07-31 nightly.
///
/// Cannot be re-exported from `unsupported_fs` the way `rename` and friends are: that version takes
/// `unsupported_fs::FilePermissions`, and this module defines its own, so the signature does not
/// match. It refuses for the same reason `set_perm` does — permissions are not a thing this contract
/// has (§27), so there is nothing to set and nothing to follow or not follow.
pub fn set_perm_nofollow(_p: &Path, _perm: FilePermissions) -> io::Result<()> {
    unsupported()
}

/// **`std::fs::set_times`** (milestone 64, rank 28): `SETMTIME_AT`, the caller's time, by name.
///
/// Never `SETMTIME`: a caller's `SystemTime` asserts history, which DECISIONS §112 puts behind
/// `dir::SETTIME`, so the walk asks for `WRITE | SETTIME` and a grant without it answers
/// `ReadOnlyFilesystem`. Falling back would write the server's logical "now" and report success.
/// Refused before any message, so nothing is half done: an access time (no verb sets one), a time
/// past `i64::MAX` seconds (`GETMTIME`'s signed reply could never read it back), and the granted
/// directory itself (``, `.`, `/`), which has no name. A sub-second part is truncated, as on a
/// second-granularity Unix filesystem. An empty `FileTimes` still resolves the name, so a missing
/// file is `NotFound`. See notes/std/file-times.md.
pub fn set_times(p: &Path, times: FileTimes) -> io::Result<()> {
    if !is_reachable() {
        return Err(unsupported_err());
    }
    if times.accessed.is_some() {
        return Err(io::const_error!(
            io::ErrorKind::Unsupported,
            "no verb in the file contract sets an access time; nothing was changed"
        ));
    }
    let Some(modified) = times.modified else {
        return stat(p).map(|_| ());
    };
    if count_names(p)? == 0 {
        return Err(io::const_error!(
            io::ErrorKind::Unsupported,
            "the granted directory has no name to set its mtime by"
        ));
    }
    // `SystemTime` on this target is a `Duration` since the epoch, so this cannot fail; a time
    // before 1970 is not representable here at all, which is the time PAL's own limit.
    let since = modified.sub_time(&UNIX_EPOCH).map_err(|_| {
        io::const_error!(io::ErrorKind::InvalidInput, "an mtime before 1970 cannot cross this contract")
    })?;
    let secs = since.as_secs();
    if secs > i64::MAX as u64 {
        return Err(io::const_error!(
            io::ErrorKind::InvalidInput,
            "an mtime this far out could be written but never read back over this contract"
        ));
    }
    let mut page = page();
    let (at, name) =
        walk(&mut page, proto::ROOT, p, fsproto::dir::WRITE | fsproto::dir::SETTIME)?;
    page.put(name.as_bytes());
    request(proto::req(proto::SETMTIME_AT, at.0, name.len() as u64), secs)?;
    Ok(())
}

/// No symlinks cross this contract, so not following one is [`set_times`] itself.
pub fn set_times_nofollow(p: &Path, times: FileTimes) -> io::Result<()> {
    set_times(p, times)
}

/// Directories are created with `MKDIR`, which needs no options: the contract has no mode bits to
/// set, and `recursive` is std's own loop over this.
#[derive(Copy, Clone, Debug)]
pub struct DirBuilder {}

impl DirBuilder {
    pub fn new() -> DirBuilder {
        DirBuilder {}
    }

    /// `MKDIR` under the granted directory. The verb hands back a **capability to the new
    /// directory** (it is descend-with-creation, §47), and std's `create_dir` returns `()`, so the
    /// handle is closed immediately. Nothing is lost by that: a later `read_dir` of the same name
    /// mints another one, and holding this one would be a handle-table slot leaked per `mkdir`.
    pub fn mkdir(&self, p: &Path) -> io::Result<()> {
        if !is_reachable() {
            return Err(unsupported_err());
        }
        let mut page = page();
        let (at, name) = walk(&mut page, proto::ROOT, p, fsproto::dir::CREATE | fsproto::dir::DESCEND)?;
        page.put(name.as_bytes());
        // **The second word asks for rights on the child, and asking for too much is a refusal
        // rather than an attenuation.** The server intersects the request with the parent's rights
        // and answers `EPERM` if the result is smaller than what was asked (§47's monotonicity is
        // the intersection; the refusal is it telling the truth about it). A PAL cannot know what
        // its own directory capability carries, so it must ask for the *minimum the operation
        // needs* or it breaks under every narrowed grant. `create_dir` needs nothing from the
        // handle it gets back, because it closes it.
        let handle = request(proto::req(proto::MKDIR, at.0, name.len() as u64), 0)?;
        let _ = request(proto::req(proto::CLOSE, handle, 0), 0);
        Ok(())
    }
}

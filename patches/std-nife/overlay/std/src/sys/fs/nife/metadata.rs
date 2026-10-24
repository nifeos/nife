//! **What a name or a handle reports**: `FileAttr` and the modification time it may carry,
//! `FileType`, `FilePermissions` and `FileTimes`.
//!
//! A seam of `sys/fs/nife.rs` (§282 (when a file is split, the parent becomes an index)):
//! `pub(super)` here reaches `nife` and its children.
//!
//! Name: provisional (lane maintainer/near-ceiling-splits, 2026-10-12 UTC), for the module and the
//! file.

use super::*;

/// A file's metadata, as much of it as the contract carries: the size, whether the name was a
/// directory, and (when it was reached by name) the modification time. Everything else std asks for
/// either does not exist on this service or is not on the wire.
///
/// **A directory's `size` is 0 and that is a placeholder, not a measurement.** `FSTAT` reports one
/// number and the only handles that reach it are files; the directory case is reached from a
/// listing, where `READDIR` said "this one is a directory" and nothing said how big it is.
#[derive(Clone)]
pub struct FileAttr {
    pub(super) size: u64,
    pub(super) dir: bool,
    pub(super) mtime: Mtime,
}

impl FileAttr {
    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn perm(&self) -> FilePermissions {
        FilePermissions {}
    }

    pub fn file_type(&self) -> FileType {
        FileType { dir: self.dir }
    }

    /// **`Metadata::modified`** (milestone 64, rank 19), from the `GETMTIME` [`stat`] issued.
    ///
    /// The value is the contract's, Unix seconds, and it is only as true as the server that stamped
    /// it. **A file written on this system carries the FS server's logical clock, not a wall-clock
    /// time**: `redoxfs_server` has no clock grant, so every mutation stamps a counter that starts
    /// at 1 on each mount (notes/touch.md's `BUGS`). Such a file reads as a moment in early 1970,
    /// orders correctly against other writes in the same boot, and orders wrongly against a file
    /// the host tool made. That is recorded in notes/std/file-times.md's BUGS and
    /// proposed as its own piece of work; it is not something this function can detect, because
    /// `SETMTIME_AT` may legitimately assert a small number too.
    pub fn modified(&self) -> io::Result<SystemTime> {
        match self.mtime {
            Mtime::Seconds(s) => {
                UNIX_EPOCH.checked_add_duration(&crate::time::Duration::from_secs(s)).ok_or_else(
                    || {
                        io::const_error!(
                            io::ErrorKind::InvalidData,
                            "the FS server reported an mtime past what SystemTime can hold"
                        )
                    },
                )
            }
            // `ENOTDIR` is the one refusal that is about the grant's *shape*: a per-file caretaker
            // answers it for all three mtime verbs because it has no directory to resolve a name
            // under (`filesystem_contract::grant::POLICY`). Reported as what it is rather than as
            // `NotADirectory`, which would read as a statement about the file.
            Mtime::Refused(r0) if fsproto::reply_errno(r0 as i64) == Some(20) => {
                Err(io::const_error!(
                    io::ErrorKind::Unsupported,
                    "this process holds a single-file grant, and the contract reads an mtime by \
                     name under a directory"
                ))
            }
            Mtime::Refused(r0) => match reply(r0) {
                Err(e) => Err(e),
                // Unreachable by construction (`stat` keeps only negative replies here), and answered
                // as an error rather than a time so a future mistake cannot become a fabricated one.
                Ok(_) => Err(io::const_error!(
                    io::ErrorKind::InvalidData,
                    "an mtime refusal was recorded for a reply that was not a refusal"
                )),
            },
            Mtime::Unnamed => Err(io::const_error!(
                io::ErrorKind::Unsupported,
                "the contract reads an mtime by name, and this metadata came from a handle: use \
                 std::fs::metadata(path)"
            )),
        }
    }

    /// No verb reports an access time.
    pub fn accessed(&self) -> io::Result<SystemTime> {
        unsupported()
    }

    /// RedoxFS keeps a creation time, but no verb carries it across the contract.
    pub fn created(&self) -> io::Result<SystemTime> {
        unsupported()
    }
}

/// **What the contract said about a modification time**, kept until `modified()` asks.
///
/// `GETMTIME` (milestone 47, DECISIONS §112) resolves a **name under a directory handle** and never
/// takes an open file. `std::fs::metadata(path)` and `DirEntry::metadata` reach a name, so they ask
/// and keep [`Mtime::Seconds`] or [`Mtime::Refused`]; `File::metadata` and a held `Dir`'s own
/// metadata have **no name**, so they get [`Mtime::Unnamed`]. Asking by the name a `File` was opened
/// by would answer for whatever holds that name *now*, the silent degradation DECISIONS §42
/// forbids. A refused `GETMTIME` fails only `modified()`, never `metadata()`. See
/// notes/std/file-times.md; a handle-taking verb is proposed in
/// design/roadmap/0504-an-mtime-for-an-open-file.md.
#[derive(Clone, Copy)]
pub(super) enum Mtime {
    /// Unix seconds, as the server reported them. See `modified()` for what the number means on an
    /// image this system wrote, which is not always a wall-clock time.
    Seconds(u64),
    /// The server (or a caretaker in front of it) refused `GETMTIME`; the reply word, so the error
    /// can be rebuilt with [`reply`] when it is asked for.
    Refused(u64),
    /// Reached through a handle rather than a name, so there was nothing to ask by.
    Unnamed,
}

/// **The times a `set_times` call asks for** (milestone 64, rank 28). Its own type rather than the
/// unsupported backend's, because `SETMTIME_AT` carries a modification time. The access time is
/// kept only so [`set_times`] can refuse it loudly: no verb sets one, and dropping it would report
/// success for half of what the caller asked.
#[derive(Copy, Clone, Debug, Default)]
pub struct FileTimes {
    pub(super) accessed: Option<SystemTime>,
    pub(super) modified: Option<SystemTime>,
}

impl FileTimes {
    pub fn set_accessed(&mut self, t: SystemTime) {
        self.accessed = Some(t);
    }

    pub fn set_modified(&mut self, t: SystemTime) {
        self.modified = Some(t);
    }
}

/// Permissions do not exist on this service. `readonly` is honestly false (a granted directory
/// endpoint carries WRITE or it does not, and that is a capability, not a mode bit).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FilePermissions {}

impl FilePermissions {
    pub fn readonly(&self) -> bool {
        false
    }

    pub fn set_readonly(&mut self, _readonly: bool) {
        // std's API cannot report a failure here, and this service has no permission bits, so the
        // only honest thing is to do nothing; `set_permissions` (which CAN fail) refuses.
    }
}

/// A name is a file or a directory. Symlinks are not exposed over this contract at all, so
/// `is_symlink` is not "we did not look", it is "there is no such thing here".
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FileType {
    pub(super) dir: bool,
}

impl FileType {
    pub fn is_dir(&self) -> bool {
        self.dir
    }

    pub fn is_file(&self) -> bool {
        !self.dir
    }

    pub fn is_symlink(&self) -> bool {
        false
    }
}

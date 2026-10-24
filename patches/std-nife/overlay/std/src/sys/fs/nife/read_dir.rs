//! **A directory listing**: `ReadDir` and the `DirEntry` values it yields.
//!
//! A seam of `sys/fs/nife.rs` (§282 (when a file is split, the parent becomes an index)):
//! `pub(super)` here reaches `nife` and its children.
//!
//! Name: provisional (lane maintainer/near-ceiling-splits, 2026-10-12 UTC), for the module and the
//! file.

use super::*;

/// A directory listing, **read whole at `read_dir` time** rather than streamed.
///
/// The listing arrives through the one page shared with the FS server, behind the lock every other
/// operation wants, and std lets the caller hold the iterator across other work, including opening
/// what it listed. Fetching per `next()` would hold the lock across user code, or reacquire it with
/// the directory moved under its cursor. The cost: a huge directory is a `Vec` of every name, and
/// an entry removed before the caller reaches it opens as `NotFound`, the ordinary readdir caveat
/// (`filesystem_contract::fs::READDIR` records the same one for its cursor).
pub struct ReadDir {
    /// The path the caller passed, so `DirEntry::path()` can rebuild what they would expect.
    pub(super) root: PathBuf,
    pub(super) entries: crate::vec::IntoIter<(OsString, bool)>,
}

impl fmt::Debug for ReadDir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReadDir").field("root", &self.root).finish()
    }
}

impl ReadDir {
    /// **The directory this listing came from, reopened by its path** (`ReadDir::dir`,
    /// `#![feature(dirfd)]`, added upstream in rust-lang/rust#163701). Unix derives the handle
    /// from the stream's own descriptor, so it names the directory that was listed even if the
    /// path has since moved. This listing holds no handle to derive from, because `readdir`
    /// drained it and let the server's handle go (the type's header says why), so it does what
    /// upstream's hermit, SOLID and UEFI backends do: open `root` again. That is the race std's
    /// own documentation of `dir` warns about, a rename between `read_dir` and `dir` reaches
    /// whatever is at the path now.
    pub fn dir(&self) -> io::Result<Dir> {
        let mut opts = OpenOptions::new();
        opts.read(true);
        Dir::open(&self.root, &opts)
    }
}

impl Iterator for ReadDir {
    type Item = io::Result<DirEntry>;

    fn next(&mut self) -> Option<io::Result<DirEntry>> {
        // The whole listing was fetched by `readdir`, so nothing here can fail. The `io::Result`
        // is std's shape for platforms that read one entry at a time, not a claim that this one
        // does.
        let (name, dir) = self.entries.next()?;
        Some(Ok(DirEntry { root: self.root.clone(), name, dir }))
    }
}

pub struct DirEntry {
    pub(super) root: PathBuf,
    pub(super) name: OsString,
    pub(super) dir: bool,
}

impl DirEntry {
    /// The path a caller would use to reach this entry, which is the path they passed to
    /// `read_dir` with the name appended, so feeding an entry's `path()` straight back to
    /// `File::open` works. It would not if this returned a bare name, because `PathBuf` joining is
    /// std's job and std joins against what it was given.
    ///
    /// **This is the call milestone 122 exists for.** `read_dir(".")` yields `./name` and always
    /// opened; `read_dir("sub")` yields `sub/name`, which is two components, which was refused. A
    /// std program could list a subdirectory and then not open one thing it had just been told was
    /// there, and every walker in the ecosystem is built out of exactly this pair.
    pub fn path(&self) -> PathBuf {
        self.root.join(&self.name)
    }

    pub fn file_name(&self) -> OsString {
        self.name.clone()
    }

    /// **From the listing when it can be, from the name when it must be.** `READDIR` said whether
    /// the entry is a directory, so `file_type` is free and costs no message. Metadata is not on the
    /// listing, so it is [`stat`] of the entry's path, for a directory as for a file: a
    /// directory's size is still the placeholder [`FileAttr`] describes, but its mtime is real, and
    /// `GETMTIME` answers for a directory exactly as for a file. That made a directory entry's
    /// metadata cost a walk where it used to cost nothing, which is the price of a `modified()`
    /// that answers; a caller that only wants the kind should ask `file_type`, as it would anywhere.
    pub fn metadata(&self) -> io::Result<FileAttr> {
        stat(&self.path())
    }

    pub fn file_type(&self) -> io::Result<FileType> {
        Ok(FileType { dir: self.dir })
    }
}

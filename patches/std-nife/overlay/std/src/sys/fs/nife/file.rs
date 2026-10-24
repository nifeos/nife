//! **File**.
//!
//! A seam of `sys/fs/nife.rs` (§282 (when a file is split, the parent becomes an index)):
//! `pub(super)` here reaches `nife` and its children.
//!
//! Name: provisional (lane maintainer/near-ceiling-splits, 2026-10-12 UTC), for the module and the
//! file.

use super::*;

/// An open file: a handle the server minted, plus the offset std's positional API keeps on this
/// side (the contract's `READ`/`WRITE` are both explicitly positional, so there is no cursor in
/// the server to get out of step with).
///
/// The offset is an atomic rather than a `Cell` because `std::fs::File` is `Sync` on every
/// platform and this target should not be the exception.
pub struct File {
    pub(super) handle: u64,
    pub(super) pos: AtomicU64,
    /// `append` mode: every write goes at the current end of file, so the position is refreshed
    /// from `FSTAT` before each one. Not atomic against another writer (no lock verb in the
    /// contract), which is the same caveat every non-POSIX backend carries.
    pub(super) append: bool,
    /// The size `OPEN`'s reply carried (milestone 606, ruling B), for sizing a buffer and for
    /// nothing else: [`File::open_size_hint`] is the only reader, and `metadata()` always asks the
    /// server. 0 is "no hint". A write or a truncate through this handle clears it.
    pub(super) size_hint: AtomicU64,
}

impl File {
    pub fn open(path: &Path, opts: &OpenOptions) -> io::Result<File> {
        if !opts.read && !opts.write && !opts.append {
            return Err(io::const_error!(
                io::ErrorKind::InvalidInput,
                "an open must ask for read, write, or append"
            ));
        }
        // POSIX's order: open, create only on `NotFound` when asked to, and truncate after a
        // successful open of an existing file. `std::fs::write` is `create(true).truncate(true)`,
        // so another order would leave the old tail behind on the path that exists to replace a
        // file's contents. See notes/std/fs.md.
        let mut opened_size = 0;
        let handle = match open_handle(path, opts) {
            Ok((h, _)) if opts.create_new => {
                // `create_new` means "must not already exist", and it does. Close the handle the open
                // just minted rather than leaking it for the life of the process: the error path is
                // the one nobody exercises, so it is the one that leaks.
                let _ = request(proto::req(proto::CLOSE, h, 0), 0);
                return Err(io::const_error!(
                    io::ErrorKind::AlreadyExists,
                    "the file already exists"
                ));
            }
            Ok((h, size)) => {
                opened_size = size;
                h
            }
            // Not there, and the caller asked for it to be made.
            Err(e) if (opts.create || opts.create_new) && e.kind() == io::ErrorKind::NotFound => {
                create_handle(path, opts)?
            }
            Err(e) => return Err(e),
        };

        // Truncate after the open, so a fresh file (already empty) pays nothing and an existing one
        // is emptied before the first write rather than after it.
        if opts.truncate {
            if let Err(e) = request(proto::req(proto::TRUNCATE, handle, 0), 0) {
                let _ = request(proto::req(proto::CLOSE, handle, 0), 0);
                return Err(e);
            }
        }

        // A truncated file's size is 0 whatever the open said, and 0 already means "no hint".
        let hint = if opts.truncate { 0 } else { opened_size };
        let file = File {
            handle,
            pos: AtomicU64::new(0),
            append: opts.append,
            size_hint: AtomicU64::new(hint),
        };
        if opts.append {
            let end = file.file_attr()?.size;
            file.pos.store(end, Ordering::Relaxed);
        }
        Ok(file)
    }

    pub fn file_attr(&self) -> io::Result<FileAttr> {
        let size = request(proto::req(proto::FSTAT, self.handle, 0), 0)?;
        // Anything that got as far as a handle came through `OPEN`, which refuses a directory
        // (`EISDIR`), so this is a file by construction. No mtime: `GETMTIME` takes a name and a
        // handle has none ([`Mtime`] says why the PAL does not re-resolve the one it was opened by).
        Ok(FileAttr { size, dir: false, mtime: Mtime::Unnamed })
    }

    pub fn fsync(&self) -> io::Result<()> {
        // Nothing is buffered on this side, and the server commits a RedoxFS transaction per write
        // (that is what makes a mid-write kill recoverable), so a successful write is already
        // durable and there is nothing left for a sync verb to do.
        Ok(())
    }

    pub fn datasync(&self) -> io::Result<()> {
        self.fsync()
    }

    pub fn lock(&self) -> io::Result<()> {
        unsupported()
    }

    pub fn lock_shared(&self) -> io::Result<()> {
        unsupported()
    }

    pub fn try_lock(&self) -> Result<(), TryLockError> {
        Err(TryLockError::Error(unsupported_err()))
    }

    pub fn try_lock_shared(&self) -> Result<(), TryLockError> {
        Err(TryLockError::Error(unsupported_err()))
    }

    pub fn unlock(&self) -> io::Result<()> {
        unsupported()
    }

    /// **`File::set_len`** (milestone 64, rank 8). The contract's `TRUNCATE` is POSIX `ftruncate` in
    /// both directions already (`filesystem_contract::fs::TRUNCATE` says so and the server implements it), and
    /// the size rides in the second word. This was refused here for the same reason the five verbs
    /// in milestone 64's first pass were: the binding was never written, not the verb missing.
    ///
    /// `File::open` already sends this verb with a size of 0 for `OpenOptions::truncate`, so the one
    /// difference between "the file was emptied" and "the file is now n bytes" was the word this
    /// function passes.
    pub fn truncate(&self, size: u64) -> io::Result<()> {
        self.size_hint.store(0, Ordering::Relaxed);
        request(proto::req(proto::TRUNCATE, self.handle, 0), size)?;
        Ok(())
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let pos = self.pos.load(Ordering::Relaxed);
        let want = buf.len().min(PAGE);
        let mut p = page();
        let n = (request(proto::req(proto::READ, self.handle, want as u64), pos)? as usize).min(want);
        p.get(&mut buf[..n]);
        drop(p);
        self.pos.store(pos + n as u64, Ordering::Relaxed);
        Ok(n)
    }

    pub fn read_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        crate::io::default_read_vectored(|b| self.read(b), bufs)
    }

    #[inline]
    pub fn is_read_vectored(&self) -> bool {
        false
    }

    pub fn read_buf(&self, cursor: BorrowedCursor<'_, u8>) -> io::Result<()> {
        crate::io::default_read_buf(|b| self.read(b), cursor)
    }

    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        self.size_hint.store(0, Ordering::Relaxed);
        if self.append {
            let end = self.file_attr()?.size;
            self.pos.store(end, Ordering::Relaxed);
        }
        let pos = self.pos.load(Ordering::Relaxed);
        let chunk = buf.len().min(PAGE);
        let mut p = page();
        p.put(&buf[..chunk]);
        let n =
            (request(proto::req(proto::WRITE, self.handle, chunk as u64), pos)? as usize).min(chunk);
        drop(p);
        self.pos.store(pos + n as u64, Ordering::Relaxed);
        Ok(n)
    }

    pub fn write_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        crate::io::default_write_vectored(|b| self.write(b), bufs)
    }

    #[inline]
    pub fn is_write_vectored(&self) -> bool {
        false
    }

    pub fn flush(&self) -> io::Result<()> {
        Ok(())
    }

    /// The position is this side's, since the contract's `READ` and `WRITE` both carry an explicit
    /// offset: there is no cursor in the server to get out of step with, and a seek costs no
    /// message at all except `SeekFrom::End`, which needs the size.
    pub fn seek(&self, pos: SeekFrom) -> io::Result<u64> {
        let (base, delta) = match pos {
            SeekFrom::Start(off) => {
                self.pos.store(off, Ordering::Relaxed);
                return Ok(off);
            }
            SeekFrom::Current(off) => (self.pos.load(Ordering::Relaxed), off),
            SeekFrom::End(off) => (self.file_attr()?.size, off),
        };
        // A resulting offset before the start is `InvalidInput`, std's contract, not a wrap-around.
        let target = base.checked_add_signed(delta).ok_or_else(|| {
            io::const_error!(io::ErrorKind::InvalidInput, "cannot seek before the start of a file")
        })?;
        self.pos.store(target, Ordering::Relaxed);
        Ok(target)
    }

    /// **The size `OPEN` reported, as a buffer-sizing hint** (milestone 606, calef's ruling B, form
    /// 2, 2026-09-27). The overlay's patch to `std::fs` asks this before `metadata()` when it sizes
    /// the buffer for `fs::read`, `fs::read_to_string` and `read_to_end`, which is what saves the
    /// `FSTAT` in front of every whole-file read. It is never used to answer `metadata()`: calef's
    /// reason is that a stale hint costs at most one resize, while a stale `metadata()` lies.
    pub fn open_size_hint(&self) -> Option<u64> {
        match self.size_hint.load(Ordering::Relaxed) {
            0 => None,
            size => Some(size),
        }
    }

    pub fn size(&self) -> Option<io::Result<u64>> {
        // `FSTAT` is one message, so answering the size hint is cheaper than the read-until-EOF
        // loop `read_to_string`/`read_to_end` fall back to.
        Some(self.file_attr().map(|a| a.size))
    }

    pub fn tell(&self) -> io::Result<u64> {
        Ok(self.pos.load(Ordering::Relaxed))
    }

    pub fn duplicate(&self) -> io::Result<File> {
        // A handle is a token the server minted for one session; there is no dup verb, and copying
        // the number would forge a second owner of the same handle, including its close.
        unsupported()
    }

    pub fn set_permissions(&self, _perm: FilePermissions) -> io::Result<()> {
        unsupported()
    }

    /// **Refused**: `SETMTIME_AT` (milestone 47) takes a **name under a directory**, and an open
    /// file has a handle and no name. Stamping whatever holds the name this file was opened by
    /// would set the wrong file's time after a rename, a silent wrong answer rather than a refusal
    /// ([`Mtime`] has the argument). `std::fs::set_times(path, ..)` is bound and is what a caller
    /// can use; a handle-taking verb is proposed in
    /// design/roadmap/0504-an-mtime-for-an-open-file.md.
    pub fn set_times(&self, _times: FileTimes) -> io::Result<()> {
        Err(io::const_error!(
            io::ErrorKind::Unsupported,
            "the contract sets an mtime by name, and an open file carries none: use \
             std::fs::set_times(path, ..)"
        ))
    }
}

impl fmt::Debug for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("File").field("handle", &self.handle).finish()
    }
}

impl Drop for File {
    fn drop(&mut self) {
        // `CLOSE` frees the server's handle-table slot. A failure here has nowhere to go and
        // nothing to fix: the process is done with the file either way.
        let _ = request(proto::req(proto::CLOSE, self.handle, 0), 0);
    }
}

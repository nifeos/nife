//! One file-service request, decoded and answered ([`Server::handle`]).
//!
//! The serve loop's dispatch, in the library so a host fuzz target
//! (`fuzz/fuzz_targets/redoxfs_server_session.rs`) and host tests drive the code the running server
//! runs. The binary keeps the IO, including the two edges [`ServeEdges`] reaches: the device flush
//! behind [`filesystem_contract::fs::SYNC`] and milestone 37's count of `WRITE` requests.
//!
//! Names: `handle`, `ServeEdges` and this module's are provisional, minted by the fuzz lane
//! (`lane/fuzz-service-handlers`) on 2026-10-04 (UTC); an architect names things.

//! # A name is copied out of the client's window before it is judged
//!
//! The window is the client's shared page (milestone 599 (a frame per filesystem client channel)),
//! and a confined client can run a second thread that rewrites it (milestone 812 (threads in one
//! address space)). So every name-taking arm copies the name into a [`NameCopy`], which the check
//! and the resolve both read; an arm that skipped the copy would not compile. This is milestone 825
//! (a hostile client races the file server's name window)'s fix, and `notes/confinement-claims.md`
//! rows 19 and 24 are the claims it holds. The attribute-name arms take bytes in a separate
//! namespace, checked by `xattr::is_valid_name`; `get_xattr_request` copies its name too.

use filesystem_contract::{blk, fs, operation, reply_err, xattr};
use redoxfs::Disk;
use syscall::error::{EINVAL, EPERM, Error, Result};

use crate::{NameCopy, Server};

/// One filesystem block, in bytes: the size of every reply whose length the server chooses.
const BLOCK: usize = blk::BLOCK_SIZE;

/// **The serve loop's IO, as the dispatch reaches it.** Everything else a request does is the
/// [`Server`]'s own logic.
pub trait ServeEdges {
    /// Called as an admitted [`fs::WRITE`] begins, before the server writes. The EL0 binary arms its
    /// crash injector here (milestone 37); a host caller usually does nothing.
    fn note_write(&mut self) {}

    /// Make the device durable, and return the block server's answer untouched: a count of
    /// completed synces, or a negative errno passed through unmapped (`fs::SYNC` documents why).
    fn sync(&mut self) -> i64;
}

impl<D: Disk> Server<D> {
    /// **Answer one request**: `w0` and `w1` as the client sent them, `badge` as the kernel
    /// delivered it, and `window` the client's channel, which the request's names and bytes are
    /// read from and its reply's bytes written into (milestone 599 (a frame per filesystem client
    /// channel)). Returns the two reply words.
    ///
    /// `window` must be at least [`fs::TRANSFER_MAX`] bytes, which the EL0 binary's mapped window
    /// is exactly. A shorter slice panics on the first verb that reaches past it.
    ///
    /// This is the **only** place a RedoxFS error becomes a wire value ([`reply_err`]); everything
    /// below it speaks `syscall::error::Result`.
    pub fn handle(
        &mut self,
        badge: u64,
        w0: u64,
        w1: u64,
        window: &mut [u8],
        edges: &mut impl ServeEdges,
    ) -> (i64, u64) {
        let code = operation(w0);
        // A sync-only badge (milestone 805 (`reboot` at the prompt), `fs::BIND_SYNC`) may `SYNC`
        // and nothing else, decided before a handle is read: it names no directory. `EPERM` for
        // everything else, the answer a missing non-naming right gets.
        if self.is_sync_only(badge) {
            return if code == fs::SYNC {
                (edges.sync(), 0)
            } else {
                (reply_err(EPERM), 0)
            };
        }
        // A bound badge's handles go through `subtree_scope` (milestone 606 (a directory walk costs
        // what it does on Linux), ruling D). `BIND`, `UNBIND` and `BIND_SYNC` name a handle of the
        // binder's own, and a bound badge may not close its grant's `ROOT`.
        let raw = fs::req_handle(w0);
        let admitted = if code == fs::BIND || code == fs::UNBIND || code == fs::BIND_SYNC {
            Ok(raw as u32)
        } else if code == fs::CLOSE && raw == fs::ROOT && self.scoped(badge) {
            Err(Error::new(EINVAL))
        } else {
            self.admit(badge, raw)
        };
        let handle = match admitted {
            Ok(h) => h,
            Err(e) => return (reply_err(e.errno), 0),
        };
        // Two clamps (milestone 138 step 3): a reply length this server chooses stays inside the one
        // page every client maps, and only `READ` and `WRITE`, whose length the client chooses, may
        // use the whole channel. See `filesystem_contract::fs::TRANSFER_PAGES`.
        let len = fs::req_len(w0).min(BLOCK);
        let bulk_len = fs::req_len(w0).min(fs::TRANSFER_MAX);
        let offset = w1;
        let request = Request {
            code,
            badge,
            handle,
            len,
            bulk_len,
            offset,
        };
        let result = self.dispatch_file_verbs(&request, window, edges);

        // A handle a bound badge minted is that badge's, and no other badge can name it.
        if let (fs::OPEN | fs::OPENDIR | fs::CREATE | fs::MKDIR, Ok(h)) = (code, &result) {
            self.claim(*h as u32, badge);
        }
        // `OPEN`'s second reply word is the file's size (milestone 606, ruling B); every other
        // verb's is 0.
        let size = match (code, &result) {
            (fs::OPEN, Ok(h)) => self.fstat(*h as u32).unwrap_or(0),
            _ => 0,
        };
        // The one error-mapping site: RedoxFS's Error -> the negated-errno wire value.
        (result.unwrap_or_else(|e| reply_err(e.errno)), size)
    }

    /// **The file and directory verbs**: open, read, write, create, list, rename and remove. Any
    /// other code goes on to [`Server::dispatch_other_verbs`].
    #[inline]
    fn dispatch_file_verbs(
        &mut self,
        request: &Request,
        window: &mut [u8],
        edges: &mut impl ServeEdges,
    ) -> Result<i64> {
        let Request {
            code,
            badge,
            handle,
            len,
            bulk_len,
            offset,
        } = *request;
        match code {
            // The handle field is the **parent directory**, not a file: `fs::ROOT` is the endpoint's
            // bound directory, which is what every client that predates directory handles sends.
            fs::OPEN => {
                // The second word is the rights each directory on a path asks for (ruling A;
                // `fs::OPEN`). The check and the resolve read the server's copy of the name.
                match NameCopy::copy_in(&window[..len]) {
                    Ok(name) => self.open_file_path(handle, &name, offset).map(|h| h as i64),
                    Err(e) => Err(e),
                }
            }
            // Read straight into the channel, up to the whole of it.
            fs::READ => self
                .read(handle, offset, &mut window[..bulk_len])
                .map(|n| n as i64),
            fs::WRITE => {
                edges.note_write(); // milestone 37: arm the crash if this is the named request
                self.write(handle, offset, &window[..bulk_len])
                    .map(|n| n as i64)
            }
            fs::FSTAT => self.fstat(handle).map(|s| s as i64),
            fs::CLOSE => self.close(handle).map(|()| 0),
            // OPEN's shape: a name at the start of the channel, a handle back.
            fs::CREATE => match NameCopy::copy_in(&window[..len]) {
                Ok(name) => self.create_file_at(handle, &name).map(|h| h as i64),
                Err(e) => Err(e),
            },
            // The verbs that hand back authority (milestone 47), in OPEN's shape. The second word
            // is the rights asked for the child, which the server intersects with the parent's.
            fs::OPENDIR | fs::MKDIR => match NameCopy::copy_in(&window[..len]) {
                Ok(name) if code == fs::OPENDIR => {
                    self.open_dir(handle, &name, offset).map(|h| h as i64)
                }
                Ok(name) => self.make_dir(handle, &name, offset).map(|h| h as i64),
                Err(e) => Err(e),
            },
            // The cursor rides in the second word, since `len` is clamped to one page.
            fs::READDIR => self
                .read_dir(handle, offset as u32, &mut window[..BLOCK])
                .map(|n| n as i64),
            // Two directories: the second word packs the destination's (handle, length), both names
            // ride in the channel, and an overrun is `EINVAL`, because clamping a name renames
            // something else.
            fs::RENAME => self.rename_request(badge, handle, len, offset, window),
            // `rm`'s two verbs, in OPEN's shape with a 0 reply. `UNLINK` refuses a directory and
            // `RMDIR` a non-empty one, so neither removes whatever it finds.
            fs::UNLINK | fs::RMDIR => match NameCopy::copy_in(&window[..len]) {
                Ok(name) if code == fs::UNLINK => self.unlink(handle, &name).map(|()| 0),
                Ok(name) => self.rmdir(handle, &name).map(|()| 0),
                Err(e) => Err(e),
            },
            _ => self.dispatch_other_verbs(request, window, edges),
        }
    }

    /// **Every verb that is not a file or directory verb**: attributes, truncation, durability,
    /// the filesystem's size, mtimes and binding. An unknown code is `EINVAL` here.
    #[inline]
    fn dispatch_other_verbs(
        &mut self,
        request: &Request,
        window: &mut [u8],
        edges: &mut impl ServeEdges,
    ) -> Result<i64> {
        let Request {
            code,
            badge,
            handle,
            len,
            offset,
            ..
        } = *request;
        match code {
            // Extended attributes (milestone 57): the handle is the node the attribute is on, not
            // a parent, since an attribute has no name to resolve.
            fs::GETXATTR => self.get_xattr_request(handle, len, window),
            // Name then value on the page, the value's length and type code in the second word; an
            // overrun is `EINVAL`, because clipping a value stores something nobody wrote.
            fs::SETXATTR => self.set_xattr_request(handle, len, offset, window),
            fs::LISTXATTR => self
                .list_xattr(handle, &mut window[..BLOCK])
                .map(|n| n as i64),
            fs::REMOVEXATTR => self.remove_xattr(handle, &window[..len]).map(|()| 0),
            // The new size rides in the second word: `len` is clamped to one page and would cap it.
            fs::TRUNCATE => self.truncate(handle, offset).map(|()| 0),
            // Durability (milestone 55): the rights check is the core's, the flush is the edge's.
            // The block server's reply (a flush count, or a negative errno) passes through
            // unmapped, the one exception to mapping errors once (notes/fs-server/caches-and-io.md).
            fs::SYNC => self.sync_permitted(handle).map(|()| edges.sync()),
            // A record in the channel, `r0` its length: three u64s do not fit a reply word, and
            // encoding here keeps the core's verbs free of the wire's layout.
            fs::STATFS => self.statfs(handle).and_then(|(block, total, free)| {
                filesystem_contract::statfs::encode(&mut window[..BLOCK], block, total, free)
                    .map(|n| n as i64)
                    .ok_or(Error::new(EINVAL))
            }),
            // The mtime verbs (milestone 47's `touch`; DECISIONS §112), in UNLINK's shape. The
            // rights split lives in [`Server::mtime`] and its two siblings.
            fs::GETMTIME => match NameCopy::copy_in(&window[..len]) {
                Ok(name) => self.mtime(handle, &name).map(|t| t as i64),
                Err(e) => Err(e),
            },
            fs::SETMTIME => match NameCopy::copy_in(&window[..len]) {
                Ok(name) => self.set_mtime_now(handle, &name).map(|()| 0),
                Err(e) => Err(e),
            },
            // The asserted seconds ride in the second word, TRUNCATE's reason.
            fs::SETMTIME_AT => match NameCopy::copy_in(&window[..len]) {
                Ok(name) => self.set_mtime_at(handle, &name, offset).map(|()| 0),
                Err(e) => Err(e),
            },
            // Ruling D's two control verbs; `fs::BIND` has the rules, `subtree_scope` enforces them.
            fs::BIND => self.bind(badge, handle, offset).map(|()| 0),
            fs::UNBIND => self.unbind(badge, offset).map(|()| 0),
            fs::BIND_SYNC => self.bind_sync_only(badge, offset).map(|()| 0),
            _ => Err(Error::new(EINVAL)),
        }
    }

    /// `RENAME`: both names ride in the channel, source first, and the second word packs the
    /// destination's handle and length.
    fn rename_request(
        &mut self,
        badge: u64,
        handle: u32,
        len: usize,
        offset: u64,
        window: &[u8],
    ) -> Result<i64> {
        let dst_len = fs::dst_len(offset);
        if len + dst_len > BLOCK {
            Err(Error::new(EINVAL))
        } else {
            // Each name copied off the window once, into a value the server owns, before either is
            // judged; `rename` checks the copies.
            let src = NameCopy::copy_in(&window[..len]);
            let dst = NameCopy::copy_in(&window[len..len + dst_len]);
            match (src, dst) {
                (Ok(src), Ok(dst)) => self
                    .admit(badge, fs::dst_handle(offset))
                    .and_then(|to| self.rename(handle, &src, to, &dst))
                    .map(|()| 0),
                _ => Err(Error::new(EINVAL)),
            }
        }
    }

    /// `GETXATTR`: copy the name off the page, then write the attribute's value onto it.
    fn get_xattr_request(&mut self, handle: u32, len: usize, window: &mut [u8]) -> Result<i64> {
        // The name comes in on the page and the value goes back out on it, so the name is
        // copied to the stack before the reply is written over it. 255 bytes against a
        // measured 127 KiB high-water on a 397 KiB grant (notes/fs-server.md).
        let mut name = [0u8; xattr::MAX_NAME];
        if len > name.len() {
            Err(Error::new(xattr::ERANGE))
        } else {
            name[..len].copy_from_slice(&window[..len]);
            // The server refuses a value that will not fit rather than writing past it.
            self.get_xattr(handle, &name[..len], &mut window[..BLOCK])
                .map(|(kind, n)| xattr::reply(kind, n))
        }
    }

    /// `SETXATTR`: the name and then the value on the page, the value's length and type code in
    /// the second word.
    fn set_xattr_request(
        &mut self,
        handle: u32,
        len: usize,
        offset: u64,
        window: &[u8],
    ) -> Result<i64> {
        let value_len = xattr::spec_value_len(offset);
        if len + value_len > BLOCK {
            Err(Error::new(EINVAL))
        } else {
            let (name, value) = window[..len + value_len].split_at(len);
            self.set_xattr(handle, name, xattr::spec_kind(offset), value)
                .map(|()| 0)
        }
    }
}

/// One admitted request's decoded words, as the dispatch below [`Server::handle`] reads them.
#[derive(Clone, Copy)]
struct Request {
    code: u64,
    badge: u64,
    handle: u32,
    /// The name or reply length, clamped to one page.
    len: usize,
    /// `READ`'s and `WRITE`'s length, clamped to the whole channel.
    bulk_len: usize,
    /// The second request word, whatever the verb makes of it.
    offset: u64,
}

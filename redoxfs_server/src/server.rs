//! **The file service itself**: one RedoxFS image on one [`Disk`], its open-handle table, and every
//! verb the wire's operations reach.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// What one handle names, and what may be done through it. The rights ride on the handle, so a
/// child directory handle is a different authority from its parent held at the same time; see
/// notes/fs-server/the-core.md.
#[derive(Clone, Copy)]
pub(crate) enum Entry {
    /// A directory. Its rights are the full ladder.
    Dir(TreePtr<Node>, Rights),
    /// A file, carrying the [`dir::READ`]/[`dir::WRITE`] bits of the directory it was opened under.
    /// The other rungs are meaningless on a file and are simply never asked about.
    File(TreePtr<Node>, Rights),
}

/// A file service over one RedoxFS image on one [`Disk`]. Generic over the disk so the host tests
/// drive a `DiskMemory`/`DiskFile` and the EL0 binary drives the block-IPC client, with identical
/// filesystem code between them.
pub struct Server<D: Disk> {
    pub(crate) fs: FileSystem<D>,
    /// The open-handle table: `handles[h]` is what handle `h` names, or `None` for a freed slot. A
    /// capability the server issues and checks, never trusts. Slot 0 is the bound directory
    /// ([`filesystem_contract::fs::ROOT`]), installed by [`Server::open`] and never closeable, so
    /// every name resolves under a handle.
    handles: Vec<Option<Entry>>,
    /// A monotonically increasing stand-in for wall-clock seconds, so a write advances the node's
    /// mtime deterministically. The server has no RTC; the value only needs to move forward, and the
    /// host tests assert on file *contents*, not timestamps.
    clock: u64,
    /// Listings and small files, remembered until something changes them (milestone 606). Only
    /// [`Server::look`], [`Server::change`] and [`Server::change_file`] open a transaction, and
    /// the last two are what keep this honest; `memo.rs` has the rule.
    memo: Memo,
    /// The badge that minted each handle, beside [`Server::handles`] and indexed the same way;
    /// 0 for a handle no bound badge minted (milestone 606, ruling D).
    owners: Vec<u64>,
    /// Which badges are scoped to a subtree grant (ruling D, `subtree_scope`).
    bindings: subtree_scope::Bindings<{ filesystem_contract::fs::CLIENT_WINDOWS }>,
}

impl<D: Disk> Server<D> {
    /// Open an existing RedoxFS image and bind the service to its root directory.
    ///
    ///
    /// `FileSystem::open` scans all 256 header-ring slots and keeps the newest whose seahash checks
    /// out, whatever `cleanup` is: that scan is the crash recovery. `cleanup: true` only releases
    /// the nodes nothing points at and commits, so a mount does not leak. `tests/crash_consistency.rs`
    /// measures the recovery; see [`crate::crash`], DECISIONS §34 and
    /// notes/fs-server/crash-consistency.md. The server never uses the creation APIs, which are
    /// std-gated and stay host-side.
    pub fn open(disk: D) -> Result<Self> {
        let fs = FileSystem::open(disk, None, None, true)?;
        Ok(Self {
            fs,
            // Handle 0 carries every right; narrowing happens at the endpoint above, never here.
            // This is `Rights::root`'s only caller.
            handles: alloc::vec![Some(Entry::Dir(TreePtr::root(), Rights::root(dir::ALL)))],
            clock: 1,
            memo: Memo::default(),
            owners: alloc::vec![0],
            bindings: subtree_scope::Bindings::new(),
        })
    }

    /// Resolve `name` under the **bound directory** and return a handle for it: the shorthand for
    /// [`Server::open_file_at`] with [`filesystem_contract::fs::ROOT`], which is what every caller that predates
    /// directory handles means.
    pub fn open_file(&mut self, name: &NameCopy) -> Result<u32> {
        self.open_file_at(filesystem_contract::fs::ROOT as u32, name)
    }

    /// Resolve `name` under the directory `handle` names and return a file handle for it.
    ///
    /// `name` is a single component, never a path. `EINVAL` for anything that is not one component,
    /// [`dir::EISDIR`] for a directory ([`Server::open_dir`] is that verb), and `ENOENT` if there is
    /// no such entry or the directory carries neither [`dir::READ`] nor [`dir::WRITE`]: a holder
    /// that may not reach a name must not learn it is there. The file handle inherits the
    /// directory's read and write bits and cannot widen them (milestone 47).
    pub fn open_file_at(&mut self, handle: u32, name: &NameCopy) -> Result<u32> {
        self.open_file_path(handle, name, 0)
    }

    /// [`Server::open_file_at`], where `name` may be a **relative path** (milestone 606, calef's
    /// ruling A on #1387, 2026-09-27): `a/b/f` is resolved one component at a time, exactly as the
    /// hop-by-hop walk a client used to send, and nothing else. Each directory on the way is an
    /// [`Server::open_dir`] asking for `DESCEND | requested`: it needs `DESCEND` on its parent,
    /// narrows to that request, and refuses with `EPERM` if it would come up short. So the file
    /// handle carries exactly the bits the hop-by-hop walk would have given it. Every component
    /// passes `check_component`, so `..` and empty steps are still `EINVAL`. `requested` is
    /// `OPEN`'s second word; a single-component name ignores it, as `OPEN` always has.
    pub fn open_file_path(&mut self, handle: u32, name: &NameCopy, requested: u64) -> Result<u32> {
        let (parent, rights, name) = self.walk_prefix(handle, name, requested)?;
        if rights.denies_all(dir::READ | dir::WRITE) {
            return Err(Error::new(syscall::error::ENOENT));
        }
        let (ptr, kind) = self.lookup(parent, name)?;
        if kind == Kind::Directory {
            return Err(Error::new(EISDIR));
        }
        // A file, and never a symbolic link or a mount point: `subtree_scope` owns that rule.
        subtree_scope::landing(kind, false).map_err(refused)?;
        // Registered open in the engine's usage table, so an unlink leaves this handle a live node
        // ([`Server::unlink`]). Done directly rather than by `Transaction::on_open_node`, whose
        // transaction clones the allocator; see notes/fs-server/the-core.md.
        let uses = self.fs.node_usages.entry(ptr.id()).or_insert(0);
        *uses = uses.checked_add(1).ok_or(Error::new(EINVAL))?;
        Ok(self.install(Entry::File(ptr, rights.attenuate(dir::READ | dir::WRITE))))
    }

    /// Descend: resolve `name` under the directory `handle` names and hand back a handle to the
    /// child directory, carrying `requested` rights (milestone 47's keystone).
    ///
    /// The child's rights are `parent & requested`, and [`Rights::attenuate`] is the only way this
    /// crate makes a non-root rights set, so no descendant carries a right its parent lacked.
    /// `EPERM` if the grant would be smaller than the request (DECISIONS §42: no silent
    /// degradation). Needs [`dir::DESCEND`] on the parent, whose absence answers `ENOENT`, so a
    /// holder that may not walk in cannot map what is there.
    pub fn open_dir(&mut self, handle: u32, name: &NameCopy, requested: u64) -> Result<u32> {
        // A relative path (milestone 606, ruling A): every directory before the last is a descent
        // asking for `DESCEND | requested`, the shape the hop-by-hop walk sends.
        let (parent, rights, name) = self.walk_prefix(handle, name, requested)?;
        let (ptr, rights) = self.child_dir(parent, rights, name, dir::DESCEND)?;
        let granted = rights.attenuate(requested);
        if granted != Rights::root(requested) {
            return Err(Error::new(EPERM));
        }
        Ok(self.install(Entry::Dir(ptr, granted)))
    }

    /// Make a child directory and hand back a capability to it: `mkdir` as descend-with-creation,
    /// sharing [`Server::open_dir`]'s rights arithmetic exactly.
    ///
    /// Needs [`dir::CREATE`] (else [`dir::EROFS`]) and [`dir::DESCEND`] (else `EPERM`) on the
    /// parent: a directory you could not walk into would mint a capability from a withheld right.
    /// `EEXIST` if the name is already there, and nothing is modified.
    pub fn make_dir(&mut self, handle: u32, name: &NameCopy, requested: u64) -> Result<u32> {
        let name = name.component()?;
        let (parent, rights) = self.dir_at(handle)?;
        if !rights.allows(dir::CREATE) {
            return Err(Error::new(EROFS));
        }
        if !rights.allows(dir::DESCEND) {
            return Err(Error::new(EPERM));
        }
        let granted = rights.attenuate(requested);
        if granted != Rights::root(requested) {
            return Err(Error::new(EPERM));
        }
        self.clock += 1;
        let now = self.clock;
        let node = self.change(|tx| {
            if tx.find_node(parent, name).is_ok() {
                return Err(Error::new(EEXIST));
            }
            tx.create_node(parent, name, Node::MODE_DIR | 0o755, now, 0)
        })?;
        Ok(self.install(Entry::Dir(node.ptr(), granted)))
    }

    /// Enumerate the directory `handle` names into `out` as [`filesystem_contract::dirent`] records
    /// from index `cursor` on, returning the bytes used (0 past the end). [`dir::EPERM`] without
    /// [`dir::ENUMERATE`], never an empty listing (DECISIONS §42). Sorted by name so a cursor means
    /// the same thing across calls, and memoized until the next namespace change.
    ///
    /// # BUGS
    ///
    /// - A name added or removed mid-walk can be seen twice or missed (readdir's usual caveat).
    /// - The first listing of a directory costs a node read per entry; `list` says why.
    pub fn read_dir(&mut self, handle: u32, cursor: u32, out: &mut [u8]) -> Result<usize> {
        let (ptr, rights) = self.dir_at(handle)?;
        if !rights.allows(dir::ENUMERATE) {
            return Err(Error::new(EPERM));
        }
        if let Some(listing) = self.memo.listing(ptr) {
            return Ok(encode_listing(listing, cursor, out));
        }
        let listing = self.look(|tx| list(tx, ptr))?;
        let used = encode_listing(&listing, cursor, out);
        self.memo.keep_listing(ptr, listing);
        Ok(used)
    }

    /// Read up to `buf.len()` bytes from `handle` at `offset`, returning the count (0 at EOF). Passes
    /// atime 0 so a read never advances the node's access time (never *newer* than what is stored),
    /// which keeps reads from triggering a copy-on-write of the node on every call.
    ///
    /// `EBADF` if the handle was not opened for reading, which is POSIX's own answer for reading a
    /// write-only descriptor and is a fact about the handle rather than a policy about the file.
    pub fn read(&mut self, handle: u32, offset: u64, buf: &mut [u8]) -> Result<usize> {
        let ptr = self.file_at(handle, dir::READ, EBADF)?;
        if self.memo.file(ptr).is_none() && !self.memo.is_large(ptr) {
            // First read of this file since anything changed: a small one is read whole, once.
            let body = self.look(|tx| {
                let size = tx.read_tree(ptr)?.data().size();
                if !Memo::would_keep(size) {
                    return Ok(None);
                }
                let mut body = alloc::vec![0u8; size as usize];
                let n = tx.read_node(ptr, 0, &mut body, 0, 0)?;
                body.truncate(n);
                Ok(Some(body))
            })?;
            match body {
                Some(body) => self.memo.keep_file(ptr, body),
                None => self.memo.mark_large(ptr),
            }
        }
        if let Some(body) = self.memo.file(ptr) {
            let start = (offset as usize).min(body.len());
            let n = (body.len() - start).min(buf.len());
            buf[..n].copy_from_slice(&body[start..start + n]);
            return Ok(n);
        }
        self.look(|tx| tx.read_node(ptr, offset, buf, 0, 0))
    }

    /// Write `data` to `handle` at `offset`, returning the count, and advance the logical clock so
    /// the node's mtime moves forward.
    ///
    /// [`dir::EROFS`] without [`dir::WRITE`]. `EFBIG` when `offset + data.len()` overflows or passes
    /// [`MAX_FILE_END`], checked before the engine, which adds the two unchecked (found by the
    /// `redoxfs_server_session` fuzz target; notes/fs-server/the-core.md).
    pub fn write(&mut self, handle: u32, offset: u64, data: &[u8]) -> Result<usize> {
        let ptr = self.file_at(handle, dir::WRITE, EROFS)?;
        match offset.checked_add(data.len() as u64) {
            Some(end) if end <= MAX_FILE_END => {}
            _ => return Err(Error::new(EFBIG)),
        }
        self.clock += 1;
        let now = self.clock;
        self.change_file(ptr, |tx| tx.write_node(ptr, offset, data, now, 0))
    }

    /// The current size, in bytes, of the file a handle names. Allowed through any file handle: a
    /// holder that may write but not read still has to be able to find the end.
    pub fn fstat(&mut self, handle: u32) -> Result<u64> {
        let ptr = self.file_at(handle, 0, EBADF)?;
        if let Some(body) = self.memo.file(ptr) {
            return Ok(body.len() as u64);
        }
        self.look(|tx| Ok(tx.read_tree(ptr)?.data().size()))
    }

    /// Create `name` under the bound directory and return a handle for it (milestone 31 phase 2).
    ///
    /// `EEXIST` if `name` exists, and nothing is created: create is create, not create-or-open,
    /// because silently opening an existing file makes a partly-working write look like a working
    /// one (§27). `check_component` rejects a path rather than walking one.
    pub fn create_file(&mut self, name: &NameCopy) -> Result<u32> {
        self.create_file_at(filesystem_contract::fs::ROOT as u32, name)
    }

    /// [`Server::create_file`], under the directory `handle` names. Needs [`dir::CREATE`] on it;
    /// without it the answer is [`dir::EROFS`], because through this capability that directory takes
    /// no new names.
    pub fn create_file_at(&mut self, handle: u32, name: &NameCopy) -> Result<u32> {
        let name = name.component()?;
        let (parent, rights) = self.dir_at(handle)?;
        if !rights.allows(dir::CREATE) {
            return Err(Error::new(EROFS));
        }
        // Existence is checked inside the same transaction as the create, so two clients racing on
        // one name cannot both be told they created it.
        self.clock += 1;
        let now = self.clock;
        let ptr = self.change(|tx| {
            if tx.find_node(parent, name).is_ok() {
                return Err(Error::new(EEXIST));
            }
            let node = tx.create_node(parent, name, Node::MODE_FILE | 0o644, now, 0)?;
            // Registered as open exactly as `open_file_at` registers, and for its reason: a file
            // created and then unlinked with its handle still open is the temp-file idiom.
            tx.on_open_node(node.ptr())?;
            Ok(node.ptr())
        })?;
        Ok(self.install(Entry::File(ptr, rights.attenuate(dir::READ | dir::WRITE))))
    }

    /// Set the size of the file a handle names to exactly `size` bytes (milestone 31 phase 2).
    /// Shrinking discards the tail and growing extends with zeroes, POSIX `ftruncate`, so a shorter
    /// write can replace a file's contents (§27). [`dir::EROFS`] without [`dir::WRITE`], `EFBIG`
    /// past [`MAX_FILE_END`].
    ///
    /// # BUGS
    ///
    /// - A shrink costs time linear in the logical size, sparse or not, so a writable handle can
    ///   stall every client. Proposed as
    ///   `design/roadmap/0772-a-client-cannot-stall-the-file-server-with-a-sparse-file.md`.
    pub fn truncate(&mut self, handle: u32, size: u64) -> Result<()> {
        // A truncate destroys a file as thoroughly as a write, so it takes the same right and word.
        let ptr = self.file_at(handle, dir::WRITE, EROFS)?;
        // A file grown past the tree reads past it later; [`MAX_FILE_END`] has the defect.
        if size > MAX_FILE_END {
            return Err(Error::new(EFBIG));
        }
        self.clock += 1;
        let now = self.clock;
        self.change_file(ptr, |tx| tx.truncate_node(ptr, size, now, 0))
    }

    /// Move `src` under the directory `src_dir` names to `dst` under the directory `dst_dir` names
    /// (milestone 47): `mv`'s verb, concurrency-atomic and crash-atomic (DECISIONS §42).
    ///
    /// Needs [`dir::REMOVE`] on the source and [`dir::CREATE`] on the destination, each refused
    /// with [`dir::EROFS`] before anything is resolved. `EINVAL` for a directory moved between
    /// directories ([`filesystem_contract::fs::RENAME`]) or a name that is not one component.
    /// `EISDIR` or `ENOTDIR` for replacing a destination of the other kind, a check the engine does
    /// not make; `ENOTEMPTY` from the engine. See notes/fs-server/the-core.md.
    pub fn rename(
        &mut self,
        src_dir: u32,
        src: &NameCopy,
        dst_dir: u32,
        dst: &NameCopy,
    ) -> Result<()> {
        let src = src.component()?;
        let dst = dst.component()?;
        let (src_parent, src_rights) = self.dir_at(src_dir)?;
        let (dst_parent, dst_rights) = self.dir_at(dst_dir)?;
        if !src_rights.allows(dir::REMOVE) || !dst_rights.allows(dir::CREATE) {
            return Err(Error::new(EROFS));
        }
        let same_dir = src_parent.id() == dst_parent.id();
        self.change(|tx| {
            let node = tx.find_node(src_parent, src)?;
            if node.data().is_dir() && !same_dir {
                return Err(Error::new(EINVAL));
            }
            // A replaced destination losing its last link has its attributes purged in this
            // transaction: `rename_node` discards the freed id, and a leaked blob would pass to
            // the next node the engine gives that id.
            let mut replaced = None;
            if let Ok(existing) = tx.find_node(dst_parent, dst) {
                // A no-op rename is a success, not a self-replacement: `remove_node` would take the
                // only link and `link_node` would then have nothing to point at.
                if existing.id() == node.id() {
                    return Ok(());
                }
                if existing.data().is_dir() != node.data().is_dir() {
                    return Err(Error::new(if existing.data().is_dir() {
                        EISDIR
                    } else {
                        ENOTDIR
                    }));
                }
                if existing.data().links() == 1 {
                    replaced = Some(existing.id());
                }
            }
            tx.rename_node(src_parent, src, dst_parent, dst)?;
            // The source keeps its node, and so its attributes: the store keys on nodes, not paths.
            if let Some(id) = replaced {
                purge_attrs(tx, id)?;
            }
            Ok(())
        })
    }

    /// Remove `name` from the directory `handle` names: `rm`'s verb, an unlink and not a revoke.
    ///
    /// Needs [`dir::REMOVE`], refused with [`dir::EROFS`] before the name is resolved. `ENOENT` if
    /// there is no such entry, `EINVAL` if it is not one component, [`dir::EISDIR`] for a directory
    /// ([`filesystem_contract::fs::UNLINK`]). A file still open survives until its last
    /// [`Server::close`] (milestone 47; notes/fs-server/the-core.md).
    ///
    /// # BUGS
    ///
    /// - A handle never closed pins its node for the server's life; the fix is per-client handles.
    pub fn unlink(&mut self, handle: u32, name: &NameCopy) -> Result<()> {
        let name = name.component()?;
        let (parent, rights) = self.dir_at(handle)?;
        if !rights.allows(dir::REMOVE) {
            return Err(Error::new(EROFS));
        }
        // `remove_node` checks the kind against the mode, so `EISDIR` is the engine's. It answers
        // `Some(id)` exactly when the last link went, when the attributes must go too, in this
        // transaction so a crash cannot leave them for the id's next node (DECISIONS §34).
        self.change(|tx| {
            if let Some(id) = tx.remove_node(parent, name, Node::MODE_FILE)? {
                purge_attrs(tx, id)?;
            }
            Ok(())
        })
    }

    /// Remove the empty directory `name` from the directory `handle` names: `rmdir(2)`, and
    /// empty-only is the safety property (milestone 47's `rm -r`, which recurses in userspace).
    /// Needs [`dir::REMOVE`], refused with [`dir::EROFS`] before the name is resolved. `ENOTEMPTY`,
    /// `ENOTDIR` for a file, `EINVAL` if not one component, `ENOENT` if there is no such name.
    ///
    /// # BUGS
    ///
    /// - Directory handles are not registered open, so a handle to the removed directory names
    ///   whichever directory next takes its id, with its rights: a reach outside the grant
    ///   (milestone 633, read not run). See notes/fs-server/the-core.md.
    pub fn rmdir(&mut self, handle: u32, name: &NameCopy) -> Result<()> {
        let name = name.component()?;
        let (parent, rights) = self.dir_at(handle)?;
        if !rights.allows(dir::REMOVE) {
            return Err(Error::new(EROFS));
        }
        // `remove_node` answers `ENOTDIR` and `ENOTEMPTY` itself, inside this transaction; a check
        // out here could only disagree with it. A directory's attributes are purged as a file's
        // are ([`Server::unlink`]).
        self.change(|tx| {
            if let Some(id) = tx.remove_node(parent, name, Node::MODE_DIR)? {
                purge_attrs(tx, id)?;
            }
            Ok(())
        })
    }

    // --- mtime (milestone 47's `touch`, DECISIONS §112) ---
    //
    // All three resolve `name` under a directory handle and check the parent's rights: a file
    // handle's rights are attenuated to `dir::READ | dir::WRITE`, so `dir::SETTIME` could never be
    // checked against one.

    /// **The modification time of `name`, in Unix seconds** (`filesystem_contract::fs::GETMTIME`).
    ///
    /// Needs [`dir::READ`], and its absence is `ENOENT` rather than a mutating-right's `EROFS`:
    /// this is a naming right, [`Server::open_file_at`]'s own rule, checked before the name is
    /// resolved so a holder that may not read a time cannot use this to learn whether a name is
    /// there. Works on a directory exactly as on a file, [`Server::get_xattr`]'s reason.
    pub fn mtime(&mut self, handle: u32, name: &NameCopy) -> Result<u64> {
        let name = name.component()?;
        let (parent, rights) = self.dir_at(handle)?;
        if !rights.allows(dir::READ) {
            return Err(Error::new(ENOENT));
        }
        let node = self.look(|tx| tx.find_node(parent, name))?;
        Ok(node.data().mtime().0)
    }

    /// Set `name`'s modification time to now (`filesystem_contract::fs::SETMTIME`, bare `touch`).
    ///
    /// "Now" is this server's advancing logical clock, not a wall clock (it has no RTC): two touches
    /// in sequence observe increasing times. Needs [`dir::WRITE`], refused with [`dir::EROFS`]
    /// before the name is resolved, and no more, because the server observed the value (DECISIONS
    /// §112). See `filesystem_contract::fs::SETMTIME` and notes/touch.md's `BUGS`.
    pub fn set_mtime_now(&mut self, handle: u32, name: &NameCopy) -> Result<()> {
        let name = name.component()?;
        let (parent, rights) = self.dir_at(handle)?;
        if !rights.allows(dir::WRITE) {
            return Err(Error::new(EROFS));
        }
        self.clock += 1;
        let now = self.clock;
        self.change(|tx| {
            let mut node = tx.find_node(parent, name)?;
            node.data_mut().set_mtime(now, 0);
            tx.sync_tree(node)
        })
    }

    /// Set `name`'s modification time to a caller-supplied Unix-seconds value
    /// (`filesystem_contract::fs::SETMTIME_AT`, `touch -t`): the ability to lie about history.
    ///
    /// Needs [`dir::WRITE`] and [`dir::SETTIME`] (DECISIONS §112 (`touch`'s two behaviors need two
    /// rights: write covers "now", a separate right covers "arbitrary")), refused with
    /// [`dir::EROFS`] before the name is resolved. Any `seconds` is accepted.
    pub fn set_mtime_at(&mut self, handle: u32, name: &NameCopy, seconds: u64) -> Result<()> {
        let name = name.component()?;
        let (parent, rights) = self.dir_at(handle)?;
        if !rights.allows(dir::WRITE | dir::SETTIME) {
            return Err(Error::new(EROFS));
        }
        self.change(|tx| {
            let mut node = tx.find_node(parent, name)?;
            node.data_mut().set_mtime(seconds, 0);
            tx.sync_tree(node)
        })
    }

    // --- Extended attributes (milestone 57, DECISIONS §34's 2026-07-31 amendment) ---
    //
    // A layer the engine knows nothing of: a store in the image keyed on the node's `TreePtr` id
    // (`filesystem_contract::xattr::store`), which nothing can bypass because every path to these
    // bytes goes through `filesystem_contract`. See notes/xattr.md.

    /// Read one extended attribute of the node `handle` names, returning its type code and the
    /// number of bytes written into `out`.
    ///
    /// `EBADF` without [`dir::READ`], as [`Server::read`] answers. `ENODATA` if the attribute is
    /// not set, the only error that means absence (notes/fs-server.md). `ERANGE` if `out` is
    /// shorter than the value, never a short read (DECISIONS §42); on the wire `out` is the shared
    /// page, larger than [`xattr::MAX_VALUE`], so it cannot fire there.
    pub fn get_xattr(&mut self, handle: u32, name: &[u8], out: &mut [u8]) -> Result<(u32, usize)> {
        if !xattr::is_valid_name(name) {
            return Err(Error::new(xattr::ERANGE));
        }
        let ptr = self.node_at(handle, dir::READ, EBADF)?;
        self.look(|tx| {
            let blob = read_attrs(tx, ptr.id())?;
            let (kind, value) = xattr::store::get(&blob, name).ok_or(Error::new(xattr::ENODATA))?;
            if out.len() < value.len() {
                return Err(Error::new(xattr::ERANGE));
            }
            out[..value.len()].copy_from_slice(value);
            Ok((kind, value.len()))
        })
    }

    /// Set one extended attribute on the node `handle` names; creating and replacing are one
    /// operation ([`filesystem_contract::fs::SETXATTR`]).
    ///
    /// [`dir::EROFS`] without [`dir::WRITE`]. The three ceilings answer [`xattr::ERANGE`],
    /// [`xattr::E2BIG`] and [`xattr::ENOSPC`], so a caller can tell which it hit. Crash-atomic with
    /// the file's bytes: the read, rewrite and store write are one transaction (DECISIONS §34).
    pub fn set_xattr(&mut self, handle: u32, name: &[u8], kind: u32, value: &[u8]) -> Result<()> {
        // Checked before anything is allocated or opened, so a client cannot make the server size a
        // buffer from a length it invented. `store::set` checks again, and that is the copy the host
        // tests exercise; this one is the boundary.
        if !xattr::is_valid_name(name) {
            return Err(Error::new(xattr::ERANGE));
        }
        if value.len() > xattr::MAX_VALUE {
            return Err(Error::new(xattr::E2BIG));
        }
        let ptr = self.node_at(handle, dir::WRITE, EROFS)?;
        self.clock += 1;
        let now = self.clock;
        self.change(|tx| {
            let blob = read_attrs(tx, ptr.id())?;
            // Replacing can only shrink, so the old blob plus one whole new record is always enough
            // and `store::set` never has to refuse for want of room.
            let mut out =
                alloc::vec![0u8; blob.len() + xattr::store::record_len(name.len(), value.len())];
            let n = xattr::store::set(&blob, name, kind, value, &mut out).map_err(Error::new)?;
            write_attrs(tx, ptr.id(), &out[..n], now)
        })
    }

    /// **List the attribute names on the node `handle` names**, filling `out` with
    /// [`xattr::list`] records and returning the bytes used. Needs [`dir::READ`], refused with
    /// `EBADF`.
    ///
    /// No cursor, unlike [`Server::read_dir`]: a complete listing fits one page by construction
    /// ([`xattr::MAX_COUNT`]), so it cannot be observed half-changed the way a cursored directory
    /// walk can.
    pub fn list_xattr(&mut self, handle: u32, out: &mut [u8]) -> Result<usize> {
        let ptr = self.node_at(handle, dir::READ, EBADF)?;
        self.fs
            .tx(|tx| Ok(xattr::store::list(&read_attrs(tx, ptr.id())?, out)))
    }

    /// **Remove one extended attribute from the node `handle` names.** Needs [`dir::WRITE`],
    /// refused with [`dir::EROFS`]. `ENODATA` if it was not set, rather than a quiet success: the
    /// caller asked about one specific thing.
    pub fn remove_xattr(&mut self, handle: u32, name: &[u8]) -> Result<()> {
        if !xattr::is_valid_name(name) {
            return Err(Error::new(xattr::ERANGE));
        }
        let ptr = self.node_at(handle, dir::WRITE, EROFS)?;
        self.clock += 1;
        let now = self.clock;
        self.change(|tx| {
            let blob = read_attrs(tx, ptr.id())?;
            let mut out = alloc::vec![0u8; blob.len()];
            let n = xattr::store::remove(&blob, name, &mut out).map_err(Error::new)?;
            write_attrs(tx, ptr.id(), &out[..n], now)
        })
    }

    /// The disk under the filesystem, for a host test that counts what reached it
    /// (`walk_model::Counting`). The serve loop never needs it.
    #[cfg(feature = "hosttest")]
    pub fn disk_mut(&mut self) -> &mut D {
        &mut self.fs.disk
    }

    /// Take the disk back out of the server, **dropping the mount without unmounting**, which is
    /// exactly how a dying process leaves it: no flush, no clean shutdown, whatever is on the platter
    /// is the whole story. Milestone 37's harness uses it to get the recorded write log back after a
    /// workload; the crash tests then reconstruct the platter from that log alone.
    pub fn into_disk(self) -> D {
        self.fs.disk
    }

    /// Release a handle. `EBADF` if it was not open.
    ///
    /// `EINVAL` for the bound directory, and for a bound badge's grant root while it is bound
    /// ([`Server::unbind`] releases that one): closing either would fail every later request or
    /// let a later open reuse the slot outside the grant (found by the `redoxfs_server_session`
    /// fuzz target, 2026-10-04 UTC). Closing a file handle also frees a node unlinked while it was
    /// open. See notes/fs-server/the-core.md.
    pub fn close(&mut self, handle: u32) -> Result<()> {
        if handle as u64 == filesystem_contract::fs::ROOT || self.is_grant_root(handle) {
            return Err(Error::new(EINVAL));
        }
        let entry = self.entry(handle)?;
        self.handles[handle as usize] = None;
        if let Entry::File(ptr, _) = entry {
            // A transaction outside `look`/`change` that keeps the memo: the unlink already forgot
            // the listings, and a freed id is reused only through `change`. The engine's
            // `on_close_node` runs only for a last use with a non-empty release list.
            match self.fs.node_usages.get_mut(&ptr.id()) {
                Some(uses) if *uses > 1 => *uses -= 1,
                Some(_) if self.fs.header.release.is_null() => {
                    self.fs.node_usages.remove(&ptr.id());
                }
                Some(_) => self.fs.tx(|tx| tx.on_close_node(ptr))?,
                None => {}
            }
        }
        Ok(())
    }

    /// How much room the image has ([`filesystem_contract::fs::STATFS`], milestone 54 (a network file service a Mac can actually mount)), as
    /// `(block_size, total_blocks, free_blocks)`.
    ///
    /// The handle is validated and not used: holding any minted handle entitles a caller to ask, so
    /// `EBADF` is the whole check and no right is demanded. Both numbers are read outside a
    /// transaction, from the committed header and allocator, so a read-only verb makes no commit.
    /// See notes/fs-server/the-core.md.
    pub fn statfs(&mut self, handle: u32) -> Result<(u64, u64, u64)> {
        self.entry(handle)?;
        let block = redoxfs::BLOCK_SIZE;
        // Integer division, so a size that is not a whole number of blocks reports the blocks that
        // exist rather than a fraction the caller would round the wrong way.
        Ok((
            block,
            self.fs.header.size() / block,
            self.fs.allocator().free(),
        ))
    }

    /// May this handle ask for the storage to be made durable? (milestone 55). `Ok(())` when it
    /// may; the caller then does the IO.
    ///
    /// The permission half of `filesystem_contract::fs::SYNC`, and it performs no flush: RedoxFS's
    /// `Disk` has no sync method, so the flush is a `filesystem_contract::blk::FLUSH` in the binary.
    /// Takes a handle of either kind and needs [`dir::WRITE`], refused with [`dir::EROFS`];
    /// `filesystem_contract::fs::SYNC` has the rights argument.
    ///
    /// Name: provisional, minted by milestone 55's durability lane on 2026-08-18.
    pub fn sync_permitted(&self, handle: u32) -> Result<()> {
        self.node_at(handle, dir::WRITE, EROFS).map(|_| ())
    }

    /// Install an entry in the handle table, reusing a freed slot before growing. Returns the
    /// handle. Slot 0 is never free, so a minted handle is never 0 and never shadows the root.
    fn install(&mut self, entry: Entry) -> u32 {
        if let Some(i) = self.handles.iter().position(|s| s.is_none()) {
            self.handles[i] = Some(entry);
            self.owners[i] = 0;
            i as u32
        } else {
            self.handles.push(Some(entry));
            self.owners.push(0);
            (self.handles.len() - 1) as u32
        }
    }

    // --- Ruling D: subtree grants this server enforces itself (milestone 606) ---------------------

    /// **The handle a request under `badge` may use.** `subtree_scope::admit` decides: an unbound
    /// badge's handle passes through, a bound badge's `ROOT` is its grant's root and any other
    /// handle must be one it minted, and a revoked badge reaches nothing (`EBADF`).
    pub fn admit(&self, badge: u64, requested: u64) -> Result<u32> {
        let owner = match self.handles.get(requested as usize) {
            Some(Some(_)) => Some(self.owners[requested as usize]),
            _ => None,
        };
        subtree_scope::admit(self.bindings.of(badge), badge, requested, owner)
            .map(|h| h as u32)
            .map_err(refused)
    }

    /// Whether some badge's grant is rooted at `handle` (see [`Server::close`]).
    fn is_grant_root(&self, handle: u32) -> bool {
        (1..filesystem_contract::fs::CLIENT_WINDOWS as u64).any(|b| {
            self.bindings.of(b)
                == subtree_scope::Binding::Bound {
                    root: handle as u64,
                }
        })
    }

    /// Whether `badge` is bound or revoked, rather than carrying the endpoint's whole authority.
    pub fn scoped(&self, badge: u64) -> bool {
        self.bindings.of(badge) != subtree_scope::Binding::Open
    }

    /// Record that `badge` minted `handle`, if `badge` is bound. Called for every handle a verb
    /// hands back, so a bound badge can name what it opened and nothing any other client opened.
    pub fn claim(&mut self, handle: u32, badge: u64) {
        if matches!(
            self.bindings.of(badge),
            subtree_scope::Binding::Bound { .. }
        ) && let Some(owner) = self.owners.get_mut(handle as usize)
        {
            *owner = badge;
        }
    }

    /// **`BIND`: scope `badge` to the directory `handle` names**, asked for by a caller whose
    /// badge is `caller`. The rules are `subtree_scope::Bindings::bind`'s; the directory's rights
    /// are what the grant carries, fixed when the binder opened it.
    pub fn bind(&mut self, caller: u64, handle: u32, badge: u64) -> Result<()> {
        self.dir_at(handle)?;
        self.bindings
            .bind(caller, badge, handle as u64)
            .map_err(refused)
    }

    /// **`BIND_SYNC`: make `badge` sync-only** (milestone 805 (`reboot` at the prompt)), asked
    /// for by a caller whose badge is `caller`. `subtree_scope::Bindings::bind_sync_only` has the
    /// rules; [`Server::handle`] enforces what it means.
    ///
    /// Name: ratified 2026-10-06 (calef, #1783: "Approve sync for the file-server
    /// request and its capability, and flush for the device cache only.").
    pub fn bind_sync_only(&mut self, caller: u64, badge: u64) -> Result<()> {
        self.bindings.bind_sync_only(caller, badge).map_err(refused)
    }

    /// Whether `badge` may `SYNC` and nothing else.
    ///
    /// Name: ratified 2026-10-06 (calef, #1783: "Approve sync for the file-server
    /// request and its capability, and flush for the device cache only.").
    pub fn is_sync_only(&self, badge: u64) -> bool {
        self.bindings.of(badge) == subtree_scope::Binding::SyncOnly
    }

    /// **`UNBIND`: take `badge`'s grant back.** Closes every handle the badge minted and the grant's
    /// root, and leaves the badge revoked, so it reaches nothing from then on.
    pub fn unbind(&mut self, caller: u64, badge: u64) -> Result<()> {
        let root = self.bindings.unbind(caller, badge).map_err(refused)?;
        let minted: Vec<u32> = (0..self.owners.len())
            .filter(|&h| self.owners[h] == badge && self.handles[h].is_some())
            .map(|h| h as u32)
            .collect();
        for h in minted {
            self.owners[h as usize] = 0;
            let _ = self.close(h);
        }
        if root != filesystem_contract::fs::ROOT {
            let _ = self.close(root as u32);
        }
        Ok(())
    }

    /// What a handle names, or `EBADF`. Every operation reaches the table through here or through
    /// one of the two typed wrappers below, so a forged or stale handle is refused in one place.
    fn entry(&self, handle: u32) -> Result<Entry> {
        self.handles
            .get(handle as usize)
            .copied()
            .flatten()
            .ok_or(Error::new(EBADF))
    }

    /// The directory a handle names and the rights it carries. [`dir::ENOTDIR`] for a file handle,
    /// which is a fact about the handle rather than a refusal of the request.
    fn dir_at(&self, handle: u32) -> Result<(TreePtr<Node>, Rights)> {
        match self.entry(handle)? {
            Entry::Dir(ptr, rights) => Ok((ptr, rights)),
            Entry::File(..) => Err(Error::new(ENOTDIR)),
        }
    }

    /// The file a handle names, once its rights carry `needed`. `refusal` is the errno to answer
    /// when they do not, which differs by direction on purpose: a read through a write-only handle
    /// is `EBADF` (POSIX's answer, a fact about the handle) and a write through a read-only one is
    /// [`dir::EROFS`] (`fs_file_caretaker`'s answer, a fact about the capability).
    fn file_at(&self, handle: u32, needed: u64, refusal: i32) -> Result<TreePtr<Node>> {
        match self.entry(handle)? {
            Entry::File(ptr, rights) if rights.allows(needed) => Ok(ptr),
            Entry::File(..) => Err(Error::new(refusal)),
            Entry::Dir(..) => Err(Error::new(EISDIR)),
        }
    }

    /// The node a handle names, **file or directory alike**, once its rights carry `needed`.
    ///
    /// The only place in this file that does not care which kind a handle is, and extended
    /// attributes are the only thing that does not care: a directory carries them exactly as a file
    /// does, so a verb that refused one kind would be inventing a distinction the feature has not
    /// got. `refusal` differs by direction for [`Server::file_at`]'s reason.
    fn node_at(&self, handle: u32, needed: u64, refusal: i32) -> Result<TreePtr<Node>> {
        let (ptr, rights) = match self.entry(handle)? {
            Entry::Dir(ptr, rights) | Entry::File(ptr, rights) => (ptr, rights),
        };
        if rights.allows(needed) {
            Ok(ptr)
        } else {
            Err(Error::new(refusal))
        }
    }
    /// Every component of `path` but the last, descended from `handle`, and the last component.
    /// The descent is `subtree_scope::walk`, which ruling D (milestone 606, 2026-09-27) makes the one
    /// place any path is resolved: each step is exactly the `OPENDIR` a client could have sent,
    /// asking for `DESCEND | hop`, and a step through a symbolic link or a mount point is refused.
    /// This server's own naming rules (`check_component`) run on every component first, so a
    /// malformed path is `EINVAL` whatever the tree holds.
    fn walk_prefix<'n>(
        &mut self,
        handle: u32,
        path: &'n NameCopy,
        hop: u64,
    ) -> Result<(TreePtr<Node>, Rights, &'n str)> {
        let (ptr, rights) = self.dir_at(handle)?;
        // Checked here, after `dir_at`, so a bad handle answers before a bad name, as it always has.
        let path = path.path()?;
        // `subtree_scope::walk` takes bytes rather than a `NameCopy`: it is a shared crate and cannot
        // name this server's type. A deliberate exception to the typed boundary, and safe only because
        // this is its one caller here and the bytes are the checked copy's, never the window's.
        let (ptr, rights, last) =
            subtree_scope::walk(ptr, rights, path.as_bytes(), hop, |dir, name| {
                let name = core::str::from_utf8(name).map_err(|_| Fault(Error::new(EINVAL)))?;
                self.lookup(dir, name).map_err(Fault)
            })
            .map_err(|Fault(e)| e)?;
        // `last` is a suffix of `path`, which is UTF-8, cut at a `/`.
        Ok((ptr, rights, &path[path.len() - last.len()..]))
    }

    /// `name` in the directory `parent`, and what it is: from the memo when it can answer, from
    /// RedoxFS otherwise, and remembered.
    fn lookup(&mut self, parent: TreePtr<Node>, name: &str) -> Result<(TreePtr<Node>, Kind)> {
        match self.memo.lookup(parent, name) {
            Some(Some(found)) => Ok(found),
            Some(None) => Err(Error::new(ENOENT)),
            None => {
                let node = self.look(|tx| tx.find_node(parent, name))?;
                let kind = kind_of(node.data());
                self.memo.keep_found(parent, name, node.ptr(), kind);
                Ok((node.ptr(), kind))
            }
        }
    }

    /// The child directory `name` of `parent`, once `rights` allows `needed` (`ENOENT` otherwise).
    fn child_dir(
        &mut self,
        parent: TreePtr<Node>,
        rights: Rights,
        name: &str,
        needed: u64,
    ) -> Result<(TreePtr<Node>, Rights)> {
        if !rights.allows(needed) {
            return Err(Error::new(syscall::error::ENOENT));
        }
        let (ptr, kind) = self.lookup(parent, name)?;
        subtree_scope::landing(kind, true).map_err(refused)?;
        Ok((ptr, rights))
    }

    /// **A transaction that changes nothing**: every read-only verb's way in. RedoxFS commits a
    /// transaction that wrote nothing as a no-op, so this costs no device write.
    fn look<T>(&mut self, f: impl FnOnce(&mut Transaction<D>) -> Result<T>) -> Result<T> {
        self.fs.tx(f)
    }

    /// **A transaction that changes the namespace or a node**: every mutating verb's way in, and
    /// the reason [`Memo`] is never stale. It forgets everything first; forgetting is always safe,
    /// so it does not matter whether the change then succeeds.
    fn change<T>(&mut self, f: impl FnOnce(&mut Transaction<D>) -> Result<T>) -> Result<T> {
        self.memo.forget();
        self.fs.tx(f)
    }

    /// [`Server::change`] for a change to one file's bytes, which no listing depends on.
    fn change_file<T>(
        &mut self,
        ptr: TreePtr<Node>,
        f: impl FnOnce(&mut Transaction<D>) -> Result<T>,
    ) -> Result<T> {
        self.memo.forget_file(ptr);
        self.fs.tx(f)
    }
}

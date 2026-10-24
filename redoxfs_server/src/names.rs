//! **What a name may be, and how a walk under a subtree grant meets this server**: the single
//! component rule, and the glue between RedoxFS nodes and errors and `subtree_scope`.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// What a RedoxFS node is, in the terms `subtree_scope` decides with. RedoxFS has no mount points.
pub(crate) fn kind_of(node: &Node) -> Kind {
    if node.is_dir() {
        Kind::Directory
    } else if node.is_symlink() {
        Kind::Symlink
    } else {
        Kind::File
    }
}

/// A `subtree_scope` refusal as this crate's error.
pub(crate) fn refused(r: subtree_scope::Refusal) -> Error {
    Error::new(r.errno())
}

/// An engine error carried through `subtree_scope::walk`'s lookup, so a device error on the way
/// down is reported as itself rather than as a refusal. It exists because neither the engine's
/// error type nor the scope crate's is this crate's, so no `From` between them may be written.
pub(crate) struct Fault(pub(crate) Error);

impl From<subtree_scope::Refusal> for Fault {
    fn from(r: subtree_scope::Refusal) -> Self {
        Fault(refused(r))
    }
}

/// A name the server owns, copied out of where it arrived before anything judges it (milestone
/// 825 (a hostile client races the file server's name window), calef's "Refused names" ruling,
/// 2026-10-15 UTC). The only constructor, [`NameCopy::copy_in`], copies into this value's own
/// bounded buffer and validates UTF-8 on the copy, so a name read straight from a client's window
/// cannot reach a verb. The text is reachable only through `NameCopy::component` and
/// `NameCopy::path`, which run `check_component` first. See notes/fs-server/the-core.md.
///
/// A window-borrowed `&str` does not reach a verb:
///
/// ```compile_fail,E0308
/// # fn reach<D: redoxfs::Disk>(srv: &mut redoxfs_server::Server<D>, window: &[u8]) {
/// let name = core::str::from_utf8(window).unwrap();
/// let _ = srv.open_file_at(0, name);
/// # }
/// ```
///
/// A copy does:
///
/// ```
/// # fn reach<D: redoxfs::Disk>(
/// #     srv: &mut redoxfs_server::Server<D>,
/// #     window: &[u8],
/// # ) -> redoxfs_server::engine_error::Result<u32> {
/// let name = redoxfs_server::NameCopy::copy_in(window)?;
/// srv.open_file_at(0, &name)
/// # }
/// ```
///
/// Name: provisional (lane milestone/825, 2026-10-15 UTC); an architect names types.
pub struct NameCopy {
    bytes: [u8; NAME_MAX],
    len: usize,
}

/// The longest name a [`NameCopy`] holds: one block, the length `Server::handle` clamps every
/// name to, so a copy from the window never truncates.
const NAME_MAX: usize = filesystem_contract::blk::BLOCK_SIZE;

impl NameCopy {
    /// Copy `src` into a new value and validate UTF-8 on the copy. `EINVAL` for invalid UTF-8 or a
    /// name longer than one block, the answers a malformed name has always had.
    pub fn copy_in(src: &[u8]) -> Result<Self> {
        if src.len() > NAME_MAX {
            return Err(Error::new(EINVAL));
        }
        let mut bytes = [0u8; NAME_MAX];
        bytes[..src.len()].copy_from_slice(src);
        core::str::from_utf8(&bytes[..src.len()]).map_err(|_| Error::new(EINVAL))?;
        Ok(Self {
            bytes,
            len: src.len(),
        })
    }

    /// The copy as text. `copy_in` validated it and the buffer is never written again, so the
    /// empty fallback cannot happen; if it ever did, the empty name fails every check closed.
    fn text(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap_or_default()
    }

    /// The copy as one checked component: `check_component`'s rule, on the server's bytes.
    pub(crate) fn component(&self) -> Result<&str> {
        let name = self.text();
        check_component(name)?;
        Ok(name)
    }

    /// The copy as a checked relative path: every `/`-separated component passes
    /// `check_component`, so `..` and empty steps are `EINVAL` whatever the tree holds.
    pub(crate) fn path(&self) -> Result<&str> {
        let path = self.text();
        for step in path.split('/') {
            check_component(step)?;
        }
        Ok(path)
    }
}

/// Reject anything that is not a single path component, with `EINVAL`.
///
/// Our contract's rule, enforced here rather than in the vendored engine, whose `check_name` passes
/// `/`, `.` and `..` (notes/fs-server.md). It also refuses `\`, `:` and the attribute store's
/// directory name, in every directory, so attributes are never reachable as ordinary bytes
/// (DECISIONS §34 (RedoxFS is the primary filesystem, on three conditions)'s amendment);
/// [`Server::read_dir`] does not list it either. `EINVAL` because such a name is not expressible
/// here. See notes/fs-server/the-core.md.
pub(crate) fn check_component(name: &str) -> Result<()> {
    // What a name is anywhere is `subtree_scope`'s (ruling D); what RedoxFS adds is this server's.
    if !subtree_scope::is_name(name.as_bytes())
        || name == xattr::STORE_DIR
        || name.contains('\\')
        || name.contains(':')
    {
        return Err(Error::new(EINVAL));
    }
    Ok(())
}

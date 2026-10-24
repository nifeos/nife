//! What the server remembers between requests, for as long as nothing has changed (milestone 606
//! (a directory walk costs what it does on Linux), provisional number).
//!
//! The dentry and page caches a Unix kernel answers a walk from, sized for an 8 MiB heap:
//!
//! - a directory's listing, sorted and filtered as `READDIR` sends it, with each child's node and
//!   kind. `READDIR` pages come from it and `OPEN` and `OPENDIR` binary-search it instead of
//!   `find_node`. It is kept whole, so a name it lacks is a name the directory lacks.
//! - a name found one at a time in a directory nobody listed. Only names that exist are kept.
//! - a small file's bytes, read whole on first `READ`. `FSTAT` answers from them too.

//! # The one rule that keeps it correct
//!
//! Every verb that changes the image goes through [`crate::Server`]'s `change` (which forgets
//! everything) or `change_file` (which forgets one file's bytes), and read-only verbs go through
//! `look`; a verb added later has to say which it is. The one exception is `close`, whose comment
//! says why. See notes/fs-server/caches-and-io.md and notes/walk-cost.md.

//! # BUGS
//!
//! - Coarse: one `mkdir` anywhere forgets every listing everywhere, so a workload that creates
//!   files while it walks gets the uncached cost back after every create. Per-directory
//!   invalidation is the refinement, once something measures that workload.
//! - No eviction policy: a full budget drops that whole half and refills it, so a working set
//!   larger than the budget thrashes.
//! - Bytes, not pages: a file is memoized whole, up to [`FILE_MAX`], or not at all.
//!
//! Name: provisional 2026-09-26 (milestone 606's lane); calef names modules.

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use redoxfs::{Node, TreePtr};
use subtree_scope::Kind;

/// The largest file whose bytes are kept. The walk fixture's largest is 256 KiB, and a source
/// tree's files are almost all smaller.
pub const FILE_MAX: usize = 256 * 1024;

/// The most file bytes kept at once. A quarter of the server's 8 MiB heap; the walk fixture is
/// 334,000 bytes.
pub const FILE_BUDGET: usize = 2 * 1024 * 1024;

/// The most names kept across every listing, about 64 bytes each with their node: roughly 1 MiB.
pub const NAME_BUDGET: usize = 16 * 1024;

/// One child in a memoized listing.
pub struct Child {
    pub name: Box<str>,
    pub ptr: TreePtr<Node>,
    /// What the node is, as `subtree_scope` needs to know it: a symbolic link is kept apart from
    /// a file so that no walk steps through one and no open lands on one (ruling D).
    pub kind: Kind,
}

/// The names found in one directory: name to its node and what it is.
type Found = BTreeMap<Box<str>, (TreePtr<Node>, Kind)>;

/// See the module header.
#[derive(Default)]
pub struct Memo {
    /// Directory node id to its listing, sorted by name.
    listings: BTreeMap<u32, Vec<Child>>,
    /// Names found one at a time in directories that were never listed: what `OPEN` and
    /// `OPENDIR` learn from `find_node`, kept so the next open of the same name does not ask again.
    /// Only names that exist; a miss is asked every time.
    found: BTreeMap<u32, Found>,
    /// Names held across `listings` and `found`, against [`NAME_BUDGET`].
    names: usize,
    /// File node id to its whole contents.
    files: BTreeMap<u32, Vec<u8>>,
    file_bytes: usize,
    /// Files known to be larger than [`FILE_MAX`], so a read of one does not first walk the tree
    /// to learn its size again.
    large: BTreeSet<u32>,
}

impl Memo {
    /// Forget everything: a namespace changed.
    pub fn forget(&mut self) {
        self.listings.clear();
        self.found.clear();
        self.names = 0;
        self.forget_files();
    }

    fn forget_files(&mut self) {
        self.files.clear();
        self.file_bytes = 0;
        self.large.clear();
    }

    /// Forget one file's bytes: it was written, truncated or closed for the last time.
    pub fn forget_file(&mut self, ptr: TreePtr<Node>) {
        if let Some(body) = self.files.remove(&ptr.id()) {
            self.file_bytes -= body.len();
        }
        self.large.remove(&ptr.id());
    }

    /// The listing of the directory `dir`, if it is memoized.
    pub fn listing(&self, dir: TreePtr<Node>) -> Option<&[Child]> {
        self.listings.get(&dir.id()).map(Vec::as_slice)
    }

    /// Keep `children` (already sorted) as `dir`'s listing, dropping every listing first if it
    /// would not fit.
    pub fn keep_listing(&mut self, dir: TreePtr<Node>, children: Vec<Child>) {
        if children.len() > NAME_BUDGET {
            return;
        }
        if self.names + children.len() > NAME_BUDGET {
            self.listings.clear();
            self.found.clear();
            self.names = 0;
        }
        // A listing answers every name, so the names found one at a time in it are dropped.
        if let Some(found) = self.found.remove(&dir.id()) {
            self.names -= found.len();
        }
        self.names += children.len();
        if let Some(old) = self.listings.insert(dir.id(), children) {
            self.names -= old.len();
        }
    }

    /// Keep one name `find_node` found in `dir`.
    pub fn keep_found(&mut self, dir: TreePtr<Node>, name: &str, child: TreePtr<Node>, kind: Kind) {
        if self.listings.contains_key(&dir.id()) {
            return; // the listing already answers it
        }
        if self.names + 1 > NAME_BUDGET {
            self.listings.clear();
            self.found.clear();
            self.names = 0;
        }
        if self
            .found
            .entry(dir.id())
            .or_default()
            .insert(name.into(), (child, kind))
            .is_none()
        {
            self.names += 1;
        }
    }

    /// `name` in `dir`: `None` if nothing is memoized that answers it, `Some(None)` if `dir`'s
    /// listing is memoized and the name is not in it, which is as good as `find_node`'s `ENOENT`.
    pub fn lookup(&self, dir: TreePtr<Node>, name: &str) -> Option<Option<(TreePtr<Node>, Kind)>> {
        let Some(list) = self.listing(dir) else {
            let found = self.found.get(&dir.id())?.get(name)?;
            return Some(Some(*found));
        };
        Some(
            list.binary_search_by(|c| (*c.name).cmp(name))
                .ok()
                .map(|i| (list[i].ptr, list[i].kind)),
        )
    }

    /// A file's memoized bytes.
    pub fn file(&self, ptr: TreePtr<Node>) -> Option<&[u8]> {
        self.files.get(&ptr.id()).map(Vec::as_slice)
    }

    /// Whether a file of `size` bytes is one this would keep.
    pub fn would_keep(size: u64) -> bool {
        size as usize <= FILE_MAX
    }

    /// Whether the file is known to be one this will not keep.
    pub fn is_large(&self, ptr: TreePtr<Node>) -> bool {
        self.large.contains(&ptr.id())
    }

    /// Remember that the file is too large to keep, until it changes.
    pub fn mark_large(&mut self, ptr: TreePtr<Node>) {
        self.large.insert(ptr.id());
    }

    /// Keep `body` as the file's bytes, dropping every file first if it would not fit.
    pub fn keep_file(&mut self, ptr: TreePtr<Node>, body: Vec<u8>) {
        if body.len() > FILE_MAX {
            return;
        }
        if self.file_bytes + body.len() > FILE_BUDGET {
            self.forget_files();
        }
        self.file_bytes += body.len();
        if let Some(old) = self.files.insert(ptr.id(), body) {
            self.file_bytes -= old.len();
        }
    }
}

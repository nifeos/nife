//! **`ls` and `cat`**: what an entry is, and the two read verbs that report one.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// What an entry is. `Other` covers the modes RedoxFS can hold and a host directory cannot
/// meaningfully receive (sockets, and anything a later format version adds).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    File,
    Dir,
    Symlink,
    Other,
}

impl Kind {
    /// Fixed-width label for `ls` output.
    pub fn label(self) -> &'static str {
        match self {
            Kind::File => "file",
            Kind::Dir => "dir ",
            Kind::Symlink => "link",
            Kind::Other => "?   ",
        }
    }
}

pub(crate) fn kind_of(node: &Node) -> Kind {
    if node.is_dir() {
        Kind::Dir
    } else if node.is_file() {
        Kind::File
    } else if node.is_symlink() {
        Kind::Symlink
    } else {
        Kind::Other
    }
}

/// One directory entry, as `ls` reports it.
pub struct LsEntry {
    pub name: String,
    pub size: u64,
    pub kind: Kind,
    /// How many extended attributes this entry carries. Rendered as `@`, which is the marker macOS
    /// `ls -l` uses for the same fact, so a listing says that there is metadata here **before** the
    /// reader has to know that `.nife-attrs` exists.
    pub attrs: usize,
}

/// List a directory: name, size, and kind of every entry, sorted by name. `path` may be `/` or
/// empty for the image root.
pub fn ls(volume: Volume, path: &str) -> Result<Vec<LsEntry>, String> {
    let comps = components(path)?;
    let mut fs = open_ro(volume)?;
    fs.tx(|transaction| Ok(ls_transaction(transaction, &comps)))
        .map_err(|e| format!("listing {} failed: {e}", volume.path.display()))?
}

pub(crate) fn ls_transaction<D: Disk>(
    transaction: &mut Transaction<D>,
    comps: &[&str],
) -> Result<Vec<LsEntry>, String> {
    let node = resolve(transaction, comps)?;
    if !node.data().is_dir() {
        return Err(format!("/{}: not a directory", comps.join("/")));
    }
    let mut children = Vec::new();
    transaction
        .child_nodes(node.ptr(), &mut children)
        .map_err(|e| format!("/{}: listing failed: {e}", comps.join("/")))?;
    let store = find_store(transaction)?;
    // Names and pointers first, so the store lookup below can borrow `transaction` mutably (a `DirEntry`
    // would keep it borrowed for the whole loop). Same shape as `extract_node`'s.
    let entries: Vec<(String, TreePtr<Node>)> = children
        .iter()
        .filter_map(|e| e.name().map(|n| (n.to_string(), e.node_ptr())))
        .collect();
    let mut out = Vec::new();
    for (name, ptr) in entries {
        let child = transaction
            .read_tree(ptr)
            .map_err(|e| format!("{name}: cannot read its node: {e}"))?;
        // A store that cannot be read is a listing that says nothing about attributes, not a listing
        // that fails: `ls` is the first thing a person runs on a damaged image.
        let attrs = match attr_blob(transaction, store, ptr.id()) {
            Ok(blob) => xattr::store::count(&blob),
            Err(msg) => {
                eprintln!("redoxfs_host: {name}: {msg}");
                0
            }
        };
        out.push(LsEntry {
            name,
            size: child.data().size(),
            kind: kind_of(child.data()),
            attrs,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Read a whole file out of the image. A directory is an error, not an empty result.
pub fn cat(volume: Volume, path: &str) -> Result<Vec<u8>, String> {
    let comps = components(path)?;
    let mut fs = open_ro(volume)?;
    fs.tx(|transaction| Ok(cat_transaction(transaction, &comps)))
        .map_err(|e| format!("reading {path} from {} failed: {e}", volume.path.display()))?
}

pub(crate) fn cat_transaction<D: Disk>(
    transaction: &mut Transaction<D>,
    comps: &[&str],
) -> Result<Vec<u8>, String> {
    let node = resolve(transaction, comps)?;
    if node.data().is_dir() {
        return Err(format!("/{}: is a directory", comps.join("/")));
    }
    read_all(transaction, &node)
}

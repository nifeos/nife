//! **Extended attributes, read back off an image**: the store's directory, one node's blob, and
//! the decode that says when a blob had bytes it could not read.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

// --- The attribute store, from the recovery side (milestone 57) --------------------------------
//
// `.nife-attrs` in an image's root holds one file per node with extended attributes, named for
// its `TreePtr` id in hex, in the records `filesystem_contract::xattr::store` defines (a crate, per
// CLAUDE.md rule 7); only the FS server writes it. Nothing here may fail an extraction: a problem
// is a skip the caller counts and walks past, so the files come out when the metadata does not.

/// One extended attribute, as recovery sees it: the name and value as bytes, plus the type code
/// [`filesystem_contract::xattr`] carries and no host filesystem can hold.
pub struct Attr {
    pub name: Vec<u8>,
    pub kind: u32,
    pub value: Vec<u8>,
}

/// The attribute store's directory, or `None` if this image has never held an attribute.
///
/// **`ENOENT` is the only error read as absence.** Any other one is propagated, because a dropped
/// read of the root's tree block answers `EIO`, and treating that as "there are no attributes" would
/// quietly recover a backup without its metadata and report success. That is the same false negative
/// the FS server's own `find_store` refuses to make (notes/fs-server.md).
pub(crate) fn find_store<D: Disk>(
    tx: &mut Transaction<D>,
) -> Result<Option<TreePtr<Node>>, String> {
    match tx.find_node(TreePtr::root(), xattr::STORE_DIR) {
        Ok(node) => Ok(Some(node.ptr())),
        Err(e) if e.errno == syscall::error::ENOENT => Ok(None),
        Err(e) => Err(format!(
            "cannot read the attribute store {}: {e}",
            xattr::STORE_DIR
        )),
    }
}

/// One node's raw attribute blob, empty if it has none.
///
/// A blob longer than [`xattr::store::MAX_BYTES`] is refused rather than clipped: the layer cannot
/// have written it, so reading its first 53 KB would be reading a file this format does not describe.
pub(crate) fn attr_blob<D: Disk>(
    tx: &mut Transaction<D>,
    store: Option<TreePtr<Node>>,
    node_id: u32,
) -> Result<Vec<u8>, String> {
    let Some(dir_ptr) = store else {
        return Ok(Vec::new());
    };
    let file = xattr::store::file_name(node_id);
    // ASCII by construction (`file_name` emits hex digits), so this cannot fail; the arm exists so a
    // future change there cannot silently alias two nodes onto one blob.
    let name = std::str::from_utf8(&file)
        .map_err(|_| format!("node {node_id:#010x}: its store file name is not UTF-8"))?;
    let node = match tx.find_node(dir_ptr, name) {
        Ok(node) => node,
        Err(e) if e.errno == syscall::error::ENOENT => return Ok(Vec::new()),
        Err(e) => return Err(format!("{}/{name}: {e}", xattr::STORE_DIR)),
    };
    let size = node.data().size() as usize;
    if size > xattr::store::MAX_BYTES {
        return Err(format!(
            "{}/{name}: {size} bytes, more than the {} an attribute blob can be; not read",
            xattr::STORE_DIR,
            xattr::store::MAX_BYTES,
        ));
    }
    read_all(tx, &node)
}

/// Decode a blob into attributes, plus the number of **trailing bytes that are not a record**.
///
/// `store::iter` stops at the first record that runs off the end, which is what a torn write leaves
/// and is the behaviour that keeps a truncated blob reading short rather than wrong. That is right
/// for the FS server and not enough for a recovery tool, which should say that something was there
/// and could not be read. Trailing zeroes do not count: an empty blob is removed rather than
/// zero-filled, so a zero tail means a shorter record list and nothing lost.
pub(crate) fn decode_attrs(blob: &[u8]) -> (Vec<Attr>, usize) {
    let mut out = Vec::new();
    let mut used = 0;
    for (name, kind, value) in xattr::store::iter(blob) {
        used += xattr::store::record_len(name.len(), value.len());
        out.push(Attr {
            name: name.to_vec(),
            kind,
            value: value.to_vec(),
        });
    }
    let tail = &blob[used.min(blob.len())..];
    let unread = if tail.iter().all(|b| *b == 0) {
        0
    } else {
        tail.len()
    };
    (out, unread)
}

/// Every extended attribute on `path` inside the image, in the order the store holds them (which is
/// the order they were set, since a replacement keeps its place).
pub fn attrs(volume: Volume, path: &str) -> Result<Vec<Attr>, String> {
    let comps = components(path)?;
    let mut fs = open_ro(volume)?;
    fs.tx(|tx| Ok(attrs_tx(tx, &comps)))
        .map_err(|e| format!("reading the attributes of {path} failed: {e}"))?
}

pub(crate) fn attrs_tx<D: Disk>(
    tx: &mut Transaction<D>,
    comps: &[&str],
) -> Result<Vec<Attr>, String> {
    let node = resolve(tx, comps)?;
    let store = find_store(tx)?;
    let blob = attr_blob(tx, store, node.ptr().id())?;
    let (attrs, unread) = decode_attrs(&blob);
    if unread > 0 {
        eprintln!(
            "redoxfs_host: /{}: {unread} trailing bytes in its attribute blob are not a record \
             and were not read",
            comps.join("/"),
        );
    }
    Ok(attrs)
}

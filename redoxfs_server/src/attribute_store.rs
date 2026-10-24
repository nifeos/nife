//! **Extended attributes in the image**: the store directory in the root, one blob file per node,
//! and the reads, writes and purges that keep it.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// The name of the file holding one node's attribute blob: its `TreePtr` id in hex. ASCII by
/// construction, so the conversion cannot fail; the error arm exists so a future change to
/// [`xattr::store::file_name`] cannot silently alias two nodes onto one blob.
pub(crate) fn store_file(id: u32) -> Result<([u8; 8], usize)> {
    let bytes = xattr::store::file_name(id);
    if core::str::from_utf8(&bytes).is_err() {
        return Err(Error::new(EIO));
    }
    Ok((bytes, 8))
}

/// The attribute store's directory in the image root, or `None` if nothing has ever set an
/// attribute on this filesystem.
///
/// **`ENOENT` is the only error read as absence**, and every other one is propagated. A dropped
/// write to the root's tree block makes a lookup answer `EIO`, and reading that as "there are no
/// attributes" is exactly the false negative that made milestone 37's harness report filesystems
/// that never existed (notes/fs-server.md). Here it would silently hand a client an empty attribute
/// set for a file that has some.
pub(crate) fn find_store<D: Disk>(tx: &mut Transaction<D>) -> Result<Option<TreePtr<Node>>> {
    match tx.find_node(TreePtr::root(), xattr::STORE_DIR) {
        Ok(node) => Ok(Some(node.ptr())),
        Err(e) if e.errno == ENOENT => Ok(None),
        Err(e) => Err(e),
    }
}

/// One node's attribute blob, or empty if it has none. Never partial: a blob longer than the layer
/// can hold is `EIO`, because the alternative is reading whatever is in the first
/// [`xattr::store::MAX_BYTES`] of a file this layer did not write.
pub(crate) fn read_attrs<D: Disk>(tx: &mut Transaction<D>, node_id: u32) -> Result<Vec<u8>> {
    let Some(dir_ptr) = find_store(tx)? else {
        return Ok(Vec::new());
    };
    let (bytes, n) = store_file(node_id)?;
    let name = core::str::from_utf8(&bytes[..n]).map_err(|_| Error::new(EIO))?;
    let node = match tx.find_node(dir_ptr, name) {
        Ok(node) => node,
        Err(e) if e.errno == ENOENT => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let size = node.data().size() as usize;
    if size > xattr::store::MAX_BYTES {
        return Err(Error::new(EIO));
    }
    let mut blob = alloc::vec![0u8; size];
    let read = tx.read_node(node.ptr(), 0, &mut blob, 0, 0)?;
    blob.truncate(read);
    Ok(blob)
}

/// Replace one node's attribute blob. An **empty** blob removes the store file rather than leaving a
/// zero-length one behind, so "no file" and "no attributes" stay the same statement and the store
/// does not accumulate an entry per node that ever held an attribute.
pub(crate) fn write_attrs<D: Disk>(
    tx: &mut Transaction<D>,
    node_id: u32,
    blob: &[u8],
    now: u64,
) -> Result<()> {
    if blob.is_empty() {
        return purge_attrs(tx, node_id);
    }
    let (bytes, n) = store_file(node_id)?;
    let name = core::str::from_utf8(&bytes[..n]).map_err(|_| Error::new(EIO))?;
    let dir_ptr = match find_store(tx)? {
        Some(ptr) => ptr,
        // 0o700 rather than 0o755: the mode is decoration here (this contract has no uid to check
        // it against), and the narrower one is what a recovery tool's listing should suggest.
        None => tx
            .create_node(
                TreePtr::root(),
                xattr::STORE_DIR,
                Node::MODE_DIR | 0o700,
                now,
                0,
            )?
            .ptr(),
    };
    let ptr = match tx.find_node(dir_ptr, name) {
        Ok(node) => node.ptr(),
        Err(e) if e.errno == ENOENT => tx
            .create_node(dir_ptr, name, Node::MODE_FILE | 0o600, now, 0)?
            .ptr(),
        Err(e) => return Err(e),
    };
    tx.write_node(ptr, 0, blob, now, 0)?;
    // Then shrink to exactly the new length: a write does not truncate, and a shorter blob would
    // leave the old tail for the reader to walk (DECISIONS §27).
    tx.truncate_node(ptr, blob.len() as u64, now, 0)?;
    Ok(())
}

/// Take one node's attributes away. Called from [`Server::unlink`], [`Server::rmdir`] and
/// [`Server::rename`] **inside the same transaction as the removal**, which is the whole of why a
/// freed node id cannot hand its attributes to the next node issued that id.
///
/// A node with no store file is not an error: most nodes have no attributes.
pub(crate) fn purge_attrs<D: Disk>(tx: &mut Transaction<D>, node_id: u32) -> Result<()> {
    let Some(dir_ptr) = find_store(tx)? else {
        return Ok(());
    };
    let (bytes, n) = store_file(node_id)?;
    let name = core::str::from_utf8(&bytes[..n]).map_err(|_| Error::new(EIO))?;
    match tx.remove_node(dir_ptr, name, Node::MODE_FILE) {
        // Only when a blob really went, so the cost falls on attribute removals rather than on
        // every unlink on the filesystem.
        Ok(_) => drop_store_if_empty(tx),
        Err(e) if e.errno == ENOENT => Ok(()),
        Err(e) => Err(e),
    }
}

/// Take the store directory away with the last blob in it, so a filesystem that holds no
/// attributes is indistinguishable from one that never held any.
///
/// `remove_node` with `MODE_DIR` refuses a non-empty directory with `ENOTEMPTY`, so the engine's
/// check is the emptiness test and costs one lookup. `ENOENT` is tolerated too; every other error is
/// propagated, because a store that failed to be removed is not a store known to be gone. See
/// notes/fs-server/the-core.md.
pub(crate) fn drop_store_if_empty<D: Disk>(tx: &mut Transaction<D>) -> Result<()> {
    match tx.remove_node(TreePtr::root(), xattr::STORE_DIR, Node::MODE_DIR) {
        Ok(_) => Ok(()),
        Err(e) if e.errno == ENOTEMPTY || e.errno == ENOENT => Ok(()),
        Err(e) => Err(e),
    }
}

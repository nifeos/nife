//! **Paths inside an image**: splitting one into components, walking it from the root, and reading
//! a node's data whole.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// Split an image path into components. Leading, trailing and doubled slashes are ignored, `.` is
/// ignored, and **`..` is refused**: paths resolve from the image root downward and nothing else.
/// That is the same rule the FS server enforces on the wire (notes/fs-server.md), kept here so a
/// path means the same thing whether nife or this tool resolves it.
pub(crate) fn components(path: &str) -> Result<Vec<&str>, String> {
    let mut out = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                return Err(format!(
                    "{path}: `..` is not accepted, paths resolve from the image root downward"
                ));
            }
            name => out.push(name),
        }
    }
    Ok(out)
}

/// Walk `comps` from the image root and return the node it names.
pub(crate) fn resolve<D: Disk>(
    transaction: &mut Transaction<D>,
    comps: &[&str],
) -> Result<TreeData<Node>, String> {
    // Annotated because `read_tree` is generic over the block type it decodes.
    let mut node: TreeData<Node> = transaction
        .read_tree(TreePtr::root())
        .map_err(|e| format!("cannot read the image root: {e}"))?;
    for (i, name) in comps.iter().enumerate() {
        if !node.data().is_dir() {
            return Err(format!("/{} is not a directory", comps[..i].join("/")));
        }
        node = transaction
            .find_node(node.ptr(), name)
            .map_err(|e| format!("/{}: {e}", comps[..=i].join("/")))?;
    }
    Ok(node)
}

/// Read a whole node's data. Loops because a short read is legal (`read_node_inner` stops at a
/// record boundary in principle), so a file bigger than one record must not depend on a single call
/// filling the buffer. The record size is a per-node field and this loop does not care what it is.
pub(crate) fn read_all<D: Disk>(
    transaction: &mut Transaction<D>,
    node: &TreeData<Node>,
) -> Result<Vec<u8>, String> {
    let size = node.data().size() as usize;
    let mut buf = vec![0u8; size];
    let mut got = 0;
    while got < size {
        let n = transaction
            .read_node_inner(node, got as u64, &mut buf[got..])
            .map_err(|e| format!("read failed at offset {got}: {e}"))?;
        if n == 0 {
            break;
        }
        got += n;
    }
    buf.truncate(got);
    Ok(buf)
}

//! **A directory's listing as `READDIR` sends it**, and the encode that writes it from a cursor.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// A directory's listing as `READDIR` sends it: every child but the attribute store, sorted by
/// name so a cursor means the same thing across calls, each with its node and kind.
///
/// # BUGS
///
/// - Every child costs a node read, because a RedoxFS directory entry does not store the child's
///   type and the record carries [`filesystem_contract::dirent::IS_DIR`]. Milestone 121 (`ripgrep`
///   on nife: enumeration as a capability) measured 43 to 85 us per entry under HVF on 2026-09-26;
///   since milestone 606 (a directory walk costs what it does on Linux) the [`Memo`] pays it once
///   per directory until something changes.
pub(crate) fn list<D: Disk>(
    tx: &mut Transaction<D>,
    dir: TreePtr<Node>,
) -> Result<Vec<memo::Child>> {
    let mut children = Vec::new();
    tx.child_nodes(dir, &mut children)?;
    // The attribute store is not part of the namespace (milestone 57 (extended attributes)). It is
    // filtered before any cursor applies, so the cursor counts only what the client can see.
    let mut out = Vec::with_capacity(children.len());
    for entry in &children {
        let Some(name) = entry.name() else { continue };
        if name == xattr::STORE_DIR {
            continue;
        }
        let child: redoxfs::TreeData<Node> = tx.read_tree(entry.node_ptr())?;
        out.push(memo::Child {
            name: name.into(),
            ptr: entry.node_ptr(),
            kind: kind_of(child.data()),
        });
    }
    out.sort_unstable_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Fill `out` with `listing`'s records from `cursor` on, and return the bytes used.
pub(crate) fn encode_listing(listing: &[memo::Child], cursor: u32, out: &mut [u8]) -> usize {
    let mut used = 0;
    for child in listing.iter().skip(cursor as usize) {
        match filesystem_contract::dirent::encode(
            &mut out[used..],
            child.name.as_bytes(),
            child.kind == Kind::Directory,
        ) {
            Some(n) => used += n,
            // A record is never split: stop before one that will not fit and let the caller's next
            // cursor start here. That is what makes "the buffer filled" and "the directory ended"
            // distinguishable rather than a guess.
            None => break,
        }
    }
    used
}

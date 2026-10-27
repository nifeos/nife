//! **`extract`**: copying a subtree out of an image onto the host, attributes included.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// What an `extract` moved, for the one-line summary the tool prints.
#[derive(Default)]
pub struct ExtractStats {
    pub files: u64,
    pub dirs: u64,
    pub symlinks: u64,
    pub bytes: u64,
    /// Entries the host directory cannot receive; named on stderr as they are met.
    pub skipped: u64,
    /// Extended attributes put back onto the extracted files.
    pub attrs: u64,
    /// Attributes the host would not take. **Counted and named on stderr, never dropped quietly**
    /// (DECISIONS §42): the usual causes are a destination filesystem with no attribute support, a
    /// Linux host refusing a name without a `user.` prefix, and a Linux host refusing any attribute
    /// on a symlink. Each is something a person can act on, and each has a different answer.
    pub attrs_skipped: u64,
    /// Attributes whose bytes were reattached but whose **type code** could not be, because no host
    /// filesystem has a per-attribute type word. The codes stay readable in the extracted
    /// `.nife-attrs` blobs. Only counted for a code that is not [`xattr::RAW`], since dropping a
    /// zero loses nothing.
    pub kinds_dropped: u64,
}

/// Extract the subtree at `path` to the host path `dest`, which **becomes** that subtree: a
/// directory in the image lands as a directory at `dest`, a file lands as the file `dest`. That is
/// `cp -R SRC DEST` with a `DEST` that does not exist yet, and it is unambiguous in a way "copy
/// into" is not.
///
/// **Extended attributes come back with the files** where the host will take them (milestone 57).
/// Anything it refuses is counted and named on stderr rather than dropped, and the raw store is
/// copied out beside the tree either way, so a refusal costs presentation and not data. See
/// notes/host-recovery.md.
pub fn extract(volume: Volume, path: &str, dest: &Path) -> Result<ExtractStats, String> {
    let comps = components(path)?;
    let mut fs = open_ro(volume)?;
    fs.tx(|transaction| Ok(extract_transaction(transaction, &comps, dest)))
        .map_err(|e| {
            format!(
                "extracting {path} from {} failed: {e}",
                volume.path.display()
            )
        })?
}

pub(crate) fn extract_transaction<D: Disk>(
    transaction: &mut Transaction<D>,
    comps: &[&str],
    dest: &Path,
) -> Result<ExtractStats, String> {
    let node = resolve(transaction, comps)?;
    // Found once, from the image root, and carried down the walk. The store's location does not
    // depend on which subtree is being extracted, which is why extracting `nested/deep` still
    // recovers the attributes of what is in it.
    let store = find_store(transaction)?;
    let mut stats = ExtractStats::default();
    extract_node(transaction, &node, dest, store, &mut stats)?;
    Ok(stats)
}

/// Put one node's extended attributes back onto the host file just written for it.
///
/// Every failure here is reported and counted, and none of them stops the extraction. The three that
/// actually happen are worth naming where a reader meets them: a destination filesystem that holds
/// no attributes (`ENOTSUP`), a Linux host refusing a name with no `user.` prefix (`EPERM`), and a
/// Linux host refusing any attribute on a symlink (`EPERM` again, which is why the message carries
/// the errno rather than an interpretation of it).
pub(crate) fn reattach<D: Disk>(
    transaction: &mut Transaction<D>,
    node: &TreeData<Node>,
    dest: &Path,
    store: Option<TreePtr<Node>>,
    stats: &mut ExtractStats,
) {
    let blob = match attr_blob(transaction, store, node.ptr().id()) {
        Ok(blob) => blob,
        Err(msg) => {
            eprintln!("redoxfs_host: {}: {msg}", dest.display());
            stats.attrs_skipped += 1;
            return;
        }
    };
    if blob.is_empty() {
        return;
    }
    let (attrs, unread) = decode_attrs(&blob);
    if unread > 0 {
        eprintln!(
            "redoxfs_host: {}: {unread} trailing bytes in its attribute blob are not a record \
             and were not reattached",
            dest.display(),
        );
        stats.attrs_skipped += 1;
    }
    for attr in attrs {
        let shown = String::from_utf8_lossy(&attr.name).into_owned();
        match host_xattr::set(dest, &attr.name, &attr.value) {
            Ok(()) => {
                stats.attrs += 1;
                if attr.kind != xattr::RAW {
                    // Named per attribute rather than summarised, because the code is the one part
                    // of this record the host cannot carry and a reader who wants it needs to know
                    // which file to go back to `.nife-attrs` for.
                    eprintln!(
                        "redoxfs_host: {}: attribute {shown} kept its {} bytes but not its type \
                         code {:#010x}; host filesystems have no field for it (see \
                         .nife-attrs in the extracted tree)",
                        dest.display(),
                        attr.value.len(),
                        attr.kind,
                    );
                    stats.kinds_dropped += 1;
                }
            }
            Err(e) => {
                eprintln!(
                    "redoxfs_host: {}: cannot reattach attribute {shown} ({} bytes): {e}",
                    dest.display(),
                    attr.value.len(),
                );
                stats.attrs_skipped += 1;
            }
        }
    }
}

/// Reject a name that would not stay inside the destination directory.
///
/// The image is the untrusted input here: it may be damaged, or it may have been written by
/// something other than this engine. RedoxFS does not itself permit `/` in a name, so this is
/// belt and braces rather than a known hole, and it is exactly the check a tool that writes
/// image-supplied names into a host filesystem must not omit.
pub(crate) fn safe_component(name: &str) -> Result<(), String> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\0') {
        return Err(format!(
            "refusing to extract an entry named {name:?}: it would not stay inside the destination"
        ));
    }
    Ok(())
}

/// Permission bits to give an extracted file or directory.
///
/// Masked to `0o777`, dropping setuid, setgid and the sticky bit. Those three carry authority on
/// the host, and an image is data we are recovering, not something that gets to hand out authority
/// on the machine doing the recovery.
pub(crate) fn host_mode(node: &Node) -> u32 {
    u32::from(node.mode()) & 0o777
}

pub(crate) fn extract_node<D: Disk>(
    transaction: &mut Transaction<D>,
    node: &TreeData<Node>,
    dest: &Path,
    store: Option<TreePtr<Node>>,
    stats: &mut ExtractStats,
) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    match kind_of(node.data()) {
        Kind::Dir => {
            std::fs::create_dir_all(dest)
                .map_err(|e| format!("cannot create {}: {e}", dest.display()))?;
            let mut children = Vec::new();
            transaction
                .child_nodes(node.ptr(), &mut children)
                .map_err(|e| format!("cannot list {}: {e}", dest.display()))?;
            // Names and pointers first: the recursion needs `transaction` mutably, and a borrowed
            // `DirEntry` would keep it borrowed for the whole loop.
            let entries: Vec<(String, TreePtr<Node>)> = children
                .iter()
                .filter_map(|e| e.name().map(|n| (n.to_string(), e.node_ptr())))
                .collect();
            for (name, ptr) in entries {
                safe_component(&name)?;
                let child = transaction
                    .read_tree(ptr)
                    .map_err(|e| format!("{name}: cannot read its node: {e}"))?;
                extract_node(transaction, &child, &dest.join(&name), store, stats)?;
            }
            // Attributes before the permissions, for the same reason the permissions come last: a
            // directory already made read-only can still refuse a `setxattr` on some filesystems,
            // and a directory whose own attributes failed for that reason would be a self-inflicted
            // skip.
            reattach(transaction, node, dest, store, stats);
            // Permissions last: a directory extracted read-only would otherwise refuse its own
            // children.
            std::fs::set_permissions(dest, PermissionsExt::from_mode(host_mode(node.data())))
                .map_err(|e| format!("cannot set the mode of {}: {e}", dest.display()))?;
            stats.dirs += 1;
        }
        Kind::File => {
            let data = read_all(transaction, node)?;
            std::fs::write(dest, &data)
                .map_err(|e| format!("cannot write {}: {e}", dest.display()))?;
            reattach(transaction, node, dest, store, stats);
            std::fs::set_permissions(dest, PermissionsExt::from_mode(host_mode(node.data())))
                .map_err(|e| format!("cannot set the mode of {}: {e}", dest.display()))?;
            stats.files += 1;
            stats.bytes += data.len() as u64;
        }
        Kind::Symlink => {
            // A RedoxFS symlink stores its target as the node's data, which is how `archive_at`
            // writes one. Recreate it as a symlink rather than following it: a dangling link
            // recovered faithfully beats a copy of whatever the *host* has at that path today.
            let target = read_all(transaction, node)?;
            let target = String::from_utf8(target)
                .map_err(|_| format!("{}: symlink target is not UTF-8", dest.display()))?;
            let _ = std::fs::remove_file(dest);
            std::os::unix::fs::symlink(&target, dest)
                .map_err(|e| format!("cannot link {} -> {target}: {e}", dest.display()))?;
            // The attributes of the **link**, not of what it points at. macOS takes them; Linux
            // refuses `user.*` on a symlink and the refusal is counted rather than hidden.
            reattach(transaction, node, dest, store, stats);
            stats.symlinks += 1;
        }
        Kind::Other => {
            eprintln!(
                "redoxfs_host: skipping {} (mode {:#06o} is not a file, directory or symlink)",
                dest.display(),
                node.data().mode()
            );
            stats.skipped += 1;
        }
    }
    Ok(())
}

//! **The verbs that change an image**: `mkfs`, `put` and `import`, and the read-write open they
//! share.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// Seconds and nanoseconds since the epoch, the timestamp shape the engine wants.
pub(crate) fn now() -> (u64, u32) {
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    (t.as_secs(), t.subsec_nanos())
}

/// Open an existing image read-write, for the paths that change it. `cleanup: true` matches
/// upstream's mount path (it replays the header ring to the newest consistent generation and tidies
/// allocations).
pub(crate) fn open_rw(image: &Path) -> Result<FileSystem<DiskFile>, String> {
    let disk =
        DiskFile::open(image).map_err(|e| format!("cannot open {}: {e}", image.display()))?;
    FileSystem::open(disk, None, None, true).map_err(|e| open_failed(image, 0, e))
}

/// Create a fresh, empty RedoxFS image of `size` bytes at `image`. Unencrypted, no reserved
/// bootloader area. Fails if the size cannot hold the header ring plus a minimal tree.
///
/// **Start from an empty file.** `DiskFile::create` opens with `create(true)` but does NOT
/// truncate, so running `mkfs` over an existing image leaves that image's stale blocks past the
/// new (smaller) write, and the result fails to open ("not a RedoxFS image: I/O error"). Removing
/// the file first makes `mkfs` idempotent, which the phase-2 test flow relies on (it regenerates
/// the image every run).
pub fn mkfs(image: &Path, size: u64) -> Result<(), String> {
    let _ = std::fs::remove_file(image); // ignore "not found"; any other error surfaces below
    let disk = DiskFile::create(image, size)
        .map_err(|e| format!("cannot create {}: {e}", image.display()))?;
    let (secs, nsec) = now();
    FileSystem::create(disk, None, secs, nsec)
        .map_err(|e| format!("mkfs on {} failed: {e}", image.display()))?;
    Ok(())
}

/// Write `data` as the file `path`, creating it or truncating an existing one. The parent
/// directory must already exist: this makes files, and `import` makes trees.
pub fn put(image: &Path, path: &str, data: &[u8]) -> Result<(), String> {
    let comps = components(path)?;
    let mut fs = open_rw(image)?;
    fs.tx(|tx| Ok(put_tx(tx, &comps, data)))
        .map_err(|e| format!("putting {path} into {} failed: {e}", image.display()))?
}

pub(crate) fn put_tx<D: Disk>(
    tx: &mut Transaction<D>,
    comps: &[&str],
    data: &[u8],
) -> Result<(), String> {
    let Some((name, parents)) = comps.split_last() else {
        return Err("no file name given".to_string());
    };
    let (secs, nsec) = now();
    let parent = resolve(tx, parents)?;
    if !parent.data().is_dir() {
        return Err(format!("/{}: not a directory", parents.join("/")));
    }
    let node_ptr = match tx.find_node(parent.ptr(), name) {
        Ok(node) => {
            tx.truncate_node(node.ptr(), 0, secs, nsec)
                .map_err(|e| format!("{name}: truncate failed: {e}"))?;
            node.ptr()
        }
        Err(_) => tx
            .create_node(parent.ptr(), name, Node::MODE_FILE | 0o644, secs, nsec)
            .map_err(|e| format!("{name}: create failed: {e}"))?
            .ptr(),
    };
    tx.write_node(node_ptr, 0, data, secs, nsec)
        .map_err(|e| format!("{name}: write failed: {e}"))?;
    Ok(())
}

/// Copy the *contents* of the host directory `dir` into the image root, recursively.
///
/// This is upstream's own archiver (`redoxfs::archive`, the engine half of `redoxfs-ar`), so an
/// `extract` proven against it is proven against a writer other than ours; `redoxfs-ar` itself
/// creates the filesystem, and this fills one `mkfs` made. A failure part-way leaves what was
/// written committed and consistent: the import is incomplete, and the tool says so and exits
/// non-zero.
pub fn import(image: &Path, dir: &Path) -> Result<(), String> {
    let mut fs = open_rw(image)?;
    redoxfs::archive(&mut fs, dir).map_err(|e| {
        format!(
            "importing {} into {} failed: {e}",
            dir.display(),
            image.display()
        )
    })?;
    Ok(())
}

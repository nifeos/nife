//! **The farm's input stamp**: a hash of everything that decides what the farm holds, so a stale
//! farm is rebuilt rather than trusted.
//!
//! A child of `farm` (§282 (when a file is split, the parent becomes an index)): what was private
//! to `farm` is `pub(super)` here, which reaches the same modules.
//!
//! Name: provisional (lane maintainer/near-ceiling-splits, 2026-10-12 UTC), for the module and the
//! file.

use std::collections::BTreeMap;

use super::*;

/// A cheap FNV-1a over a byte slice, folded into the running hash. No crypto, no dep: this only
/// needs to notice when a PAL input changed so the farm (and thus the build-std cache) is rebuilt.
pub(super) fn fnv(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// Hash everything that determines the farm's contents: the toolchain version, the patch-logic
/// version, the crate files copied in verbatim ([`COPIED_FILES`]), the target specs, and every overlay file.
/// A mismatch means the linked toolchain is stale and std must be rebuilt from patched source.
pub(crate) fn std_inputs_stamp() -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    h = fnv(h, &STD_SRC_PATCH_VERSION.to_le_bytes());
    if let Some(v) = capture("rustc", &["-vV"]) {
        h = fnv(h, v.as_bytes());
    }
    let root = workspace_root();
    // The crate files `std_generate_modules` copies, from the same directories it reads them in.
    // A failure to find them is hashed too, so the stamp cannot match a built farm and `std_src`
    // goes on to the copy, which names the failure.
    let empty = BTreeMap::new();
    let packages = match copied_package_directories() {
        Ok(directories) => directories,
        Err(e) => {
            h = fnv(h, e.as_bytes());
            &empty
        }
    };
    let mut files: Vec<PathBuf> = COPIED_FILES
        .iter()
        .filter_map(|copied| Some(packages.get(copied.package)?.join(copied.file)))
        .collect();
    files.extend([
        root.join("targets/aarch64-unknown-nife.json"),
        root.join("targets/riscv64-unknown-nife.json"),
        root.join("targets/x86_64-unknown-nife.json"),
    ]);
    // A crate root that names its modules out of line (`pub mod x;`) is copied with each `x.rs`
    // inlined, so each of those files is an input too: milestone 845 (the filesystem-protocol crate
    // root is split along its seams), and §274 (the filesystem-protocol crate root splits into its
    // module files).
    let roots = files.clone();
    for f in &roots {
        if let Ok(text) = std::fs::read_to_string(f) {
            files.extend(out_of_line_modules(f, &text).into_iter().map(|(_, p)| p));
        }
    }
    collect_files(&root.join("patches/std-nife/overlay"), &mut files);
    // The heap's crate, vendored, which `std_generate_talc` copies from.
    collect_files(&talc_source_dir().join("src"), &mut files);
    files.sort();
    for f in files {
        // Hash a name that does not depend on where the checkout lives (`stamp_name`): the stamp
        // answers "are the farm's inputs unchanged", and where the checkout lives is not an input.
        // An absolute path would make two trees with identical inputs never match, and `std_src`
        // would rebuild the farm every time, at minutes a rebuild.
        h = fnv(h, stamp_name(&f, &root, packages).as_bytes());
        if let Ok(bytes) = std::fs::read(&f) {
            h = fnv(h, &bytes);
        }
    }
    h
}

/// Walk `dir` and push every regular file into `out` (used to fingerprint the overlay tree).
pub(super) fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_files(&p, out);
        } else {
            out.push(p);
        }
    }
}

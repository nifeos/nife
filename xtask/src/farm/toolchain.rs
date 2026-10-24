//! **The farm and this worktree's toolchain link**: where the farm lives, the toolchain name it
//! is linked under, and the sweep of links that point nowhere.
//!
//! A child of `farm` (§282 (when a file is split, the parent becomes an index)): what was private
//! to `farm` is `pub(super)` here, which reaches the same modules.
//!
//! Name: provisional (lane maintainer/near-ceiling-splits, 2026-10-12 UTC), for the module and the
//! file.

use super::*;

pub(super) fn farm_dir() -> PathBuf {
    workspace_root().join("target/nife-farm")
}

/// **This worktree's toolchain name**: `nife-dev-<directory>-<hash>`, such as
/// `nife-dev-per-worktree-toolchain-3f9a12c0`.
///
/// The directory's name is there so `rustup toolchain list` reads as a list of worktrees. The hash
/// is of the canonical workspace path, because two checkouts can share a directory name (a
/// stranger-test clone and the main checkout are both `nife`) and a shared name is the whole bug
/// this replaces. It is a function of the path alone, so a worktree keeps its name across runs and
/// nothing has to be recorded.
pub(crate) fn toolchain_name() -> String {
    let root = workspace_root();
    toolchain_name_for(&std::fs::canonicalize(&root).unwrap_or(root))
}

pub(super) fn toolchain_name_for(root: &Path) -> String {
    let base: String = root
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .take(48)
        .collect();
    let h = fnv(
        0xcbf2_9ce4_8422_2325u64,
        root.as_os_str().as_encoded_bytes(),
    );
    // The fold keeps eight hex digits from all sixty-four bits rather than the low half alone.
    format!("{TOOLCHAIN_PREFIX}{base}-{:08x}", (h ^ (h >> 32)) as u32)
}

/// `$RUSTUP_HOME/toolchains`, where `rustup toolchain link` writes its symlinks.
pub(super) fn toolchains_dir() -> Option<PathBuf> {
    std::env::var_os("RUSTUP_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".rustup")))
        .map(|h| h.join("toolchains"))
}

/// **Remove every per-worktree link whose farm is gone**, which is what a pruned worktree leaves.
///
/// A dangling per-worktree link breaks nothing else (no other worktree resolves through it), so
/// this is housekeeping, run from `std-src` so it needs nobody to remember it. It removes only a
/// symlink whose name starts with [`TOOLCHAIN_PREFIX`] and whose target no longer exists: a live
/// worktree's farm exists, and the legacy bare `nife-dev` does not carry the prefix's trailing
/// hyphen, so neither can match. A worktree caught between `std-src`'s `remove_dir_all` and its
/// `create_dir_all` could lose its link here; it relinks at the end of that same `std-src`, and no
/// build reads the name. Returns the names removed.
pub(super) fn sweep_dangling_links(dir: &Path) -> Vec<String> {
    let mut gone = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return gone;
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let p = e.path();
        let is_link = std::fs::symlink_metadata(&p).is_ok_and(|m| m.file_type().is_symlink());
        // `exists()` follows the link, so it is false exactly when the farm is gone.
        if name.starts_with(TOOLCHAIN_PREFIX)
            && is_link
            && !p.exists()
            && std::fs::remove_file(&p).is_ok()
        {
            gone.push(name);
        }
    }
    gone.sort();
    gone
}

/// The real nightly sysroot the farm is hardlink-cloned from.
pub(super) fn real_sysroot() -> Option<PathBuf> {
    capture("rustc", &["--print", "sysroot"]).map(|s| PathBuf::from(s.trim()))
}

/// The farm's patched std source root (`.../library/std/src`).
pub(super) fn farm_std_src() -> PathBuf {
    farm_dir().join("lib/rustlib/src/rust/library/std/src")
}

/// Where this worktree's own toolchain name currently resolves, if it is a link we can read.
///
/// `rustup toolchain link` writes a symlink under `$RUSTUP_HOME/toolchains`, so the target is
/// readable without shelling out. `None` covers every shape we cannot interpret (no such link, a
/// real directory rather than a symlink, an unreadable home), and the caller treats `None` as
/// "cannot prove it is ours", which links. Linking when it was already correct costs one idempotent
/// `rustup` call.
pub(super) fn linked_farm() -> Option<PathBuf> {
    std::fs::read_link(toolchains_dir()?.join(toolchain_name())).ok()
}

/// Link this worktree's own name at its farm if it does not already point there, and sweep the
/// links pruned worktrees left behind.
///
/// The name is this worktree's alone, so a mismatch here means the farm moved or the link was
/// never made (a fresh worktree, a cleaned `target/`), never that another lane took it. Builds name
/// the farm by path and do not depend on this; it is for a person typing `+<name>`.
pub(super) fn link_own_toolchain() -> bool {
    if let Some(dir) = toolchains_dir() {
        for name in sweep_dangling_links(&dir) {
            eprintln!("std-src: removed `{name}`, whose worktree's farm no longer exists");
        }
    }
    let farm = farm_dir();
    let name = toolchain_name();
    // Canonicalize both sides: a worktree reached through a symlinked path (/tmp on macOS is one)
    // would otherwise compare unequal to the same directory recorded literally, and relink on every
    // single call.
    let canon = |p: &PathBuf| std::fs::canonicalize(p).unwrap_or_else(|_| p.clone());
    if linked_farm().map(|l| canon(&l)) == Some(canon(&farm)) {
        return true;
    }
    if !run("rustup", &["toolchain", "link", &name, &s(farm.clone())]) {
        eprintln!("std-src: `rustup toolchain link {name}` failed");
        return false;
    }
    eprintln!("std-src: linked this worktree's farm as `{name}` (cargo +{name} ...)");
    true
}

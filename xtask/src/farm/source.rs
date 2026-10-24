//! **`std-src`**: materialize the patched std source as a farm and link it as this worktree's
//! toolchain.
//!
//! A child of `farm` (§282 (when a file is split, the parent becomes an index)): what was private
//! to `farm` is `pub(super)` here, which reaches the same modules.
//!
//! Name: provisional (lane maintainer/near-ceiling-splits, 2026-10-12 UTC), for the module and the
//! file.

use super::*;

/// **Materialize the patched std toolchain** (milestone 27 (Rust `std` on the native ABI)), linked
/// as [`toolchain_name`]. Returns whether the farm is built and linked.
///
/// build-std reads std's source from the sysroot of the rustc it invokes, so the farm is a clone of
/// the real nightly (`cp -al`, sharing blocks) whose rustc anchors are real copies
/// ([`unlink_sysroot_anchors`]) and whose `src` subtree is a real copy, patched: the overlay PAL
/// files, the ABI/heap crates generated verbatim, and a `target_os = "nife"` arm in std's
/// `cfg_select!` dispatchers. The real toolchain is never touched. A stamp of all inputs guards the
/// rebuild, so a warm farm and its build-std cache survive across runs. See notes/std.md, "The
/// toolchain: build-std against a patched rust-src".
pub(crate) fn std_src() -> bool {
    let stamp = std_inputs_stamp();
    let stamp_file = farm_dir().join(".nife-stamp");
    if farm_std_src().is_dir()
        && std::fs::read_to_string(&stamp_file).ok().as_deref() == Some(&stamp.to_string())
    {
        // A warm farm still checks its link: a cleaned `toolchains/`, or a worktree whose link
        // was swept, would otherwise stay unnamed until its stamp next changed.
        if !link_own_toolchain() {
            return false;
        }
        return true;
    }

    let Some(real) = real_sysroot() else {
        eprintln!("std-src: cannot find the nightly sysroot (rustc --print sysroot)");
        return false;
    };
    let farm = farm_dir();
    eprintln!("--- std-src: building the patched std toolchain (this recompiles std) ---");

    // Fresh farm. `cp -al` clones bin+lib as hardlinks; the src subtree is then a real copy so
    // patching it never mutates the shared rustup toolchain.
    let _ = std::fs::remove_dir_all(&farm);
    if let Err(e) = std::fs::create_dir_all(&farm) {
        eprintln!("std-src: cannot create {}: {e}", farm.display());
        return false;
    }
    let cp = |args: &[&str]| run("cp", args);
    // Hard links cannot cross filesystems, and a container can mount the checkout on a different
    // one from `~/.rustup`, as can a Mac whose worktrees sit on their own APFS volume
    // (notes/dev-machine-disk.md). A real copy costs about 1.2 GiB per farm and works everywhere.
    // Ask the device numbers first rather than let `cp -al` find out: across volumes it prints one
    // "Cross-device link" error per file, hundreds of lines that read as a broken build to anyone
    // skimming the log, before this function falls back and succeeds.
    let same_volume = matches!(
        (std::fs::metadata(&real), std::fs::metadata(&farm)),
        (Ok(a), Ok(b)) if a.dev() == b.dev()
    );
    let hardlinked = same_volume
        && cp(&["-al", &s(real.join("bin")), &s(farm.join("bin"))])
        && cp(&["-al", &s(real.join("lib")), &s(farm.join("lib"))]);
    if !hardlinked {
        if same_volume {
            eprintln!("std-src: hardlink-clone failed; copying instead");
        } else {
            eprintln!(
                "std-src: the farm and {} are on different volumes, so the toolchain is copied, \
                 not hard-linked (notes/dev-machine-disk.md)",
                real.display()
            );
        }
        let _ = std::fs::remove_dir_all(farm.join("bin"));
        let _ = std::fs::remove_dir_all(farm.join("lib"));
        if !cp(&["-R", &s(real.join("bin")), &s(farm.join("bin"))])
            || !cp(&["-R", &s(real.join("lib")), &s(farm.join("lib"))])
        {
            eprintln!("std-src: copying the toolchain failed");
            return false;
        }
    } else if let Err(e) = unlink_sysroot_anchors(&farm) {
        eprintln!("std-src: cannot replace the hard-linked rustc with a real copy: {e}");
        return false;
    }
    let src = farm.join("lib/rustlib/src");
    let _ = std::fs::remove_dir_all(&src);
    if !cp(&["-R", &s(real.join("lib/rustlib/src")), &s(src)]) {
        eprintln!("std-src: real copy of rust-src failed");
        return false;
    }

    if !std_apply_overlay()
        || !std_generate_modules()
        || !std_generate_talc()
        || !std_patch_dispatch()
        || !std_patch_size_hint()
    {
        return false;
    }

    if !link_own_toolchain() {
        return false;
    }

    if let Err(e) = std::fs::write(&stamp_file, stamp.to_string()) {
        eprintln!("std-src: cannot write stamp {}: {e}", stamp_file.display());
        return false;
    }
    true
}

/// Is this farm file one that rustc reads its own sysroot from?
///
/// rustc finds its sysroot from where `librustc_driver` was loaded (`dladdr` on the driver), and
/// the proxy finds rustc by `bin/rustc`. Those two are the whole set; the hash in the driver's name
/// changes with every nightly, so it is matched by prefix. Everything else (`libLLVM`, `cargo`,
/// the sanitizer runtimes, rustlib) is read by path and is safe to share an inode.
pub(super) fn is_sysroot_anchor(rel: &Path) -> bool {
    let Some(name) = rel.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    match rel.parent().and_then(|p| p.to_str()) {
        Some("bin") => name == "rustc",
        Some("lib") => name.starts_with("librustc_driver-"),
        _ => false,
    }
}

/// Replace the hard links to `bin/rustc` and `lib/librustc_driver-*` in a freshly `cp -al`ed farm
/// with real, independent-inode copies.
///
/// A hard link is one inode with two names, and macOS can report the driver's load path under the
/// pinned nightly's name, so rustc takes the nightly as its sysroot and builds unpatched std.
/// Measured on 2026-10-03 and 2026-10-06: through the rustup proxy `rustc --print sysroot` named
/// the nightly in 1 run of 8, and every time under `cargo xtask std-exerciser`; with real copies,
/// the farm in 30 of 30. The copies cost about 90 MB per farm (nightly-2026-10-06); the rest of
/// `lib`, libLLVM at 140 MB among it, is found by path and keeps sharing blocks. See
/// notes/std/caveats.md, "`cargo xtask` through the rustup proxy built against unpatched std".
pub(super) fn unlink_sysroot_anchors(farm: &Path) -> std::io::Result<()> {
    for dir in ["bin", "lib"] {
        for e in std::fs::read_dir(farm.join(dir))? {
            let path = e?.path();
            let rel = Path::new(dir).join(path.file_name().unwrap_or_default());
            if !is_sysroot_anchor(&rel) || !path.is_file() {
                continue;
            }
            // Copy beside it, then rename over the link: the farm never holds a missing rustc,
            // and `fs::copy` carries the permission bits, so `bin/rustc` stays executable.
            let tmp = path.with_extension("nife-copy");
            std::fs::copy(&path, &tmp)?;
            std::fs::rename(&tmp, &path)?;
        }
    }
    Ok(())
}

/// Path-to-string helper for the `cp`/`rustup` argument lists.
pub(super) fn s(p: PathBuf) -> String {
    p.display().to_string()
}

/// Copy the PAL overlay (`patches/std-nife/overlay/std/src/...`) over the farm's std source.
pub(super) fn std_apply_overlay() -> bool {
    let overlay = workspace_root().join("patches/std-nife/overlay/std/src");
    let dst_root = farm_std_src();
    let mut files = Vec::new();
    collect_files(&overlay, &mut files);
    for f in files {
        let rel = f.strip_prefix(&overlay).unwrap();
        let dst = dst_root.join(rel);
        if let Some(parent) = dst.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::copy(&f, &dst) {
            eprintln!("std-src: overlay copy {} failed: {e}", rel.display());
            return false;
        }
    }
    true
}

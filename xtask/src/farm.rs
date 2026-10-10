//! Rust `std` on the native ABI (milestone 27).
//!
//! std's Platform Abstraction Layer for nife lives in patches/std-nife (the Hermit shape:
//! a `sys` backend on the capability ABI, not a libc shim). `std-src` materializes a patched
//! rust-src into a linked `nife-dev` toolchain; `std-exerciser` builds the `std_exerciser` program for the
//! custom targets with -Zbuild-std against it. See notes/std.md.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::host::{capture, run, workspace_root};

/// The custom-target triples the std demo builds for, one per supported ISA. The name is the
/// JSON spec's file stem, which is also cargo's target-dir subdirectory.
const STD_TARGETS: [&str; 3] = [
    "aarch64-unknown-nife",
    "riscv64-unknown-nife",
    "x86_64-unknown-nife",
];

/// The linked toolchain name (`rustup toolchain link`) whose rust-src carries the nife PAL.
const NIFE_TOOLCHAIN: &str = "nife-dev";

/// Bump to force every farm to rebuild after a change to the patch logic itself (not the inputs).
const STD_SRC_PATCH_VERSION: u32 = 10;

fn farm_dir() -> PathBuf {
    workspace_root().join("target/nife-farm")
}

/// The real nightly sysroot the farm is hardlink-cloned from.
fn real_sysroot() -> Option<PathBuf> {
    capture("rustc", &["--print", "sysroot"]).map(|s| PathBuf::from(s.trim()))
}

/// The farm's patched std source root (`.../library/std/src`).
fn farm_std_src() -> PathBuf {
    farm_dir().join("lib/rustlib/src/rust/library/std/src")
}

/// The `std_exerciser` ELF for a given custom-target triple. `std_exerciser` is its own workspace, so its
/// artifacts land under `std_exerciser/target/<triple>/release/`.
pub(crate) fn std_exerciser_elf(triple: &str) -> PathBuf {
    workspace_root().join(format!(
        "std_exerciser/target/{triple}/release/std_exerciser"
    ))
}

/// **`std_echo`**, the second binary of the same workspace (milestone 205 (how a foreign program is
/// told what to do)): built by the same `cargo xtask std-exerciser`, and put on the disk to be run
/// by path.
pub(crate) fn std_echo_elf(triple: &str) -> PathBuf {
    workspace_root().join(format!("std_exerciser/target/{triple}/release/std_echo"))
}

/// **`std_grep`**, the workspace's third binary (milestone 205's designation half), put on the disk
/// to be run by path.
pub(crate) fn std_grep_elf(triple: &str) -> PathBuf {
    workspace_root().join(format!("std_exerciser/target/{triple}/release/std_grep"))
}

/// **`std_resolve`**, the same workspace's fourth binary (milestone 801 (packages over the
/// internet)): packed into the archive beside `std_exerciser`, because a kernel test runs it.
pub(crate) fn std_resolve_elf(triple: &str) -> PathBuf {
    workspace_root().join(format!("std_exerciser/target/{triple}/release/std_resolve"))
}

/// **The package index's in-guest client, if somebody built it**: milestone 801 (packages over the
/// internet). The second binary of `pinned_tls_exerciser`'s workspace, which
/// `helpers/build-pinned-tls-exerciser.sh` puts beside the first, on its terms and for its reason.
pub(crate) fn package_fetch_exerciser_elf(triple: &str) -> PathBuf {
    workspace_root().join(format!(
        "target/pinned-tls-exerciser/{triple}/package_fetch_exerciser"
    ))
}

/// **Unmodified `ripgrep` from crates.io, if somebody built it** (milestone 121).
///
/// `helpers/build-ripgrep.sh` puts it here. Nothing in this build produces it, and that is the
/// point: fetching `ripgrep` and its transitive crates is a crates.io dependency tree, which
/// DECISIONS §46 makes calef's decision rather than a gate's. So the initrd carries it when it is
/// on disk and does not when it is not, exactly as `std_exerciser` rides along, and
/// `system_tests/src/user/ripgrep_tests.rs` skips rather than fails when the archive has no `rg`.
pub(crate) fn ripgrep_elf(triple: &str) -> PathBuf {
    workspace_root().join(format!("target/ripgrep/{triple}/rg"))
}

/// **The C programs milestone 835 (a C library, stage 1: files, clock and memory) runs unmodified,
/// by name.** Each is built by its own `helpers/build-<name>.sh`, which fetches somebody else's C
/// and links it with nife's C library; the archive carries whichever are on disk.
pub(crate) const C_PROGRAMS: [&str; 2] = ["speedtest1", "ioping"];

/// **One of [`C_PROGRAMS`], if somebody built it**, on [`ripgrep_elf`]'s terms and for its reason:
/// fetching SQLite or ioping in a gate is a dependency decision (§46 (thin primitives or whole
/// subsystems; we write everything in between)), which the pull request that added this asks calef
/// to make. `system_tests/src/user/c_program_tests.rs` skips each test whose program is absent.
pub(crate) fn c_program_elf(name: &str, triple: &str) -> PathBuf {
    workspace_root().join(format!("target/{name}/{triple}/{name}"))
}

/// **The crypto-provider workload, if somebody built it**: milestone 442 (a crypto provider `rustls` can use on all three bare-metal targets).
///
/// `helpers/build-cryptography-exerciser.sh` puts it here, and it rides in the archive on exactly
/// `ripgrep`'s terms and for exactly its reason. The program depends on `rustls` and a crypto
/// provider; DECISIONS §196 (nife carries TLS: `rustls` for the protocol, and a crypto provider we
/// make work) ruled on the first and explicitly not on the second, so making a gate fetch a
/// hundred crates would take a dependency decision that is an architect's. The archive carries it
/// when it is on disk and does not when it is not, and `system_tests/src/user/cryptography_tests.rs`
/// skips.
pub(crate) fn cryptography_exerciser_elf(triple: &str) -> PathBuf {
    workspace_root().join(format!(
        "target/cryptography-exerciser/{triple}/cryptography_exerciser"
    ))
}

/// **The pinned TLS client's workload, if somebody built it**: milestone 501 (a TLS client that
/// speaks to one pinned peer). `helpers/build-pinned-tls-exerciser.sh` puts it here, on
/// [`cryptography_exerciser_elf`]'s terms and for its reason, and
/// `system_tests/src/user/pinned_tls_tests.rs` skips when the archive has none.
pub(crate) fn pinned_tls_exerciser_elf(triple: &str) -> PathBuf {
    workspace_root().join(format!(
        "target/pinned-tls-exerciser/{triple}/pinned_tls_exerciser"
    ))
}

/// A cheap FNV-1a over a byte slice, folded into the running hash. No crypto, no dep: this only
/// needs to notice when a PAL input changed so the farm (and thus the build-std cache) is rebuilt.
fn fnv(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// Hash everything that determines the farm's contents: the toolchain version, the patch-logic
/// version, the ABI/heap crates copied in verbatim, the target specs, and every overlay file.
/// A mismatch means the linked toolchain is stale and std must be rebuilt from patched source.
pub(crate) fn std_inputs_stamp() -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    h = fnv(h, &STD_SRC_PATCH_VERSION.to_le_bytes());
    if let Some(v) = capture("rustc", &["-vV"]) {
        h = fnv(h, v.as_bytes());
    }
    let root = workspace_root();
    let mut files: Vec<PathBuf> = vec![
        root.join("crates/abi/src/lib.rs"),
        root.join("crates/user_mode_heap/src/lib.rs"),
        // The net PAL generates its wire constants verbatim from the net_stack contract; a change to it
        // must rebuild the farm just like a change to the ABI crate.
        root.join("crates/socket_protocol/src/lib.rs"),
        // Likewise the FS-service contract: `std::fs` is a client of it (milestone 27 phase two),
        // and its wire constants are generated verbatim into the PAL.
        root.join("crates/filesystem_protocol/src/lib.rs"),
        // The wall-clock and entropy contracts, for the same reason: `sys/time` reads the clock
        // page's layout out of one and `sys/random` packs its requests with the other, so a change
        // to either must rebuild the farm or the PAL silently drifts from the service.
        root.join("crates/clock_protocol/src/lib.rs"),
        root.join("crates/entropy_protocol/src/lib.rs"),
        // The inert-configuration contract (milestone 47's environment-variable fork, DECISIONS
        // §111): `sys/env` reads the page's layout and `PageBuilder`'s validated domains out of
        // this crate, generated verbatim into the PAL, so a change to either must rebuild the
        // farm or the PAL silently drifts from what assembles the page.
        root.join("crates/environment_protocol/src/lib.rs"),
        // The argument page (milestone 205, DECISIONS §170 (how a foreign program is told what to do)): `sys/args` reads the byte argv's
        // layout out of this crate, so a change to it must rebuild the farm.
        root.join("crates/argument_protocol/src/lib.rs"),
        // The byte-sink contract, generated into the PAL as `sinkproto` like the rest, and missing
        // from this list until 2026-10-09 (UTC): a change to it did not rebuild the farm.
        root.join("crates/byte_sink_protocol/src/lib.rs"),
        // The std runtime contract (milestone 595 (provisional)): `rt`'s slot numbers and page
        // addresses, generated verbatim into the PAL, so the loader and the PAL read one file.
        root.join("crates/std_runtime_protocol/src/lib.rs"),
        // The name resolver's client wire (milestone 801 (packages over the internet)): `sys/net`'s
        // `lookup_host` speaks it.
        root.join("crates/name_resolution_protocol/src/wire.rs"),
        root.join("targets/aarch64-unknown-nife.json"),
        root.join("targets/riscv64-unknown-nife.json"),
        root.join("targets/x86_64-unknown-nife.json"),
        // The timebase page, from milestone 184 (extend the `std` port to x86_64); riscv64 joined
        // 2026-09-21. `rt::cntfrq` reads it on
        // the two architectures with no register stating the rate, and its layout is generated
        // verbatim into the PAL like every contract above.
        root.join("crates/counter_frequency_protocol/src/lib.rs"),
    ];
    collect_files(&root.join("patches/std-nife/overlay"), &mut files);
    files.sort();
    for f in files {
        // **Hash the path RELATIVE to the workspace root, never the absolute path.** An absolute path
        // makes the stamp a function of *where the checkout lives*, so two trees with byte-identical
        // inputs never match, `std_src` rebuilds the farm unconditionally, and `rustup toolchain link`
        // repoints `nife-dev`, which is global to the machine, not to the worktree. That is the
        // race behind three broken toolchains on 2026-07-31: an agent worktree ran `script/test`, took
        // the link, and deleting that worktree left `nife-dev` dangling for everything else, failing
        // far from the cause as "override toolchain 'nife-dev' is not installed".
        //
        // The stamp is meant to answer "are the farm's *inputs* unchanged", and a checkout's location
        // is not one of its inputs. `strip_prefix` cannot fail here (every path is built from `root` or
        // collected beneath it), but fall back to the full path rather than panicking in a build tool.
        let rel = f.strip_prefix(&root).unwrap_or(&f);
        h = fnv(h, rel.to_string_lossy().as_bytes());
        if let Ok(bytes) = std::fs::read(&f) {
            h = fnv(h, &bytes);
        }
    }
    h
}

/// Walk `dir` and push every regular file into `out` (used to fingerprint the overlay tree).
fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
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

/// Where the machine-global `nife-dev` name currently resolves, if it is a link we can read.
///
/// `rustup toolchain link` writes a symlink under `$RUSTUP_HOME/toolchains`, so the target is
/// readable without shelling out. `None` covers every shape we cannot interpret (no such link, a
/// real directory rather than a symlink, an unreadable home), and the caller treats `None` as
/// "cannot prove it is ours", which relinks. Relinking when it was already correct costs one
/// idempotent `rustup` call; assuming it was correct costs a silently wrong build.
fn linked_farm() -> Option<PathBuf> {
    let home = std::env::var_os("RUSTUP_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".rustup")))?;
    std::fs::read_link(home.join("toolchains").join(NIFE_TOOLCHAIN)).ok()
}

/// Point `nife-dev` at *this* worktree's farm, loudly, if it currently points anywhere else.
///
/// Called on the warm-farm path, which is the one that used to trust the name without checking it.
/// See the comment at that call site for the failure this closes.
fn relink_farm_if_stolen() -> bool {
    let farm = farm_dir();
    // Canonicalize both sides: a worktree reached through a symlinked path (/tmp on macOS is one)
    // would otherwise compare unequal to the same directory recorded literally, and relink on every
    // single call. Falling back to the uncanonicalized path keeps a missing directory readable in
    // the message rather than swallowing it.
    let canon = |p: &PathBuf| std::fs::canonicalize(p).unwrap_or_else(|_| p.clone());
    if linked_farm().map(|l| canon(&l)) == Some(canon(&farm)) {
        return true;
    }
    eprintln!(
        "--- std-src: `{NIFE_TOOLCHAIN}` did not point at this worktree's farm; relinking ---"
    );
    match linked_farm() {
        Some(other) => eprintln!("std-src:   it pointed at {}", other.display()),
        None => eprintln!("std-src:   it pointed at nothing this tool could read"),
    }
    eprintln!("std-src:   now {}", farm.display());
    eprintln!(
        "std-src: if another lane is mid-gate it has just lost the link, which is how this shared \
         name has always worked (AGENTS.md). The integrator relinks from the main checkout at merge."
    );
    if !run("rustup", &["toolchain", "link", NIFE_TOOLCHAIN, &s(farm)]) {
        eprintln!("std-src: `rustup toolchain link {NIFE_TOOLCHAIN}` failed");
        return false;
    }
    true
}

/// **Materialize the patched `nife-dev` toolchain** (milestone 27).
///
/// build-std reads std's source from the sysroot of the rustc it invokes, so a patched std means
/// a toolchain whose sysroot IS patched. We hardlink-clone the real nightly (`cp -al`, near-zero
/// disk since blocks are shared) so rustc resolves *this* directory as its sysroot, copy
/// `bin/rustc` and the driver dylib for real (see [`unlink_sysroot_anchors`]), then replace the
/// `src` subtree with a real (independent-inode) copy and patch that copy: the overlay PAL files,
/// the ABI/heap crates generated verbatim, and a `target_os = "nife"` arm inserted into std's
/// `cfg_select!` dispatchers. The real toolchain is never touched.
///
/// Idempotent: a stamp of all inputs guards the rebuild, so a warm farm (and its build-std cache)
/// survives across runs and only a PAL change forces std to recompile.
pub(crate) fn std_src() -> bool {
    let stamp = std_inputs_stamp();
    let stamp_file = farm_dir().join(".nife-stamp");
    if farm_std_src().is_dir()
        && std::fs::read_to_string(&stamp_file).ok().as_deref() == Some(&stamp.to_string())
    {
        // A warm, correctly-stamped farm is not enough, and this early return used to be the whole
        // check. The stamp says *this worktree's farm is built*; it says nothing about where the
        // machine-global `nife-dev` name currently points, and every build downstream of here
        // resolves std through that name rather than through `farm_dir()`.
        //
        // So two lanes gating at once silently built each other's std. That is not hypothetical:
        // on 2026-08-18 lane `55-durability` relinked mid-run and lane `64-more`'s `std_exerciser`
        // compiled against 55's farm, caught only by a person reading the `Compiling std` path out
        // of the build output. AGENTS.md predicted this failure in prose and nothing looked for it.
        //
        // Relink rather than refuse. The lane calling this is about to build and needs the name to
        // mean its own farm, taking the link is what every lane already does by design, and failing
        // here would only convert a silent wrong build into a stopped gate. What changes is that
        // the theft is now deliberate and printed, so `Compiling std` from a foreign path cannot
        // happen without a line above it saying who took what.
        if !relink_farm_if_stolen() {
            return false;
        }
        return true;
    }

    let Some(real) = real_sysroot() else {
        eprintln!("std-src: cannot find the nightly sysroot (rustc --print sysroot)");
        return false;
    };
    let farm = farm_dir();
    eprintln!("--- std-src: building the patched nife-dev toolchain (this recompiles std) ---");

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
        || !std_patch_dispatch()
        || !std_patch_size_hint()
    {
        return false;
    }

    // Link (or relink) the farm as `nife-dev`. Idempotent: rustup replaces an existing link to
    // the same path.
    if !run(
        "rustup",
        &["toolchain", "link", NIFE_TOOLCHAIN, &s(farm.clone())],
    ) {
        eprintln!("std-src: `rustup toolchain link {NIFE_TOOLCHAIN}` failed");
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
fn is_sysroot_anchor(rel: &Path) -> bool {
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
/// **Why these two are copies and the rest stay hard links.** A hard link is one inode with two
/// names, and when rustc asks the OS where its driver was loaded from, macOS can answer with the
/// *other* name: the pinned nightly's. rustc then reports the nightly as its sysroot and the build
/// uses unpatched std, which fails in std's `cfg_select!` dispatchers. Measured on 2026-10-03 and
/// 2026-10-06: `rustc --print sysroot` through the rustup proxy named the nightly in 1 run of 8,
/// and every time under `cargo xtask std-exerciser`; with real copies of these two files it named
/// the farm in 30 runs of 30. See notes/std/caveats.md, "`cargo xtask` through the rustup proxy
/// built against unpatched std". The copies cost about 90 MB per farm (nightly-2026-10-06); the
/// rest of `lib`, libLLVM at 140 MB among it, is located by path and keeps sharing blocks.
fn unlink_sysroot_anchors(farm: &Path) -> std::io::Result<()> {
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
fn s(p: PathBuf) -> String {
    p.display().to_string()
}

/// Copy the PAL overlay (`patches/std-nife/overlay/std/src/...`) over the farm's std source.
fn std_apply_overlay() -> bool {
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

/// Remove `# Examples` sections from the doc comments of a file about to be copied into the patched
/// std sysroot.
///
/// A doctest in one of these crates says `use entropy_protocol::...`, and in the copy there is no such
/// crate: the file arrives as `sys/pal/nife/entropyproto.rs`, an inner module of `std`. So the
/// example is *false* in its destination, in the specific way milestone 68 cares about, which is
/// that it teaches a reader of the PAL something that is not true of the code they are reading.
/// Nothing runs std's doctests here, so this is a documentation fix rather than a build fix; it is
/// done at the copy because the alternative is refusing the workspace crates real examples, and the
/// workspace is where the example is checked.
///
/// Prose and `text` blocks survive: this drops a `# Examples` heading and everything under it, up to
/// the next heading at the same level or the end of the doc block. Fence state is tracked, so a
/// hidden doctest line (`# use ...`) inside a code block is not mistaken for that next heading.
fn strip_doc_examples(body: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut skipping = false;
    let mut in_fence = false;
    for line in body.lines() {
        let trimmed = line.trim_start();
        let Some(content) = trimmed
            .strip_prefix("//!")
            .or_else(|| trimmed.strip_prefix("///"))
        else {
            // Any non-doc line ends the doc block, and with it the section being skipped.
            skipping = false;
            in_fence = false;
            out.push(line);
            continue;
        };
        let content = content.trim();
        if skipping {
            if content.starts_with("```") {
                in_fence = !in_fence;
            } else if !in_fence && content.starts_with("# ") {
                skipping = false;
                out.push(line);
            }
            continue;
        }
        if content == "# Examples" || content == "# Example" {
            skipping = true;
            continue;
        }
        out.push(line);
    }
    out.join("\n")
}

/// Generate `abi.rs` and `user_mode_heap.rs` verbatim from the host-tested crates, so the ABI numbers and
/// the heap algorithm have exactly one definition. The transform strips crate-level inner
/// attributes (`#![no_std]`, illegal in a non-root module), any trailing `#[cfg(test)]` module, and
/// any `# Examples` section (see [`strip_doc_examples`] for why that last one).
fn std_generate_modules() -> bool {
    let root = workspace_root();
    let jobs = [
        (
            root.join("crates/abi/src/lib.rs"),
            farm_std_src().join("sys/pal/nife/abi.rs"),
        ),
        (
            root.join("crates/user_mode_heap/src/lib.rs"),
            farm_std_src().join("sys/alloc/nife/user_mode_heap.rs"),
        ),
        // The net_stack socket-contract wire format, verbatim, so the net PAL cannot drift from the
        // server it talks to (same discipline as the ABI and heap crates above).
        (
            root.join("crates/socket_protocol/src/lib.rs"),
            farm_std_src().join("sys/pal/nife/netproto.rs"),
        ),
        // The FS-service wire protocol (DECISIONS §27), so `std::fs`'s PAL cannot drift from the
        // server it opens files through. Same discipline as the three above.
        (
            root.join("crates/filesystem_protocol/src/lib.rs"),
            farm_std_src().join("sys/pal/nife/fsproto.rs"),
        ),
        // The wall-clock contract (DECISIONS §43), so the time PAL reads the clock page with the
        // same seqlock and the same layout the clock service publishes it with. Same discipline as
        // the four above; this one matters more than most, because a drift here would be a torn
        // read of a timestamp rather than a compile error.
        (
            root.join("crates/clock_protocol/src/lib.rs"),
            farm_std_src().join("sys/pal/nife/clockproto.rs"),
        ),
        // The entropy contract (DECISIONS §44), so the random PAL packs its requests and reads its
        // replies exactly the way the entropy service serves them. Same discipline as the five
        // above; a drift here would be a program reading the wrong bytes as a key.
        (
            root.join("crates/entropy_protocol/src/lib.rs"),
            farm_std_src().join("sys/pal/nife/entropyproto.rs"),
        ),
        // The inert-configuration contract (milestone 47's environment-variable fork, DECISIONS
        // §111), so `sys/env`'s seeding reads the config page with the same layout and the same
        // validated domains whoever assembles a page uses. Same discipline as the six above.
        (
            root.join("crates/environment_protocol/src/lib.rs"),
            farm_std_src().join("sys/pal/nife/envproto.rs"),
        ),
        // The argument page (milestone 205 (how a foreign program is told what to do), DECISIONS
        // §170), so `sys/args` reads the argv with the layout the shell assembles it in.
        (
            root.join("crates/argument_protocol/src/lib.rs"),
            farm_std_src().join("sys/pal/nife/argproto.rs"),
        ),
        // The byte-sink contract (milestone 50), so `println!`'s framing and the classification of
        // a failed SEND are one definition shared with every sink and with the kernel-side tests.
        // Same discipline as the six above, and the one that would hurt most to get wrong: a drift
        // in `GONE` would be a program that keeps printing into a pipe whose reader has exited.
        (
            root.join("crates/byte_sink_protocol/src/lib.rs"),
            farm_std_src().join("sys/pal/nife/sinkproto.rs"),
        ),
        // The std runtime contract (milestone 595 (provisional)): the eight fixed slots and the three
        // shared pages, which `rt` re-exports. The progenitor places a std child's capabilities by
        // these numbers, so a drift here would be a program reading its heap budget out of the
        // slot its stdout went in.
        (
            root.join("crates/std_runtime_protocol/src/lib.rs"),
            farm_std_src().join("sys/pal/nife/runtimeproto.rs"),
        ),
        // The name resolver's client wire (milestone 384 (in a capability system the resolver is a
        // grant)), so `lookup_host` asks the resolver with the words it serves. Only `wire.rs`: the
        // rest of that crate needs `domain_name_system`, which the PAL cannot name. Milestone 801
        // (packages over the internet).
        (
            root.join("crates/name_resolution_protocol/src/wire.rs"),
            farm_std_src().join("sys/pal/nife/resolveproto.rs"),
        ),
        // The timebase page (milestone 184), so `rt::cntfrq` reads the machine's rate at the
        // address and with the magic the kernel writes it with. Generated for every farm and
        // compiled only on x86_64 and riscv64 (`sys/pal/nife/mod.rs` gates the module), because the
        // farm is one source tree for all three targets and aarch64 reads a register instead.
        (
            root.join("crates/counter_frequency_protocol/src/lib.rs"),
            farm_std_src().join("sys/pal/nife/counterfreqproto.rs"),
        ),
    ];
    for (src, dst) in jobs {
        let Ok(text) = std::fs::read_to_string(&src) else {
            eprintln!("std-src: cannot read {}", src.display());
            return false;
        };
        let mut body: String = text
            .lines()
            .filter(|l| !l.trim_start().starts_with("#!["))
            .collect::<Vec<_>>()
            .join("\n");
        if let Some(idx) = body.find("\n#[cfg(test)]\nmod tests") {
            body.truncate(idx);
        }
        let body = format!("{}\n", strip_doc_examples(&body).trim_end());
        if let Some(parent) = dst.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(&dst, body) {
            eprintln!("std-src: cannot write {}: {e}", dst.display());
            return false;
        }
    }
    true
}

/// Insert `text` immediately after the first occurrence of `anchor` in `path`.
fn patch_after(path: &Path, anchor: &str, insert: &str) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        eprintln!("std-src: cannot read {}", path.display());
        return false;
    };
    let Some(pos) = text.find(anchor) else {
        eprintln!(
            "std-src: anchor not found in {} (std internals changed?): {anchor:?}",
            path.display()
        );
        return false;
    };
    let at = pos + anchor.len();
    let new = format!(
        "{}\n{}\n{}",
        &text[..at],
        insert.trim_end_matches('\n'),
        &text[at..]
    );
    if let Err(e) = std::fs::write(path, new) {
        eprintln!("std-src: cannot write {}: {e}", path.display());
        return false;
    }
    true
}

/// Replace every occurrence of `old` in `path` with `new`, failing loudly if there are not exactly
/// `count`: the same tripwire as [`patch_after`], for the one patch that has to change a line rather
/// than add one.
fn patch_replace(path: &Path, old: &str, new: &str, count: usize) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        eprintln!("std-src: cannot read {}", path.display());
        return false;
    };
    let found = text.matches(old).count();
    if found != count {
        eprintln!(
            "std-src: expected {count} of {old:?} in {}, found {found} (std internals changed?)",
            path.display()
        );
        return false;
    }
    if let Err(e) = std::fs::write(path, text.replace(old, new)) {
        eprintln!("std-src: cannot write {}: {e}", path.display());
        return false;
    }
    true
}

/// **Size a whole-file read from `OPEN`'s reply rather than an `FSTAT`** (milestone 606 (a
/// directory walk costs what it does on Linux), calef's ruling B, form 2, 2026-09-27).
///
/// `std::fs::read`, `read_to_string` and `File::read_to_end` size their buffer from
/// `file.metadata()`, which on nife is one more message per file. Since ruling B the server's
/// `OPEN` reply carries the size, and the PAL keeps it as a hint (`File::open_size_hint`). This
/// routes those three buffer sizings through a helper that asks the hint first and falls back to
/// `metadata()`. `metadata()` itself is untouched and always asks the server: calef's reason is
/// that a stale hint costs at most one resize, while a stale `metadata()` lies to the program.
fn std_patch_size_hint() -> bool {
    let fs = farm_std_src().join("fs.rs");
    let read_size =
        "let size = file.metadata().map(|m| usize::try_from(m.len()).unwrap_or(usize::MAX)).ok();";
    patch_replace(
        &fs,
        read_size,
        "let size = file_size_hint(&file).map(|n| usize::try_from(n).unwrap_or(usize::MAX));",
        2,
    ) && patch_replace(
        &fs,
        "fn buffer_capacity_required(mut file: &File) -> Option<usize> {\n    let size = file.metadata().map(|m| m.len()).ok()?;",
        "/// nife (milestone 606, ruling B): the size `OPEN` reported, else `metadata()`.\nfn file_size_hint(file: &File) -> Option<u64> {\n    #[cfg(target_os = \"nife\")]\n    if let Some(size) = file.inner.open_size_hint() {\n        return Some(size);\n    }\n    file.metadata().map(|m| m.len()).ok()\n}\n\nfn buffer_capacity_required(mut file: &File) -> Option<usize> {\n    let size = file_size_hint(file)?;",
        1,
    )
}

/// Add a `target_os = "nife"` arm to std's `cfg_select!` dispatchers so they pick the nife
/// backend, and add nife to std's `build.rs` known-platform chain (so std is not
/// `restricted_std` and ordinary programs need no `#![feature]`). These string anchors couple us
/// to the pinned nightly's std internals; a rustc bump that reshapes them fails loudly here, which
/// is the intended tripwire (see notes/std.md).
fn std_patch_dispatch() -> bool {
    let sys = farm_std_src().join("sys");
    patch_after(
        &sys.join("pal/mod.rs"),
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        pub(crate) mod nife;\n        pub use self::nife::*;\n    }",
    ) && patch_after(
        &sys.join("alloc/mod.rs"),
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod nife;\n        use nife as imp;\n    }",
    ) && patch_after(
        &sys.join("stdio/mod.rs"),
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod nife;\n        pub use nife::*;\n    }",
    ) && patch_after(
        // random: `fill_bytes` AND `hashmap_random_keys`, because milestone 56 splits them. The
        // first promises cryptographic strength and panics without the entropy capability; the
        // second is a hash seed and degrades to the old counter-seeded stream. Exporting both means
        // std's blanket `hashmap_random_keys` (the `#[cfg(not(any(...)))]` fallback at the bottom of
        // the same file) must exclude nife, or the two definitions collide; that is the next
        // patch, and it is anchored on the wasi line because "xous" appears twice in the file.
        &sys.join("random/mod.rs"),
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod nife;\n        pub use nife::{fill_bytes, hashmap_random_keys};\n    }",
    ) && patch_after(
        &sys.join("random/mod.rs"),
        "    all(target_os = \"wasi\", not(target_env = \"p1\")),",
        "    target_os = \"nife\",",
    ) && patch_after(
        &sys.join("thread/mod.rs"),
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod nife;\n        pub use nife::{Thread, available_parallelism, current_os_id, set_name, sleep, yield_now, DEFAULT_MIN_STACK_SIZE};\n    }",
    ) && patch_after(
        &sys.join("time/mod.rs"),
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod nife;\n        use nife as imp;\n    }",
    ) && patch_after(
        // net: TcpStream + outbound UdpSocket over the net_stack socket contract (milestone 27 phase
        // two). The first cfg_select in connection/mod.rs is the backend dispatcher; the nife
        // arm precedes the `_ =>` unsupported fallback that phase one used. hostname has its own
        // `_ =>` fallback to unsupported, so it needs no arm.
        &sys.join("net/connection/mod.rs"),
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod nife;\n        pub use nife::*;\n    }",
    ) && patch_after(
        // fs: File open/read/metadata over the FS-service contract (milestone 27 phase two). The
        // arm precedes the `_ =>` unsupported fallback phase one used, and mirrors the shape of
        // the other single-backend arms (`use nife as imp`).
        // `pub(crate) mod` rather than `mod`: `sys/paths/nife.rs` asks `fs::nife::is_reachable()`
        // whether this process holds a directory capability, because `current_dir` must refuse for
        // a process that holds none rather than name a place it cannot reach (milestone 47).
        &sys.join("fs/mod.rs"),
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        pub(crate) mod nife;\n        use nife as imp;\n    }",
    ) && patch_after(
        // env: a process-local variable table (milestone 64, rank 4). The arm precedes the `_ =>`
        // unsupported fallback, whose `env()` is `panic!("not supported on this platform")`: without
        // this, `std::env::vars()` aborted the process rather than yielding nothing. `sys/env/nife.rs`
        // defines its own `Env` instead of reusing `sys/env/common.rs`, so this is the only anchor
        // env costs us; `common` is gated on a `#[cfg(any(...))]` platform list that would have been
        // a second one to keep in step across nightlies.
        &sys.join("env/mod.rs"),
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod nife;\n        pub use nife::*;\n    }",
    ) && patch_after(
        // args: the byte argv (milestone 205, DECISIONS §170). The arm precedes the `_ =>`
        // unsupported fallback, whose `args()` yields nothing, which is where unmodified `ripgrep`
        // stopped. `sys/args/nife.rs` defines its own `Args`, as `sys/env/nife.rs` does, because
        // `sys/args/common.rs` is gated on a platform list that would be a second anchor.
        &sys.join("args/mod.rs"),
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod nife;\n        pub use nife::*;\n    }",
    ) && patch_after(
        // paths: `temp_dir`, `split_paths` and `join_paths` (milestone 64). The arm precedes the
        // `_ =>` unsupported fallback, whose `temp_dir()` is `panic!("no filesystem on this
        // platform")` and whose `split_paths()` is `panic!("unsupported")`: without this,
        // `std::env::temp_dir()` aborted the process, which is what `tempfile` reached before it
        // ever got to its own "not supported" arm. `getcwd`, `chdir`, `current_exe` and `home_dir`
        // keep refusing, in `sys/paths/nife.rs` rather than by falling through, so one file holds
        // both halves and a reader meets the reasons together.
        &sys.join("paths/mod.rs"),
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod nife;\n        use nife as imp;\n    }",
    ) && patch_after(
        // process: `getpid` only (milestone 64). Everything else stays the shared `unsupported`
        // backend, which refuses honestly; `getpid` alone was `panic!("no pids on this platform")`,
        // so `std::process::id()` killed the program. The arm is spelled as a split `imp` rather
        // than a whole nife backend because `unsupported.rs` opens with `use super::env::...`, so
        // it cannot be pulled in through a `#[path]` module the way `sys/fs/nife.rs` does.
        &sys.join("process/mod.rs"),
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        #[allow(dead_code)]\n        mod unsupported;\n        mod nife;\n        mod imp {\n            pub use super::nife::getpid;\n            pub use super::unsupported::{\n                ChildPipe, Command, CommandArgs, EnvKey, ExitCode, ExitStatus, ExitStatusError,\n                Process, Stdio, output, read_output,\n            };\n        }\n    }",
    ) && patch_after(
        // **exit: `std::process::exit` was a trap instruction** (milestone 64, fourth pass).
        //
        // `sys/exit.rs` is not a `sys/<module>/mod.rs` backend dispatcher; it is one file whose
        // `cfg_select!` sits *inside* `pub fn exit`, and its `_ =>` arm is
        // `crate::intrinsics::abort()`. So a nife program calling `std::process::exit(0)` compiled
        // perfectly and then executed `brk`, which the kernel reports as `EVENT_FAULT` with a pc
        // and an address: a clean exit arriving at its supervisor as a crash, and a fault report on
        // the console for a program that did nothing wrong.
        //
        // Nothing noticed because the normal path never goes through here. `sys/pal/nife/mod.rs`'s
        // `_start` calls `rt::exit` on `main`'s return value directly, and `std::process::exit` is
        // the *only* caller of `sys::exit::exit` in the whole of std. The two ways a Rust program
        // ends took different exits, and only one of them was wired.
        //
        // The arm is what `_start` already does, which is why this needs no new decision: the same
        // `SYS_EXIT` with the same code. The kernel discards the code (`sched::exit` is
        // `depart(EVENT_EXIT, 0, 0)`), which is a real limitation recorded in notes/std.md rather
        // than something this arm can fix; what it fixes is exit-versus-fault, which is observable
        // today and which `a_whole_std_program_runs_on_the_native_abi` now asserts.
        &sys.join("exit.rs"),
        "pub fn exit(code: i32) -> ! {\n    cfg_select! {",
        "        target_os = \"nife\" => {\n            crate::sys::pal::nife::rt::exit(code as i64)\n        }",
    ) && patch_after(
        // io/error has no fallback arm; route nife to the generic backend.
        &sys.join("io/error/mod.rs"),
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod generic;\n        pub use generic::*;\n    }",
    ) && patch_after(
        // Single-threaded, no native TLS: storage is a plain static (no_threads).
        &sys.join("thread_local/mod.rs"),
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod no_threads;\n        pub use no_threads::{EagerStorage, LazyStorage, thread_local_inner};\n        pub(crate) use no_threads::{LocalPointer, local_pointer};\n    }",
    ) && patch_after(
        // ... and the TLS-destructor guard is a no-op.
        &sys.join("thread_local/mod.rs"),
        "pub(crate) mod guard {\n    cfg_select! {",
        "        target_os = \"nife\" => {\n            pub(crate) fn enable() {}\n        }",
    ) && patch_after(
        // std::env::consts::OS. `cfg_unordered!` turns each arm's cfg into the fallback's
        // exclusion set, so adding a nife arm both defines OS and keeps the fallback off it.
        &sys.join("env_consts.rs"),
        "cfg_unordered! {",
        "#[cfg(target_os = \"nife\")]\npub mod os {\n    pub const FAMILY: &str = \"\";\n    pub const OS: &str = \"nife\";\n    pub const DLL_PREFIX: &str = \"\";\n    pub const DLL_SUFFIX: &str = \"\";\n    pub const DLL_EXTENSION: &str = \"\";\n    pub const EXE_SUFFIX: &str = \"\";\n    pub const EXE_EXTENSION: &str = \"\";\n}",
    ) && patch_after(
        // nife has a real PAL: not restricted_std.
        &farm_std_src().parent().unwrap().join("build.rs"),
        "        || target_os == \"vexos\"\n",
        "        || target_os == \"nife\"",
    )
}

/// The `CARGO_TARGET_DIR` the `std_exerciser` build runs under, and the note to print when an
/// exported value had to be overridden to get it there.
///
/// Everything that consumes this build looks under `std_exerciser/target` and nowhere else:
/// [`std_exerciser_elf`], [`std_echo_elf`], [`std_grep_elf`], and the dep-info [`std_aborts`]
/// reads. An inherited `CARGO_TARGET_DIR` (AGENTS.md tells every lane to export one for its
/// gates) moves the build while leaving every one of those readers behind, so the sweep judges
/// the last unredirected run's evidence and the initrd packs its ELFs. That is not hypothetical:
/// on 2026-09-30 the 1377 lane built std from a sysroot this worktree never chose under exactly
/// that export, twice, and `std-aborts` reported "all accounted for" over stale evidence both
/// times. Pinning makes the wrong state unrepresentable; printing the override keeps the
/// correction from being one more thing that happened silently.
///
/// Name: provisional, minted 2026-09-30 by the sysroot-theft lane.
fn exerciser_target_dir(inherited: Option<&str>) -> (PathBuf, Vec<String>) {
    let pinned = workspace_root().join("std_exerciser/target");
    let canon = |p: PathBuf| std::fs::canonicalize(&p).unwrap_or(p);
    if let Some(exported) = inherited.map(PathBuf::from).map(canon)
        && exported != canon(pinned.clone())
    {
        let note = format!(
            "pinning CARGO_TARGET_DIR to {}; the caller exported {}, which would have put \
             the build and its dep-info where std-aborts never reads (the 1377 silent pass, \
             2026-09-30; see notes/std.md's BUGS)",
            pinned.display(),
            exported.display()
        );
        return (pinned, vec![note]);
    }
    (pinned, Vec::new())
}

/// **Build the `std_exerciser` program for every custom target** (milestone 27; `x86_64` since 184), via -Zbuild-std against
/// the patched `nife-dev` toolchain. panic=abort and singlethread come from the target specs;
/// `compiler-builtins-mem` supplies memcpy/memset for the bare target.
///
/// `RUSTUP_TOOLCHAIN` is set explicitly rather than via `+nife-dev`, because the cargo proxy
/// that launched this xtask already exports `RUSTUP_TOOLCHAIN=nightly`, which would override a
/// `+` selector and silently build std from the *unpatched* sysroot.
///
/// **It names this worktree's farm by path, not `nife-dev` by name** (a correction, found by
/// milestone 606's lane on 2026-09-27). `nife-dev` is one symlink for the whole machine, and a
/// lane gating beside another lane had it relinked away mid-build: `std_src` relinked it to this
/// farm, another lane's gate relinked it to theirs a moment later, and this build compiled an
/// unpatched std and failed three times running. rustup accepts a toolchain path in
/// `RUSTUP_TOOLCHAIN`, so the build now uses the farm it just checked and cannot be pointed
/// elsewhere. The link is still made, for people who type `+nife-dev`; the `helpers/` builds
/// name the farm by path too, since 2026-09-30, for this same reason.
///
/// **The build is pinned to `std_exerciser/target` by `CARGO_TARGET_DIR`** (the sysroot-theft
/// fix, 2026-09-30). `Command` inherits the environment, and AGENTS.md tells every lane to gate
/// with `CARGO_TARGET_DIR=$PWD/target` exported, so the cargo child obeyed it: the build landed
/// in the workspace's shared `target/`, dep-info and ELFs with it, while [`std_aborts`] and the
/// initrd's ELF paths read `std_exerciser/target`. The sweep then judged the previous
/// unredirected run's evidence, and a build that had resolved a foreign or unpatched sysroot
/// passed as green; the 1377 lane had exactly that, twice. [`exerciser_target_dir`] pins, and
/// names the override out loud rather than absorbing it silently.
pub(crate) fn std_exerciser() -> bool {
    if !std_src() {
        return false;
    }
    let manifest = s(workspace_root().join("std_exerciser/Cargo.toml"));
    // Pin where the build and its evidence land before the first child runs, so the ELFs the
    // initrd packs, the dep-info the sweep reads, and this build cannot be separated by an
    // exported variable they never agreed to.
    let inherited_target_dir = std::env::var("CARGO_TARGET_DIR").ok();
    let (target_dir, notes) = exerciser_target_dir(inherited_target_dir.as_deref());
    for note in &notes {
        eprintln!("std-exerciser: {note}");
    }
    for triple in STD_TARGETS {
        let spec = s(workspace_root().join(format!("targets/{triple}.json")));
        let ok = Command::new("cargo")
            .env("RUSTUP_TOOLCHAIN", farm_dir())
            .env("CARGO_TARGET_DIR", &target_dir)
            .args([
                "build",
                "--release",
                "--manifest-path",
                &manifest,
                "-Zjson-target-spec",
                "-Zbuild-std=core,alloc,std,panic_abort",
                "-Zbuild-std-features=compiler-builtins-mem",
                "--target",
                &spec,
            ])
            .status()
            .map(|st| st.success())
            .unwrap_or(false);
        if !ok {
            eprintln!("std-exerciser: building std_exerciser for {triple} failed");
            return false;
        }
    }
    // The build just produced the dep-info the sweep reads, so this costs a few file reads and
    // nothing else. Running it here rather than in `script/lint` is deliberate: the sweep's input
    // is "which std sources did rustc actually compile for nife", which only exists after a build.
    std_aborts()
}

// ===========================================================================================
// The abort sweep (milestone 64, fourth pass).
// ===========================================================================================

/// **Every std call that kills a nife process instead of refusing it** (milestone 64).
///
/// This exists because milestone 64's own `BUGS` section said it did not, and named the cost:
/// *"Nothing runs the sweep that found the three aborts. It is a person reading every module the
/// PAL falls through and asking what its neighbours do, which is rung four of AGENTS.md's ladder.
/// The three found so far were each found by accident or by one deliberate pass, and a fourth
/// would be found the same way."* It was, and the fourth (`std::process::exit`) is the one that
/// argues hardest for a gate: it does not live in a `sys/<module>/mod.rs` backend at all, so the
/// by-hand method of reading module dispatchers would not have reached it however carefully
/// somebody ran it.
///
/// **The method, and why it is exact rather than a grep over std.** The prioritised gap list in
/// notes/crates-io-on-nife.md is built from PAL functions that answer `Unsupported`, and a
/// function that aborts never answers, so it is structurally invisible there. This asks the
/// complementary question directly: of the std sources rustc **actually compiled for this
/// target**, which ones contain a body that terminates the process? The compiled set comes from
/// cargo's own dep-info rather than from reading `cfg_select!` arms, so it is what the compiler
/// did and not what we believe it did; nothing here has to model `cfg` evaluation.
///
/// **What it deliberately does not do.** It does not judge. Most of what it finds is correct
/// (`Once::wait` cannot work without threads; a recursive `RwLock` on a single-threaded target is
/// a deadlock either way), so the output is a set compared against [`ABORTS_ACCEPTED`], where each
/// entry carries the reason it is allowed. A new one fails the build and has to be answered:
/// either bind it in the PAL, or add it with its reason. That is the whole mechanism, and it is
/// rung two of AGENTS.md's ladder where the milestone had rung four.
pub(crate) fn std_aborts() -> bool {
    let compiled = compiled_std_sources();
    if compiled.is_empty() {
        eprintln!(
            "std-aborts: found no compiled std sources in the dep-info under std_exerciser/target.\n\
             std-aborts: this check is meaningless without them; run `cargo xtask std-exerciser` first."
        );
        return false;
    }

    let foreign = foreign_std_sources(&compiled);
    if !foreign.is_empty() {
        eprintln!(
            "std-aborts: the dep-info under std_exerciser/target names sources outside this \
             worktree's own farm ({}):",
            farm_dir().display()
        );
        for p in &foreign {
            eprintln!("  {}", p.display());
        }
        eprintln!(
            "\nstd-aborts: this is not a defect in the file or line above; it is the account-wide \
             `nife-dev` rustup link having pointed at a DIFFERENT worktree's farm the last time \
             `cargo xtask std-exerciser` ran here (two worktrees racing `xtask std-src` on one \
             machine). Fix: rm -rf std_exerciser/target && cargo xtask std-exerciser, which \
             rebuilds the dep-info against this worktree's own farm. Re-running without clearing it \
             first reproduces this exact failure in about thirty seconds, because cargo considers \
             the (foreign) build unit fresh. See notes/std.md's BUGS."
        );
        return false;
    }

    let mut found = Vec::new();
    for path in &compiled {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        // The path as std names it (`sys/exit.rs`), which is what a reader greps for and what the
        // accepted list below is written in.
        let rel = std_relative(path);
        for (n, line) in text.lines().enumerate() {
            if let Some(what) = abort_shaped(line) {
                found.push((
                    rel.clone(),
                    n + 1,
                    what.to_string(),
                    line.trim().to_string(),
                ));
            }
        }
    }

    let mut unexpected = Vec::new();
    for (file, line, _what, text) in &found {
        if !ABORTS_ACCEPTED
            .iter()
            .any(|(f, needle, _why)| f == file && text.contains(needle))
        {
            unexpected.push((file, line, text));
        }
    }

    if unexpected.is_empty() {
        println!(
            "std-aborts: {} process-ending bodies across {} compiled std sources, all accounted for",
            found.len(),
            compiled.len()
        );
        return true;
    }

    eprintln!("std-aborts: a std source compiled for nife ends the process somewhere new:");
    for (file, line, text) in &unexpected {
        eprintln!("  {file}:{line}: {text}");
    }
    eprintln!(
        "\nstd-aborts: each of these is one of two things, and the difference is milestone 64's whole line.\n\
         If a nife program can REACH it, it is a defect: the call compiles, then kills the process,\n\
         which is what `env::vars`, `env::temp_dir`, `env::split_paths`, `process::id` and\n\
         `process::exit` each were. Bind it in patches/std-nife (and add a `target_os = \"nife\"` arm\n\
         in `std_patch_dispatch` if the fallback is a dispatcher's).\n\
         If it cannot be reached, or if ending the process is the honest answer, add it to\n\
         `ABORTS_ACCEPTED` in xtask/src/main.rs WITH THE REASON. An entry with no reason is the\n\
         thing this check exists to stop.\n\
         See notes/std.md, \"What still ends a nife process\"."
    );
    false
}

/// Is this line a body that ends the process, rather than a mention of one?
///
/// Returns the shape it matched, or `None`. Doc comments and `//` comments are skipped, because
/// this tree's PAL files talk *about* the panics they replaced at length, and a check that could
/// not tell a fix from its own explanation would be useless the day it was written.
fn abort_shaped(line: &str) -> Option<&'static str> {
    let t = line.trim_start();
    if t.starts_with("//") || t.starts_with("///") || t.starts_with("*") {
        return None;
    }
    // `unreachable!` is not here: it asserts an invariant of the code rather than declaring a
    // platform's answer, and including it would bury the signal under std's own assertions.
    for pat in [
        "panic!(",
        "unimplemented!(",
        "todo!(",
        "rtabort!(",
        "intrinsics::abort()",
        "panic_nounwind(",
    ] {
        if t.contains(pat) {
            return Some(match pat {
                "panic!(" => "panic",
                "unimplemented!(" => "unimplemented",
                "todo!(" => "todo",
                "rtabort!(" => "rtabort",
                "intrinsics::abort()" => "abort",
                _ => "panic_nounwind",
            });
        }
    }
    None
}

/// Every process-ending body a nife build compiles today, with the reason it stays.
///
/// `(file as std names it, a substring of the line, why it is allowed)`. Matching on a substring
/// rather than a line number is what keeps this from being rewritten by every nightly that adds a
/// blank line; it still moves when upstream rewords the panic, which is a rebuild-and-reread this
/// check is *for*.
///
/// **Read the third column before adding a fourth entry.** Three distinct reasons appear, and only
/// one of them is a licence:
///
///   - *unreachable on nife*: the body sits behind a `cfg` nife does not satisfy, so it is
///     compiled-adjacent rather than compiled. These are the safe ones.
///   - *no answer exists*: single-threaded, so the call can only deadlock or end. Upstream chose
///     to end, and there is no third option to build.
///   - *ours, and deliberate*: the PAL's own, where ending the process is the honest report.
const ABORTS_ACCEPTED: &[(&str, &str, &str)] = &[
    // ---- unreachable on nife ------------------------------------------------------------------
    (
        "sys/alloc/mod.rs",
        "add a value for MIN_ALIGN",
        "a const-eval arm for architectures with no known minimum alignment; aarch64 and riscv64 both have one",
    ),
    (
        "sys/exit.rs",
        "std::process::exit called re-entrantly",
        "inside the `target_os = \"linux\"` arm of `unique_thread_exit`",
    ),
    (
        "sys/exit.rs",
        "rtabort!(\"exit({}) called\", code)",
        "the `solid_asp3` arm of `exit`",
    ),
    (
        "sys/exit.rs",
        "TA should not call `exit`",
        "the `teeos` arm of `exit`",
    ),
    (
        "sys/exit.rs",
        "crate::intrinsics::abort()",
        "two sites: the `uefi` arm's last resort, and the `_ =>` arm nife USED to take. Milestone 64 \
         added a nife arm above it, so the fallback is no longer ours; the line stays compiled \
         because `cfg_select!` keeps every arm's source in the file",
    ),
    (
        "sys/pipe/unsupported.rs",
        "creating pipe on this platform is unsupported!",
        "inside `mod unix_traits`, gated `#[cfg(any(unix, hermit, wasi))]`; nife is none of them. \
         The reachable half of this backend refuses honestly: `pipe()` returns `UNSUPPORTED_PLATFORM` \
         and `Pipe` is uninhabited",
    ),
    (
        "sys/process/unsupported.rs",
        "no pids on this platform",
        "`getpid` here is the one item the nife arm of `sys/process/mod.rs` does NOT re-export; it \
         takes `sys/process/nife.rs`'s instead. The module is pulled in `#[allow(dead_code)]` for \
         everything else, so this body is compiled and unreachable",
    ),
    (
        "sys/personality/mod.rs",
        "core::intrinsics::abort()",
        "the `msvc`/`wasm` arm's stub personality routine",
    ),
    (
        "sys/path/mod.rs",
        "path_separator_bytes must be ASCII bytes",
        "a `const` assertion inside the separator macro, evaluated at compile time",
    ),
    // ---- no answer exists: single-threaded ----------------------------------------------------
    (
        "sys/sync/condvar/no_threads.rs",
        "condvar wait not supported",
        "a wait with no other thread to notify it can only block forever. Upstream ends the process \
         instead, and there is no third answer to build until milestone 64's `thread::spawn` fork is \
         decided. Recorded in notes/std.md rather than fixed",
    ),
    (
        "sys/sync/once/no_threads.rs",
        "not implementable on this target",
        "`Once::wait` waits for another thread's initialisation; same reason as the condvar above",
    ),
    (
        "sys/sync/once/no_threads.rs",
        "Once instance has previously been poisoned",
        "poison propagation, which is `Once`'s documented behaviour on every platform",
    ),
    (
        "sys/sync/once/no_threads.rs",
        "one-time initialization may not be performed recursively",
        "a recursive `call_once`, which is a bug in the caller on every platform",
    ),
    (
        "sys/sync/rwlock/no_threads.rs",
        "rwlock locked for writing",
        "taking a read lock while this same thread holds the write lock. On a threaded platform it \
         deadlocks; here it is caught and named, which is strictly better",
    ),
    (
        "sys/sync/rwlock/no_threads.rs",
        "rwlock locked for reading",
        "the mirror case, and the same argument",
    ),
    (
        "sys/thread_local/mod.rs",
        "thread local panicked on drop",
        "a destructor that panicked; unwinding out of TLS teardown is undefined on every platform",
    ),
    (
        "sys/thread_local/no_threads.rs",
        "Attempted to initialize thread-local while it is being dropped",
        "a TLS access from inside TLS teardown, a caller bug on every platform",
    ),
    (
        "sys/os_str/bytes.rs",
        "is not an OsStr boundary",
        "a slicing bounds assertion, the `OsStr` twin of `str`'s",
    ),
    // ---- ours, and deliberate ------------------------------------------------------------------
    (
        "sys/random/nife.rs",
        "panic!(",
        "the entropy service's own refusals: `std::random` promises cryptographic strength, so a \
         service that cannot deliver it must not return bytes (DECISIONS §44, milestone 56)",
    ),
    (
        "sys/time/nife.rs",
        "panic!(",
        "the clock page's refusals: a wall clock that reads a torn or unrecognised page must not \
         invent a time (milestone 51)",
    ),
    (
        "sys/pal/nife/clockproto.rs",
        "panic!(",
        "the clock contract's own host-side test assertions, generated verbatim from \
         crates/clock_protocol and unreachable in a target build",
    ),
];

/// Every `library/std/src/**` source cargo recorded as an input to this target's builds.
///
/// **From the dep-info, not from reading `cfg_select!`.** Every `.d` file under the `std_exerciser`
/// target directories is scanned and the std paths unioned, which makes this robust to cargo
/// moving where it files dep-info and to the two ISAs compiling slightly different sets: a union
/// over both targets is exactly the set the sweep wants, since a body reachable on either ISA is
/// reachable.
fn compiled_std_sources() -> Vec<PathBuf> {
    let mut deps = Vec::new();
    for triple in STD_TARGETS {
        collect_files(
            &workspace_root().join(format!("std_exerciser/target/{triple}")),
            &mut deps,
        );
    }
    let mut out: Vec<PathBuf> = Vec::new();
    for d in deps {
        if d.extension().and_then(|e| e.to_str()) != Some("d") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&d) else {
            continue;
        };
        for tok in text.split_whitespace() {
            // **`sys/` only, and the boundary is the claim rather than a convenience.** `sys` IS
            // std's platform abstraction layer: everything under it is one platform's answer, and
            // everything above it is portable code that behaves the same here as on Linux. A panic
            // in `sys/` says "this platform has nothing to offer"; a panic in `thread/scoped.rs`
            // or `path.rs` says "you called this wrong", and it says it identically everywhere.
            // Sweeping all of std mixes the two and buries about forty of the second under none of
            // the first, which is what the first version of this check did. The limit is recorded
            // in notes/std.md's BUGS, because it is a real gap: portable std code that is only
            // *reachable* on a platform this thin would not be caught here.
            if tok.contains("library/std/src/sys/") && tok.ends_with(".rs") {
                let p = PathBuf::from(tok);
                if p.is_file() && !out.contains(&p) {
                    out.push(p);
                }
            }
        }
    }
    out.sort();
    out
}

/// Which of `compiled` were not, in fact, compiled out of this worktree's own farm.
///
/// `nife-dev` is an account-wide `rustup toolchain link`: two worktrees racing `cargo xtask
/// std-src` on one machine leave the loser's toolchain pointed at the winner's `target/nife-farm`,
/// and `-Zbuild-std`'s dep-info then caches the winner's absolute paths as inputs to what looks
/// like this worktree's own build. Left unchecked, [`std_aborts`] reads those paths, finds a body
/// it has never seen, and reports it as a defect in this project's source with a file and a line
/// number that in fact name a different checkout entirely. Comparing each path's canonical form
/// against this worktree's own `farm_dir()` is the one comparison that turns that false accusation
/// into a true statement about the machine. Found 2026-08-18 by milestone 117's fifth stranger, in
/// its first `script/test` from a fresh clone, in its first ten minutes. See notes/std.md's BUGS.
fn foreign_std_sources(compiled: &[PathBuf]) -> Vec<PathBuf> {
    let Ok(farm) = farm_std_src().canonicalize() else {
        // No farm resolves here at all. That is not this function's question: std_exerciser
        // could not have produced the dep-info compiled_std_sources() read without one, and
        // std_aborts()'s own empty-set check is what answers a farm that never got built.
        return Vec::new();
    };
    compiled
        .iter()
        .filter(|p| match p.canonicalize() {
            Ok(canon) => !canon.starts_with(&farm),
            Err(_) => false,
        })
        .cloned()
        .collect()
}

/// `.../library/std/src/sys/exit.rs` as `sys/exit.rs`, which is how std's own source refers to it.
fn std_relative(p: &Path) -> String {
    let s = p.display().to_string();
    match s.split_once("library/std/src/") {
        Some((_, rest)) => rest.to_string(),
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// After the hard-link clone, rustc and its driver must be independent inodes and nothing else
    /// may be, since copying `libLLVM` would add 140 MB per worktree for no benefit. The
    /// driver's name carries a per-nightly hash, which is the case an exact-name match would miss.
    #[cfg(unix)]
    #[test]
    fn the_farm_copies_rustc_and_its_driver_and_links_the_rest() {
        use std::os::unix::fs::MetadataExt;
        let base =
            std::env::temp_dir().join(format!("nife-farm-anchor-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (real, farm) = (base.join("real"), base.join("farm"));
        let files = [
            ("bin/rustc", true),
            ("bin/cargo", false),
            ("lib/librustc_driver-3d7ac1467fbd50fa.dylib", true),
            ("lib/libLLVM.dylib", false),
            ("lib/librustc-nightly_rt.asan.dylib", false),
        ];
        for d in ["bin", "lib"] {
            std::fs::create_dir_all(real.join(d)).unwrap();
            std::fs::create_dir_all(farm.join(d)).unwrap();
        }
        for (f, _) in files {
            std::fs::write(real.join(f), f).unwrap();
            std::fs::hard_link(real.join(f), farm.join(f)).unwrap();
        }
        unlink_sysroot_anchors(&farm).unwrap();
        for (f, copied) in files {
            let ino = |root: &Path| std::fs::metadata(root.join(f)).unwrap().ino();
            assert_eq!(ino(&real) != ino(&farm), copied, "{f}");
            assert_eq!(
                std::fs::read_to_string(farm.join(f)).unwrap(),
                f,
                "{f} keeps its bytes"
            );
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The exerciser build's target dir is pinned, and an export that would move it is named
    /// rather than obeyed or ignored. An export that already equals the pin is the no-op case:
    /// announcing a "correction" that changes nothing would train readers to skim past the real
    /// one.
    #[test]
    fn the_exerciser_target_dir_is_pinned_and_an_override_is_named() {
        let (pinned, notes) = exerciser_target_dir(None);
        assert!(notes.is_empty(), "nothing exported, nothing to correct");
        assert!(pinned.ends_with("std_exerciser/target"));

        let (_, notes) = exerciser_target_dir(Some(&pinned.display().to_string()));
        assert!(
            notes.is_empty(),
            "an export equal to the pin is already right"
        );

        let (still, notes) = exerciser_target_dir(Some("/nonexistent/elsewhere/target"));
        assert_eq!(still, pinned, "the pin does not follow the export");
        assert_eq!(notes.len(), 1);
        assert!(
            notes[0].contains("/nonexistent/elsewhere/target"),
            "the note names what it overrode"
        );
    }

    /// The copy into the patched std sysroot must drop a `# Examples` section and keep everything
    /// else, including the `text` diagrams the protocol crates lead with. The two cases worth
    /// pinning are the ones a naive line filter gets wrong: a hidden doctest line (`# use ...`)
    /// looks exactly like a heading, and a section that runs to the end of the doc block has no
    /// following heading to stop at.
    #[test]
    fn the_std_copy_drops_doc_examples_and_keeps_the_prose() {
        let src = "\
//! A contract.
//!
//! ```text
//!   a diagram
//! ```
//!
//! # Examples
//!
//! ```
//! # use entropy_protocol::GET;
//! assert_eq!(GET, 1);
//! ```
//!
//! # Nothing here transforms a byte
//!
//! Prose that must survive.

/// An item.
///
/// # Examples
///
/// ```
/// let x = 1;
/// ```
pub const GET: u64 = 1;
";
        let got = strip_doc_examples(src);
        assert!(got.contains("a diagram"), "text blocks are documentation");
        assert!(got.contains("# Nothing here transforms a byte"));
        assert!(got.contains("Prose that must survive."));
        assert!(got.contains("/// An item."));
        assert!(got.contains("pub const GET: u64 = 1;"));
        assert!(!got.contains("# Examples"));
        assert!(
            !got.contains("entropy_protocol"),
            "the copy is an inner module of std, where that crate does not exist"
        );
        assert!(
            !got.contains("let x = 1;"),
            "a trailing section, with no heading after it to stop at"
        );
    }
}

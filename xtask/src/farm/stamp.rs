//! **The farm's input stamp**: a hash of everything that decides what the farm holds, so a stale
//! farm is rebuilt rather than trusted.
//!
//! Cut out of `farm.rs` unchanged on 2026-10-12 (UTC) by lane maintainer/near-ceiling-splits, for
//! §266 (a Rust source file stays under 2,000 lines) and §282 (when a file is split, the parent
//! becomes an index). What was private to `farm` is `pub(super)` here, which reaches the same
//! modules.
//!
//! Name: provisional (lane maintainer/near-ceiling-splits, 2026-10-12 UTC), for the module and the
//! file.

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
        // Each thread's cache in front of the heap (milestone 561), generated beside it.
        root.join("crates/allocation_cache/src/lib.rs"),
        // The net PAL generates its wire constants verbatim from the net_stack contract; a change to it
        // must rebuild the farm just like a change to the ABI crate.
        root.join("crates/socket_contract/src/lib.rs"),
        // Likewise the FS-service contract: `std::fs` is a client of it (milestone 27 phase two),
        // and its wire constants are generated verbatim into the PAL.
        root.join("crates/filesystem_contract/src/lib.rs"),
        // The wall-clock and entropy contracts, for the same reason: `sys/time` reads the clock
        // page's layout out of one and `sys/random` packs its requests with the other, so a change
        // to either must rebuild the farm or the PAL silently drifts from the service.
        root.join("crates/clock_contract/src/lib.rs"),
        root.join("crates/entropy_contract/src/lib.rs"),
        // The inert-configuration contract (milestone 47's environment-variable fork, DECISIONS
        // §111): `sys/env` reads the page's layout and `PageBuilder`'s validated domains out of
        // this crate, generated verbatim into the PAL, so a change to either must rebuild the
        // farm or the PAL silently drifts from what assembles the page.
        root.join("crates/environment_contract/src/lib.rs"),
        // The argument page (milestone 205, DECISIONS §170 (how a foreign program is told what to do)): `sys/args` reads the byte argv's
        // layout out of this crate, so a change to it must rebuild the farm.
        root.join("crates/program_argument_contract/src/lib.rs"),
        // The byte-sink contract, generated into the PAL as `sinkproto` like the rest, and missing
        // from this list until 2026-10-09 (UTC): a change to it did not rebuild the farm.
        root.join("crates/byte_sink_contract/src/lib.rs"),
        // The std runtime contract (milestone 595 (provisional)): `rt`'s slot numbers and page
        // addresses, generated verbatim into the PAL, so the loader and the PAL read one file.
        root.join("crates/std_runtime_contract/src/lib.rs"),
        // The name resolver's client wire (milestone 801 (packages over the internet)): `sys/net`'s
        // `lookup_host` speaks it.
        root.join("crates/domain_name_resolution_contract/src/wire.rs"),
        root.join("targets/aarch64-unknown-nife.json"),
        root.join("targets/riscv64-unknown-nife.json"),
        root.join("targets/x86_64-unknown-nife.json"),
        // The timebase page, from milestone 184 (extend the `std` port to x86_64); riscv64 joined
        // 2026-09-21. `rt::cntfrq` reads it on
        // the two architectures with no register stating the rate, and its layout is generated
        // verbatim into the PAL like every contract above.
        root.join("crates/counter_frequency_contract/src/lib.rs"),
        // The current-CPU page and the map it is placed by (milestone 812 (`std::thread::spawn`
        // runs real threads in one address space)): `available_parallelism` reads the allowance
        // off it, so a change to either must rebuild the farm.
        root.join("crates/current_cpu_contract/src/lib.rs"),
        root.join("crates/address_space_map/src/lib.rs"),
    ];
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
    // The heap's crate, by its lockfile entry: a version and checksum name its source exactly.
    if let Some((version, checksum)) = talc_lock_entry() {
        h = fnv(h, format!("{TALC} {version} {checksum}").as_bytes());
    }
    files.sort();
    for f in files {
        // **Hash the path RELATIVE to the workspace root, never the absolute path.** An absolute path
        // makes the stamp a function of *where the checkout lives*, so two trees with byte-identical
        // inputs never match and `std_src` rebuilds the farm unconditionally. Until 2026-10-10 that
        // rebuild also repointed the one machine-wide `nife-dev` link, which is the race behind
        // three broken toolchains on 2026-07-31: an agent worktree ran `script/test`, took the
        // link, and deleting that worktree left `nife-dev` dangling for everything else, failing
        // far from the cause as "override toolchain 'nife-dev' is not installed". Each worktree
        // links its own name now ([`toolchain_name`]); a needless rebuild still costs minutes.
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

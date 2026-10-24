//! **Where each std-built program's ELF lands**, for the archive to pack.
//!
//! A child of `farm` (§282 (when a file is split, the parent becomes an index)): what was private
//! to `farm` is `pub(super)` here, which reaches the same modules.
//!
//! Name: provisional (lane maintainer/near-ceiling-splits, 2026-10-12 UTC), for the module and the
//! file.

use super::*;

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

/// **`std_threads`**, milestone 812 (`std::thread::spawn` runs real threads in one address
/// space)'s exit test as an ordinary `std` program: built by the same `cargo xtask std-exerciser`
/// and packed into each archive beside `std_exerciser`.
pub(crate) fn std_threads_elf(triple: &str) -> PathBuf {
    workspace_root().join(format!("std_exerciser/target/{triple}/release/std_threads"))
}

/// **`std_heap_contention`**, milestone 812's measurement of the `std` heap's spinlock under four
/// threads, packed beside `std_threads`.
pub(crate) fn std_heap_contention_elf(triple: &str) -> PathBuf {
    workspace_root().join(format!(
        "std_exerciser/target/{triple}/release/std_heap_contention"
    ))
}

/// **`std_rayon`**, milestone 812's second exit-test check: unmodified `rayon`'s parallel sum,
/// packed beside `std_threads`.
pub(crate) fn std_rayon_elf(triple: &str) -> PathBuf {
    workspace_root().join(format!("std_exerciser/target/{triple}/release/std_rayon"))
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

/// **`jig`, the package manager** (milestone 809 (the package client becomes a program)): its own
/// workspace, built by `cargo xtask std-exerciser`, and packed into every archive as
/// `grant_plan::Program::Jig`. Since milestone 801 (packages over the internet) it links the TLS
/// graph and builds in that graph's target directory, [`TLS_GRAPH_TARGET`].
pub(crate) fn jig_elf(triple: &str) -> PathBuf {
    workspace_root().join(format!("{TLS_GRAPH_TARGET}/{triple}/release/jig"))
}

/// **`mdr`** (milestone 784 (reuse what the tree wrote), item (f2)), built by
/// [`crate::farm::build_mdr`] beside `jig`. Name: provisional, 2026-10-20 (UTC).
pub(crate) fn mdr_elf(triple: &str) -> PathBuf {
    workspace_root().join(format!("{TLS_GRAPH_TARGET}/{triple}/release/mdr"))
}

/// **Where the TLS graph builds**: `helpers/build-cryptography-exerciser.sh` and
/// `helpers/build-pinned-tls-exerciser.sh` share it, with `cryptography_exerciser/.cargo/config.toml`'s
/// flags, and `jig` joins them (milestone 801), so the standard library is built once for all
/// three. Name: provisional 2026-10-12 (UTC).
pub(crate) const TLS_GRAPH_TARGET: &str = "target/tls-graph";

/// **The package index's in-guest client**: milestone 801 (packages over the internet). The second
/// binary of `pinned_tls_exerciser`'s workspace, which `helpers/build-pinned-tls-exerciser.sh`
/// puts beside the first; [`tls_graph`] runs that for every `cargo xtask test`.
pub(crate) fn package_fetch_exerciser_elf(triple: &str) -> PathBuf {
    workspace_root().join(format!(
        "target/pinned-tls-exerciser/{triple}/package_fetch_exerciser"
    ))
}

/// **The C programs milestones 835 (a C library, stage 1: files, clock and memory) and 836 (a C
/// library, stage 2: threads) run unmodified, by name.** Each is built by its own
/// `helpers/build-<name>.sh`, which fetches somebody else's C and links it with nife's C library;
/// the archive carries whichever are on disk. `c_library_contract` is the exception that fetches
/// nothing: the C library's own contract program (`fixtures/c/c_library_contract.c`, milestone
/// 833), here until milestone 869 (the C library's headers cannot drift, and its contract is tested
/// in nife) builds it in the gate.
/// speedtest1, ioping and hackbench left this list for basalt's packages (milestones 897 (SQLite is
/// packaged in basalt), 906 (ioping is packaged in basalt) and 907 (hackbench is packaged in
/// basalt)); the archive takes them from `crate::basalt`.
pub(crate) const C_PROGRAMS: [&str; 1] = ["c_library_contract"];

/// **One of [`C_PROGRAMS`], if somebody built it**: present when on disk, absent otherwise.
/// `system_tests/src/user/c_thread_tests.rs` skips its test when the program is absent.
pub(crate) fn c_program_elf(name: &str, triple: &str) -> PathBuf {
    workspace_root().join(format!("target/{name}/{triple}/{name}"))
}

/// **The crypto-provider workload**: milestone 442 (a crypto provider `rustls` can use on all three
/// bare-metal targets).
///
/// `helpers/build-cryptography-exerciser.sh` puts it here, and [`tls_graph`] runs that for every
/// `cargo xtask test` since milestone 855 (the TLS graph enters the gated build). Until then it
/// rode in the archive on `ripgrep`'s terms, because the provider's crates were calef's to rule on;
/// §196 (nife carries TLS) and §198 (the glue is ours, the primitives are not) ruled them. The
/// archive still packs it only when it is on disk, so a boot that skips the build (`cargo xtask
/// run`, a bench) carries none and `system_tests/src/user/cryptography_tests.rs` skips there.
pub(crate) fn cryptography_exerciser_elf(triple: &str) -> PathBuf {
    workspace_root().join(format!(
        "target/cryptography-exerciser/{triple}/cryptography_exerciser"
    ))
}

/// **The pinned TLS client's workload**: milestone 501 (a TLS client that speaks to one pinned
/// peer). `helpers/build-pinned-tls-exerciser.sh` puts it here, on [`cryptography_exerciser_elf`]'s
/// terms, and `system_tests/src/user/pinned_tls_tests.rs` skips when the archive has none.
pub(crate) fn pinned_tls_exerciser_elf(triple: &str) -> PathBuf {
    workspace_root().join(format!(
        "target/pinned-tls-exerciser/{triple}/pinned_tls_exerciser"
    ))
}

/// **Build the TLS graph's two programs for `triples`**, and return whether both built: milestone
/// 855 (the TLS graph enters the gated build).
///
/// `cryptography_exerciser` (milestone 442) and `pinned_tls_exerciser`'s workspace, whose second
/// binary is milestone 801 (packages over the internet)'s `package_fetch_exerciser`, built by their
/// two helpers (no second copy of the cargo lines) wherever the kernel suite runs, so a toolchain
/// bump or `std` overlay change that breaks them turns a pull request red. Only `triples` are built
/// (`NIFE_CRYPTO_TRIPLES`). Cold, both measured about 28 s a triple on patagonia (the block's
/// figure); warm, nothing to do. The helpers start with `cargo xtask std-src`, already run.
///
/// Name: provisional, 2026-10-10 (UTC), after the milestone's own words.
pub(crate) fn tls_graph(triples: &[&str]) -> bool {
    let started = std::time::Instant::now();
    for helper in [
        "helpers/build-cryptography-exerciser.sh",
        "helpers/build-pinned-tls-exerciser.sh",
    ] {
        let ok = Command::new(workspace_root().join(helper))
            .current_dir(workspace_root())
            .env("NIFE_CRYPTO_TRIPLES", triples.join(" "))
            .status()
            .map(|st| st.success())
            .unwrap_or(false);
        if !ok {
            eprintln!("tls-graph: {helper} failed for {}", triples.join(" "));
            return false;
        }
    }
    eprintln!(
        "tls-graph: built for {} in {:.1} s",
        triples.join(" "),
        started.elapsed().as_secs_f64()
    );
    true
}

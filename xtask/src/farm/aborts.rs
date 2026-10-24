//! **The abort sweep** (milestone 64 (enough `std` to run somebody else's crate), fourth pass):
//! every std call that kills a nife process instead of refusing it, and the list of the ones
//! accepted.
//!
//! A child of `farm` (§282 (when a file is split, the parent becomes an index)): what was private
//! to `farm` is `pub(super)` here, which reaches the same modules.
//!
//! Name: provisional (lane maintainer/near-ceiling-splits, 2026-10-12 UTC), for the module and the
//! file.

use super::*;

/// **Every std call that kills a nife process instead of refusing it** (milestone 64). Returns
/// false if the sweep cannot run or finds a body not in [`ABORTS_ACCEPTED`].
///
/// It reads which `library/std/src/sys/` sources rustc actually compiled for nife out of cargo's
/// own dep-info, so nothing here models `cfg`, and finds each body there that ends the process. A
/// function that aborts never answers `Unsupported`, so the gap list in notes/crates-io-on-nife.md
/// cannot see one. It does not judge: most of what it finds is correct, so a new body fails the
/// build until it is bound in the PAL or accepted with its reason. See notes/std/std-aborts.md.
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
            "\nstd-aborts: this is not a defect in the file or line above; it is a build that resolved \
             a DIFFERENT worktree's farm the last time `cargo xtask std-exerciser` ran here (before \
             2026-10-10, two worktrees racing `xtask std-src` for the one account-wide `nife-dev` \
             rustup link did this). Fix: rm -rf std_exerciser/target && cargo xtask std-exerciser, which \
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
pub(super) fn abort_shaped(line: &str) -> Option<&'static str> {
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
/// `(file as std names it, a substring of the line, why it is allowed)`. A substring, not a line
/// number, so a nightly that adds a blank line does not rewrite the list; an upstream rewording
/// still moves it, which is a reread this check is for. Read the third column before adding an
/// entry. Three reasons appear, and only one of them is a licence: *unreachable on nife* (behind
/// a `cfg` nife does not satisfy; the safe ones), *no answer exists* (single-threaded, so the call
/// can only deadlock or end, and upstream chose to end), and *ours, and deliberate* (the PAL's
/// own, where ending the process is the honest report). See notes/std/std-aborts.md.
pub(super) const ABORTS_ACCEPTED: &[(&str, &str, &str)] = &[
    // ---- unreachable on nife ------------------------------------------------------------------
    // The heap's generated `talc` core (milestone 784 (reuse what the tree wrote), item d): each is
    // a misconfiguration check, and std's one configuration, `DefaultBinning` on a 64-bit target,
    // passes all three.
    (
        "sys/alloc/nife/talc/base/binning.rs",
        "only 64-bit and 32-bit architectures are currently supported",
        "the arm after `cfg!(target_pointer_width = \"64\")`, and every nife target is 64-bit",
    ),
    (
        "sys/alloc/nife/talc/base/binning.rs",
        "expects at least 29 bins to be available",
        "a check on `B::BIN_COUNT`, a constant; `DefaultBinning` has 191",
    ),
    (
        "sys/alloc/nife/talc/base/bitfield.rs",
        "panic!(",
        "in `TwoLevelBitField`, which `DefaultBinning` does not use: its bit field is `[usize; 3]`. \
         The site's line is a bare `panic!(`, so this entry would also pass a second one in the file",
    ),
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
    // ---- the same on every platform -------------------------------------------------------------
    (
        "sys/sync/once/futex.rs",
        "Once instance has previously been poisoned",
        "poison propagation, which is `Once`'s documented behaviour on every platform",
    ),
    (
        "sys/thread_local/os.rs",
        "Allocation failure",
        "upstream's key-based thread-local storage failing to allocate a value, the same on every \
         platform that uses keys (milestone 812 moved nife from `no_threads` to keys)",
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
        "sys/thread_local/key/nife.rs",
        "no memory for the main thread's thread-local block",
        "the heap cannot give the main thread its first two kilobytes, so no thread-local can work \
         and the allocator would end the process the same way a moment later (milestone 812)",
    ),
    (
        "sys/thread_local/key/nife.rs",
        "out of thread-local keys",
        "a process has a fixed number of keys and never reuses one, as on xous; recorded in that \
         file's BUGS (milestone 812)",
    ),
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
         crates/clock_contract and unreachable in a target build",
    ),
];

/// Every `library/std/src/**` source cargo recorded as an input to this target's builds.
///
/// **From the dep-info, not from reading `cfg_select!`.** Every `.d` file under the `std_exerciser`
/// target directories is scanned and the std paths unioned, which makes this robust to cargo
/// moving where it files dep-info and to the two ISAs compiling slightly different sets: a union
/// over both targets is exactly the set the sweep wants, since a body reachable on either ISA is
/// reachable.
pub(super) fn compiled_std_sources() -> Vec<PathBuf> {
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
            // `sys/` only: it is std's platform layer, where a panic says "this platform has
            // nothing to offer", while one above it says "you called this wrong" on every platform.
            // The gap this leaves is in notes/std/std-aborts.md's BUGS.
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
/// `-Zbuild-std`'s dep-info caches absolute paths, so a build that resolved another checkout's farm
/// leaves paths [`std_aborts`] would report as a defect in this project's source, with a file and
/// line that name a different checkout. Comparing each path's canonical form against this
/// worktree's own `farm_dir()` turns that false accusation into a true statement about the
/// machine. See notes/std/std-aborts.md's BUGS.
pub(super) fn foreign_std_sources(compiled: &[PathBuf]) -> Vec<PathBuf> {
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
pub(super) fn std_relative(p: &Path) -> String {
    let s = p.display().to_string();
    match s.split_once("library/std/src/") {
        Some((_, rest)) => rest.to_string(),
        None => s,
    }
}

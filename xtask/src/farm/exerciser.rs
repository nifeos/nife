//! **`std-exerciser`**: build the std programs against the farm, for every std target.
//!
//! Cut out of `farm.rs` unchanged on 2026-10-12 (UTC) by lane maintainer/near-ceiling-splits, for
//! §266 (a Rust source file stays under 2,000 lines) and §282 (when a file is split, the parent
//! becomes an index). What was private to `farm` is `pub(super)` here, which reaches the same
//! modules.
//!
//! Name: provisional (lane maintainer/near-ceiling-splits, 2026-10-12 UTC), for the module and the
//! file.

use super::*;

/// The custom-target triples the std demo builds for, one per supported ISA. The name is the
/// JSON spec's file stem, which is also cargo's target-dir subdirectory.
pub(super) const STD_TARGETS: [&str; 3] = [
    "aarch64-unknown-nife",
    "riscv64-unknown-nife",
    "x86_64-unknown-nife",
];

/// The `CARGO_TARGET_DIR` the `std_exerciser` build runs under, and the note to print when an
/// exported value had to be overridden to get it there.
///
/// Everything that consumes this build looks under `std_exerciser/target` and nowhere else:
/// [`std_exerciser_elf`], [`std_echo_elf`], [`std_grep_elf`], and the dep-info [`std_aborts`]
/// reads. An inherited `CARGO_TARGET_DIR` (AGENTS.md tells every lane to export one for its
/// gates) moves the build while leaving every one of those readers behind, so the sweep judges
/// the last unredirected run's evidence and the archive packs its ELFs. That is not hypothetical:
/// on 2026-09-30 the 1377 lane built std from a sysroot this worktree never chose under exactly
/// that export, twice, and `std-aborts` reported "all accounted for" over stale evidence both
/// times. Pinning makes the wrong state unrepresentable; printing the override keeps the
/// correction from being one more thing that happened silently.
///
/// Name: provisional, minted 2026-10-18 (UTC) by maintainer/names-comply-free-crates. Refused
/// `exerciser_target_dir` (`dir` is spelled out, per §154 (the acronym test is whether the phrase
/// is spoken)).
pub(super) fn exerciser_target_directory(inherited: Option<&str>) -> (PathBuf, Vec<String>) {
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
/// this worktree's patched std farm. panic=abort and singlethread come from the target specs;
/// `compiler-builtins-mem` supplies memcpy/memset for the bare target.
///
/// `RUSTUP_TOOLCHAIN` is set explicitly rather than via a `+toolchain` selector, because the cargo proxy
/// that launched this xtask already exports `RUSTUP_TOOLCHAIN=nightly`, which would override a
/// `+` selector and silently build std from the *unpatched* sysroot.
///
/// **It names this worktree's farm by path, not `nife-dev` by name** (a correction, found by
/// milestone 606's lane on 2026-09-27). `nife-dev` was then one symlink for the whole machine, and a
/// lane gating beside another lane had it relinked away mid-build: `std_src` relinked it to this
/// farm, another lane's gate relinked it to theirs a moment later, and this build compiled an
/// unpatched std and failed three times running. rustup accepts a toolchain path in
/// `RUSTUP_TOOLCHAIN`, so the build now uses the farm it just checked and cannot be pointed
/// elsewhere. The `helpers/` builds name the farm by path too, since 2026-09-30, for this same
/// reason. Since 2026-10-10 the link is per worktree ([`toolchain_name`]), for a person typing
/// `+<name>`, and nothing here reads it.
///
/// **The build is pinned to `std_exerciser/target` by `CARGO_TARGET_DIR`** (the sysroot-theft
/// fix, 2026-09-30). `Command` inherits the environment, and AGENTS.md tells every lane to gate
/// with `CARGO_TARGET_DIR=$PWD/target` exported, so the cargo child obeyed it: the build landed
/// in the workspace's shared `target/`, dep-info and ELFs with it, while [`std_aborts`] and the
/// archive's ELF paths read `std_exerciser/target`. The sweep then judged the previous
/// unredirected run's evidence, and a build that had resolved a foreign or unpatched sysroot
/// passed as green; the 1377 lane had exactly that, twice. [`exerciser_target_directory`] pins, and
/// names the override out loud rather than absorbing it silently.
pub(crate) fn std_exerciser() -> bool {
    if !std_src() {
        return false;
    }
    let manifests = [s(workspace_root().join("std_exerciser/Cargo.toml"))];
    // Pin where the build and its evidence land before the first child runs, so the ELFs the
    // archive packs, the dep-info the sweep reads, and this build cannot be separated by an
    // exported variable they never agreed to.
    let inherited_target_dir = std::env::var("CARGO_TARGET_DIR").ok();
    let (target_dir, notes) = exerciser_target_directory(inherited_target_dir.as_deref());
    for note in &notes {
        eprintln!("std-exerciser: {note}");
    }
    for (triple, manifest) in STD_TARGETS
        .iter()
        .flat_map(|t| manifests.iter().map(move |m| (t, m)))
    {
        let spec = s(workspace_root().join(format!("targets/{triple}.json")));
        let ok = Command::new("cargo")
            .env("RUSTUP_TOOLCHAIN", farm_dir())
            .env("CARGO_TARGET_DIR", &target_dir)
            .args([
                "build",
                "--release",
                "--manifest-path",
                manifest,
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
            eprintln!("std-exerciser: building {manifest} for {triple} failed");
            return false;
        }
    }
    // **And `jig`**, for every target, because this is the step every gate already runs before it
    // packs an archive. Each archive also builds it for its own target ([`build_jig`]).
    for triple in STD_TARGETS {
        if let Err(e) = build_jig(triple).and_then(|()| build_mdr(triple)) {
            eprintln!("std-exerciser: {e}");
            return false;
        }
    }
    // The build just produced the dep-info the sweep reads, so this costs a few file reads and
    // nothing else. Running it here rather than in `script/lint` is deliberate: the sweep's input
    // is "which std sources did rustc actually compile for nife", which only exists after a build.
    std_aborts()
}

/// **Build `jig` for `triple`, farm first** (milestone 809 (the package client becomes a
/// program)): its own workspace, a program every image ships rather than an exerciser. Every
/// archive calls this for its own target, so an image built in a fresh worktree carries `jig`
/// without `std-exerciser` having run, and `crate::archive` refuses an archive that still lacks
/// it. Measured 2026-10-18 (UTC) on patagonia: 16 s for a cold farm and 30 s for a cold `jig`
/// on one target, once per worktree; under a second warm.
///
/// Since milestone 801 (packages over the internet) it speaks TLS to the index, so it builds with
/// the TLS graph's flags, read from the one file that lists them (`--config`, so there is no second
/// copy), into that graph's target directory.
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/image-needs-jig.
pub(crate) fn build_jig(triple: &str) -> Result<(), String> {
    build_shipped_std_program("jig", triple)
}

/// **Build `mdr` for `triple`, farm first** (milestone 784 (reuse what the tree wrote), item (f2)):
/// the markdown renderer, a `std` program every image ships, on `jig`'s terms and into the same
/// target directory, so the standard library is built once for both.
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/784-mdr-pulldown.
pub(crate) fn build_mdr(triple: &str) -> Result<(), String> {
    build_shipped_std_program("mdr", triple)
}

/// Build the shipped `std` program whose workspace is the directory `program` for `triple`, with the
/// TLS graph's flags and target directory. Name: provisional, 2026-10-20 (UTC).
fn build_shipped_std_program(program: &str, triple: &str) -> Result<(), String> {
    if !std_src() {
        return Err(format!(
            "building {program} for {triple}: the std farm did not build"
        ));
    }
    let spec = s(workspace_root().join(format!("targets/{triple}.json")));
    let ok = Command::new("cargo")
        .env("RUSTUP_TOOLCHAIN", farm_dir())
        .env("CARGO_TARGET_DIR", workspace_root().join(TLS_GRAPH_TARGET))
        .args([
            "build",
            "--release",
            "--locked",
            "--manifest-path",
            &s(workspace_root().join(format!("{program}/Cargo.toml"))),
            "--config",
            &s(workspace_root().join("cryptography_exerciser/.cargo/config.toml")),
            "-Zjson-target-spec",
            "-Zbuild-std=core,alloc,std,panic_abort",
            "-Zbuild-std-features=compiler-builtins-mem",
            "--target",
            &spec,
        ])
        .status()
        .map(|st| st.success())
        .unwrap_or(false);
    if ok {
        Ok(())
    } else {
        Err(format!("building {program} for {triple} failed"))
    }
}

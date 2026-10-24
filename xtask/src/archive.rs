//! The userspace archives: the initial ramdisk each architecture's kernel is booted with.
//!
//! One packer per ISA, over the same program list, plus the check that every program the
//! tree loads by name is declared in a manifest the packer reads.

use crate::disk::{mkfs_elf, redoxfs_server_elf};
use crate::farm::{
    C_PROGRAMS, c_program_elf, cryptography_exerciser_elf, jig_elf, mdr_elf,
    package_fetch_exerciser_elf, pinned_tls_exerciser_elf, std_exerciser_elf,
    std_heap_contention_elf, std_rayon_elf, std_resolve_elf, std_threads_elf,
};
use crate::host::{bin_elf, workspace_root};
use crate::inspect::read_stripped;
use crate::measure::{boot_programs, measurement_table, write_measure_manifest};
use crate::{RISCV_TARGET, TARGET, X86_TARGET, profile_dir};

/// **Build the two program packages, `components` and `fixtures`, for `target`**, in the run's
/// profile and with the attacker roles only when [`crate::CONFINEMENT_ATTACKERS`] says so. The one
/// place the three initial ramdisk builders ([`crate::user`], [`initial_ramdisk_riscv`],
/// [`initial_ramdisk_x86`]) spell that build, so the feature cannot reach one architecture's
/// archive and not another's. Switching between a test and any other boot recompiles
/// `components`, because cargo fingerprints features; that is the price of the shipped image not
/// containing the attacker. Name provisional.
pub(crate) fn build_programs(target: &str) -> bool {
    let mut args = vec![
        "build",
        "-p",
        "components",
        "-p",
        "fixtures",
        "--target",
        target,
    ];
    if crate::CONFINEMENT_ATTACKERS.load(std::sync::atomic::Ordering::Relaxed) {
        args.extend(["--features", "components/confinement_attackers"]);
    }
    // A swish-check probe's own program feature (`--input-race`), on no other boot.
    if let Some(feature) = crate::swish_check::probe_program_feature() {
        args.extend(["--features", feature]);
    }
    crate::cargo_profiled(&args)
}

/// Where the packed initial ramdisk archive is written.
pub(crate) fn initial_ramdisk_path() -> String {
    workspace_root()
        .join("target/initial-ramdisk.img")
        .display()
        .to_string()
}

/// Where the RISC-V initial ramdisk archive is written (milestone 20 (a portable HAL)). Separate
/// from the aarch64 one because it holds riscv64 ELFs, not aarch64 ones.
pub(crate) fn riscv_initial_ramdisk_path() -> String {
    workspace_root()
        .join("target/initial-ramdisk-riscv.img")
        .display()
        .to_string()
}

/// The packages a program can live in (milestone 175's split). See notes/adding-a-program.md for
/// which one a new program belongs in.
const PROGRAM_PACKAGES: [&str; 2] = ["components", "fixtures"];

/// **Every program this tree builds**, read from the one place it is declared: the `[[bin]]` blocks
/// in `components/Cargo.toml` and `fixtures/Cargo.toml`, less the programs a package lists as
/// [`packaged_only`] (milestone 150; name provisional). All three archives pack exactly this list,
/// in `Cargo.toml` order, which nothing depends on: the progenitor looks entries up by name and
/// [`measurement_table`] is sorted. It is deliberately not filtered per architecture.
///
/// Refuses, rather than packing a partial archive, when a `[[bin]]` block has a shape
/// [`bin_names`] does not understand, and when [`check_declared_programs`] fails. See
/// notes/adding-a-program/the-archives.md.
fn declared_programs() -> Result<&'static [String], String> {
    // Read once per `xtask` run: `test` packs three archives, and they must pack the same list.
    static DECLARED: std::sync::OnceLock<Result<Vec<String>, String>> = std::sync::OnceLock::new();
    let declared = DECLARED.get_or_init(|| {
        let mut names = Vec::new();
        let mut absent = Vec::new();
        for package in PROGRAM_PACKAGES {
            let path = workspace_root().join(package).join("Cargo.toml");
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            names.extend(bin_names(&text).map_err(|e| format!("{}: {e}", path.display()))?);
            absent.extend(packaged_only(&text).map_err(|e| format!("{}: {e}", path.display()))?);
        }
        check_declared_programs(&names)?;
        for name in &absent {
            if !names.contains(name) {
                return Err(format!(
                    "`{name}` is packaged_only and no [[bin]] builds it"
                ));
            }
            if grant_plan::Program::ALL.iter().any(|p| p.name() == name)
                || boot_programs().contains(&name.as_str())
            {
                return Err(format!(
                    "`{name}` is packaged_only, and the shell or the boot needs it in the image"
                ));
            }
        }
        names.retain(|name| !absent.contains(name));
        Ok(names)
    });
    declared.as_deref().map_err(Clone::clone)
}

/// Read the stripped ELF of every [`declared_programs`] entry for one archive, `elf` mapping a
/// program name to where this architecture's build put it. `None` after saying why on stderr,
/// prefixed with `archive` so a failure names which of the three packers hit it.
fn declared_program_blobs(
    archive: &str,
    elf: impl Fn(&str) -> String,
) -> Option<Vec<(&'static str, Vec<u8>)>> {
    let names = match declared_programs() {
        Ok(names) => names,
        Err(e) => {
            eprintln!("{archive}: {e}");
            return None;
        }
    };
    let mut blobs = Vec::with_capacity(names.len());
    for name in names {
        match read_stripped(&elf(name)) {
            Ok(b) => blobs.push((name.as_str(), b)),
            Err(e) => {
                eprintln!("{archive}: cannot read {}: {e}", elf(name));
                return None;
            }
        }
    }
    Some(blobs)
}

/// **The programs a `Cargo.toml` says no boot image packs**: the `packaged_only` list under
/// `[package.metadata.nife]`, which cargo ignores and this reads (milestone 198 (a package
/// manager) rung 3a). They are built like every other program and reach a machine only as a
/// package. Empty when there is no such table. One line, `packaged_only = ["a", "b"]`, is the
/// whole shape it reads; any other line in that table is an error, because skipping it would pack
/// a program that was meant to arrive only as a package.
fn packaged_only(manifest: &str) -> Result<Vec<String>, String> {
    let mut inside = false;
    let mut names = Vec::new();
    for (i, raw) in manifest.lines().enumerate() {
        let line = raw.trim();
        if line.starts_with('[') {
            inside = line == "[package.metadata.nife]";
            continue;
        }
        if !inside || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let list = line
            .strip_prefix("packaged_only")
            .map(str::trim_start)
            .and_then(|l| l.strip_prefix('='))
            .map(str::trim)
            .and_then(|l| l.strip_prefix('['))
            .and_then(|l| l.strip_suffix(']'))
            .ok_or_else(|| {
                format!(
                    "line {}: [package.metadata.nife] holds only `packaged_only = [\"name\", ...]`",
                    i + 1
                )
            })?;
        for item in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let name = item
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .ok_or_else(|| format!("line {}: `{item}` is not a quoted name", i + 1))?;
            names.push(name.to_string());
        }
    }
    Ok(names)
}

/// The `name` of every `[[bin]]` table in a `Cargo.toml`, in order.
///
/// **A reader for the subset of TOML this tree writes, and strict about it**, because DECISIONS
/// §46 keeps a TOML parser out of `xtask` for one list and a lenient scanner would be the worst of
/// both: it would silently skip the block it did not understand, and that program would be missing
/// from every archive. So a key it does not know inside a `[[bin]]` block is an error naming the key.
/// `required-features` in particular would mean cargo does not build the binary by default, which
/// is a thing the packer has to be taught rather than guess.
fn bin_names(manifest: &str) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    let mut in_bin = false;
    let mut current: Option<String> = None;
    let finish = |in_bin: bool, current: &mut Option<String>, names: &mut Vec<String>| {
        if !in_bin {
            return Ok(());
        }
        match current.take() {
            Some(n) => {
                names.push(n);
                Ok(())
            }
            None => Err("a [[bin]] block with no `name`".to_string()),
        }
    };
    for (i, raw) in manifest.lines().enumerate() {
        let line = raw.trim();
        if line.starts_with('[') {
            finish(in_bin, &mut current, &mut names)?;
            in_bin = line == "[[bin]]";
            continue;
        }
        if !in_bin || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(format!(
                "line {}: not `key = value` inside a [[bin]] block",
                i + 1
            ));
        };
        match key.trim() {
            "name" => {
                let value = value.trim();
                let name = value
                    .strip_prefix('"')
                    .and_then(|v| v.strip_suffix('"'))
                    .filter(|v| !v.contains('"'))
                    .ok_or_else(|| format!("line {}: `name` is not a plain string", i + 1))?;
                current = Some(name.to_string());
            }
            "path" | "test" | "bench" => {}
            other => {
                return Err(format!(
                    "line {}: `{other}` in a [[bin]] block, which `xtask`'s bin_names does not \
                     know how to pack; teach it what the key means for the archives",
                    i + 1
                ));
            }
        }
    }
    finish(in_bin, &mut current, &mut names)?;
    Ok(names)
}

/// **Archive entries packed from outside `components/` and `fixtures/`**, each present only when its
/// own build ran (see the `initial_ramdisk_*` functions). [`check_declared_programs`] and the
/// lookup test both read it.
const BUILT_ELSEWHERE: [&str; 16] = [
    "redoxfs_server",
    "mkfs",
    "std_exerciser",
    "std_threads",
    "std_heap_contention",
    "std_rayon",
    "rg",
    "ioping",
    "hackbench",
    "speedtest1",
    "cryptography_exerciser",
    "pinned_tls_exerciser",
    "std_resolve",
    "package_fetch_exerciser",
    // Milestone 809 (the package client becomes a program): the package manager, a `std` program
    // in the image, built by every archive for its own target (`farm::build_jig`).
    "jig",
    // Milestone 784 (reuse what the tree wrote), item (f2): the markdown renderer, a `std`
    // program on `jig`'s terms (`farm::build_mdr`).
    "mdr",
];

/// **A `std` program the shell can spawn, built by its own workspace rather than a `[[bin]]`**
/// (milestone 595 (provisional)). `grant_plan::Program::StdExerciser` is the first: it is compiled with
/// `-Zbuild-std` against the patched std farm (`cargo xtask std-exerciser`) and packed iff that
/// ran, so the `[[bin]]` rule cannot apply to it. Both halves are required, so a native `Program`
/// with no binary still fails, and so does a `std` one nothing here packs.
fn is_std_built_elsewhere(p: grant_plan::Program) -> bool {
    p.manifest().startup == grant_plan::StartupContract::Std && BUILT_ELSEWHERE.contains(&p.name())
}

/// **What the declared list must agree with, checked every time an archive is packed.**
///
/// - It holds `progenitor`, so a scanner that stopped finding blocks fails here rather than
///   packing an archive that boots nothing (see
///   design/roadmap/proposals/a-gate-that-selects-the-set-it-judges.md).
/// - No name is declared twice, across both packages.
/// - Every program `grant_plan` lets the shell spawn is built; otherwise it passes every host test
///   and fails only at boot.
/// - Every program the kernel measures at boot ([`boot_programs`]) is built.
fn check_declared_programs(names: &[String]) -> Result<(), String> {
    let has = |n: &str| names.iter().any(|m| m == n);
    if !has("progenitor") {
        return Err(format!(
            "found {} programs and no `progenitor`: the [[bin]] reader is looking at the wrong \
             files or no longer understands them",
            names.len()
        ));
    }
    let mut sorted: Vec<&String> = names.iter().collect();
    sorted.sort();
    if let Some(w) = sorted.windows(2).find(|w| w[0] == w[1]) {
        return Err(format!("`{}` is declared by two [[bin]] blocks", w[0]));
    }
    for p in grant_plan::Program::ALL {
        if !has(p.name()) && !is_std_built_elsewhere(*p) {
            return Err(format!(
                "grant_plan declares `{}` spawnable from the shell, and no [[bin]] in {} builds it",
                p.name(),
                PROGRAM_PACKAGES.join(" or ")
            ));
        }
    }
    for b in boot_programs() {
        if !has(b) {
            return Err(format!("`{b}` is a boot program and no [[bin]] builds it"));
        }
    }
    Ok(())
}

/// **Build the RISC-V userspace archive** (milestone 20, the richer initial ramdisk step): build
/// both program packages for riscv64 and pack every entry under its own name into a nifefs archive.
/// The kernel enters `progenitor` and nothing else. Point `NIFE_INITIAL_RAMDISK` at the result and
/// boot the riscv kernel, e.g.:
///
/// ```text
/// cargo xtask initial-ramdisk-riscv
/// NIFE_INITIAL_RAMDISK=target/initial-ramdisk-riscv.img cargo run -p kernel \
///     --target riscv64imac-unknown-none-elf
/// ```
///
/// Name: ratified 2026-10-12 (calef, #1998, milestone 139 (drive the unsafe count down) round 11).
/// His words: "B for #1998." Refused `initrd_riscv` (the foreign spelling, which stops at the
/// boundary where it arrives).
pub(crate) fn initial_ramdisk_riscv() -> bool {
    // The whole package, never a `--bin` list: a list has to be kept in step with
    // `declared_programs` by hand. Every program compiles for this target.
    if !build_programs(RISCV_TARGET) {
        return false;
    }

    let bin = |name: &str| {
        workspace_root()
            .join(format!(
                "target/{RISCV_TARGET}/{}/{name}",
                crate::profile_dir()
            ))
            .display()
            .to_string()
    };
    // Every declared program, packed under its own name (milestone 150).
    let Some(mut blobs) = declared_program_blobs("initial-ramdisk-riscv", bin) else {
        return false;
    };
    // The `std` programs and the ported ones, exactly as on aarch64. DECISIONS §19 is why `rg`'s
    // leg exists here at all: the same experiment on both ISAs, or a scope note says which one it
    // skipped and why.
    blobs.extend(std_program_blobs("riscv64-unknown-nife"));
    blobs.extend(ported_program_blobs("riscv64-unknown-nife"));
    // The FS server (milestone 32 (a real filesystem: RedoxFS behind a capability FS server) phase
    // 2) and `mkfs`, built for the riscv bare target, ride along
    // when present, exactly as std_exerciser does; `test` builds them first.
    blobs.extend(file_server_blobs(RISCV_TARGET));
    // Measure the boot programs before the riscv kernel is built (milestone 22 phase B.1).
    let Some((size, _)) = write_archive(
        "initial-ramdisk-riscv",
        "riscv64",
        &riscv_initial_ramdisk_path(),
        "the archive",
        &blobs,
    ) else {
        return false;
    };
    eprintln!(
        "wrote {} ({size} bytes): progenitor, least_authority_demo",
        riscv_initial_ramdisk_path()
    );
    true
}

/// Where the `x86_64` initial ramdisk archive is written (milestone 161 (the x86_64 kernel port)).
/// Separate from the other two because it holds `x86_64` ELFs, and the kernel's loader refuses
/// anything whose `e_machine` is not its own (`crates/elf`'s `EXPECTED_MACHINE`).
pub(crate) fn x86_initial_ramdisk_path() -> String {
    workspace_root()
        .join("target/initial-ramdisk-x86_64.img")
        .display()
        .to_string()
}

/// **Build the `x86_64` userspace archive** (milestone 161): build both program packages for
/// `x86_64-unknown-none` and pack the same programs the other two archives pack
/// ([`declared_programs`]).
///
/// ```text
/// cargo xtask initial-ramdisk-x86
/// NIFE_INITIAL_RAMDISK=target/initial-ramdisk-x86_64.img cargo run -p kernel \
///     --target x86_64-unknown-none
/// ```
///
/// Name: ratified 2026-10-12 (calef, #1998, milestone 139 (drive the unsafe count down) round 11).
/// His words: "B for #1998." Refused `initrd_x86` (the foreign spelling, which stops at the
/// boundary where it arrives).
pub(crate) fn initial_ramdisk_x86() -> bool {
    if !build_programs(X86_TARGET) {
        return false;
    }

    let bin = |name: &str| {
        workspace_root()
            .join(format!("target/{X86_TARGET}/{}/{name}", profile_dir()))
            .display()
            .to_string()
    };
    let Some(mut blobs) = declared_program_blobs("initial-ramdisk-x86", bin) else {
        return false;
    };
    // The FS server and `mkfs` (milestone 164 (x86_64 userspace can't build `aes`)), present iff
    // something built them for this target; `test` builds them first. They build here only because
    // `.cargo/config.toml` sets `--cfg aes_force_soft` for this target.
    blobs.extend(file_server_blobs(X86_TARGET));
    // The std demo (milestone 184 (extend the `std` port to x86_64)) and the rest of the `std` and
    // ported programs, each present iff it was built for `x86_64-unknown-nife`.
    blobs.extend(std_program_blobs("x86_64-unknown-nife"));
    blobs.extend(ported_program_blobs("x86_64-unknown-nife"));
    // Measure the boot programs before the x86 kernel is built (milestone 22 phase B.1). With no
    // manifest the generated `TRUST_ROOT` is empty and `trust::require` refuses every boot program
    // as `Unmeasured`. `write_archive` also packs the image's package source (§19 (architectural
    // parity is a tenet)), though no fetch test reads it here: this runner attaches no network.
    let Some((size, entries)) = write_archive(
        "initial-ramdisk-x86",
        "x86_64",
        &x86_initial_ramdisk_path(),
        "the archive",
        &blobs,
    ) else {
        return false;
    };
    eprintln!(
        "wrote {} ({size} bytes): progenitor plus {entries} entries",
        x86_initial_ramdisk_path(),
    );
    true
}

/// **Build the aarch64 userspace archive**: pack the built user ELFs into the nifefs initial
/// ramdisk the kernel hands the progenitor (milestone 19f (run a real workload)). The kernel reads
/// the `progenitor` entry to boot, and the progenitor loads the rest by name. Unlike its siblings
/// this only packs: `user()` builds and then calls it, and so does every command that boots the
/// aarch64 kernel, including the `initial-ramdisk-aarch64` subcommand. The image is generated and
/// never checked in. See notes/adding-a-program/the-archives.md.
///
/// Name: ratified 2026-10-12 (calef, #1998, milestone 139 (drive the unsafe count down) round 11).
/// His words: "B for #1998." Refused `initrd_aarch64` (the foreign spelling, which stops at the
/// boundary where it arrives).
pub(crate) fn initial_ramdisk_aarch64() -> bool {
    let Some(mut blobs) = declared_program_blobs("initial-ramdisk-aarch64", bin_elf) else {
        return false;
    };
    blobs.extend(std_program_blobs("aarch64-unknown-nife"));
    blobs.extend(file_server_blobs(TARGET));
    blobs.extend(ported_program_blobs("aarch64-unknown-nife"));
    // Measure the boot program before the kernel is built (milestone 22 phase B.1). Every caller
    // reaches the kernel build through `user()`, which calls this, so the manifest is always current
    // by the time `kernel/build.rs` reads it.
    write_archive(
        "initial-ramdisk-aarch64",
        "aarch64",
        &initial_ramdisk_path(),
        "the initial ramdisk archive",
        &blobs,
    )
    .is_some()
}

/// **The `std` programs the patched farm builds for `triple`.** `jig` is built here for `triple`
/// ([`crate::farm::build_jig`]), and [`write_archive`] refuses an archive without it
/// ([`SHIPPED_STD_PROGRAMS`]). The rest are exercisers, each packed iff it was built: an
/// interactive `run` that never built them simply ships an archive without them, and nothing loads
/// them there. Every archive packs them in this order.
fn std_program_blobs(triple: &str) -> Vec<(&'static str, Vec<u8>)> {
    // Said here and refused by `write_archive`, which names the fix.
    for built in [
        crate::farm::build_jig(triple),
        crate::farm::build_mdr(triple),
    ] {
        if let Err(e) = built {
            eprintln!("{e}");
        }
    }
    let programs: [FarmProgram; 7] = [
        // The std demo (milestone 27 (Rust `std` on the native ABI)), built by `cargo xtask
        // std-exerciser`, which `test` runs, through a separate toolchain and target.
        ("std_exerciser", std_exerciser_elf),
        // `std_resolve` (milestone 801 (packages over the internet)), built by the same step.
        ("std_resolve", std_resolve_elf),
        // `jig` (milestone 809 (the package client becomes a program)), built above.
        ("jig", jig_elf),
        // `mdr` (milestone 784 (reuse what the tree wrote), item (f2)), built above.
        ("mdr", mdr_elf),
        // Milestone 812 (`std::thread::spawn` runs real threads in one address space)'s exit test,
        // on the same terms, and its two companions.
        ("std_threads", std_threads_elf),
        ("std_heap_contention", std_heap_contention_elf),
        ("std_rayon", std_rayon_elf),
    ];
    present_blobs(triple, &programs)
}

/// **The ported programs and the TLS graph's workloads for `triple`**, each packed iff its helper
/// has been run, in the order every archive packs them.
fn ported_program_blobs(triple: &str) -> Vec<(&'static str, Vec<u8>)> {
    // **Unmodified `ripgrep`** (milestone 121 (`ripgrep` on nife: enumeration as a capability)),
    // the program member of basalt's published package (milestone 810 (`ripgrep` is packaged in
    // basalt and installed with `jig`)), present iff `cargo xtask basalt-fetch` has run, as CI's
    // `rg-threads` and `swish-check` rows do. The archive name is `rg`, which is what the program
    // is called everywhere else in the world.
    let mut blobs = present_blobs(triple, &[("rg", crate::basalt::ripgrep_elf)]);
    // **Unmodified ioping, hackbench and SQLite's speedtest1** (milestones 834, 827 and 831, each
    // on nife and Linux), the program members of basalt's packages (milestones 906 (ioping is
    // packaged in basalt), 907 (hackbench is packaged in basalt) and 897 (SQLite is packaged in
    // basalt)), on `rg`'s terms.
    blobs.extend(present_blobs(
        triple,
        &[
            ("ioping", crate::basalt::ioping_elf),
            ("hackbench", crate::basalt::hackbench_elf),
            ("speedtest1", crate::basalt::speedtest1_elf),
        ],
    ));
    // **The other unmodified C programs** (milestone 835 (a C library, stage 1: files, clock and
    // memory)): each present iff its `helpers/build-<name>.sh` has run.
    for name in C_PROGRAMS {
        let path = c_program_elf(name, triple).display().to_string();
        if let Ok(bytes) = read_stripped(&path) {
            blobs.push((name, bytes));
        }
    }
    let workloads: [FarmProgram; 3] = [
        // **The crypto-provider workload** (milestone 442): present iff
        // `helpers/build-cryptography-exerciser.sh` has run, which `cargo xtask test` does for
        // every leg it boots since milestone 855 (the TLS graph enters the gated build). Other
        // boots (`cargo xtask run`, a bench) carry it only if somebody ran the helper.
        ("cryptography_exerciser", cryptography_exerciser_elf),
        // **The pinned TLS client's workload** (milestone 501 (a TLS client that speaks to one
        // pinned peer)), on exactly those terms: present iff
        // `helpers/build-pinned-tls-exerciser.sh` has run.
        ("pinned_tls_exerciser", pinned_tls_exerciser_elf),
        // The package index's client (milestone 801), built by the same helper, on the same terms.
        ("package_fetch_exerciser", package_fetch_exerciser_elf),
    ];
    blobs.extend(present_blobs(triple, &workloads));
    blobs
}

/// A program's archive name, and where its build for a triple puts it.
type FarmProgram = (&'static str, fn(&str) -> std::path::PathBuf);

/// The stripped bytes of each of `programs` built for `triple`, skipping any that was not built.
fn present_blobs(triple: &str, programs: &[FarmProgram]) -> Vec<(&'static str, Vec<u8>)> {
    programs
        .iter()
        .filter_map(|&(name, elf)| {
            read_stripped(&elf(triple).display().to_string())
                .ok()
                .map(|bytes| (name, bytes))
        })
        .collect()
}

/// **The FS server (milestone 32 phase 2) and `mkfs` (milestone 57 (partitioning and formatting a
/// real drive)'s write half) for a bare
/// `target`**, each packed iff built: they are their own workspace and target, `test` builds them,
/// and a plain interactive boot without them simply skips the FS-server test.
fn file_server_blobs(target: &str) -> Vec<(&'static str, Vec<u8>)> {
    let mut blobs = Vec::new();
    if let Ok(bytes) = read_stripped(&redoxfs_server_elf(target)) {
        blobs.push(("redoxfs_server", bytes));
    }
    if let Ok(bytes) = read_stripped(&mkfs_elf(target)) {
        blobs.push(("mkfs", bytes));
    }
    blobs
}

/// **The `std` programs no image ships without**, though they are built outside `components/` and
/// packed by [`std_program_blobs`] rather than by the `[[bin]]` reader. `jig` is the package
/// client (milestone 809 (the package client becomes a program)); an image without it boots, and
/// the shell answers `jig` with a spawn failure, so the archive is refused instead.
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/image-needs-jig.
const SHIPPED_STD_PROGRAMS: [&str; 2] = ["jig", "mdr"];

/// Refuse `files` if one of [`SHIPPED_STD_PROGRAMS`] is not among them, naming the command that
/// builds it.
fn shipped_std_programs_present(arch: &str, files: &[(&str, &[u8])]) -> Result<(), String> {
    match SHIPPED_STD_PROGRAMS
        .iter()
        .find(|p| !files.iter().any(|(name, _)| name == *p))
    {
        None => Ok(()),
        Some(missing) => Err(format!(
            "the {arch} archive has no `{missing}`, which every image ships; building it failed \
             (the error is above), and `cargo xtask std-exerciser` builds it on its own"
        )),
    }
}

/// **Pack `blobs` and the entries every archive carries into one nifefs image**, write it to
/// `path`, and write the measurement manifest the kernel build reads. `Some((bytes, entries))` on
/// success; `None` after saying why, prefixed with `label`. `what` names the archive in the one
/// message that has always spelled it per architecture.
fn write_archive(
    label: &str,
    arch: &str,
    path: &str,
    what: &str,
    blobs: &[(&'static str, Vec<u8>)],
) -> Option<(usize, usize)> {
    let mut files: Vec<(&str, &[u8])> = blobs.iter().map(|(n, b)| (*n, b.as_slice())).collect();
    // **The image's package source** (milestone 198 (a package manager) rung 3a): every recipe
    // under `packages/` for this architecture is built, written where the package tests' HTTP peer
    // serves it, and its catalogue line packed here, *above* the measurement table so the kernel's
    // trust root vouches for it. `package_archive::CATALOG`'s doc has why.
    let catalogue = match crate::package::image_catalogue(arch) {
        Ok(catalogue) => catalogue,
        Err(complaint) => {
            eprintln!("{label}: the image's package source: {complaint}");
            return None;
        }
    };
    files.push((package_archive::CATALOG, catalogue.as_bytes()));
    // And where the image says its index is (milestone 801), measured the same way.
    let sources = match crate::package::image_index_sources() {
        Ok(sources) => sources,
        Err(complaint) => {
            eprintln!("{label}: the image's index sources: {complaint}");
            return None;
        }
    };
    files.push((package_index::SOURCES_ENTRY, sources.as_bytes()));
    if let Err(complaint) = shipped_std_programs_present(arch, &files) {
        eprintln!("{label}: {complaint}");
        return None;
    }
    // **The measurement table, last, so it measures everything above it** (milestone 104). The progenitor
    // reads this entry out of the archive it already holds and refuses to load a program whose
    // bytes it does not match, vouched for by the kernel's trust root so its refusals mean
    // something. Parity is the point (§19): the same table, the same parser, the same policy on
    // every architecture. See [`measurement_table`] for why it lives here rather than inside the
    // progenitor's own image.
    if !programs_fit_the_address_space_map(label, &files) {
        return None;
    }
    let table = measurement_table(&files);
    files.push((measured_boot::PROGRAM_MEASUREMENTS, table.as_bytes()));

    let size = nifefs::image_size(&files);
    let mut img = std::vec![0u8; size];
    // Carry the error: it names which of `MAX_FILES`, `image_size` and `write_image`'s bounds check
    // refused, and those are otherwise hunted for by hand.
    if let Err(e) = nifefs::write_image(&files, &mut img) {
        eprintln!(
            "{label}: could not build {what}: {e:?} ({} files, {} bytes)",
            files.len(),
            size
        );
        return None;
    }
    if let Err(e) = std::fs::write(path, &img) {
        eprintln!("{label}: could not write {path}: {e}");
        return None;
    }
    write_measure_manifest(arch, &img).then_some((size, files.len()))
}

/// **Every program in an archive fits the address-space map's image band** (milestone 206 (a
/// program image has under 896 KiB)), checked at packing because the kernel's loader only judges
/// the programs a boot loads. `mkfs` once sat outside the band for eight weeks unnoticed.
///
/// The program headers are read by hand rather than with `crates/elf`, which refuses a foreign
/// `e_machine`; this packs riscv64 and `x86_64` archives on an aarch64 host. Entries that are not
/// ELF (the catalogue, the measurement table) are skipped.
fn programs_fit_the_address_space_map(archive: &str, files: &[(&str, &[u8])]) -> bool {
    let mut fits = true;
    for (name, bytes) in files {
        let Some((lo, hi)) = image_span(bytes) else {
            continue;
        };
        if let Err(e) = address_space_map::check_image(lo, hi) {
            eprintln!("{archive}: `{name}` does not fit the address-space map: {e}");
            fits = false;
        }
    }
    fits
}

/// The page-rounded span of a 64-bit little-endian ELF's `PT_LOAD` segments, or `None` if `bytes`
/// is not one or has no loadable segment.
fn image_span(bytes: &[u8]) -> Option<(u64, u64)> {
    const PAGE: u64 = address_space_map::PAGE;
    let u16_at = |o: usize| Some(u16::from_le_bytes(bytes.get(o..o + 2)?.try_into().ok()?));
    let u32_at = |o: usize| Some(u32::from_le_bytes(bytes.get(o..o + 4)?.try_into().ok()?));
    let u64_at = |o: usize| Some(u64::from_le_bytes(bytes.get(o..o + 8)?.try_into().ok()?));
    if bytes.get(0..6)? != [0x7f, b'E', b'L', b'F', 2, 1] {
        return None;
    }
    let phoff = usize::try_from(u64_at(32)?).ok()?;
    let phentsize = usize::from(u16_at(54)?);
    let phnum = usize::from(u16_at(56)?);
    let (mut lo, mut hi) = (u64::MAX, 0u64);
    for i in 0..phnum {
        let ph = phoff.checked_add(i.checked_mul(phentsize)?)?;
        if u32_at(ph)? != 1 {
            continue; // not PT_LOAD
        }
        let vaddr = u64_at(ph + 16)?;
        let end = vaddr.checked_add(u64_at(ph + 40)?)?;
        lo = lo.min(vaddr / PAGE * PAGE);
        hi = hi.max(end.div_ceil(PAGE) * PAGE);
    }
    (lo < hi).then_some((lo, hi))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;

    /// The `[[bin]]` reader against the shape both packages write, and the two it must refuse
    /// rather than skip (milestone 150): a key it does not know, and a block with no name.
    #[test]
    fn bin_names_reads_the_blocks_and_refuses_what_it_does_not_understand() {
        let manifest = "[package]\nname = \"components\"\n\n# a comment\n[[bin]]\n\
                        name = \"wc\"\npath = \"src/wc.rs\"\ntest = false\nbench = false\n\n\
                        [[bin]]\nname = \"date\"\npath = \"src/date.rs\"\n\n\
                        [dependencies]\nname = \"not a bin\"\n";
        assert_eq!(bin_names(manifest).unwrap(), ["wc", "date"]);

        let gated = "[[bin]]\nname = \"x\"\nrequired-features = [\"y\"]\n";
        let e = bin_names(gated).unwrap_err();
        assert!(e.contains("required-features"), "{e}");

        let nameless = "[[bin]]\npath = \"src/x.rs\"\n[[bin]]\nname = \"y\"\n";
        assert!(bin_names(nameless).unwrap_err().contains("no `name`"));
    }

    /// An archive without `jig`, or without `mdr`, is refused with the command that builds it,
    /// rather than packed into an image whose shell answers the name with a spawn failure.
    #[test]
    fn an_archive_without_jig_is_refused_naming_the_build() {
        let e = shipped_std_programs_present("x86_64", &[("progenitor", b"")]).unwrap_err();
        assert!(e.contains("no `jig`"), "{e}");
        assert!(e.contains("cargo xtask std-exerciser"), "{e}");
        let e = shipped_std_programs_present("x86_64", &[("jig", b"")]).unwrap_err();
        assert!(e.contains("no `mdr`"), "{e}");
        assert!(shipped_std_programs_present("x86_64", &[("jig", b""), ("mdr", b"")]).is_ok());
    }

    /// A program linked where the map says passes, and one linked at lld's default base, where
    /// `mkfs` was, is named.
    #[test]
    fn an_archive_program_outside_the_image_band_is_named() {
        fn elf_at(vaddr: u64) -> Vec<u8> {
            let mut v = vec![0u8; 64 + 56];
            v[0..6].copy_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1]);
            v[32..40].copy_from_slice(&64u64.to_le_bytes());
            v[54..56].copy_from_slice(&56u16.to_le_bytes());
            v[56..58].copy_from_slice(&1u16.to_le_bytes());
            v[64..68].copy_from_slice(&1u32.to_le_bytes());
            v[64 + 16..64 + 24].copy_from_slice(&vaddr.to_le_bytes());
            v[64 + 40..64 + 48].copy_from_slice(&0x1800u64.to_le_bytes());
            v
        }
        let good = elf_at(address_space_map::IMAGE_BASE);
        let mkfs_as_it_was = elf_at(0x20_0000);
        assert_eq!(
            image_span(&good),
            Some((
                address_space_map::IMAGE_BASE,
                address_space_map::IMAGE_BASE + 0x2000
            ))
        );
        assert!(programs_fit_the_address_space_map(
            "t",
            &[("good", &good), ("catalogue", b"text")]
        ));
        assert!(!programs_fit_the_address_space_map(
            "t",
            &[("mkfs", &mkfs_as_it_was)]
        ));
    }

    /// **`packaged_only` reads its one shape and refuses anything else in its table** (milestone
    /// 198 rung 3a), and a manifest without the table packs everything.
    #[test]
    fn packaged_only_reads_its_list_and_refuses_what_it_does_not_understand() {
        let manifest = "[[bin]]\nname = \"greeting\"\n\n[package.metadata.nife]\n# why\n\
                        packaged_only = [\"greeting\", \"other\"]\n\n[dependencies]\n\
                        packaged_only = \"not this table\"\n";
        assert_eq!(packaged_only(manifest).unwrap(), ["greeting", "other"]);
        assert!(packaged_only("[[bin]]\nname = \"x\"\n").unwrap().is_empty());
        let stray = "[package.metadata.nife]\nsomething = 1\n";
        assert!(
            packaged_only(stray)
                .unwrap_err()
                .contains("only `packaged_only")
        );
        let bare = "[package.metadata.nife]\npackaged_only = [greeting]\n";
        assert!(
            packaged_only(bare)
                .unwrap_err()
                .contains("not a quoted name")
        );
    }

    /// **The tree's own declaration reads, and agrees with everything that checks it** (milestone
    /// 150): the two `Cargo.toml`s parse, every program `grant_plan` lets the shell spawn has a
    /// binary, and so do the boot programs. This is what `initial_ramdisk_*` runs before packing,
    /// run here so `script/lint`'s host pass catches a disagreement without building an archive.
    #[test]
    fn the_declared_programs_agree_with_grant_plan_and_the_boot_list() {
        let names = declared_programs().unwrap_or_else(|e| panic!("{e}"));
        // A floor, not a pin: a reader that silently stopped at the first package would still
        // find `progenitor`, and would pack a third of the system.
        assert!(names.len() > 60, "only {} programs declared", names.len());
        for p in grant_plan::Program::ALL {
            assert!(
                names.iter().any(|n| n == p.name()) || is_std_built_elsewhere(*p),
                "{}",
                p.name()
            );
        }
    }

    /// **Every program something in the tree loads by name is one the tree builds** (milestone
    /// 150's gate on removal). Deleting a `[[bin]]` block takes the program out of all three
    /// archives at once; this stops that being silent while a test or the progenitor still asks
    /// for it, where it would `skip!()` forever or fail at boot.
    ///
    /// **A textual scan, rung two rather than rung one**: it reads `program("name")` and
    /// `.read("name")` literals out of `kernel/src`, `system_tests/src` and
    /// `crates/system_initializer/src`. A name built at runtime or spelled otherwise is invisible to
    /// it. The set it judges is counted, so a scan that stopped matching fails rather than passes.
    #[test]
    fn every_program_the_tree_loads_by_name_is_declared() {
        let declared = declared_programs().unwrap_or_else(|e| panic!("{e}"));
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, out);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    out.push(path);
                }
            }
        }
        let mut files = Vec::new();
        walk(&workspace_root().join("kernel/src"), &mut files);
        // The system tests look most programs up by name, and they left `kernel/src` for their own
        // image (milestone 609 (the system tests leave the kernel crate)).
        walk(&workspace_root().join("system_tests/src"), &mut files);
        walk(
            &workspace_root().join("crates/system_initializer/src"),
            &mut files,
        );
        let mut named = std::collections::BTreeSet::new();
        for file in &files {
            let text = std::fs::read_to_string(file).unwrap();
            for opener in ["program(\"", ".read(\""] {
                for (at, _) in text.match_indices(opener) {
                    let rest = &text[at + opener.len()..];
                    let Some(end) = rest.find("\")") else {
                        continue;
                    };
                    let name = &rest[..end];
                    if !name.is_empty()
                        && name
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                    {
                        named.insert((name.to_string(), file.clone()));
                    }
                }
            }
        }
        assert!(
            named.len() > 50,
            "the scan found only {} lookups; it has stopped matching the tree",
            named.len()
        );
        for (name, file) in &named {
            assert!(
                declared.iter().any(|d| d == name)
                    || BUILT_ELSEWHERE.contains(&name.as_str())
                    || C_PROGRAMS.contains(&name.as_str()),
                "{} looks up `{name}` by name, and no [[bin]] in components/ or fixtures/ builds it",
                file.display()
            );
        }
    }
}

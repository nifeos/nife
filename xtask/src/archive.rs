//! The userspace archives: the initrd each architecture's kernel is booted with.
//!
//! One packer per ISA, over the same program list, plus the check that every program the
//! tree loads by name is declared in a manifest the packer reads.

use crate::disk::{mkfs_elf, redoxfs_server_elf};
use crate::farm::{
    C_PROGRAMS, c_program_elf, cryptography_exerciser_elf, package_fetch_exerciser_elf,
    pinned_tls_exerciser_elf, ripgrep_elf, std_exerciser_elf, std_resolve_elf,
};
use crate::host::{bin_elf, workspace_root};
use crate::inspect::read_stripped;
use crate::measure::{boot_programs, measurement_table, write_measure_manifest};
use crate::{RISCV_TARGET, TARGET, X86_TARGET, profile_dir};

/// Where the packed initrd archive is written.
pub(crate) fn initrd_path() -> String {
    workspace_root()
        .join("target/initrd.img")
        .display()
        .to_string()
}

/// Where the RISC-V initrd archive is written (milestone 20). Separate from the aarch64 one because
/// it holds riscv64 ELFs, not aarch64 ones.
pub(crate) fn riscv_initrd_path() -> String {
    workspace_root()
        .join("target/initrd-riscv.img")
        .display()
        .to_string()
}

/// The packages a program can live in (milestone 175's split). See notes/adding-a-program.md for
/// which one a new program belongs in.
const PROGRAM_PACKAGES: [&str; 2] = ["components", "fixtures"];

/// **Every program this tree builds, read from the one place it is declared** (milestone 150; name
/// provisional): the `[[bin]]` blocks in `components/Cargo.toml` and `fixtures/Cargo.toml`, which
/// cargo needs anyway. All three archives pack exactly this list, so a program is added to them by
/// adding its `[[bin]]` block and removed by deleting it.
///
/// **This replaced two hand-maintained tables**, `initrd_aarch64()`'s own and the
/// `portable_archive_entries()` riscv64 and `x86_64` shared. By 2026-09-19 they disagreed about two
/// programs nobody had decided to leave out (`serial_driver` and `jh7110_entropy` were missing from
/// aarch64's) and both had missed a third (`pmap`, built and packed nowhere). That was drift rather
/// than policy, because the rule the shared table's own comment stated is the one this implements:
///
/// **Not filtered per architecture, deliberately.** Several programs cannot do their job everywhere
/// (`console`, `input` and `keyboard_driver` need port I/O a ring-3 process cannot reach on
/// `x86_64`, DECISIONS §121; `serial_driver` drives riscv64's UART; `jh7110_entropy` is radon's).
/// They are packed anyway: an archive entry costs a directory slot and some bytes, nothing spawns a
/// program by accident, and the tests that would spawn them `skip!()` with the reason. A
/// per-architecture filter would put the same fact in two places and let them disagree, which is
/// what the two tables did.
///
/// **Order does not matter.** The progenitor looks entries up by name and [`measurement_table`] is
/// sorted, so this is `Cargo.toml` order and nothing depends on it.
///
/// Refuses, rather than packing a partial archive, when a `[[bin]]` block has a shape
/// [`bin_names`] does not understand, and when something else in the tree names a program no
/// `[[bin]]` builds: see [`check_declared_programs`].
///
/// **Less the programs a package lists as `packaged_only`** (milestone 198 (a package manager)
/// rung 3a): built like every other, packed by no archive, so they reach a machine only as a
/// package. See [`packaged_only`].
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
            if grant_plan::Prog::ALL.iter().any(|p| p.name() == name)
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
/// manager) rung 3a). Empty when there is no such table. Strict for [`bin_names`]' reason: a
/// line in that table this does not understand is an error, because skipping it would pack a
/// program that was meant to arrive only as a package.
///
/// One line, `packaged_only = ["a", "b"]`, is the whole of the shape it reads.
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
/// own build ran (see the `initrd_*` functions). Hoisted out of the one test that used to hold it
/// (milestone 595 (provisional)), because [`check_declared_programs`] now needs the same list.
const BUILT_ELSEWHERE: [&str; 8] = [
    "redoxfs_server",
    "mkfs",
    "std_exerciser",
    "rg",
    "cryptography_exerciser",
    "pinned_tls_exerciser",
    "std_resolve",
    "package_fetch_exerciser",
];

/// **A `std` program the shell can spawn, built by its own workspace rather than a `[[bin]]`**
/// (milestone 595 (provisional)). `grant_plan::Prog::StdExerciser` is the first: it is compiled with
/// `-Zbuild-std` against the `nife-dev` toolchain (`cargo xtask std-exerciser`) and packed iff that
/// ran, so the `[[bin]]` rule cannot apply to it. Both halves are required, so a native `Prog`
/// with no binary still fails, and so does a `std` one nothing here packs.
fn is_std_built_elsewhere(p: grant_plan::Prog) -> bool {
    p.manifest().runtime == grant_plan::Runtime::Std && BUILT_ELSEWHERE.contains(&p.name())
}

/// **What the declared list must agree with, checked every time an archive is packed.**
///
/// - It is not empty and it holds `progenitor`, so a scanner that stopped finding blocks fails
///   here rather than packing an archive that boots nothing (a gate that computes the set it
///   judges goes blind when the set empties; see
///   design/roadmap/proposals/a-gate-that-selects-the-set-it-judges.md).
/// - No name is declared twice, across both packages.
/// - Every program `grant_plan` lets the shell spawn is built. Before milestone 150 a `Prog` whose
///   binary was not packed compiled, passed every host test, and could not be spawned, and nothing
///   said so until a boot.
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
    for p in grant_plan::Prog::ALL {
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

/// **Build the RISC-V userspace archive** (milestone 20, the richer-initrd step). Compiles the
/// portable programs the second architecture runs and packs them into a nifefs archive. The kernel
/// enters `progenitor` and nothing else: the tour used to enter `builder` as well, to load
/// `least_authority_demo` by name from userspace, and milestone 295 retired it. Every entry is packed
/// under its own name since milestone 266. Point `NIFE_INITRD` at the result and boot the riscv
/// kernel, e.g.:
///
/// ```text
/// cargo xtask initrd-riscv
/// NIFE_INITRD=target/initrd-riscv.img cargo run -p kernel --target riscv64imac-unknown-none-elf
/// ```
pub(crate) fn initrd_riscv() -> bool {
    // **Builds the whole package rather than naming binaries** (fixed 2026-08-27; see
    // [`initrd_x86`]'s doc comment, which used to describe this as the one structural
    // difference between the two). The `--bin` list this used to carry predated every program
    // in `user/` compiling for this target, and had to be kept in step with the packing table
    // (itself generated since milestone 150, see [`declared_programs`]) by hand; it fell out of step twice in one night when
    // `audit_sink` (milestone 49) landed in `Cargo.toml` and the packaging table but not here,
    // and CI caught it both times with "cannot read .../audit_sink: No such file or directory".
    // Verified 2026-08-27: `cargo build -p user --target riscv64imac-unknown-none-elf`, unfiltered,
    // (`user` being the package milestone 175 split into `components` and `fixtures`),
    // compiles clean on current `main` (every program is already riscv64-portable), so the list
    // bought nothing but a place to forget an entry. Now a missing binary is structurally
    // impossible instead of a gate someone has to remember to update.
    //
    // Profiled since `swish-check --release` (milestone progenitor-stack (provisional)), which
    // packed debug programs under a release kernel until this read `profile_dir()`.
    if !crate::build_programs(RISCV_TARGET) {
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
    let Some(mut blobs) = declared_program_blobs("initrd-riscv", bin) else {
        return false;
    };
    // The std demo (milestone 27), built through the nife-dev toolchain for the riscv custom
    // target, rides along when present, exactly as on aarch64. `test` builds it first.
    if let Ok(bytes) = read_stripped(
        &std_exerciser_elf("riscv64-unknown-nife")
            .display()
            .to_string(),
    ) {
        blobs.push(("std_exerciser", bytes));
    }
    // `std_resolve` (milestone 801 (packages over the internet)), built by the same step.
    if let Ok(bytes) = read_stripped(
        &std_resolve_elf("riscv64-unknown-nife")
            .display()
            .to_string(),
    ) {
        blobs.push(("std_resolve", bytes));
    }
    // **Unmodified `ripgrep`** (milestone 121), on the same terms as aarch64's: present iff
    // `helpers/build-ripgrep.sh` has been run, absent from every ordinary build and from CI.
    // DECISIONS §19 is why this leg exists at all: the same experiment on both ISAs, or a scope
    // note says which one it skipped and why.
    if let Ok(bytes) = read_stripped(&ripgrep_elf("riscv64-unknown-nife").display().to_string()) {
        blobs.push(("rg", bytes));
    }
    // **The unmodified C programs** (milestone 835), on `rg`'s terms: each present iff its
    // `helpers/build-<name>.sh` has been run.
    for name in C_PROGRAMS {
        let path = c_program_elf(name, "riscv64-unknown-nife")
            .display()
            .to_string();
        if let Ok(bytes) = read_stripped(&path) {
            blobs.push((name, bytes));
        }
    }
    // **The crypto-provider workload** of milestone 442 (a crypto provider `rustls` can use on all three bare-metal targets), on the same terms and for the same
    // reason: present iff `helpers/build-cryptography-exerciser.sh` has been run.
    if let Ok(bytes) = read_stripped(
        &cryptography_exerciser_elf("riscv64-unknown-nife")
            .display()
            .to_string(),
    ) {
        blobs.push(("cryptography_exerciser", bytes));
    }
    // **The pinned TLS client's workload** of milestone 501 (a TLS client that speaks to one pinned
    // peer), on the same terms: present iff `helpers/build-pinned-tls-exerciser.sh` has been run.
    if let Ok(bytes) = read_stripped(
        &pinned_tls_exerciser_elf("riscv64-unknown-nife")
            .display()
            .to_string(),
    ) {
        blobs.push(("pinned_tls_exerciser", bytes));
    }
    // The package index's client (milestone 801), built by the same helper, on the same terms.
    if let Ok(bytes) = read_stripped(
        &package_fetch_exerciser_elf("riscv64-unknown-nife")
            .display()
            .to_string(),
    ) {
        blobs.push(("package_fetch_exerciser", bytes));
    }
    // The FS server (milestone 32 phase 2), built for the riscv bare target, rides along when
    // present, exactly as std_exerciser does; `test` builds it first.
    if let Ok(bytes) = read_stripped(&redoxfs_server_elf(RISCV_TARGET)) {
        blobs.push(("redoxfs_server", bytes));
    }
    // And `mkfs` (milestone 57's write half), on the same terms.
    if let Ok(bytes) = read_stripped(&mkfs_elf(RISCV_TARGET)) {
        blobs.push(("mkfs", bytes));
    }
    let mut files: Vec<(&str, &[u8])> = blobs.iter().map(|(n, b)| (*n, b.as_slice())).collect();
    // The image's package source, on the same terms as aarch64's (see there).
    let catalogue = match crate::package::image_catalogue("riscv64") {
        Ok(catalogue) => catalogue,
        Err(complaint) => {
            eprintln!("initrd-riscv: the image's package source: {complaint}");
            return false;
        }
    };
    files.push((package_archive::CATALOG, catalogue.as_bytes()));
    // The measurement table (milestone 104), on the same terms as aarch64's: last, so it measures
    // everything above it, and vouched for by the kernel's trust root so the progenitor's refusals mean
    // something. Parity is the point (§19): the same table, the same parser, the same policy.
    if !programs_fit_the_address_space_map("initrd-riscv", &files) {
        return false;
    }
    let table = measurement_table(&files);
    files.push((measured_boot::PROGRAM_MEASUREMENTS, table.as_bytes()));
    let size = nifefs::image_size(&files);
    let mut img = std::vec![0u8; size];
    // Carry the reason. "could not build the archive" with the error thrown away sent me hunting
    // through MAX_FILES, image_size and write_image's bounds check by hand; the error names which.
    if let Err(e) = nifefs::write_image(&files, &mut img) {
        eprintln!(
            "initrd-riscv: could not build the archive: {e:?} ({} files, {} bytes)",
            files.len(),
            size
        );
        return false;
    }
    if let Err(e) = std::fs::write(riscv_initrd_path(), &img) {
        eprintln!("initrd-riscv: could not write {}: {e}", riscv_initrd_path());
        return false;
    }
    // Measure the boot programs before the riscv kernel is built (milestone 22 phase B.1).
    if !write_measure_manifest("riscv64", &img) {
        return false;
    }
    eprintln!(
        "wrote {} ({size} bytes): progenitor, least_authority_demo",
        riscv_initrd_path()
    );
    true
}

/// Where the `x86_64` initrd archive is written (milestone 161). Separate from the other two for the
/// reason they are separate from each other: it holds `x86_64` ELFs, and the kernel's loader refuses
/// anything whose `e_machine` is not its own (`crates/elf`'s `EXPECTED_MACHINE`, which was itself
/// wrong for this architecture until item 4 found it).
pub(crate) fn x86_initrd_path() -> String {
    workspace_root()
        .join("target/initrd-x86_64.img")
        .display()
        .to_string()
}

/// **Build the `x86_64` userspace archive** (milestone 161, item 4's hand-off). The third archive,
/// packing the same programs the other two do ([`declared_programs`]), built for
/// `x86_64-unknown-none`.
///
/// **It builds the whole package rather than naming binaries.** This used to be the one structural
/// difference from [`initrd_riscv`], whose `--bin` list predated every program in `user/` compiling
/// for its target and had to be kept in step with the table by hand; that list is gone as of
/// 2026-08-27 and `initrd_riscv` now builds unfiltered too, the same way this function always has.
/// Since milestone 150 there is no packing table either: a `[[bin]]` block is packed here and by
/// both siblings with no second edit.
///
/// ```text
/// cargo xtask initrd-x86
/// NIFE_INITRD=target/initrd-x86_64.img cargo run -p kernel --target x86_64-unknown-none
/// ```
///
/// # BUGS
///
/// Two things both other archives carry were absent when this was written, and the second was a
/// real toolchain failure rather than work not yet done. (A third, `std_exerciser`, was packed at
/// milestone 184, when `x86_64-unknown-nife` and its farm landed.)
///
/// **No disk fixture is generated**, so even a packed `fs_server` would have nothing to open. The
/// runner attaches no drive; attaching one is a smaller piece of work here than it looks (q35's
/// virtio is PCI, and the PCIe transport of DECISIONS §18 is already built and is x86's native bus)
/// and is not this milestone's.
///
/// **`fs_server` and `mkfs` do not compile for `x86_64-unknown-none` at all**, and this one is
/// worth writing down because it will surprise whoever tries next. The vendored RedoxFS engine
/// pulls in the `aes` crate for its encrypted-volume support, and building `aes` for this target
/// ends in `rustc-LLVM ERROR: Do not know how to split the result of this operator!`, at **every**
/// optimisation level including zero. The cause is the target spec rather than the crate: this
/// target is `-mmx,-sse,+soft-float`, so LLVM has no 128-bit vector register to legalise `aes`'s
/// block operations into and no scalar fallback for that operator. It is not a nife bug and there
/// is no flag on this side that fixes it; the routes out are a RedoxFS built without its crypto
/// feature, or an x86 target spec that keeps SSE for userspace. Both are their own work.
/// See notes/x86-port/userspace.md.
///
/// **Naming, updated 2026-08-27**: this function's own name predates a naming scheme; the mismatch
/// it used to flag against its two siblings (`mkinitrd` for aarch64, `initrd_riscv` for RISC-V,
/// `initrd_x86` here) is resolved on calef's behalf as follows, and remains **provisional** because
/// naming is an architect's call, not a lane's (per this repo's naming convention; function names
/// get more latitude than crate names but still ship provisional). aarch64's `mkinitrd` is renamed
/// to `initrd_aarch64` and given its own `initrd-aarch64` subcommand, matching the `initrd_<arch>`
/// / `initrd-<arch>` shape `initrd_riscv`/`initrd-riscv` and this function/`initrd-x86` already
/// had; this function and `initrd_riscv` are left as they were; see the PR that made this change
/// for the reasoning (chiefly: extending the pattern two of three already used costs one new
/// subcommand and one rename, where making all three agree on fully-spelled ISA names, e.g.
/// `initrd_riscv64` / `initrd_x86_64`, would also rename two already-typed, already-documented
/// subcommand names for a smaller win). Confirm or redirect.
pub(crate) fn initrd_x86() -> bool {
    if !crate::build_programs(X86_TARGET) {
        return false;
    }

    let bin = |name: &str| {
        workspace_root()
            .join(format!("target/{X86_TARGET}/{}/{name}", profile_dir()))
            .display()
            .to_string()
    };
    let Some(mut blobs) = declared_program_blobs("initrd-x86", bin) else {
        return false;
    };
    // The FS server and `mkfs` (milestone 164), on exactly the terms `initrd_riscv` carries them:
    // present iff something built them for this target, absent from a bare `initrd-x86`, and
    // `test` builds them first. Until milestone 164 they could not be built for this target at
    // all, because the vendored RedoxFS engine's `aes` dependency would not codegen without SSE;
    // `.cargo/config.toml`'s `--cfg aes_force_soft` on this target is what changed that.
    if let Ok(bytes) = read_stripped(&redoxfs_server_elf(X86_TARGET)) {
        blobs.push(("redoxfs_server", bytes));
    }
    if let Ok(bytes) = read_stripped(&mkfs_elf(X86_TARGET)) {
        blobs.push(("mkfs", bytes));
    }
    // The std demo (milestone 184), on the terms both other archives carry it: present iff
    // `cargo xtask std-exerciser` built it for `x86_64-unknown-nife`, which `test` does first.
    if let Ok(bytes) = read_stripped(
        &std_exerciser_elf("x86_64-unknown-nife")
            .display()
            .to_string(),
    ) {
        blobs.push(("std_exerciser", bytes));
    }
    // `std_resolve` (milestone 801 (packages over the internet)), built by the same step.
    if let Ok(bytes) = read_stripped(&std_resolve_elf("x86_64-unknown-nife").display().to_string())
    {
        blobs.push(("std_resolve", bytes));
    }
    // **Unmodified `ripgrep`** (milestones 121 and 184), present iff `helpers/build-ripgrep.sh` ran.
    if let Ok(bytes) = read_stripped(&ripgrep_elf("x86_64-unknown-nife").display().to_string()) {
        blobs.push(("rg", bytes));
    }
    // **The unmodified C programs** (milestone 835), each present iff its helper ran.
    for name in C_PROGRAMS {
        let path = c_program_elf(name, "x86_64-unknown-nife")
            .display()
            .to_string();
        if let Ok(bytes) = read_stripped(&path) {
            blobs.push((name, bytes));
        }
    }
    // **The crypto-provider workload** (milestone 442), on the same terms and for the same
    // reason: present iff `helpers/build-cryptography-exerciser.sh` has been run.
    if let Ok(bytes) = read_stripped(
        &cryptography_exerciser_elf("x86_64-unknown-nife")
            .display()
            .to_string(),
    ) {
        blobs.push(("cryptography_exerciser", bytes));
    }
    // **The pinned TLS client's workload** of milestone 501 (a TLS client that speaks to one pinned
    // peer), on the same terms: present iff `helpers/build-pinned-tls-exerciser.sh` has been run.
    if let Ok(bytes) = read_stripped(
        &pinned_tls_exerciser_elf("x86_64-unknown-nife")
            .display()
            .to_string(),
    ) {
        blobs.push(("pinned_tls_exerciser", bytes));
    }
    // The package index's client (milestone 801), built by the same helper, on the same terms.
    if let Ok(bytes) = read_stripped(
        &package_fetch_exerciser_elf("x86_64-unknown-nife")
            .display()
            .to_string(),
    ) {
        blobs.push(("package_fetch_exerciser", bytes));
    }
    let mut files: Vec<(&str, &[u8])> = blobs.iter().map(|(n, b)| (*n, b.as_slice())).collect();
    // The image's package source, on the same terms as aarch64's (see there). Since milestone 198
    // rung 3a's installer the progenitor reads it for `package install`, so x86_64 carries one too
    // (§19 (architectural parity is a tenet)); no fetch test reads it here, because this runner attaches no network.
    let catalogue = match crate::package::image_catalogue("x86_64") {
        Ok(catalogue) => catalogue,
        Err(complaint) => {
            eprintln!("initrd-x86: the image's package source: {complaint}");
            return false;
        }
    };
    files.push((package_archive::CATALOG, catalogue.as_bytes()));
    // The measurement table (milestone 104), on the same terms as the other two: last, so it
    // measures everything above it, and vouched for by the kernel's trust root so the progenitor's refusals
    // mean something. Parity is the point (§19): the same table, the same parser, the same policy.
    if !programs_fit_the_address_space_map("initrd-x86", &files) {
        return false;
    }
    let table = measurement_table(&files);
    files.push((measured_boot::PROGRAM_MEASUREMENTS, table.as_bytes()));
    let size = nifefs::image_size(&files);
    let mut img = std::vec![0u8; size];
    if let Err(e) = nifefs::write_image(&files, &mut img) {
        eprintln!(
            "initrd-x86: could not build the archive: {e:?} ({} files, {} bytes)",
            files.len(),
            size
        );
        return false;
    }
    if let Err(e) = std::fs::write(x86_initrd_path(), &img) {
        eprintln!("initrd-x86: could not write {}: {e}", x86_initrd_path());
        return false;
    }
    // **Measure the boot programs before the x86 kernel is built** (milestone 22 phase B.1), and on
    // this architecture that is not a nicety: with no manifest the generated `TRUST_ROOT` is empty
    // and `trust::require` refuses every boot program as `Unmeasured`, so the kernel would come up
    // and refuse to start the progenitor with an error about measurement rather than about the
    // archive.
    if !write_measure_manifest("x86_64", &img) {
        return false;
    }
    eprintln!(
        "wrote {} ({size} bytes): progenitor plus {} entries",
        x86_initrd_path(),
        files.len()
    );
    true
}

/// **Build the aarch64 userspace archive.** Pack the built user ELFs into the initrd archive the
/// kernel hands the progenitor (milestone 19f).
///
/// The initrd is a **nifefs image**, the same format the virtio disk uses, so one parser serves
/// both the RAM archive and the disk. It holds `progenitor` (the first process, milestone 266) and
/// `hello` (the init roles the kernel re-enters for milestone 19d's and 19e's tests), plus the
/// binaries lifted out of hello over 19f.2, 19f.3 and milestone 291. The kernel reads the
/// `progenitor` entry to boot; the progenitor loads the rest by name. Generated, not checked in, exactly like the disk and the flat kernel image: a blob
/// in git is a blob nobody can review.
///
/// **Renamed from `mkinitrd` (2026-08-27), and given aarch64 its own `initrd-aarch64`
/// subcommand**, to match its two siblings ([`initrd_riscv`], [`initrd_x86`]): one job, one
/// `initrd_<arch>` naming scheme, three matching `cargo xtask initrd-<arch>` subcommands. This
/// function only packs, unlike its two siblings, which both build-then-pack in one call; `main`'s
/// `"initrd-aarch64"` arm calls [`crate::user`] (build, then pack) rather than this function alone, so
/// the subcommand is self-contained the same way `initrd-riscv`/`initrd-x86` are. It is still
/// called internally by `user()` (and so by `build`, `run`, `shell`, and everything else that
/// boots the aarch64 kernel) exactly as `mkinitrd` was; the new subcommand is additive, so nothing
/// that already called this function changed. **Name and subcommand provisional**, per this
/// repo's naming convention: an architect's call to confirm or redirect.
pub(crate) fn initrd_aarch64() -> bool {
    // **No table** since milestone 150: every `[[bin]]` in `components/` and `fixtures/`, the
    // same list the other two archives pack ([`declared_programs`]). This function carried its own
    // hand-written table before that, and an older three-way copy of it before milestone 130; the
    // table had drifted from riscv64's and `x86_64`'s by two programs when it was deleted.
    let Some(blobs) = declared_program_blobs("initrd-aarch64", bin_elf) else {
        return false;
    };
    let mut files: Vec<(&str, &[u8])> = blobs.iter().map(|(n, b)| (*n, b.as_slice())).collect();
    // The std demo (milestone 27) rides along IFF it has been built (`cargo xtask std-exerciser`, which
    // `test` runs). It builds through a separate toolchain and target, so an interactive `run` that
    // never built it simply ships an initrd without it; nothing loads it there.
    let std_exerciser = read_stripped(
        &std_exerciser_elf("aarch64-unknown-nife")
            .display()
            .to_string(),
    )
    .ok();
    if let Some(bytes) = &std_exerciser {
        files.push(("std_exerciser", bytes.as_slice()));
    }
    // `std_resolve` (milestone 801 (packages over the internet)), built by the same step.
    let std_resolve = read_stripped(
        &std_resolve_elf("aarch64-unknown-nife")
            .display()
            .to_string(),
    )
    .ok();
    if let Some(bytes) = &std_resolve {
        files.push(("std_resolve", bytes.as_slice()));
    }
    // The FS server (milestone 32 phase 2) rides along IFF built (its own workspace/target; `test`
    // builds it). Absent for a plain interactive boot, which simply skips the FS-server test.
    let redoxfs_server = read_stripped(&redoxfs_server_elf(TARGET)).ok();
    if let Some(bytes) = &redoxfs_server {
        files.push(("redoxfs_server", bytes.as_slice()));
    }
    // `mkfs` (milestone 57's write half) rides along on the same terms: the same package, the
    // same build, and absent from an interactive boot that never built it.
    let mkfs = read_stripped(&mkfs_elf(TARGET)).ok();
    if let Some(bytes) = &mkfs {
        files.push(("mkfs", bytes.as_slice()));
    }
    // **Unmodified `ripgrep`** (milestone 121), on exactly the terms above: present iff
    // `helpers/build-ripgrep.sh` has been run, absent from every ordinary build and from CI. The
    // archive name is `rg`, which is what the program is called everywhere else in the world.
    let ripgrep = read_stripped(&ripgrep_elf("aarch64-unknown-nife").display().to_string()).ok();
    if let Some(bytes) = &ripgrep {
        files.push(("rg", bytes.as_slice()));
    }
    // **The unmodified C programs** (milestone 835 (a C library, stage 1: files, clock and
    // memory)), on exactly `rg`'s terms: each present iff its `helpers/build-<name>.sh` has run.
    let c_programs: Vec<(&str, Vec<u8>)> = C_PROGRAMS
        .iter()
        .filter_map(|&name| {
            let path = c_program_elf(name, "aarch64-unknown-nife")
                .display()
                .to_string();
            read_stripped(&path).ok().map(|bytes| (name, bytes))
        })
        .collect();
    for (name, bytes) in &c_programs {
        files.push((name, bytes.as_slice()));
    }
    // **The crypto-provider workload** (milestone 442), on exactly those terms: present iff
    // `helpers/build-cryptography-exerciser.sh` has been run, absent from every ordinary build and
    // from CI, because the crates under it are a dependency decision calef has not made.
    let cryptography = read_stripped(
        &cryptography_exerciser_elf("aarch64-unknown-nife")
            .display()
            .to_string(),
    )
    .ok();
    if let Some(bytes) = &cryptography {
        files.push(("cryptography_exerciser", bytes.as_slice()));
    }
    // **The pinned TLS client's workload** (milestone 501 (a TLS client that speaks to one pinned
    // peer)), on exactly those terms: present iff `helpers/build-pinned-tls-exerciser.sh` has run.
    let pinned_tls = read_stripped(
        &pinned_tls_exerciser_elf("aarch64-unknown-nife")
            .display()
            .to_string(),
    )
    .ok();
    if let Some(bytes) = &pinned_tls {
        files.push(("pinned_tls_exerciser", bytes.as_slice()));
    }
    // The package index's client (milestone 801), built by the same helper, on the same terms.
    let package_fetch = read_stripped(
        &package_fetch_exerciser_elf("aarch64-unknown-nife")
            .display()
            .to_string(),
    )
    .ok();
    if let Some(bytes) = &package_fetch {
        files.push(("package_fetch_exerciser", bytes.as_slice()));
    }
    // **The image's package source** (milestone 198 (a package manager) rung 3a): every recipe under `packages/` for
    // this architecture is built, written where the package tests' HTTP peer serves it, and its
    // catalogue line packed here, *above* the measurement table so the kernel's trust root vouches
    // for it. `package_archive::CATALOG`'s doc has why. It also runs the producer end to end on
    // every archive build, which nothing did before.
    let catalogue = match crate::package::image_catalogue("aarch64") {
        Ok(catalogue) => catalogue,
        Err(complaint) => {
            eprintln!("initrd-aarch64: the image's package source: {complaint}");
            return false;
        }
    };
    files.push((package_archive::CATALOG, catalogue.as_bytes()));
    // **The measurement table, last, so it measures everything above it** (milestone 104). The progenitor
    // reads this entry out of the archive it already holds and refuses to load a program whose
    // bytes it does not match. See [`measurement_table`] for why it lives here rather than inside
    // the progenitor's own image.
    if !programs_fit_the_address_space_map("initrd-aarch64", &files) {
        return false;
    }
    let table = measurement_table(&files);
    files.push((measured_boot::PROGRAM_MEASUREMENTS, table.as_bytes()));

    let size = nifefs::image_size(&files);
    let mut img = std::vec![0u8; size];
    // Carry the reason, as `initrd_riscv` already does. "could not build the initrd archive" with
    // the error thrown away is what milestone 291 hit on the commit that crossed `MAX_FILES`, and
    // it cost a hunt through three candidate bounds by hand; the error names which one.
    if let Err(e) = nifefs::write_image(&files, &mut img) {
        eprintln!(
            "initrd-aarch64: could not build the initrd archive: {e:?} ({} files, {} bytes)",
            files.len(),
            size
        );
        return false;
    }
    if let Err(e) = std::fs::write(initrd_path(), &img) {
        eprintln!("initrd-aarch64: could not write {}: {e}", initrd_path());
        return false;
    }
    // Measure the boot program before the kernel is built (milestone 22 phase B.1). Every caller
    // reaches the kernel build through `user()`, which calls this, so the manifest is always current
    // by the time `kernel/build.rs` reads it.
    write_measure_manifest("aarch64", &img)
}

/// **Every program in an archive fits the address-space map's image band**, checked when the
/// archive is packed rather than when some test first loads the program (milestone 206 (a program
/// image has under 896 KiB)). The kernel's loader refuses a misplaced image by name, but only the
/// programs a boot actually loads ever reach it; this reaches all of them. It exists because the
/// first run after the map landed found `mkfs` linked with lld's default layout at `0x20_0000`,
/// outside the shared linker script, where nothing had looked for eight weeks.
///
/// The program headers are read here by hand rather than with `crates/elf`, because that parser
/// refuses a foreign `e_machine` and this packs riscv64 and `x86_64` archives on an aarch64 host.
/// Entries that are not ELF (the catalogue, the measurement table) are skipped.
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
    /// binary, and so do the boot programs. This is what `initrd_*` runs before packing, run here
    /// so `script/lint`'s host pass catches a disagreement without building an archive.
    #[test]
    fn the_declared_programs_agree_with_grant_plan_and_the_boot_list() {
        let names = declared_programs().unwrap_or_else(|e| panic!("{e}"));
        // A floor, not a pin: a reader that silently stopped at the first package would still
        // find `progenitor`, and would pack a third of the system.
        assert!(names.len() > 60, "only {} programs declared", names.len());
        for p in grant_plan::Prog::ALL {
            assert!(
                names.iter().any(|n| n == p.name()) || is_std_built_elsewhere(*p),
                "{}",
                p.name()
            );
        }
    }

    /// **Every program something in the tree loads by name is one the tree builds** (milestone
    /// 150's gate on removal). Deleting a `[[bin]]` block takes the program out of all three
    /// archives at once, which is the point; this is what stops that being silent when a kernel
    /// test or the progenitor still asks for it by name. Without it, the test would `skip!()` with
    /// "no such program in this archive" forever, or the progenitor would fail at boot.
    ///
    /// **A textual scan, and rung two rather than rung one**: it reads `program("name")` and
    /// `.read("name")` string literals out of `kernel/src` and `crates/system_initializer/src`, the
    /// two places that look programs up in an archive. A name built at runtime, or looked up by some
    /// other spelling, is invisible to it. The set it judges is counted, so a scan that stopped
    /// matching fails here rather than passing on nothing.
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
                declared.iter().any(|d| d == name) || BUILT_ELSEWHERE.contains(&name.as_str()),
                "{} looks up `{name}` by name, and no [[bin]] in components/ or fixtures/ builds it",
                file.display()
            );
        }
    }
}

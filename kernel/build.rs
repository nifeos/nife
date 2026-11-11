//! Tell the linker to use our layout instead of the default one.
//!
//! `cargo:rustc-link-arg` applies to binaries *and* test binaries, which is what we
//! want: the test build has to boot in QEMU too, so it needs the same layout.

/// Select the architecture's linker script, carry the build-time knobs in, and generate the trust
/// root.
///
/// `NIFE_TEST_FILTER`, `NIFE_TEST_FILTER_ACROSS_IMAGES` and `NIFE_TEST_REPEAT` reach the kernel as
/// `rustc-env`, compile-time because a boot argument would need three boot protocols; see
/// notes/scripts/one-kernel-test.md. `NIFE_FASTPATH_PAD` and `NIFE_FASTPATH_SHIFT` are read only
/// under `fastpath_pad`, so a stray variable cannot reach or rebuild an ordinary build, and
/// anything but a plain decimal is refused. They are variables rather than features because a
/// feature's name moves unrelated code; see notes/footprint-perturbation/layout-control.md.
fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    // Each architecture has its own memory map, and so its own script. See notes/riscv-port.md.
    let (link_script, boot_asm) = match arch.as_str() {
        "aarch64" => ("link-aarch64.ld", "src/arch/aarch64/boot.s"),
        "riscv64" => ("link-riscv64.ld", "src/arch/riscv64/boot.s"),
        // A low 32-bit `.boot` and a high 64-bit rest; link-x86_64.ld's header says why.
        "x86_64" => ("link-x86_64.ld", "src/arch/x86_64/boot.s"),
        other => panic!("nife has no linker script for target arch {other}"),
    };

    // Without `rerun-if-env-changed` a new filter would silently reuse the last binary.
    // Only the coverage build reads this; it names the host file the counters land in.
    println!("cargo::rerun-if-env-changed=NIFE_COVERAGE_PROFILE");
    println!(
        "cargo::rustc-env=NIFE_COVERAGE_PROFILE={}",
        std::env::var("NIFE_COVERAGE_PROFILE").unwrap_or_default()
    );

    println!("cargo::rerun-if-env-changed=NIFE_TEST_FILTER");
    println!(
        "cargo::rustc-env=NIFE_TEST_FILTER={}",
        std::env::var("NIFE_TEST_FILTER").unwrap_or_default()
    );
    println!("cargo::rerun-if-env-changed=NIFE_TEST_FILTER_ACROSS_IMAGES");
    println!(
        "cargo::rustc-env=NIFE_TEST_FILTER_ACROSS_IMAGES={}",
        std::env::var("NIFE_TEST_FILTER_ACROSS_IMAGES").unwrap_or_default()
    );
    println!("cargo::rerun-if-env-changed=NIFE_TEST_REPEAT");
    println!(
        "cargo::rustc-env=NIFE_TEST_REPEAT={}",
        std::env::var("NIFE_TEST_REPEAT").unwrap_or_default()
    );

    if std::env::var_os("CARGO_FEATURE_FASTPATH_PAD").is_some() {
        for (var, default) in [("NIFE_FASTPATH_PAD", "1"), ("NIFE_FASTPATH_SHIFT", "0")] {
            println!("cargo::rerun-if-env-changed={var}");
            let value = std::env::var(var).unwrap_or_else(|_| default.to_string());
            if value.parse::<u32>().is_err() {
                panic!(
                    "{var}={value:?} is not a non-negative decimal; see kernel/src/fastpath_pad.rs"
                );
            }
            println!("cargo::rustc-env={var}={value}");
        }
    }

    println!("cargo::rerun-if-changed={link_script}");
    println!("cargo::rerun-if-changed={boot_asm}");
    println!("cargo::rustc-link-arg=-T{manifest_dir}/{link_script}");

    declare_initial_ramdisk_cfg(&arch);
    generate_trust_root(&manifest_dir, &arch);
    carry_jh7110_entropy_identity(&manifest_dir);
}

/// Carry the `jh7110_entropy` version and commit that `Cargo.lock` resolved into the kernel, as
/// `NIFE_JH7110_ENTROPY_VERSION` and `NIFE_JH7110_ENTROPY_COMMIT`.
///
/// The boot tour's `trng` line prints both, so a board transcript names the exact crate that ran
/// and nobody has to vouch for which build was on the board. The kernel and the archive's driver
/// program resolve the crate through the one workspace lockfile, and measured boot refuses a
/// kernel and an archive from different builds, so the pair names both. A package the lockfile
/// does not hold, or one from a source with no commit, reads `unknown`, which
/// `helpers/crate_hardware_run.py` refuses. See
/// design/roadmap/1035-a-crate-names-the-version-that-ran-on-hardware/bench-steps.md.
fn carry_jh7110_entropy_identity(manifest_dir: &str) {
    let lockfile = std::path::Path::new(manifest_dir)
        .parent()
        .expect("the kernel manifest dir has no parent")
        .join("Cargo.lock");
    println!("cargo::rerun-if-changed={}", lockfile.display());
    let text = std::fs::read_to_string(&lockfile).unwrap_or_default();
    let (version, commit) = locked_git_package(&text, "jh7110_entropy")
        .unwrap_or(("unknown".to_string(), "unknown".to_string()));
    println!("cargo::rustc-env=NIFE_JH7110_ENTROPY_VERSION={version}");
    println!("cargo::rustc-env=NIFE_JH7110_ENTROPY_COMMIT={commit}");
}

/// The version and the 40-digit commit of the git package `name` in a `Cargo.lock`'s text.
///
/// Returns `None` when no `[[package]]` carries the name, or when its source is not a git source
/// ending in `#<commit>`.
fn locked_git_package(lock: &str, name: &str) -> Option<(String, String)> {
    for block in lock.split("[[package]]") {
        let field = |key: &str| {
            block.lines().find_map(|line| {
                line.strip_prefix(key)?
                    .strip_prefix(" = \"")?
                    .strip_suffix('"')
            })
        };
        if field("name") != Some(name) {
            continue;
        }
        let version = field("version")?;
        let source = field("source")?;
        let commit = source.strip_prefix("git+")?.rsplit_once('#')?.1;
        if commit.len() != 40 || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        return Some((version.to_string(), commit.to_string()));
    }
    None
}

/// Declare `cfg(initial_ramdisk)`, set where the target has user programs to pack, and
/// `cfg(soak_watchdog)`, set where a soak arms the chipset's watchdog.
///
/// `initial_ramdisk` names the reason a test needs (a fixture read out of the archive) rather than
/// an architecture, so a new port gains those tests with one arm here. `soak_watchdog` is set for
/// `watchdog_soak_test`, and for every x86_64 and riscv64 `soak_test` unless it names
/// `unwatched_soak_test`; it is a cfg because cargo features cannot differ by target. See
/// notes/soak/supervisor.md. The name `soak_watchdog` is provisional.
///
/// Name: ratified 2026-10-12 (calef, #1998, milestone 139 (drive the unsafe count down) round 11),
/// with the `cfg` it declares. His words: "B for #1998." Refused `declare_initrd_cfg` and
/// `cfg(initrd)` (the foreign spelling, which stops at the boundary where it arrives).
fn declare_initial_ramdisk_cfg(arch: &str) {
    // Declared whatever the answer, so `unexpected_cfgs` still catches a typo'd cfg.
    println!("cargo::rustc-check-cfg=cfg(soak_watchdog)");
    let feature = |name: &str| std::env::var_os(format!("CARGO_FEATURE_{name}")).is_some();
    if feature("WATCHDOG_SOAK_TEST")
        || (feature("SOAK_TEST")
            && matches!(arch, "x86_64" | "riscv64")
            && !feature("UNWATCHED_SOAK_TEST"))
    {
        println!("cargo::rustc-cfg=soak_watchdog");
    }
    println!("cargo::rustc-check-cfg=cfg(initial_ramdisk)");
    match arch {
        "aarch64" | "riscv64" | "x86_64" => println!("cargo::rustc-cfg=initial_ramdisk"),
        // Anything a fourth port adds before it can build userspace.
        _ => {}
    }
}

/// Compile the boot program's measurement into the kernel image as `TRUST_ROOT`.
///
/// Reads `target/init-measure-<arch>.txt`, which xtask writes after packing the initial ramdisk,
/// and writes `$OUT_DIR/trust_root.rs`. A missing manifest yields an empty trust root, which the
/// kernel refuses at boot (`measured_boot::VerifyError::Unmeasured`), so a bare `cargo build` or
/// `script/lint` still works. A malformed line panics: at build time we can still stop, where the
/// progenitor, reading the same format, can only refuse. See notes/trusted-init.md.
fn generate_trust_root(manifest_dir: &str, arch: &str) {
    let manifest = std::path::Path::new(manifest_dir)
        .parent()
        .expect("the kernel manifest dir has no parent")
        .join(format!("target/init-measure-{arch}.txt"));

    println!("cargo::rerun-if-changed={}", manifest.display());

    let text = std::fs::read_to_string(&manifest).unwrap_or_default();
    let mut entries = String::new();
    for entry in measured_boot::manifest_entries(&text) {
        let (name, digest) = entry.unwrap_or_else(|bad| {
            panic!(
                "malformed measurement line in {}: {:?} ({:?}; a digest is written sha256:<hex>)",
                manifest.display(),
                bad.line,
                bad.why,
            )
        });
        let bytes: Vec<String> = digest.iter().map(|b| format!("{b:#04x}")).collect();
        entries.push_str(&format!(
            "    measured_boot::Measurement {{ name: {name:?}, digest: [{}] }},\n",
            bytes.join(", ")
        ));
    }

    let generated = format!(
        "// Generated by kernel/build.rs from {}. Do not edit.\n\
         pub static TRUST_ROOT: &[measured_boot::Measurement] = &[\n{entries}];\n",
        manifest.display()
    );
    let out = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("trust_root.rs");
    std::fs::write(&out, generated).expect("cannot write the generated trust root");
}

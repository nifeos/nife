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

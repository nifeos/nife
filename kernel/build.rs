//! Tell the linker to use our layout instead of the default one.
//!
//! `cargo:rustc-link-arg` applies to binaries *and* test binaries, which is what we
//! want: the test build has to boot in QEMU too, so it needs the same layout.

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    // Each architecture has its own memory map (aarch64 links high with an AT()-relocated load
    // address; riscv links at the OpenSBI payload address). The linker script is arch-specific, so
    // it is selected here rather than hard-coded. See notes/riscv-port.md.
    let (link_script, boot_asm) = match arch.as_str() {
        "aarch64" => ("link-aarch64.ld", "src/arch/aarch64/boot.s"),
        "riscv64" => ("link-riscv64.ld", "src/arch/riscv64/boot.s"),
        // x86_64 (milestone 161). Its script is the only one of the three with two worlds in it: a
        // low 32-bit `.boot` for the trampoline and a high 64-bit everything-else. See
        // link-x86_64.ld's header for why that is forced rather than chosen.
        "x86_64" => ("link-x86_64.ld", "src/arch/x86_64/boot.s"),
        other => panic!("nife has no linker script for target arch {other}"),
    };

    // **The kernel test filter** (milestone 210). `cargo xtask test --test <substring>` sets
    // `NIFE_TEST_FILTER`, and this is what carries it into the kernel: the value is baked into the
    // binary as a `rustc-env` and read by `testing::runner`.
    //
    // **Compile-time rather than a boot argument, and the reason is parity.** A runtime channel
    // means the boot protocol, and there are three of them: aarch64 and riscv64 take a device tree
    // (`/chosen/bootargs`, which this kernel does not parse), x86_64 arrives through PVH with no
    // device tree at all. One `env!` is the same mechanism on all three, needs no parsing, and
    // costs a kernel relink (measured at about 2.3 s) when the filter changes, against the 53 s of
    // suite it replaces. See design/roadmap/0210-run-one-kernel-test.md.
    //
    // `rerun-if-env-changed` is what makes changing the filter actually rebuild; without it a
    // second run with a different filter would silently reuse the first one's binary, which is the
    // manufactured-fact shape this tree already fails loudly on elsewhere.
    println!("cargo::rerun-if-env-changed=NIFE_TEST_FILTER");
    println!(
        "cargo::rustc-env=NIFE_TEST_FILTER={}",
        std::env::var("NIFE_TEST_FILTER").unwrap_or_default()
    );
    // **Whether the harness counts the filter across images** (milestone 609 (the system tests
    // leave the kernel crate)). A leg now boots two test images, and a filter naming a system test
    // selects nothing in the kernel's own. `cargo xtask test --test` sets this and then checks the
    // sum itself; left unset, as it is for a bare `cargo test`, an empty selection still fails.
    // Name provisional.
    println!("cargo::rerun-if-env-changed=NIFE_TEST_FILTER_ACROSS_IMAGES");
    println!(
        "cargo::rustc-env=NIFE_TEST_FILTER_ACROSS_IMAGES={}",
        std::env::var("NIFE_TEST_FILTER_ACROSS_IMAGES").unwrap_or_default()
    );
    // **How many times the filtered tests run in one boot** (milestone 321 (a corpse that did not
    // park, on four real cores)), the filter's mechanism for the filter's reason. `cargo xtask test
    // --repeat N` sets it; `testing::TEST_REPEAT` parses it at compile time. Name provisional.
    println!("cargo::rerun-if-env-changed=NIFE_TEST_REPEAT");
    println!(
        "cargo::rustc-env=NIFE_TEST_REPEAT={}",
        std::env::var("NIFE_TEST_REPEAT").unwrap_or_default()
    );

    // **E3's pad size and layout shift** (milestone 134's layout control), the same mechanism as
    // the test filter above and chosen for a measured reason: a Cargo feature per size was built
    // first, and a feature's name enters cargo's `-C metadata` hash, which reorders codegen units
    // and moved up to 11 KB of unrelated code ahead of the sled. An environment variable does not
    // enter that hash, so two images differ only in the bytes asked for. kernel/src/fastpath_pad.rs
    // has what the two numbers mean.
    //
    // **Read only under `fastpath_pad`**, so a stray variable in somebody's shell cannot reach an
    // ordinary build, and an ordinary build is not rebuilt when one changes. Anything that is not a
    // plain decimal is refused here, before four minutes of cargo, rather than defaulted.
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

/// **`cfg(initial_ramdisk)`: does this target have user programs to pack into one?** (milestone
/// 161, roadmap item 4.)
///
/// A great many `#[test_case]`s in `kernel/src/user/` are portable in every respect except that
/// their fixture is a **real ELF binary read out of the initial ramdisk archive**, so a target with
/// no archive cannot compile them into its test binary.
///
/// **The cfg names the reason rather than the architecture**, which is the whole point of spending a
/// build script on it. `#[cfg(all(test, initial_ramdisk))]` on a module reads "this needs an
/// initial ramdisk", and stays true; `#[cfg(not(target_arch = "x86_64"))]` would have said "not on
/// x86" thirty times over and would be wrong the day x86 gets user programs, in thirty places
/// nobody would think to look.
///
/// **That day came, and it was one arm** (milestone 161, item 4's hand-off, 2026-08-24). `user_mode_runtime`
/// grew its `x86_64` arms, `fixtures/build.rs` learned to compile the C components for the target,
/// and `xtask`'s `initial-ramdisk-x86` packs the archive; every one of those thirty modules came
/// back at once, with nothing else edited. That is the prediction the cfg was written to make and
/// it held, which is worth recording here rather than only in the roadmap: the alternative spelling
/// would have needed thirty edits and would have been found by whoever hit the thirty-first.
///
/// Name: ratified 2026-10-12 (calef, #1998, milestone 139 (drive the unsafe count down) round 11),
/// with the `cfg` it declares. His words: "B for #1998." Refused `declare_initrd_cfg` and
/// `cfg(initrd)` (the foreign spelling, which stops at the boundary where it arrives).
fn declare_initial_ramdisk_cfg(arch: &str) {
    // Declare it whatever the answer, so `unexpected_cfgs` stays a useful lint rather than being
    // silenced: a typo'd `#[cfg(initd)]` should still be caught.
    // **Whether this soak arms the chipset's watchdog** (milestone 593 (a wedged kernel resets
    // itself), provisional number). Every x86_64 and riscv64 `soak_test` build does, on calef's
    // ruling of 2026-09-26 (UTC), unless it names `unwatched_soak_test`: the Intel TCO on x86_64 and
    // the JH7110's on riscv64. Naming `watchdog_soak_test` insists on it, and on aarch64 that is
    // `kernel/src/soak.rs`'s `compile_error!`. A cfg rather
    // than a feature because cargo features cannot differ by target, and `cargo xtask uefi-image
    // --features soak_test` (xenon's stick) has to get the default too. `kernel/src/soak.rs`
    // restates the rule as a `compile_error!`, so an edit here that drops it fails the x86_64
    // `soak_test` build `script/lint` already makes. Name provisional.
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

/// **Compile the boot program's measurement into the kernel image** (milestone 22 phase B.1).
///
/// The build packs the initial ramdisk archive first (`xtask::initial_ramdisk_aarch64` /
/// `initial_ramdisk_riscv` / `initial_ramdisk_x86`), hashes the entries the kernel may enter as the
/// progenitor, and writes `target/init-measure-<arch>.txt`. Here we turn that manifest into
/// `TRUST_ROOT`, a `&[measured_boot::Measurement]` in the kernel's own `.rodata`. That is what
/// makes the check mean "this kernel image runs exactly this progenitor" with no key management:
/// the expected digest is part of the thing doing the checking.
///
/// **The ordering, and why it is not circular.** The kernel image contains the hash of a
/// separately-built initial ramdisk, so the initial ramdisk must exist first: userspace builds, the
/// archive is packed, the manifest is written, *then* the kernel compiles. Every xtask path already
/// had that order (`user()` before the kernel build, because the kernel boots with the archive as
/// `-initrd`), so nothing needed resequencing. The hash never feeds back into the initial ramdisk,
/// so there is no chicken-and-egg: it is a one-way dependency the build already had, now made
/// explicit.
///
/// A **missing** manifest yields an empty trust root rather than a build failure, so a bare
/// `cargo clippy -p kernel` (script/lint) and a bare `cargo build` still work. The kernel refuses to
/// boot on an empty root (`measured_boot::VerifyError::Unmeasured`), which is the right place for that
/// failure: loud at boot, not confusing at lint time. A *malformed* manifest line is a hard error,
/// because that means the build wrote something we cannot read and silently measuring nothing would
/// be the one outcome worse than stopping.
fn generate_trust_root(manifest_dir: &str, arch: &str) {
    let manifest = std::path::Path::new(manifest_dir)
        .parent()
        .expect("the kernel manifest dir has no parent")
        .join(format!("target/init-measure-{arch}.txt"));

    // Rebuild the kernel whenever the measurement changes, which is to say whenever userspace
    // changes. That relink is the honest cost of "this kernel runs exactly this progenitor."
    println!("cargo::rerun-if-changed={}", manifest.display());

    let text = std::fs::read_to_string(&manifest).unwrap_or_default();
    let mut entries = String::new();
    // The format has one definition (`measured_boot::manifest_entries`), shared with the progenitor, which
    // reads the same shape out of the archive at boot (milestone 104). This side turns an
    // unparseable line into a hard error where the progenitor treats it as a refusal, and the asymmetry is
    // deliberate: at build time we can still stop, and a manifest we cannot read means the build
    // wrote something we cannot read.
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

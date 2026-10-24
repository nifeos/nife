//! The system-test image links with the kernel's own linker script, and declares
//! `cfg(initial_ramdisk)`.
//!
//! `cargo::rustc-link-arg` is per package: the kernel's `build.rs` passes `-T link-<arch>.ld` for
//! the kernel's own targets and for nobody who depends on it. This image is the same kernel, so it
//! has to ask for the same layout itself. `cfg(initial_ramdisk)` is declared the way
//! `kernel/build.rs` declares it (`declare_initial_ramdisk_cfg`), because the suite's modules are
//! gated on it and a build script's `rustc-cfg` reaches only its own package too.

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let link_script = match arch.as_str() {
        "aarch64" => "link-aarch64.ld",
        "riscv64" => "link-riscv64.ld",
        "x86_64" => "link-x86_64.ld",
        other => panic!("system_tests: no kernel linker script for {other}"),
    };
    println!("cargo::rerun-if-changed=../kernel/{link_script}");
    println!("cargo::rustc-link-arg=-T{manifest_dir}/../kernel/{link_script}");
    // **Whether a missing `rg` fails rather than skips** (milestone 121 (`ripgrep` on nife)):
    // `ripgrep_threads_tests` reads it with `env!`. The CI rows that build `rg` set it to 1, so a
    // build that left `rg` out of the archive cannot pass as a skip.
    println!("cargo::rerun-if-env-changed=NIFE_RIPGREP_REQUIRED");
    println!(
        "cargo::rustc-env=NIFE_RIPGREP_REQUIRED={}",
        std::env::var("NIFE_RIPGREP_REQUIRED").unwrap_or_default()
    );
    // **And a missing `ioping`** (milestone 906 (ioping is packaged in basalt, and its test
    // installs it with `jig`)), on the same terms: `c_program_tests` reads it, and the CI rows that
    // fetch basalt's ioping set it to 1. Name provisional.
    println!("cargo::rerun-if-env-changed=NIFE_IOPING_REQUIRED");
    println!(
        "cargo::rustc-env=NIFE_IOPING_REQUIRED={}",
        std::env::var("NIFE_IOPING_REQUIRED").unwrap_or_default()
    );
    // **And a missing `hackbench`** (milestone 907 (hackbench is packaged in basalt)), the same
    // way, for `c_thread_tests`. Name provisional.
    println!("cargo::rerun-if-env-changed=NIFE_HACKBENCH_REQUIRED");
    println!(
        "cargo::rustc-env=NIFE_HACKBENCH_REQUIRED={}",
        std::env::var("NIFE_HACKBENCH_REQUIRED").unwrap_or_default()
    );
    // **And a missing `speedtest1`** (milestone 897 (SQLite is packaged in basalt)), the same way,
    // for `c_program_tests`. Name provisional.
    println!("cargo::rerun-if-env-changed=NIFE_SPEEDTEST1_REQUIRED");
    println!(
        "cargo::rustc-env=NIFE_SPEEDTEST1_REQUIRED={}",
        std::env::var("NIFE_SPEEDTEST1_REQUIRED").unwrap_or_default()
    );
    // **Which x86_64 devices the runner was told to attach** (milestone 420 (x86_64 has one fixture
    // of five and no NIC, GPU, keyboard or RNG)), so a test that finds one absent can tell a
    // machine fact from a runner that lost a `-device` line. The guest cannot read the host's
    // environment; this is the only way the runner's variables reach it. `cargo xtask test` sets
    // `NIFE_RNG` and `NIFE_NET` on every x86_64 leg, so there the absence is a failure; a bare
    // `cargo test` leaves them unset and the skip stays. Names provisional.
    for (host, baked) in [
        ("NIFE_RNG", "NIFE_RNG_ATTACHED"),
        ("NIFE_NET", "NIFE_NIC_ATTACHED"),
        // Set only by the PVH x86_64 leg, which attaches all five block fixtures; the OVMF leg
        // attaches the nifefs disk alone and unsets it (`xtask/src/uefi.rs`, `uefi_test`).
        ("NIFE_X86_FIXTURES_REQUIRED", "NIFE_X86_FIXTURES_REQUIRED"),
        // On every architecture, unlike the three above: `cargo xtask test` attaches an NVMe
        // controller on all three legs, and the roster test owes exactly one NVMe entry when it
        // did. The value is the image's path; only whether it is set is read.
        ("NIFE_NVME", "NIFE_NVME_ATTACHED"),
    ] {
        println!("cargo::rerun-if-env-changed={host}");
        println!(
            "cargo::rustc-env={baked}={}",
            std::env::var(host).unwrap_or_default()
        );
    }
    println!("cargo::rustc-check-cfg=cfg(initial_ramdisk)");
    if matches!(arch.as_str(), "aarch64" | "riscv64" | "x86_64") {
        println!("cargo::rustc-cfg=initial_ramdisk");
    }
}

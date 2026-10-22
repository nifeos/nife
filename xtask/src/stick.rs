//! **The stick: every architecture's boot file, and the program that writes them** (DECISIONS §157).
//!
//! ```text
//! cargo xtask stick                        # target/stick/EFI/BOOT/*.EFI, and stick_maker for this host
//! cargo xtask stick --host x86_64-unknown-linux-musl --host x86_64-pc-windows-gnu
//! cargo xtask stick-boot                   # boot target/stick under all three UEFI firmwares
//! cargo xtask acpi-boot                    # the aarch64 payload alone, booted with acpi=on
//! ```
//!
//! **The order inside each payload is the seal, and it is not re-derived here.** For every
//! architecture the archive is packed first, which writes the measurement manifest the kernel
//! compiles in, then the kernel is built, then the loader embeds both, and the loader's own build
//! (`uefi_loader/build.rs`) refuses a pair the kernel does not vouch for. `stick_maker` then embeds
//! only those finished files. Nothing downstream of a loader can pair a kernel with the wrong
//! archive, because nothing downstream handles a kernel or an archive at all.
//!
//! Names: `stick` and `stick-boot` are provisional (this lane, 2026-09-19), in the family of
//! `uefi-image` and `uefi-boot`, naming what they make and what they check.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::{RISCV_TARGET, TARGET, cargo_profiled, profile_dir, user};
use crate::archive::{initial_ramdisk_path, initial_ramdisk_riscv, riscv_initial_ramdisk_path};
use crate::host::workspace_root;
use crate::portable_executable;
use crate::uefi::uefi_image;

/// Where the stick is staged: the layout the program writes, as a directory QEMU can boot.
pub(super) fn stick_dir() -> PathBuf {
    workspace_root().join("target/stick")
}

/// The one boot file per architecture, by the name UEFI gives its removable-media path.
const PAYLOADS: [(&str, &str); 3] = [
    ("x86_64", "BOOTX64.EFI"),
    ("aarch64", "BOOTAA64.EFI"),
    ("riscv64", "BOOTRISCV64.EFI"),
];

fn boot_dir() -> PathBuf {
    stick_dir().join("EFI/BOOT")
}

fn copy_into_stick(from: &Path, name: &str) -> bool {
    let to = boot_dir().join(name);
    match std::fs::copy(from, &to) {
        Ok(bytes) => {
            eprintln!("stick: {} ({bytes} bytes)", to.display());
            true
        }
        Err(e) => {
            eprintln!(
                "stick: cannot copy {} to {}: {e}",
                from.display(),
                to.display()
            );
            false
        }
    }
}

/// Build one loader around a kernel and an archive that were just built in the sealed order.
fn build_loader(target: &str, kernel: &str, archive: &str, extra: &[(&str, &str)]) -> bool {
    let mut args = vec![
        "build",
        "-p",
        "uefi_loader",
        "--bin",
        "uefi_loader",
        "--features",
        "uefi",
        "--target",
        target,
    ];
    if profile_dir() == "release" {
        args.push("--release");
    }
    let mut command = Command::new("cargo");
    command
        .args(&args)
        .env("NIFE_UEFI_KERNEL", kernel)
        .env("NIFE_UEFI_INITIAL_RAMDISK", archive);
    for (key, value) in extra {
        if *key == "--target-dir" {
            command.args(["--target-dir", value]);
        } else {
            command.env(key, value);
        }
    }
    command.status().is_ok_and(|s| s.success())
}

/// `BOOTX64.EFI`: exactly what `cargo xtask uefi-image` stages for xenon, copied.
fn payload_x86_64() -> bool {
    // `mkfs` and the FS server go in the archive only if built first. Without them the file this
    // stick installs partitions the disk and leaves the data partition empty, which
    // `install::prepare` already avoids for the gate: the stick must install what the gate proves.
    crate::disk::redoxfs_server_build(crate::X86_TARGET)
        && uefi_image()
        && copy_into_stick(
            &workspace_root().join("target/esp/EFI/BOOT/BOOTX64.EFI"),
            "BOOTX64.EFI",
        )
}

/// `BOOTAA64.EFI`: the aarch64 archive, then the kernel, then the loader for `aarch64-unknown-uefi`.
fn payload_aarch64() -> bool {
    if !user() || !cargo_profiled(&["build", "-p", "kernel", "--target", TARGET]) {
        return false;
    }
    let kernel = workspace_root()
        .join(format!("target/{TARGET}/{}/kernel", profile_dir()))
        .display()
        .to_string();
    build_loader(
        "aarch64-unknown-uefi",
        &kernel,
        &initial_ramdisk_path(),
        &[],
    ) && copy_into_stick(
        &workspace_root().join(format!(
            "target/aarch64-unknown-uefi/{}/uefi_loader.efi",
            profile_dir()
        )),
        "BOOTAA64.EFI",
    )
}

/// **`BOOTRISCV64.EFI`**, the one that is not built as PE: rustc has no riscv64 UEFI target, so the
/// loader is linked as a static PIE for `riscv64imac-unknown-none-elf` and converted.
///
/// The kernel is the **`board`** build, the one radon runs (`script/board-image`), because the
/// stick is for the bench as well as for QEMU, and the tour it runs is the same on both: the
/// feature changes how a test run exits, which a tour never does. `stick-boot` proves it boots
/// under QEMU's EDK2.
///
/// The link arguments, each for a reason:
/// - `-pie --no-dynamic-linker`: a position-independent image with no interpreter, because the
///   firmware loads it wherever it likes.
/// - `-z notext`: the prebuilt `core` for this target is not PIC, so its vtables in `.rodata`
///   need load-time relocations; the firmware applies them before a single instruction runs.
/// - `-z separate-loadable-segments`: every segment page-aligned, so each becomes a PE section.
/// - `--image-base=0x1000`: the first page is the PE headers'.
/// - `--entry=efi_main`: the PE entry point.
///
/// A separate target directory, so these flags never touch the kernel's riscv64 build artifacts.
fn payload_riscv64() -> bool {
    if !initial_ramdisk_riscv()
        || !cargo_profiled(&[
            "build",
            "-p",
            "kernel",
            "--target",
            RISCV_TARGET,
            "--features",
            "board",
        ])
    {
        return false;
    }
    let kernel = workspace_root()
        .join(format!("target/{RISCV_TARGET}/{}/kernel", profile_dir()))
        .display()
        .to_string();
    let target_dir = workspace_root().join("target/uefi-riscv64");
    let rustflags = "-C relocation-model=pie -C link-arg=-pie -C link-arg=--no-dynamic-linker \
                     -C link-arg=-znotext -C link-arg=-zseparate-loadable-segments \
                     -C link-arg=--image-base=0x1000 -C link-arg=--entry=efi_main";
    if !build_loader(
        RISCV_TARGET,
        &kernel,
        &riscv_initial_ramdisk_path(),
        &[
            ("RUSTFLAGS", rustflags),
            ("--target-dir", &target_dir.display().to_string()),
        ],
    ) {
        return false;
    }
    let elf_path = target_dir.join(format!("{RISCV_TARGET}/{}/uefi_loader", profile_dir()));
    let elf = match std::fs::read(&elf_path) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("stick: cannot read {}: {e}", elf_path.display());
            return false;
        }
    };
    let pe = match portable_executable::from_elf(&elf) {
        Ok(pe) => pe,
        Err(e) => {
            eprintln!(
                "stick: {} cannot become a PE image: {e:?}",
                elf_path.display()
            );
            return false;
        }
    };
    let to = boot_dir().join("BOOTRISCV64.EFI");
    match std::fs::write(&to, &pe) {
        Ok(()) => {
            eprintln!(
                "stick: {} ({} bytes, converted from {})",
                to.display(),
                pe.len(),
                elf_path.display()
            );
            true
        }
        Err(e) => {
            eprintln!("stick: cannot write {}: {e}", to.display());
            false
        }
    }
}

/// A short description of the source the stick was built from, for `NIFE.TXT`.
fn build_label() -> String {
    Command::new("git")
        .args(["describe", "--always", "--dirty", "--abbrev=12"])
        .current_dir(workspace_root())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".to_owned())
}

/// The download's file name for a host triple: `stick_maker-<os>-<cpu>`, `.exe` on Windows.
fn download_name(triple: &str) -> String {
    let cpu = triple.split('-').next().unwrap_or("unknown");
    let cpu = if cpu == "aarch64" { "arm64" } else { cpu };
    let (os, suffix) = if triple.contains("apple") {
        ("macos", "")
    } else if triple.contains("windows") {
        ("windows", ".exe")
    } else if triple.contains("linux") {
        ("linux", "")
    } else {
        ("other", "")
    };
    format!("stick_maker-{os}-{cpu}{suffix}")
}

fn host_triple() -> String {
    Command::new("rustc")
        .arg("-vV")
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .find_map(|l| l.strip_prefix("host: ").map(str::to_owned))
        })
        .unwrap_or_default()
}

/// Build `stick_maker` for one host triple around the staged payloads. Always release: it is the
/// thing a stranger downloads, and its size is the payloads' plus a few hundred kilobytes.
fn build_program(triple: &str, label: &str) -> Option<PathBuf> {
    let mut command = Command::new("cargo");
    command
        .args([
            "build",
            "-p",
            "stick_maker",
            "--release",
            "--target",
            triple,
        ])
        .env("NIFE_STICK_PAYLOADS", stick_dir())
        .env("NIFE_STICK_BUILD", label);
    // A cross-built Linux binary is linked by rust-lld as a static musl executable, so it runs on
    // any Linux with no C library to match and needs no cross toolchain installed here.
    if triple.ends_with("linux-musl") {
        command.env(
            format!(
                "CARGO_TARGET_{}_LINKER",
                triple.to_uppercase().replace('-', "_")
            ),
            "rust-lld",
        );
    }
    if !command.status().is_ok_and(|s| s.success()) {
        eprintln!("stick: stick_maker did not build for {triple}");
        return None;
    }
    let exe = if triple.contains("windows") {
        "stick_maker.exe"
    } else {
        "stick_maker"
    };
    let built = workspace_root().join(format!("target/{triple}/release/{exe}"));
    let out = workspace_root().join("target/stick-maker");
    let _ = std::fs::create_dir_all(&out);
    let to = out.join(download_name(triple));
    match std::fs::copy(&built, &to) {
        Ok(bytes) => {
            eprintln!("stick: {} ({bytes} bytes)", to.display());
            Some(to)
        }
        Err(e) => {
            eprintln!("stick: cannot copy {}: {e}", built.display());
            None
        }
    }
}

/// `cargo xtask stick [--host <triple>]...` (always a release build)
pub(super) fn stick() -> bool {
    let args: Vec<String> = std::env::args().skip(2).collect();
    // **The stick is always release** (calef, 2026-10-03 UTC, the install boot slots). It is what
    // a person writes to a drive and what installs itself onto a disk, and a debug build is three
    // times the size and is refused for install by the loader. `--release` is accepted and means
    // nothing, so older command lines keep working.
    super::RELEASE.store(true, std::sync::atomic::Ordering::Relaxed);
    let mut hosts: Vec<String> = args
        .windows(2)
        .filter(|w| w[0] == "--host")
        .map(|w| w[1].clone())
        .collect();
    if hosts.is_empty() {
        hosts.push(host_triple());
    }

    // Start from nothing, so a payload that failed to build cannot be replaced by last time's.
    let _ = std::fs::remove_dir_all(stick_dir());
    if let Err(e) = std::fs::create_dir_all(boot_dir()) {
        eprintln!("stick: cannot create {}: {e}", boot_dir().display());
        return false;
    }
    if !(payload_x86_64() && payload_aarch64() && payload_riscv64()) {
        eprintln!("stick: a payload failed to build; no program was built around a partial set");
        return false;
    }

    let label = build_label();
    let mut ok = true;
    let mut apple = Vec::new();
    for triple in &hosts {
        match build_program(triple, &label) {
            Some(path) if triple.contains("apple") => apple.push(path),
            Some(_) => {}
            None => ok = false,
        }
    }
    // Both macOS builds present: one universal binary is the download, the way a Mac app ships.
    if apple.len() == 2 {
        let universal = workspace_root().join("target/stick-maker/stick_maker-macos");
        let status = Command::new("lipo")
            .arg("-create")
            .args(&apple)
            .arg("-output")
            .arg(&universal)
            .status();
        if status.is_ok_and(|s| s.success()) {
            eprintln!(
                "stick: {} (universal: arm64 and x86_64)",
                universal.display()
            );
        } else {
            eprintln!("stick: lipo could not join the two macOS builds");
            ok = false;
        }
    }
    eprintln!(
        "stick: staged at {}; boot it with `cargo xtask stick-boot`",
        stick_dir().display()
    );
    ok
}

/// **Boot the staged stick under each architecture's UEFI firmware, and check it got all the way.**
///
/// One directory, three firmwares: each finds its own file among the three and ignores the others,
/// which is the universal stick's whole claim. What is asserted, per architecture, is chosen so it
/// cannot pass for the wrong reason: the loader's banner (the firmware found and ran our file), the
/// self-test verdict with every check passing (the kernel ran on the machine the firmware
/// described), and the hand-over to the progenitor (the archive the loader placed was found through
/// the handoff and passed the measurement).
pub(super) fn stick_boot() -> bool {
    let mut ok = true;
    for (arch, file) in PAYLOADS {
        if !boot_dir().join(file).exists() {
            eprintln!("stick-boot: {file} is not staged; run `cargo xtask stick` first");
            return false;
        }
        ok &= boot_and_check("stick-boot", arch, &stick_dir(), file, &[], &[]);
    }
    ok & riscv64_from_our_volume()
}

/// **The riscv64 payload booted from a disk this tree wrote**, not from the stick directory.
/// QEMU's `vvfat` synthesizes the directory's volume, so the boots above never read
/// `file_allocation_table`'s long-name entries; this one does, since `BOOTRISCV64.EFI` is not an
/// 8.3 name and EDK2 finds it only through a run whose checksum matches its `BOOTRI~1.EFI` alias.
fn riscv64_from_our_volume() -> bool {
    match crate::cloud_image::write("riscv64", &boot_dir().join("BOOTRISCV64.EFI")) {
        Ok(disk) => {
            eprintln!("stick-boot: wrote {}", disk.display());
            boot_and_check("stick-boot", "riscv64", &disk, "BOOTRISCV64.EFI", &[], &[])
        }
        Err(e) => {
            eprintln!("stick-boot: the riscv64 boot disk: {e}");
            false
        }
    }
}

/// The lines every stick boot must print, in [`stick_boot`]'s order and for its reasons. The
/// self-test verdict is its prefix here, because what follows the count depends on the machine
/// (a check whose device it lacks is `absent`); [`verdict_failed`] is what requires it green.
const BOOTED: [&str; 3] = [
    "nife uefi_loader: milestone 87",
    boot_ladder::SELF_TEST,
    "nife: handing the system to the userspace progenitor.",
];

/// The self-test verdict line's `FAILED:` list, if this transcript has a red one.
fn verdict_failed(transcript: &str) -> Option<&str> {
    let line = transcript
        .lines()
        .find(|l| l.contains(boot_ladder::SELF_TEST))?;
    let at = line.find(boot_ladder::SELF_TEST_FAILED)?;
    Some(line[at..].trim_end())
}

/// **The aarch64 stick, booted with ACPI and no device tree** (milestone 578 (an ACPI discovery path for aarch64)): the state every
/// aarch64 cloud machine is in, since SBBR forbids it a tree, and the one the loader meets by
/// writing a tree from the tables (§278 (an ACPI machine reaches the aarch64 kernel as a device tree the loader writes)). `stick-boot` boots `acpi=off`, so without this the path
/// every rented Arm machine would take was proved only by a transcript run by hand.
///
/// It builds the aarch64 payload alone, release like the stick, rather than requiring `cargo xtask
/// stick`'s three payloads and host program, which this boot never reads. Beside [`BOOTED`] it
/// asserts the loader's own ACPI line, so a firmware that went back to offering a device tree
/// cannot pass it by booting the other path.
///
/// It boots twice, once per GIC. QEMU's default is the GICv2 with a `GICv2m` frame; the second is
/// `gic-version=3`, the GIC every aarch64 cloud machine has and the only `virt` with an ITS under
/// TCG, where it asserts the loader's GICv3 and ITS lines (the MADT's types 0x0E and 0x0F read
/// out of the tables QEMU wrote).
///
/// **The second boot is from the cloud boot disk** ([`crate::cloud_image`]), not the stick
/// directory: the same file on a GPT disk this tree wrote, which is what a person imports into
/// Oracle. So the disk is booted by every run of this gate rather than only by the person who
/// uploads it.
///
/// Name: provisional (milestone 578's lane, 2026-10-19 UTC), in the family of `stick-boot` and
/// `uefi-boot`, naming the description it boots from.
pub(super) fn acpi_boot() -> bool {
    super::RELEASE.store(true, std::sync::atomic::Ordering::Relaxed);
    if let Err(e) = std::fs::create_dir_all(boot_dir()) {
        eprintln!("acpi-boot: cannot create {}: {e}", boot_dir().display());
        return false;
    }
    if !payload_aarch64() {
        return false;
    }
    let disk = match crate::cloud_image::write("aarch64", &boot_dir().join("BOOTAA64.EFI")) {
        Ok(disk) => {
            eprintln!("acpi-boot: wrote {}", disk.display());
            disk
        }
        Err(e) => {
            eprintln!("acpi-boot: the cloud boot disk: {e}");
            return false;
        }
    };
    // `&` rather than `&&` between the boots, so one GIC's failure does not hide the other's.
    boot_and_check(
        "acpi-boot",
        "aarch64",
        &stick_dir(),
        "BOOTAA64.EFI",
        &[("NIFE_ACPI", "on")],
        &[ACPI_LINE],
    ) & boot_and_check(
        "acpi-boot",
        "aarch64",
        &disk,
        "BOOTAA64.EFI",
        &[("NIFE_ACPI", "on"), ("NIFE_GIC", "3")],
        &[
            ACPI_LINE,
            " core(s), GICv3 at 0x0000000008000000",
            "uefi_loader:   GIC ITS at 0x0000000008080000;",
        ],
    )
}

/// The loader's line for a machine it describes from ACPI, which an `acpi=off` boot never prints.
const ACPI_LINE: &str = "uefi_loader: no device tree; describing this machine from ACPI";

/// Boot `medium` (the stick directory or a raw disk) for `arch` and check the transcript holds
/// [`BOOTED`] and `also`, with no panic. `who` prefixes every line, so a failure names the gate
/// that saw it.
fn boot_and_check(
    who: &str,
    arch: &str,
    medium: &Path,
    file: &str,
    env: &[(&str, &str)],
    also: &[&str],
) -> bool {
    let output = Command::new("helpers/qemu-stick.sh")
        .args([arch, &medium.display().to_string()])
        .current_dir(workspace_root())
        .env(
            "NIFE_STICK_TIMEOUT",
            std::env::var("NIFE_STICK_TIMEOUT").unwrap_or("120".into()),
        )
        .envs(env.iter().copied())
        .stdin(std::process::Stdio::null())
        .output();
    let Ok(output) = output else {
        eprintln!("{who}: cannot run helpers/qemu-stick.sh");
        return false;
    };
    let transcript = String::from_utf8_lossy(&output.stdout).into_owned();
    let mut passed = true;
    for wanted in BOOTED.iter().chain(also) {
        if !transcript.contains(wanted) {
            eprintln!("{who}: {arch}: the transcript is missing {wanted:?}");
            passed = false;
        }
    }
    if let Some(failed) = verdict_failed(&transcript) {
        eprintln!("{who}: {arch}: the self-test verdict is red: {failed}");
        passed = false;
    }
    if transcript.contains("[PANIC]") {
        eprintln!("{who}: {arch}: the kernel panicked");
        passed = false;
    }
    if passed {
        eprintln!(
            "{who}: {arch}: booted from {file} on {} to the progenitor",
            medium.display()
        );
    } else {
        let tail: Vec<&str> = transcript.lines().rev().take(25).collect();
        for line in tail.iter().rev() {
            eprintln!("{who}: {arch}: | {line}");
        }
    }
    passed
}

#[cfg(test)]
mod tests {
    use super::download_name;

    #[test]
    fn downloads_are_named_by_os_and_cpu() {
        assert_eq!(
            download_name("aarch64-apple-darwin"),
            "stick_maker-macos-arm64"
        );
        assert_eq!(
            download_name("x86_64-apple-darwin"),
            "stick_maker-macos-x86_64"
        );
        assert_eq!(
            download_name("x86_64-unknown-linux-musl"),
            "stick_maker-linux-x86_64"
        );
        assert_eq!(
            download_name("aarch64-unknown-linux-musl"),
            "stick_maker-linux-arm64"
        );
        assert_eq!(
            download_name("x86_64-pc-windows-gnu"),
            "stick_maker-windows-x86_64.exe"
        );
    }
}

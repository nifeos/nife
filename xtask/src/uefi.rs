//! The bootable UEFI image (milestone 87): the entry real firmware can start.
//!
//! Staged at `target/esp` for a QEMU/OVMF boot or for a FAT32 stick, and booted under OVMF
//! by `uefi-boot` and `uefi-test`. See notes/x86-uefi-boot.md.

use std::path::Path;
use std::process::Command;
use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::archive::{initial_ramdisk_x86, x86_initial_ramdisk_path};
use crate::disk::{disk_path, mkdisk, mknvmedisk, nvme_disk_path};
use crate::host::workspace_root;
use crate::scanout::{ScanoutReferee, screendump};
use crate::suite::kernel_test_elf;
use crate::{RELEASE, X86_TARGET, cargo_profiled, profile_dir};

/// The `x86_64-unknown-uefi` target (milestone 87): PE/COFF rather than ELF, entered by real
/// firmware in long mode. Only `uefi_loader`'s binary half is ever built for it.
const UEFI_TARGET: &str = "x86_64-unknown-uefi";

/// **The EFI system partition, staged as a directory** (milestone 87).
///
/// A directory rather than an image, because nothing here has to build a FAT filesystem: QEMU's
/// vvfat driver synthesises one from a directory (`helpers/qemu-uefi-x86_64.sh`), and a USB stick
/// is formatted by the person holding it. That is the same fact from both ends, and it is why this
/// milestone needed no new host tooling at all.
pub(crate) fn esp_dir() -> std::path::PathBuf {
    workspace_root().join("target/esp")
}

/// **Build the bootable UEFI image** (milestone 87) at [`esp_dir`], with no kernel features: the
/// kernel, the archive, and the loader that embeds both. Only [`uefi_image_command`] reads the
/// command line; other callers' arguments are theirs. See
/// notes/x86-uefi-boot/running-under-firmware.md, "Inside the two gates".
/// ```text
/// cargo xtask uefi-image
/// helpers/qemu-uefi-x86_64.sh target/esp    # boot it under QEMU with real firmware
/// ```
/// # BUGS
/// - `BOOTX64.EFI` is the removable-media path; no boot entry of its own is installed (the note).
pub(crate) fn uefi_image() -> bool {
    uefi_image_with(None)
}

/// `cargo xtask uefi-image [--release] [--features <list>]`: the one caller whose arguments are
/// [`uefi_image`]'s.
///
/// `--features` builds the kernel with those cargo features, the one way to put a non-default
/// kernel (`watchdog_soak_test`) on xenon's stick. A kernel whose boot enters no archive program
/// through `trust::require_program` or the hand-over is still refused as `NOT SEALED`.
pub(crate) fn uefi_image_command() -> bool {
    let args: Vec<String> = std::env::args().skip(2).collect();
    // `--release` is `script/boot-file-size-check`'s: the image it budgets is the one an install
    // writes.
    let release = args.iter().any(|a| a == "--release");
    if release {
        RELEASE.store(true, Ordering::Relaxed);
        // The image an install writes carries `mkfs` and the FS server, which the archive packs
        // only if something built them (`install::prepare` says why), so the size it budgets is
        // measured with them in.
        if !crate::disk::redoxfs_server_build(X86_TARGET) {
            eprintln!("uefi-image: could not build the FS server and mkfs for {X86_TARGET}");
            return false;
        }
    }
    let rest: Vec<&str> = args
        .iter()
        .map(String::as_str)
        .filter(|a| *a != "--release")
        .collect();
    match rest.as_slice() {
        [] => uefi_image_with(None),
        ["--features", list] => uefi_image_with(Some(list)),
        _ => {
            eprintln!("usage: cargo xtask uefi-image [--release] [--features <kernel features>]");
            false
        }
    }
}

pub(crate) fn uefi_image_with(features: Option<&str>) -> bool {
    let Some(kernel) = uefi_kernel(features) else {
        return false;
    };
    if let Some(list) = features {
        eprintln!("uefi-image: this image's kernel was built with --features {list}");
    }

    uefi_stage(
        &kernel,
        &esp_dir(),
        "the loader, the kernel and the archive",
        false,
    )
}

/// **The shipping kernel and the archive it is measured against**, built in the order the comment
/// inside requires, returning the kernel's path (stripped of DWARF for a release image). Shared so
/// the screen gate stages the same kernel without duplicating that order.
pub(crate) fn uefi_kernel(features: Option<&str>) -> Option<String> {
    // **The archive FIRST, then the kernel.** Packing the archive regenerates
    // `target/init-measure-x86_64.txt`, which `kernel/build.rs` compiles in as the measured-boot
    // trust root, so a kernel built first vouches for the previous archive and the handover refuses
    // ("MEASURED BOOT REFUSED"). QEMU rarely shows it; two benches did
    // (notes/x86-uefi-boot/running-under-firmware.md). `initial_ramdisk_x86` never builds the
    // kernel, so this order is safe.
    let mut build = vec!["build", "-p", "kernel", "--target", X86_TARGET];
    if let Some(list) = features {
        build.extend(["--features", list]);
    }
    if !initial_ramdisk_x86() || !cargo_profiled(&build) {
        return None;
    }
    let built = workspace_root().join(format!("target/{X86_TARGET}/{}/kernel", profile_dir()));
    if !RELEASE.load(Ordering::Relaxed) {
        return Some(built.display().to_string());
    }
    // **A release image embeds a kernel with its DWARF removed** (calef, 2026-10-03 UTC): about
    // 5.8 MB of a 7.5 MB ELF that nothing on the machine reads. Stripped before the loader embeds
    // it, so the seal (`refuse_an_unsealed_pair`, digests in `.rodata`, which `--strip-debug`
    // leaves alone) covers the bytes that ship.
    let stripped = built.with_file_name("kernel.stripped");
    let bytes = match crate::inspect::read_stripped(&built.display().to_string()) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("uefi-image: cannot strip {}: {e}", built.display());
            return None;
        }
    };
    if let Err(e) = std::fs::write(&stripped, &bytes) {
        eprintln!("uefi-image: cannot write {}: {e}", stripped.display());
        return None;
    }
    Some(stripped.display().to_string())
}

/// **Where the screen gate's EFI system partition is staged**
/// (milestone 445 (the screen check stops sampling and starts asking)).
///
/// A third directory, for the reason [`uefi_test_esp_dir`] is a second one: its loader is built
/// with `screen_hold`, so the kernel waits up to ten seconds at the screen handover for a serial
/// byte, which is wrong for a stick. The kernel is byte-identical to [`esp_dir`]'s; only one word
/// on the loader's command line differs, so the binary under test is the binary that ships.
fn uefi_screen_esp_dir() -> std::path::PathBuf {
    workspace_root().join("target/esp-screen")
}

/// **Where the test build's EFI system partition is staged** (milestone 195).
///
/// A second directory rather than `target/esp`, and the separation is the point rather than
/// tidiness. `esp_dir()` is what the bench procedure copies to a USB stick, and a stick carries no
/// sign of which kernel is inside the one file on it (`notes/x86-uefi-boot.md`'s last `BUGS`
/// entry). If the suite run wrote over that directory, a `cargo xtask uefi-boot` followed by a copy
/// would put the *test* kernel on the machine, which boots, prints a tour, runs 200 tests and then
/// asks QEMU to exit on a port no Dell answers. Two directories make that unrepresentable.
fn uefi_test_esp_dir() -> std::path::PathBuf {
    workspace_root().join("target/esp-test")
}

/// **Build a UEFI application around one kernel ELF and stage it where firmware looks**
/// (milestone 87, split out by milestone 195 so the tour and the test suite can each have one).
///
/// The caller owns the kernel: this builds `uefi_loader` with `NIFE_UEFI_KERNEL` pointing at it,
/// and the loader's build script `include_bytes!`s both it and the archive. `what` is the phrase
/// the size line uses, because "the loader, the kernel and the archive" and "the loader, the test
/// kernel and the archive" are the one difference a reader of the transcript can act on.
pub(crate) fn uefi_stage(
    kernel: &str,
    esp: &std::path::Path,
    what: &str,
    screen_hold: bool,
) -> bool {
    // `screen_hold` is milestone 445's: it makes the loader write one more word on the kernel's
    // boot command line, and only `uefi_boot` passes it. See [`uefi_screen_esp_dir`].
    let features = if screen_hold {
        "uefi,screen_hold"
    } else {
        "uefi"
    };
    let mut args = std::vec![
        "build",
        "-p",
        "uefi_loader",
        "--bin",
        "uefi_loader",
        "--features",
        features,
        "--target",
        UEFI_TARGET,
    ];
    if RELEASE.load(Ordering::Relaxed) {
        args.push("--release");
    }
    // Not `cargo()`: it exports the aarch64 runner's `NIFE_INITIAL_RAMDISK`, which differs from
    // `NIFE_UEFI_INITIAL_RAMDISK` by one word in a way nothing would catch.
    let built = Command::new("cargo")
        .args(&args)
        .env("NIFE_UEFI_KERNEL", kernel)
        .env("NIFE_UEFI_INITIAL_RAMDISK", x86_initial_ramdisk_path())
        .status()
        .map(|s| s.success())
        .unwrap_or_else(|e| {
            eprintln!("uefi-image: failed to run cargo: {e}");
            false
        });
    if !built {
        return false;
    }

    let efi = workspace_root().join(format!(
        "target/{UEFI_TARGET}/{}/uefi_loader.efi",
        profile_dir()
    ));
    // `\EFI\BOOT\BOOTX64.EFI` is the removable-media path every UEFI implementation looks for with
    // no configuration at all, which is what makes the bench procedure "copy one file to a stick".
    let boot_dir = esp.join("EFI/BOOT");
    if let Err(e) = std::fs::create_dir_all(&boot_dir) {
        eprintln!("uefi-image: cannot create {}: {e}", boot_dir.display());
        return false;
    }
    let target = boot_dir.join("BOOTX64.EFI");
    if let Err(e) = std::fs::copy(&efi, &target) {
        eprintln!(
            "uefi-image: cannot copy {} to {}: {e}",
            efi.display(),
            target.display()
        );
        return false;
    }
    let size = std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
    eprintln!("wrote {} ({size} bytes: {what})", target.display());
    // **The breakdown, on every run**, so growth has a cause on record: the kernel and the archive
    // are embedded whole (`include_bytes!`, no compression), so the loader is what remains.
    // `script/boot-file-size-check` budgets the total; this says where it went.
    let part = |p: &str| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    let (kernel_bytes, archive_bytes) = (part(kernel), part(&x86_initial_ramdisk_path()));
    eprintln!(
        "size: {} loader, {kernel_bytes} kernel, {archive_bytes} archive = {size} bytes ({} profile)",
        size.saturating_sub(kernel_bytes + archive_bytes),
        profile_dir()
    );
    eprintln!(
        "  under QEMU with real firmware: helpers/qemu-uefi-x86_64.sh {}",
        esp.display()
    );
    // Said only for a loader a bench may have. The `screen_hold` build waits ten seconds at the
    // screen handover for a host that is not there, so telling anyone to carry it would be telling
    // them to carry a defect; `uefi_screen_esp_dir`'s doc comment is the long version.
    if screen_hold {
        eprintln!(
            "  NOT for a stick: this loader asks the kernel to hold the screen (milestone 445)"
        );
    } else {
        eprintln!("  on the bench: copy that file to a FAT32 stick as /EFI/BOOT/BOOTX64.EFI");
    }
    true
}

/// **Boot the UEFI image under OVMF and check the firmware path actually ran** (milestone 87).
///
/// Each assertion is one only the firmware path can satisfy: `(xsdt)` in the ACPI line and
/// no `rsdp 0x0` (PVH's tell), the tour's completion, the ECAM window from the MCFG read above 1 GiB
/// (milestone 165 (x86_64 PCI enumeration)), and the shell on the screen answering a serial
/// keystroke ([`screen_watch`]). The runner bounds the boot, so a hang fails rather than hangs; it
/// is stopped once the screen has answered. Returns whether everything held. See
/// notes/x86-uefi-boot/running-under-firmware.md, "What `uefi-boot` asserts".
pub(crate) fn uefi_boot() -> bool {
    uefi_boot_staged()
}

/// `cargo xtask uefi-boot [--release]`: the one caller whose arguments are this gate's, for the
/// reason [`uefi_image_command`] is the only one that parses `uefi-image`'s (`script/test` calls
/// [`uefi_boot`] with its own command line still in `std::env::args`).
///
/// **`--release` boots the profile every image for a machine is built in**: `install-boot`,
/// `network-bench` and `uefi-image --release` are all release, and the suite's debug boot cannot
/// stand in for them. An `asm!` operand that shares a register with a live input is invisible at
/// debug and fatal at release: `segments::init`'s `ltr` loaded the low bits of a code address as
/// its selector and every release image stopped after the boot's `pat` line, while every debug
/// boot passed. CI's `boot-check` job runs this (`script/ci-build`'s `release-uefi-boot` row).
pub(crate) fn uefi_boot_command() -> bool {
    let args: Vec<String> = std::env::args().skip(2).collect();
    match args.as_slice() {
        [] => {}
        [flag] if flag == "--release" => RELEASE.store(true, Ordering::Relaxed),
        _ => {
            eprintln!("usage: cargo xtask uefi-boot [--release]");
            return false;
        }
    }
    uefi_boot_staged()
}

/// The gate itself, in whichever profile [`RELEASE`] names. See [`uefi_boot`].
fn uefi_boot_staged() -> bool {
    // **The shipping kernel behind a loader built with `screen_hold`** (milestone 445), staged
    // somewhere no bench procedure names. See [`uefi_screen_esp_dir`].
    let Some(kernel) = uefi_kernel(None) else {
        return false;
    };
    if !uefi_stage(
        &kernel,
        &uefi_screen_esp_dir(),
        "the loader, the kernel and the archive",
        true,
    ) {
        return false;
    }
    eprintln!();
    eprintln!("--- boot under real firmware, x86_64 (QEMU q35 + OVMF) ---");

    // The screen half reads the monitor's screendumps; the serial transcript would be identical
    // if the screen were black. It types on the runner's stdin, which is COM1. In /tmp because a
    // unix socket path is capped at 104 bytes and a worktree's `target/` gets close.
    let sock = format!("/tmp/nife-uefi-screen-{}.sock", std::process::id());
    let shot =
        std::path::PathBuf::from(format!("/tmp/nife-uefi-screen-{}.ppm", std::process::id()));
    let _ = std::fs::remove_file(&sock);
    let _ = std::fs::remove_file(&shot);

    let Some(mut child) = start_uefi_screen_boot(&sock) else {
        return false;
    };
    let (stdout, stderr, transcript_so_far) = collect_uefi_streams(&mut child);
    let serial = child.stdin.take().expect("piped stdin");
    let watcher = {
        let (sock, shot) = (sock.clone(), shot.clone());
        let wire = std::sync::Arc::clone(&transcript_so_far);
        std::thread::spawn(move || screen_watch(&sock, &shot, serial, &wire))
    };
    let screen = watcher.join().unwrap_or_default();
    // Stopped once the screen has answered: the kernel never exits, and everything asserted below
    // is printed by then. SIGTERM is what `qemu-bounded.sh` forwards to QEMU; its killer backs it.
    let _ = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status();
    let _ = child.wait();
    let transcript = stdout.join().unwrap_or_default() + &stderr.join().unwrap_or_default();
    print!("{transcript}");

    let mut ok = uefi_serial_transcript_holds(&transcript);

    // --- What was on the SCREEN: milestone 243's tour, then the shell ---
    let _ = std::fs::remove_file(&sock);
    ok &= uefi_screen_showed_the_tour(&screen, &shot);
    ok &= uefi_screen_showed_the_shell(&screen, &shot);
    // The same image at the modes a real monitor reports, since 1280x800 is the one mode no
    // bench machine's panel has. See `uefi_screen_modes`.
    ok &= crate::uefi_screen_modes::every_mode_hands_the_screen_over(&uefi_screen_esp_dir());

    if ok {
        eprintln!("uefi-boot: booted under OVMF from \\EFI\\BOOT\\BOOTX64.EFI");
    }
    ok
}

/// **Start the OVMF machine [`uefi_boot`] reads**, its monitor on `sock` and all three standard
/// streams piped. `None` after saying why it did not start.
fn start_uefi_screen_boot(sock: &str) -> Option<std::process::Child> {
    match Command::new("helpers/qemu-uefi-x86_64.sh")
        .arg(uefi_screen_esp_dir())
        .current_dir(workspace_root())
        .env("NIFE_SCREEN_MON", sock)
        // Two cores (milestone 195 (finish the UEFI boot path)): this checks that the loader's
        // request for the trampoline page at 0x8000 worked, and that device interrupts still reach
        // the boot core with a second local APIC. The suite keeps one core until it is measured at
        // two under OVMF; see notes/x86-uefi-boot/running-under-firmware.md, "SMP under firmware".
        .env("NIFE_SMP", "2")
        // The bare firmware machine whether or not a suite leg ran first: `test()` sets these in
        // this process for the PVH leg, and the tour must be one machine either way.
        .env_remove("NIFE_DISK")
        .env_remove("NIFE_NVME")
        // The suite's display devices too: with `NIFE_GPU` the runner drops q35's VGA, whose
        // linear framebuffer is the screen this tour reads back.
        .env_remove("NIFE_GPU")
        .env_remove("NIFE_GPU_MON")
        .env_remove("NIFE_KEYBOARD")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => Some(c),
        Err(e) => {
            eprintln!("uefi-boot: failed to run helpers/qemu-uefi-x86_64.sh: {e}");
            None
        }
    }
}

/// The two reader threads [`collect_uefi_streams`] starts, and the transcript they share.
pub(crate) type CollectedStreams = (
    std::thread::JoinHandle<String>,
    std::thread::JoinHandle<String>,
    std::sync::Arc<std::sync::Mutex<String>>,
);

/// **Collect the runner's stdout and stderr on threads of their own**, each into its own string
/// for the join and both into one shared transcript as they arrive.
pub(crate) fn collect_uefi_streams(child: &mut std::process::Child) -> CollectedStreams {
    // On threads, so the watcher and not a blocking read decides when the boot has said enough;
    // and into a shared buffer, because the watcher waits for `boot_ladder::SCREEN_HELD` live.
    let transcript_so_far = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let collect = |mut from: Box<dyn std::io::Read + Send>,
                   into: std::sync::Arc<std::sync::Mutex<String>>| {
        std::thread::spawn(move || {
            let mut mine = String::new();
            let mut buf = [0u8; 4096];
            while let Ok(n) = from.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let chunk = String::from_utf8_lossy(&buf[..n]).into_owned();
                if let Ok(mut shared) = into.lock() {
                    shared.push_str(&chunk);
                }
                mine.push_str(&chunk);
            }
            mine
        })
    };
    let stdout = collect(
        Box::new(child.stdout.take().expect("piped stdout")),
        std::sync::Arc::clone(&transcript_so_far),
    );
    let stderr = collect(
        Box::new(child.stderr.take().expect("piped stderr")),
        std::sync::Arc::clone(&transcript_so_far),
    );
    (stdout, stderr, transcript_so_far)
}

/// **The serial transcript's half of the gate**: every line the firmware path has to print, and
/// none of the tells that it did not run. False after naming each that failed.
fn uefi_serial_transcript_holds(transcript: &str) -> bool {
    let mut ok = true;
    for wanted in [
        // This boot carries the archive, so it reaches the self-test verdict and the hand-over.
        boot_ladder::SELF_TEST,
        "nife: handing the system to the userspace progenitor.",
        "(xsdt)",
        "pci         : ecam at",
        // Two cores ONLINE, not two in the MADT: the difference is whether the trampoline page the
        // loader asked for was actually usable. See the `NIFE_SMP` comment above.
        "smp: 2 core(s) online",
        // And a device interrupt still landing on the boot core with two local APICs present.
        "device irq  : pit irq 0 -> gsi 2",
        // The screen went to a userspace terminal and both halves came up; said on the UART, since
        // the screen is handed away by then. OVMF's 1280x800 at the 14x26 cell is scale one, 91x30.
        "1280x800 pixels of it at scale 1 served by framebuffer_driver, a 91x30 terminal on it",
    ] {
        if !transcript.contains(wanted) {
            eprintln!("uefi-boot: the boot transcript is missing {wanted:?}");
            ok = false;
        }
    }
    if transcript.contains("rsdp 0x0") {
        eprintln!("uefi-boot: the kernel was handed a zero ACPI root pointer, which is PVH's tell");
        ok = false;
    }
    // A table the walk could not reach is printed by `table_at` rather than skipped quietly, so
    // the gate can fail on it by name. This is the one failure that looks like a working boot from
    // outside: the tour completes, and the machine simply has no devices.
    if transcript.contains("outside the boot map") {
        eprintln!(
            "uefi-boot: an ACPI table was outside the boot map's reach, so this machine's APICs, \
             PCIe window or IOMMU went undiscovered"
        );
        ok = false;
    }

    ok
}

/// **The screen's first reading, milestone 243's tour**, reported; false when it was not there,
/// after saying whether the framebuffer or the handshake is to blame.
fn uefi_screen_showed_the_tour(screen: &ScreenReadings, shot: &Path) -> bool {
    let mut ok = true;
    match (&screen.tour, screen.held) {
        (Some(text), _) => {
            let rows = text.lines().filter(|l| !l.is_empty()).count();
            // Reported, not required: the kernel is stopped while this is read, so a shallow catch
            // means the tour had not finished painting. See `UEFI_SCREEN_MARKER`.
            let depth = if text.contains(UEFI_SCREEN_DEEP_MARKER) {
                "the whole tour, banner included: it never scrolled"
            } else {
                "its last page; the tour is taller than this screen and the serial transcript above has the rest"
            };
            eprintln!(
                "uefi-boot: read {rows} non-blank row(s) of the tour back off the framebuffer \
                 ({depth}), ending"
            );
            let tail: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
            for line in tail.iter().rev().take(3).rev() {
                eprintln!("uefi-boot:   | {line}");
            }
        }
        // Held, and the screen still did not show it: the boot was stopped between painting and
        // clearing, so the framebuffer path broke and nothing about timing is left to blame.
        (None, true) => {
            eprintln!(
                "uefi-boot: the kernel HELD the screen for this gate and the tour still was not on \
                 it, so the framebuffer path is what broke: the loader's LocateProtocol, the pixel \
                 order, the stride, or the mapping surviving mmu::init. This is not a missed \
                 window; the boot was stopped while the dump was taken. Last dump: {}",
                shot.display()
            );
            if let Some(text) = &screen.last_held {
                eprintln!("uefi-boot: what the held screen DID show:");
                for line in text.lines().filter(|l| !l.is_empty()) {
                    eprintln!("uefi-boot:   | {line}");
                }
            }
            ok = false;
        }
        // Never held: the boot never got that far, the loader lacks `screen_hold`, or the kernel
        // did not read the token. Blaming the framebuffer here sends a reader after working code.
        (None, false) => {
            eprintln!(
                "uefi-boot: the kernel never said it was holding the screen ({:?} is absent from \
                 the transcript above), so no dump was taken with the tour guaranteed to be on the \
                 framebuffer. This is the HANDSHAKE, not the framebuffer: check that the loader was \
                 built with the `screen_hold` feature (target/esp-screen, not target/esp), that \
                 `screen-hold` reached the kernel's boot command line, and that the boot got as far \
                 as the screen handover at all. Last dump: {}",
                boot_ladder::SCREEN_HELD,
                shot.display()
            );
            ok = false;
        }
    }
    ok
}

/// **The screen's last two readings, the shell's prompt and its answer to a serial command**,
/// reported; false when either never reached the screen.
fn uefi_screen_showed_the_shell(screen: &ScreenReadings, shot: &Path) -> bool {
    let mut ok = true;
    match (&screen.prompt, &screen.answer) {
        (Some(_), Some(text)) => {
            eprintln!(
                "uefi-boot: the shell is on the screen, and `{UEFI_SCREEN_COMMAND}` typed on the \
                 serial line answered there:"
            );
            for line in text.lines().filter(|l| !l.is_empty()) {
                eprintln!("uefi-boot:   | {line}");
            }
        }
        (Some(text), None) => {
            eprintln!(
                "uefi-boot: the prompt reached the screen, but `{UEFI_SCREEN_COMMAND}` typed on the \
                 serial line never answered there. Last screen read:"
            );
            for line in text.lines().filter(|l| !l.is_empty()) {
                eprintln!("uefi-boot:   | {line}");
            }
            ok = false;
        }
        (None, _) => {
            eprintln!(
                "uefi-boot: the shell's prompt never reached the screen. If the transcript has the \
                 prompt, the console server is not writing to the screen terminal; if it has the \
                 `served by framebuffer_driver` line, the terminal came up and drew nothing \
                 readable. Last dump: {}",
                shot.display()
            );
            ok = false;
        }
    }
    ok
}

/// The line the screen has to be showing for milestone 243 (a machine with no serial port) to have
/// worked: the self-test verdict, about twenty lines above the handover, at the tour's tail.
///
/// The tour is taller than OVMF's 1280x800 screen and scrolls, so the banner is gone by the
/// handover; held by `console::hold_screen_for_host`, the tour's last page is deterministic. The
/// screen proves the pixels (`LocateProtocol`, byte order, stride, the mapping surviving
/// `mmu::init`, the glyphs), and any decoded row proves all five; the self-test itself is asserted
/// on the serial transcript. See notes/x86-uefi-boot/running-under-firmware.md.
const UEFI_SCREEN_MARKER: &str = boot_ladder::SELF_TEST;

/// **The tour's first line, reported when the screen was tall enough to still be holding it.**
///
/// Not required, and never there on QEMU's 1280x800 OVMF console, where the tour scrolls. Reported
/// because it separates "the whole boot is on this screen" from "its last page is", so a machine
/// whose tour stops scrolling says so.
const UEFI_SCREEN_DEEP_MARKER: &str = boot_ladder::BANNER;

/// The command `uefi-boot` types on the serial line once the prompt is on the screen, and
/// [`UEFI_SCREEN_ANSWER`] the line it must print there. Words that appear nowhere in the boot, so
/// finding the answer cannot be finding something the tour said.
const UEFI_SCREEN_COMMAND: &str = "echo typed on the wire";
/// What [`UEFI_SCREEN_COMMAND`] prints, as a whole screen row.
const UEFI_SCREEN_ANSWER: &str = "typed on the wire";

/// What the screen showed, stage by stage. Each is the decoded screen at the moment that stage was
/// seen, or `None` when it never was.
#[derive(Default)]
struct ScreenReadings {
    /// **The kernel said it was holding the screen** (milestone 445), so a dump was taken while the
    /// tour was guaranteed to be on the framebuffer. False means the handshake never happened,
    /// which is a wiring failure rather than a framebuffer one and is reported as such.
    held: bool,
    /// The kernel's tour, with [`UEFI_SCREEN_MARKER`] on it (milestone 243).
    tour: Option<String>,
    /// **The last screen decoded while the kernel was holding it**, whether or not it carried the
    /// marker. Printed when [`ScreenReadings::tour`] is `None` and [`ScreenReadings::held`] is
    /// true, because that is the one case where the reader needs to see what WAS there: the boot
    /// was stopped, so this picture is the answer rather than a near miss.
    last_held: Option<String>,
    /// The shell's prompt, a row that starts `$ `.
    prompt: Option<String>,
    /// [`UEFI_SCREEN_ANSWER`] as a whole row, after [`UEFI_SCREEN_COMMAND`] was typed.
    answer: Option<String>,
}

/// **The firmware screen read as the shell's terminal draws it**: `glyph_atlas` cells in
/// `video_terminal`'s default colors, at the scale `display_service::start_screen_terminal` chose
/// for the screen's width, one string per row with its trailing blanks trimmed. The tour before
/// the hand-over is the boot console's 7x8 bitmap, which `serial_console::screen::read` reads; this
/// reads what replaces it.
fn terminal_text(ppm: &[u8]) -> Option<String> {
    let width = serial_console::screen::decode_exact(ppm, u32::MAX, u32::MAX, 1, b"", |_, _, _| 0)
        .ok()?
        .width;
    let scale = screen_console::ScreenConsole::scale_for_cell(width, video_terminal::CELL_W);
    let printable: Vec<u8> = (0x20..=0x7e).collect();
    let (fg, bg) = video_terminal::Attr::DEFAULT.colours();
    let screen = serial_console::screen::decode_exact(
        ppm,
        video_terminal::CELL_W,
        video_terminal::CELL_H,
        scale,
        &printable,
        |ch, x, y| glyph_atlas::REGULAR.cell_pixel(ch, x, y, fg, bg),
    )
    .ok()?;
    let rows: Vec<&str> = screen.rows.iter().map(|r| r.trim_end()).collect();
    Some(rows.join("\n"))
}

/// **Ask the kernel to hold still, then read three stages of what the screen shows**, from the
/// monitor at `sock` into `shot`, typing on `serial` (COM1) and watching `wire`.
///
/// 1. The tour, held: on [`boot_ladder::SCREEN_HELD`] in `wire` it dumps and decodes, then writes
///    a byte to release the boot (milestone 445). Nothing falls back to sampling.
/// 2. The prompt, a row starting `$ `, sampled because it is a state and not an event.
/// 3. The answer: [`UEFI_SCREEN_COMMAND`] typed, and a row equal to [`UEFI_SCREEN_ANSWER`].
///
/// On its own thread, because the runner returns only after QEMU is gone. Stops when the monitor
/// stops answering, or at a deadline. See notes/x86-uefi-boot/running-under-firmware.md.
fn screen_watch(
    sock: &str,
    shot: &Path,
    mut serial: std::process::ChildStdin,
    wire: &std::sync::Mutex<String>,
) -> ScreenReadings {
    use std::io::Write;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(180);
    let mut answered = false;
    let mut seen = ScreenReadings::default();
    let mut typed = false;
    let mut released = false;
    /// How many dumps taken while the kernel is holding the screen may fail to show the tour
    /// before the boot is released anyway. Each costs about 650 ms measured, so four is well inside
    /// the kernel's ten-second bound. `screendump` waits for QEMU to finish writing the file, so the
    /// retries are margin, kept and not re-measured.
    const HELD_DUMPS: u32 = 4;
    let mut dumps_while_held = 0u32;
    while std::time::Instant::now() < deadline {
        // The wire is read before the picture is taken: a dump taken before the hold line arrived
        // shows the earlier screen, and believing it would be the original race again.
        if !seen.held {
            seen.held = wire
                .lock()
                .map(|t| t.contains(boot_ladder::SCREEN_HELD))
                .unwrap_or(false);
        }
        if !screendump(sock, shot) {
            if answered {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
            continue;
        }
        answered = true;
        if seen.held {
            dumps_while_held += 1;
        }
        // A screen that lacks what the stage wants is not stored, so it cannot read as a pass;
        // the dump file is the artifact the caller names on failure.
        let bytes = std::fs::read(shot).ok();
        let text = bytes
            .as_ref()
            .and_then(|bytes| serial_console::screen::read(bytes).ok());
        // The shell's half is drawn by `display_terminal`, in its own font, not the tour's.
        let shell = bytes.as_ref().and_then(|bytes| terminal_text(bytes));
        if let Some(shell) = &shell {
            if seen.prompt.is_none() && shell.lines().any(|l| l.starts_with("$ ")) {
                seen.prompt = Some(shell.clone());
            }
            if typed && shell.lines().any(|l| l == UEFI_SCREEN_ANSWER) {
                seen.answer = Some(shell.clone());
                break;
            }
        }
        if let Some(text) = text {
            if seen.held {
                if seen.tour.is_none() && text.contains(UEFI_SCREEN_MARKER) {
                    seen.tour = Some(text.clone());
                }
                if !text.trim().is_empty() {
                    seen.last_held = Some(text.clone());
                }
            }
            // Released once the tour is read or `HELD_DUMPS` held dumps missed it: the held state
            // is stable, so a miss is a real answer, and a wedged boot says less than a finished
            // one. Any byte does; the kernel drains it so the line editor finds no stray keystroke.
            if seen.held && !released && (seen.tour.is_some() || dumps_while_held >= HELD_DUMPS) {
                let _ = writeln!(serial);
                released = serial.flush().is_ok();
            }
            // Typed once, after the prompt is on the screen: the line editor echoes a keystroke
            // the moment it arrives, so typing earlier would be typing into the boot.
            if seen.prompt.is_some() && !typed {
                typed = writeln!(serial, "{UEFI_SCREEN_COMMAND}")
                    .and_then(|()| serial.flush())
                    .is_ok();
            }
        }
        // Short enough to spend few of the hold's ten seconds.
        let pause = 250;
        std::thread::sleep(std::time::Duration::from_millis(pause));
    }
    seen
}

/// `cargo xtask uefi-test [--test <filter> [--repeat <N>]] [--stage-only [--image <package>]]`.
///
/// `--test` and `--repeat` mean what they mean to `cargo xtask test`. `--stage-only` builds one
/// test image (`system_tests` unless `--image kernel`) into `target/esp-test` and boots nothing,
/// which is how a bench stick gets a test kernel: a filtered, repeated run is the one boot a bench
/// visit buys (milestone 321 (a corpse that did not park, on four real cores)). `--stage-only` and
/// `--image` are provisional names, after `disk-throughput --stage-only`.
pub(crate) fn uefi_test_command() -> bool {
    let filter = crate::suite::export_test_filter();
    if !crate::suite::export_test_repeat(filter.as_deref()) {
        return false;
    }
    if !std::env::args().any(|a| a == "--stage-only") {
        return uefi_test();
    }
    let package = crate::host::flag_value("--image").unwrap_or_else(|| "system_tests".into());
    if package != "system_tests" && package != "kernel" {
        eprintln!("uefi-test: --image is `system_tests` or `kernel`, not {package:?}");
        return false;
    }
    if !prepare_uefi_test() {
        return false;
    }
    let Some(kernel) = kernel_test_elf(&package, X86_TARGET, "uefi-test") else {
        return false;
    };
    if !uefi_stage(
        &kernel,
        &uefi_test_esp_dir(),
        "the loader, the test kernel and the archive",
        false,
    ) {
        return false;
    }
    eprintln!(
        "uefi-test: staged the {package} test image under {}; nothing booted (--stage-only)",
        uefi_test_esp_dir().display()
    );
    true
}

/// **Run both kernel test images under real firmware** (milestone 195), rather than the tour
/// `uefi_boot` boots: the tests pass when the memory map, ACPI root and PCIe window came from
/// firmware. Returns whether both passed.
///
/// Both the transcript and the exit status are checked: a passing run is status 3
/// ([`X86_DEBUG_EXIT_SUCCESS`]). The scan alone would pass a harness that faulted after its
/// verdict, and the status alone a QEMU that never started the guest.
///
/// # BUGS
/// - A second firmware boot, not a replacement: `uefi-boot` still boots the shipping image.
pub(crate) fn uefi_test() -> bool {
    if !prepare_uefi_test() || !mkdisk() || !mknvmedisk() {
        return false;
    }
    // The kernel's unit tests and the whole-system suite are separate images (milestone 609 (the
    // system tests leave the kernel crate)).
    uefi_test_image("kernel") && uefi_test_image("system_tests")
}

/// What every test image under firmware is built with, booted here or staged for a stick.
fn prepare_uefi_test() -> bool {
    // A test boot gets the confinement attackers however it is reached: the system_tests image
    // asserts the DMA-escape role in the archive's NVMe server, and a bare `uefi-test` needs the
    // archive `test` packs. Switching to and from any other boot recompiles `components`, since
    // cargo fingerprints features. See `CONFINEMENT_ATTACKERS`.
    crate::CONFINEMENT_ATTACKERS.store(true, std::sync::atomic::Ordering::Relaxed);
    // **The OVMF runner attaches one block fixture, not the PVH runner's five**
    // (helpers/qemu-uefi-x86_64.sh, BUGS), so the test image built for this boot must not grade
    // the roster against five. The `x86_64` leg sets this for the PVH suite; the rebuild between
    // the two is `system_tests/build.rs` re-running on the changed variable.
    // SAFETY: `remove_var` became unsafe in edition 2024 because it races other threads. xtask is
    // single-threaded here: this runs on the main thread before the child that reads it is
    // spawned, and the only thread xtask ever starts (the transcript reader in swish_check_leg)
    // copies pipe bytes into a String and never touches the environment.
    unsafe { std::env::remove_var("NIFE_X86_FIXTURES_REQUIRED") };
    initial_ramdisk_x86()
}

/// One test image's boot under OVMF, and its verdict: [`uefi_test`] runs it for each image.
fn uefi_test_image(package: &str) -> bool {
    let Some(kernel) = kernel_test_elf(package, X86_TARGET, "uefi-test") else {
        return false;
    };
    if !uefi_stage(
        &kernel,
        &uefi_test_esp_dir(),
        "the loader, the test kernel and the archive",
        false,
    ) {
        return false;
    }
    eprintln!();
    eprintln!("--- {package} tests under real firmware, x86_64 (QEMU q35 + OVMF) ---");

    let mut command = Command::new("helpers/qemu-uefi-x86_64.sh");
    command
        .arg(uefi_test_esp_dir())
        .current_dir(workspace_root())
        // The PVH runner's devices (milestone 195), so a driver reaches BARs firmware placed
        // rather than this kernel's bus walk. One core, the runner's default, until the suite is
        // measured at two under OVMF. Named here for a bare `uefi-test`; `test()` already sets them.
        .env("NIFE_DISK", disk_path())
        .env("NIFE_NVME", nvme_disk_path());
    // The GPU and keyboard ride only the system image, whose tests use them (milestone 420 (x86_64
    // has one fixture of five and no NIC, GPU, keyboard or RNG)). The referee is constructed before
    // the child, because it sets `NIFE_GPU_MON`.
    let mut referee = None;
    if package == "system_tests" {
        command.env("NIFE_GPU", "1").env("NIFE_KEYBOARD", "1");
        referee = Some(ScanoutReferee::for_leg("x86_64", "x86_64-ovmf"));
    } else {
        command
            .env_remove("NIFE_GPU")
            .env_remove("NIFE_GPU_MON")
            .env_remove("NIFE_KEYBOARD");
    }
    // A hang detector, well above an honest run: the `system_tests` boot takes about 190 s in CI
    // on the 1120x624 scanout. A caller's own `NIFE_UEFI_TIMEOUT` wins. See
    // notes/x86-uefi-boot/running-under-firmware.md, "Inside the two gates".
    if std::env::var_os("NIFE_UEFI_TIMEOUT").is_none() {
        command.env("NIFE_UEFI_TIMEOUT", "300");
    }
    // Timed and recorded (milestone 807 (the kernel suite reports what each test cost)); stderr is
    // QEMU's own, never the guest console, so it goes straight to ours and not into what the checks
    // below read.
    let (mut child, watch) = match crate::time_record::Watch::spawn(&mut command) {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("uefi-test: failed to run helpers/qemu-uefi-x86_64.sh: {e}");
            return false;
        }
    };
    let who = format!("x86_64 ovmf {package}");
    let status = match referee.as_mut() {
        None => watch.wait(&mut child, &who),
        Some(referee) => loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) => {}
                Err(e) => {
                    eprintln!("waiting for the test child failed: {e}");
                    break None;
                }
            }
            if watch.enforce(&mut child, &who) {
                break None;
            }
            referee.poll();
            std::thread::sleep(Duration::from_millis(100));
        },
    };
    let (lines, exited) = watch.finish();
    let transcript: String = lines.iter().map(|l| l.text.clone() + "\n").collect();
    let code = status.and_then(|s| s.code());
    let passed = code == Some(i32::from(X86_DEBUG_EXIT_SUCCESS));
    let accounted = crate::time_record::account("x86_64 ovmf", package, &lines, exited, passed);

    let mut ok = true;
    // `(xsdt)` and the ECAM line are `uefi_boot`'s own assertions, repeated here rather than
    // assumed: this is a different binary, and a test build that took the PVH fallback path would
    // otherwise report a green suite on a machine it had discovered the wrong way.
    for wanted in ["test result: ok.", "(xsdt)", "pci         : ecam at"] {
        if !transcript.contains(wanted) {
            eprintln!("uefi-test: the boot transcript is missing {wanted:?}");
            ok = false;
        }
    }
    if transcript.contains("rsdp 0x0") {
        eprintln!("uefi-test: the kernel was handed a zero ACPI root pointer, which is PVH's tell");
        ok = false;
    }
    // 3, not 0: see the doc comment. `helpers/qemu-uefi-x86_64.sh` execs QEMU rather than
    // translating this the way the PVH runner does, because that runner is a cargo `runner` and has
    // to speak cargo's success convention while this one has only ever had one caller.
    if !passed {
        eprintln!(
            "uefi-test: the {package} boot failed: qemu exited {code:?}, not \
             {X86_DEBUG_EXIT_SUCCESS}, after {} s",
            exited.as_secs()
        );
        for line in exit_verdict_lines(code, exited, &transcript) {
            eprintln!("uefi-test:   {line}");
        }
        ok = false;
    }
    if !accounted {
        ok = false;
    }
    // Graded whatever the guest said, as `scanout_check` does: a red suite is the run where the
    // referee's account of what reached the scanout matters most. Advisory under `--test`, for
    // `scanout_check`'s reason: a filter may not have selected the tests that paint.
    let filtered = std::env::var_os("NIFE_TEST_FILTER").is_some_and(|v| !v.is_empty());
    if let Some(referee) = referee
        && !referee.report()
        && !filtered
    {
        ok = false;
    }
    if ok {
        eprintln!("uefi-test: the {package} suite passed under OVMF");
    }
    ok
}

/// **At a failed boot, say which exit path produced `code`, and retire the VT-d theory.** Returns
/// the lines to print; `exited` and `transcript` separate the readings.
///
/// `1` is both the guest's failure write and QEMU's own error exit. The device reports
/// `(written << 1) | 1`, so its statuses are odd. Past the summary the one writer of the failure
/// code is the panic arm, which prints `[PANIC]` first (`kernel/src/panic.rs`). QEMU exits 0 on
/// the SIGTERM that the `NIFE_UEFI_TIMEOUT` bound and the host watchdog send (Homebrew QEMU 11.1.2,
/// 2026-10-15), and 1 with its reason on stderr when it fails itself. Name provisional.
fn exit_verdict_lines(code: Option<i32>, exited: Duration, transcript: &str) -> Vec<String> {
    let mut lines = vec![
        // The escape test's expected faults (milestone 215 (a PCI function's interrupt reaches
        // nothing on x86_64)): the masked fault-event interrupt (this kernel polls) and the
        // escape's translation failure. They invite a wrong theory about QEMU versions or firmware
        // in a red run's output.
        "the qemu-system-x86_64 VT-d lines in this run's output (`Interrupt Mask set, irq is not \
         generated`, `vtd_iommu_translate: detected translation failure`) are the escape test's \
         own expected faults (milestone 215), not a cause"
            .to_string(),
    ];
    match code {
        // The host watchdog (or a failed wait) ended the child, and printed its own line naming
        // the stage that overran. Nothing here can add to it.
        None => lines.push(
            "the boot was ended for us; the HOST WATCHDOG line above names the stage. A hang, \
             not an exit the guest chose"
                .to_string(),
        ),
        // The guest's failure write and QEMU's own error exit share this number, so the
        // transcript decides: a green summary means the write came after the suite's own verdict.
        Some(1) => {
            if transcript.contains("test result: ok.") {
                lines.push(
                    "1 is the status the failure code produces (the guest writes 0, the device \
                     reports (0 << 1) | 1), and the transcript shows the suite's green summary, \
                     so it was written after that verdict. In this tree one writer does that: the \
                     panic arm, which prints `[PANIC]` first; a transcript with no `[PANIC]` \
                     points instead at QEMU's own error exit, whose reason would be on stderr \
                     above"
                        .to_string(),
                );
            } else {
                lines.push(
                    "1 is the status the failure code produces (the guest writes 0) and also \
                     QEMU's own error exit. No summary in the transcript, so either the guest \
                     wrote failure before reaching one, or QEMU failed before or under the \
                     guest; QEMU's reason, if any, is on stderr above"
                        .to_string(),
                );
            }
        }
        // Not the device's: whatever ended QEMU, the guest's exit write is not it. The elapsed
        // time separates a bound that spent its whole 300 s from a boot that stopped early.
        Some(0) => lines.push(format!(
            "0 is not a status isa-debug-exit can produce (its statuses are all odd): the \
             guest's exit write is not what ended this boot, which ran {} s. The \
             NIFE_UEFI_TIMEOUT bound lands here, because QEMU exits 0 on the SIGTERM that ends \
             it; so does a reset the guest or firmware asked for, which -no-reboot turns into \
             an exit rather than a loop",
            exited.as_secs()
        )),
        // Odd and not 1 or 3: the device produced it, and the guest wrote a code this harness
        // never writes. That is an anomaly in its own right and is worth the number spelled out.
        Some(c) if c % 2 == 1 => lines.push(format!(
            "the guest wrote {} through isa-debug-exit, which is neither of the harness's two \
             codes (success writes 1, failure writes 0)",
            c >> 1
        )),
        // Even: the device cannot produce it, so QEMU was ended from the host side without its
        // cooperation; 137 is the killer's SIGKILL after a SIGTERM QEMU did not act on.
        Some(c) => lines.push(format!(
            "{c} is not a status isa-debug-exit can produce (its statuses are all odd): QEMU \
             was ended from the host side"
        )),
    }
    lines
}

/// The process status a passing `x86_64` suite produces, which is not zero and cannot be.
///
/// `isa-debug-exit` terminates QEMU with `(value << 1) | 1`, so every status it can report is odd.
/// The guest writes 1; QEMU exits 3. The matching half is `EXIT_SUCCESS` in
/// `kernel/src/arch/x86_64/semihosting.rs`, and `helpers/qemu-runner-x86_64.sh` translates the same
/// number for cargo's benefit. Three files naming one number is the cost of a convention QEMU owns.
const X86_DEBUG_EXIT_SUCCESS: u8 = 3;

#[cfg(test)]
mod exit_verdict_tests {
    use super::*;

    fn lines(code: Option<i32>, transcript: &str) -> Vec<String> {
        exit_verdict_lines(code, Duration::from_secs(41), transcript)
    }

    #[test]
    fn every_failure_says_the_vtd_lines_are_expected() {
        for code in [None, Some(0), Some(1), Some(7), Some(137)] {
            let lines = lines(code, "");
            assert!(
                lines[0].contains("milestone 215"),
                "code {code:?}: first line must retire the VT-d theory, got {:?}",
                lines[0]
            );
        }
    }

    #[test]
    fn a_one_after_a_green_summary_points_after_the_verdict() {
        let lines = lines(Some(1), "test result: ok. 277 passed, 101 skipped\n");
        assert!(
            lines.iter().any(|l| l.contains("after that verdict")),
            "{lines:?}"
        );
        assert!(lines.iter().any(|l| l.contains("`[PANIC]`")), "{lines:?}");
    }

    #[test]
    fn a_one_without_a_summary_names_qemus_own_error_exit() {
        // The transcript opens with the loader's real banner; the boot it names is milestone 87
        // (the x86_64 bare-metal machine).
        let lines = lines(Some(1), "nife uefi_loader: milestone 87\n");
        assert!(
            lines.iter().any(|l| l.contains("QEMU's own error exit")),
            "{lines:?}"
        );
    }

    #[test]
    fn zero_is_reported_as_not_the_devices() {
        let lines = lines(Some(0), "");
        assert!(
            lines
                .iter()
                .any(|l| l.contains("not a status isa-debug-exit can produce")),
            "{lines:?}"
        );
        // The bound and a reset share this shape, so the line must give the reader the elapsed
        // time that separates them.
        assert!(
            lines
                .iter()
                .any(|l| l.contains("NIFE_UEFI_TIMEOUT") && l.contains("41 s")),
            "{lines:?}"
        );
    }

    #[test]
    fn an_odd_status_decodes_the_value_the_guest_wrote() {
        let lines = lines(Some(7), "");
        assert!(
            lines.iter().any(|l| l.contains("the guest wrote 3")),
            "{lines:?}"
        );
    }

    #[test]
    fn a_watchdog_kill_defers_to_the_watchdogs_own_line() {
        let lines = lines(None, "test result: ok. 277 passed, 101 skipped\n");
        assert!(
            lines.iter().any(|l| l.contains("HOST WATCHDOG")),
            "{lines:?}"
        );
    }
}

#[cfg(test)]
mod tests {
    /// **The OVMF gate reads the shell where the terminal draws it.** A `Vt` laid out over QEMU's
    /// 1280x800 OVMF screen, as `display_service::start_screen_terminal` lays it out, is painted
    /// into a screendump; [`super::terminal_text`] must find the prompt and the answer rows the
    /// gate waits for. A reader of the boot console's 7x8 bitmap finds neither.
    #[test]
    fn the_shell_on_the_ovmf_screen_reads_back() {
        let (w, h) = (1280u32, 800u32);
        let mut vt =
            video_terminal::Vt::new(w / video_terminal::CELL_W, h / video_terminal::CELL_H);
        vt.feed(format!("$ echo x\r\n{}\r\n$ ", super::UEFI_SCREEN_ANSWER).as_bytes());
        let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
        for y in 0..h {
            for x in 0..w {
                let p = vt.pixel(x, y);
                ppm.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8]);
            }
        }
        let text = super::terminal_text(&ppm).expect("a readable dump");
        assert!(text.lines().any(|l| l.starts_with("$ ")), "{text}");
        assert!(
            text.lines().any(|l| l == super::UEFI_SCREEN_ANSWER),
            "{text}"
        );
    }
}

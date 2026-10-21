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

/// **Build the bootable UEFI image** (milestone 87): the kernel, the userspace archive, and the
/// loader that carries both, staged where firmware looks for them.
///
/// ```text
/// cargo xtask uefi-image
/// # then, under QEMU with real firmware:
/// helpers/qemu-uefi-x86_64.sh target/esp
/// # or, on the Dell OptiPlex: copy target/esp/EFI/BOOT/BOOTX64.EFI to a FAT32 stick, same path.
/// ```
///
/// **The order is a dependency, not a preference.** `uefi_loader` embeds the kernel ELF and the
/// archive with `include_bytes!`, so both have to exist and be current before it is compiled; its
/// build script takes their paths from the environment and refuses to build without them, rather
/// than guessing at `target/`. That refusal is the mechanism keeping a stale `.efi` from being
/// possible to produce by hand.
///
/// # BUGS
///
/// - **`BOOTX64.EFI` is the removable-media fallback path**, which is what a USB stick uses and
///   what OVMF finds with no configuration. Installing to the machine's own ESP with a boot entry
///   of its own (`efibootmgr`'s job on Linux) is not done here, and is what a machine that boots
///   nife by default would need.
///
/// `--features <list>` builds the kernel with those cargo features, the one way to put a
/// non-default kernel on xenon's stick. Written by the watchdog soak's lane (pull request #1301, a
/// wedged kernel resets itself) for `--features watchdog_soak_test`, and landed by milestone 563 (a
/// seal check that reads bytes cannot see a check that was dropped), the change that made a soak
/// kernel seal at all. Any kernel feature works the same way; one whose boot enters no archive
/// program through `trust::require_program` or the hand-over will still be refused as `NOT SEALED`.
///
/// **Only the `uefi-image` subcommand reads the command line** ([`uefi_image_command`]). The other
/// callers (`swish-check`'s `x86_64` leg, the stick) call this with no features, because the
/// arguments on *their* command line are theirs: `script/swish-check --arch x86_64` handed
/// `--arch x86_64` to this parser and failed with its usage line from 915a60c3d until 2026-09-26,
/// found by milestone 198 (a package manager) rung 3a's image lane.
pub(crate) fn uefi_image() -> bool {
    uefi_image_with(None)
}

/// `cargo xtask uefi-image [--features <list>]`: the one caller whose arguments are this
/// function's.
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
/// inside requires, returning the kernel's path. Split out of [`uefi_image`] by milestone 445 so
/// that the screen gate can stage the same kernel behind a differently-built loader without
/// duplicating that order, which is the one thing here nobody may get wrong.
pub(crate) fn uefi_kernel(features: Option<&str>) -> Option<String> {
    // **The archive FIRST, then the kernel, and the order is load-bearing.** Packing the archive
    // regenerates `target/init-measure-x86_64.txt`, the manifest `kernel/build.rs` compiles in as
    // the measured-boot trust root. Kernel-first builds a kernel vouching for the PREVIOUS archive,
    // and the gate then refuses the pair at the point of handover:
    //
    //     MEASURED BOOT REFUSED: no measurement for the archive entry 'progenitor'
    //
    // **This is the second time this defect has reached a bench**, which is why the comment is here
    // rather than in a note. `script/board-image` had it for riscv64 and the VisionFive 2 refused
    // the pair on 2026-08-15 (boot 12); the fix there carries a comment saying "QEMU never hit it
    // because xtask orders these correctly", which was true of the riscv64 path and false of this
    // one. xenon refused the pair the same way on 2026-09-17, after its self-test passed.
    //
    // QEMU does not catch it because a developer who runs the kernel and the archive from one
    // working tree usually has both fresh; it bites when the kernel is already built, which is
    // every time a lane has compiled it earlier in the session.
    //
    // `initial_ramdisk_x86` builds `components`, never the kernel, so the dependency runs one way
    // only and this order is the safe one as well as the correct one.
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
    // **A release image embeds a kernel with its DWARF removed**, calef's ruling of 2026-10-03 UTC
    // on the install boot slots. The release kernel ELF is about 7.5 MB, 5.8 MB of it `.debug_*`
    // that nothing in the loader or the kernel reads (symbolisation is offline, against the
    // unstripped file that stays in `target/`). Stripping happens HERE, before the loader embeds
    // the file, so the seal (`uefi_loader/build.rs`'s `refuse_an_unsealed_pair`, which looks for the
    // archive's digests in the kernel's `.rodata`) is taken over the bytes that ship; `--strip-debug`
    // never touches `.rodata`. Same tool and flag the archive's packer uses.
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
/// A third directory, for the reason [`uefi_test_esp_dir`] is a second one, and the entry in
/// `uefi_loader`'s `Cargo.toml` says what it costs to get this wrong. The loader here is built with
/// `screen_hold`, so the kernel it hands over to stops at the screen handover and waits up to ten
/// seconds for a byte on the serial line. That is exactly right for a gate holding a camera and
/// exactly wrong for a stick in somebody's pocket, and `esp_dir()` is the one the bench procedure
/// copies from.
///
/// **The kernel is byte-identical to `esp_dir()`'s.** Only the loader differs, and only by one word
/// on the command line it writes, which is the whole reason the knob is a boot-time token rather
/// than a build of the kernel: the binary under test is the binary that ships.
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
    // Not `cargo()`: that helper exports the aarch64 runner's
    // `NIFE_INITIAL_RAMDISK`/`NIFE_DISK`/`NIFE_NET`, none of which means anything to a UEFI build,
    // and the two archive variables would then differ by one character in a way nothing would
    // catch.
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
/// This is the gate, and what it asserts is chosen so that it cannot pass for the wrong reason:
///
/// - **`(xsdt)` in the ACPI line.** Under QEMU's PVH loader the RSDP is found by *scanning* the
///   BIOS area, and what turns up is an ACPI 1.0 pointer with an RSDT root. Real firmware hands
///   over a revision-2 RSDP with an XSDT. So this string is only printable if the loader read the
///   UEFI configuration table and the kernel walked the 64-bit root, which is a path that had never
///   executed before this milestone.
/// - **no `rsdp 0x0`.** The PVH handoff's own tell, and the thing this loader exists to fix.
/// - **the tour's completion line.** Everything between the two: the fine page tables, the APIC,
///   the timer, the scheduler and two ring-3 processes, on a memory map from firmware rather than
///   from a hypervisor.
/// - **the ECAM window enabled from the MCFG** (milestone 165). The runner boots at 2 GiB rather
///   than the PVH runner's 256 MiB, which is what puts firmware's tables above 1 GiB, where a real
///   machine keeps them. On 2026-09-02 that boot found no ACPI at all, because the walk's reach
///   bound disagreed with `boot.s` by 4x; asserting the *end* of the chain (a PCIe window whose
///   base came from a table read at a high physical address) is what makes the whole chain a gate
///   rather than the three separate facts it is made of.
///
/// - **the shell on the screen, answering a serial keystroke** (the shell on the firmware screen,
///   milestone 198's rung 1b). The screen is read three times: the kernel's tour (milestone 243),
///   the prompt once the userspace terminal has taken the screen over, and the answer to a command
///   typed on COM1. See [`screen_watch`].
///
/// The boot is bounded by the runner script; a kernel that hangs fails this by producing none of
/// the three rather than by hanging the gate. It is stopped as soon as the screen has answered.
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

    // **The screen half** (milestone 243). A QEMU monitor on a unix socket, a poller that asks it
    // for a screendump until the tour's last line is *on the screen*, and `board_console::screen` to
    // turn the picture back into text. Everything the framebuffer path can get wrong shows up here
    // and nowhere else: the loader's `LocateProtocol`, the byte order, the stride, the mapping
    // surviving `mmu::init`, and the glyphs. The serial transcript below would be identical if the
    // screen were black.
    //
    // **And then the shell on it** (the shell on the firmware screen, milestone 198's rung 1b): the
    // same poller keeps reading until the prompt is on the screen, types one command on the SERIAL
    // line, and reads its answer back off the SCREEN. That is the whole rung in one exchange: the
    // console server writes both surfaces, the keystrokes still arrive over COM1, and what reaches
    // the monitor is the shell and not the kernel. So it needs the runner's stdin, which is COM1.
    //
    // In /tmp rather than under target/, because a unix socket path is capped at 104 bytes by the
    // OS and a worktree checkout plus `target/` gets close. Same reason `gpu_shot` does it.
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
    // **Stop the machine once the screen has answered**, rather than waiting out the runner's
    // bound: the kernel never exits, and everything asserted below has been printed by the time the
    // prompt answered a command. SIGTERM to the wrapper, which is the signal `qemu-bounded.sh`
    // forwards to QEMU; its own killer is the backstop if this is lost.
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
        // **Two cores** (milestone 195), where every other x86_64 boot in this tree takes one.
        // `arch::x86_64::ap_boot` copies its real-mode trampoline to physical 0x8000, a page no
        // loader had ever asked the firmware for, so secondary cores under firmware worked or did
        // not by luck. `uefi_loader` asks for it by name now and this is what checks that the
        // asking worked. It also puts a second local APIC on the machine, which is the third thing
        // milestone 215's BUGS listed as answerable only on xenon: whether a machine with more than
        // one still delivers device interrupts to the boot core's id. The tour's `device irq` line
        // is that answer.
        //
        // The suite leg (`uefi-test`) keeps the runner's one-core default, and the split is a
        // measured boundary rather than a dodge: the two defects that once made two cores flake
        // are fixed (milestone 316's `boot_cpu_id` record, and `cpu_start`'s count re-read after
        // the STARTUP IPIs, fixed 2026-09-19), the tour above gates the second core on every
        // boot (`smp: 2 core(s) online` below; zero misses across the dedicated measurement,
        // notes/uefi-boot-two-core-rate.md), and the whole suite at two cores under OVMF has not
        // been measured since those fixes, so that default stands until it is.
        .env("NIFE_SMP", "2")
        // **The bare firmware machine, whether or not a suite leg ran first.** `test()` sets
        // `NIFE_DISK` and `NIFE_NVME` in this process for the PVH leg, and a child inherits them,
        // so without this the tour would attach two devices inside `script/test` and none from a
        // bare `cargo xtask uefi-boot`. One boot with two machines is one boot nobody can compare.
        // The suite below is where the devices belong.
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
    // Both streams collected on threads of their own, because the watcher below decides when the
    // boot has said enough and a blocking read here would decide it instead.
    //
    // **Into a shared buffer rather than only into a return value** (milestone 445). The watcher
    // now waits for a line the kernel prints on the SERIAL wire (`boot_ladder::SCREEN_HELD`), so it
    // has to see the transcript as it arrives; a `read_to_end` that only yields at the join is a
    // transcript nobody can read until the boot is over. The join still returns the whole of it.
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
        // The self-test verdict and the hand-over, rather than the halt line this used to want:
        // milestone 182 made x86_64 hand the machine to the progenitor instead of halting, and
        // this boot carries the archive (`uefi_stage`'s `NIFE_UEFI_INITIAL_RAMDISK`), so under real
        // firmware it reaches both.
        boot_ladder::SELF_TEST,
        "nife: handing the system to the userspace progenitor.",
        "(xsdt)",
        "pci         : ecam at",
        // Two cores ONLINE, not two in the MADT: the difference is whether the trampoline page the
        // loader asked for was actually usable. See the `NIFE_SMP` comment above.
        "smp: 2 core(s) online",
        // And a device interrupt still landing on the boot core with two local APICs present.
        "device irq  : pit irq 0 -> gsi 2",
        // The kernel handed the screen to a userspace terminal and both halves came up (the shell
        // on the firmware screen). Said on the UART because the screen has been handed away by
        // the time it is printed.
        // The terminal fills the screen at the boot console's rule for its 14x26 cell: OVMF's
        // 1280x800 is scale one, 91 columns by 30 rows.
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
            // How deep the dump caught the tour is reported and not required. It was a race against
            // the handover's clear until milestone 445; now the kernel is stopped while this is
            // read, so a shallow catch means the tour genuinely had not finished painting. See
            // `UEFI_SCREEN_MARKER`.
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
        // **Held, and the screen still did not show it: the framebuffer path is what broke.** This
        // is the verdict milestone 445 bought. The kernel was stopped between painting its tour and
        // clearing it, with the marker on the wire, and a dump taken in that window decoded to
        // something that is not the tour. Nothing about timing is left to blame.
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
        // **Never held: the handshake did not reach the kernel, which is a different afternoon.**
        // Either the boot never got that far (the serial transcript above says), or the loader was
        // built without `screen_hold`, or the kernel did not read the token. Saying "the tour was
        // never on the screen" here is what cost a reader an afternoon on 2026-09-20, chasing five
        // things in the framebuffer path that were all working.
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
/// worked.
///
/// **It is the self-test verdict, the TAIL of the tour, and putting it back there is a correction
/// of a correction** (milestone 445, 2026-09-20, hours after the change it reverses).
///
/// The marker was the self-test verdict originally. Milestone 243's second lane moved it to
/// [`boot_ladder::BANNER`], the tour's first line, on a measurement that said the tour "tops out at
/// 98 non-blank rows" against a screen of 100, so nothing scrolls and the banner stays up until the
/// handover clears it. **That premise is false, and this lane watched it be false.** With the
/// kernel stopped at the handover and the framebuffer photographed while it was held, the screen
/// read 95 rows whose FIRST line was
///
/// ```text
///                 0x00007ea8a000..0x00007eab4000  ram
/// ```
///
/// which is the middle of the firmware memory map. The tour is longer than the screen on this
/// machine, so it scrolls, and by the handover the banner is gone. What the old sampler was
/// actually catching was the banner **early in the boot**, before the scroll, which is a window at
/// the other end of the tour from the one its own comment described. That is why a loaded machine
/// read zero rows: the first dump landed after the scroll rather than after the clear.
///
/// So the marker goes back to the tail, where the handshake makes it deterministic: with
/// `console::hold_screen_for_host` stopping the boot, whatever the tour's last page is, is on the
/// screen, and the self-test verdict is about twenty lines above the handover.
///
/// **The total claim is unchanged**, which is the part worth checking before believing any of this.
/// The screen's job is to prove the *pixels*: the loader's `LocateProtocol`, the byte order, the
/// stride, the mapping surviving `mmu::init`, and the glyphs. Any decoded row proves all five. That
/// the boot got as far as the self-test is asserted separately and unconditionally, on the SERIAL
/// transcript, a few dozen lines above this one.
///
/// It was the halt line, `nife x86_64: boot complete, halting.`, until milestone 182 removed the
/// halt.
const UEFI_SCREEN_MARKER: &str = boot_ladder::SELF_TEST;

/// **The tour's first line, reported when the screen was tall enough to still be holding it.**
///
/// Not required, and on QEMU's 1280x800 OVMF console it is never there: the tour is longer than the
/// hundred rows that screen holds. It is worth reporting because it is the one thing that separates
/// "the whole boot is on this screen" from "the last page of it is", and because a machine whose
/// tour stops scrolling (a taller screen, a shorter tour) should say so rather than have somebody
/// rediscover it. See [`UEFI_SCREEN_MARKER`] for the measurement.
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

/// **Ask the kernel to hold still, then read three stages of what the screen shows**
/// (milestone 243, then the shell on the firmware screen, then milestone 445).
///
/// Runs on its own thread beside the boot, because the kernel never exits and the runner therefore
/// does not return until its own timeout fires: by then QEMU is gone and there is nothing left to
/// photograph. The stages, in the order they must happen:
///
/// 1. **The tour, while the kernel is holding it there.** This used to be a race and is now a
///    handshake, which is milestone 445 and calef's choice among three options on 2026-09-20. The
///    kernel prints [`boot_ladder::SCREEN_HELD`] on the serial wire and waits for a byte before it
///    clears the framebuffer and hands it to the userspace terminal, so the dump taken between
///    those two events cannot miss the tour: the state is no longer transient. This watcher waits
///    for that line in `wire`, dumps, decodes, and writes one byte back to release the boot.
///
///    **Nothing falls back to sampling if the line never comes.** A boot that does not print it was
///    not asked to hold, which is a wiring failure (the `screen_hold` loader feature, the token, the
///    kernel's reader) and not a framebuffer one; saying so is the whole of item 5 of this
///    milestone, because "the tour was never on the screen" sent the last reader after five things
///    that were all working.
/// 2. **The prompt.** A row starting `$ `, which the kernel's tour never prints. Sampled, and that
///    is fine: a prompt sits on the screen until somebody types, so it is a state and not an event.
/// 3. **The answer.** [`UEFI_SCREEN_COMMAND`] is written to `serial` (COM1), and a row equal to
///    [`UEFI_SCREEN_ANSWER`] must appear. The command's own echo is `$ echo ...`, a different row,
///    so this row is only there if the shell ran the command and its output reached the screen.
///
/// Stops early when the monitor stops answering after having answered once (QEMU is gone and no
/// later dump can be better), and at a deadline that bounds the whole watch. A dump that fails to
/// decode is never fatal, and the retry is the answer.
/// **The firmware screen read as the shell's terminal draws it**: `glyph_atlas` cells in
/// `video_terminal`'s default colors, at the scale `display_service::start_screen_terminal` chose
/// for the screen's width, one string per row with its trailing blanks trimmed. The tour before
/// the hand-over is the boot console's 7x8 bitmap, which `board_console::screen::read` reads; this
/// reads what replaces it.
fn terminal_text(ppm: &[u8]) -> Option<String> {
    let width = board_console::screen::decode_exact(ppm, u32::MAX, u32::MAX, 1, b"", |_, _, _| 0)
        .ok()?
        .width;
    let scale = screen_console::ScreenConsole::scale_for_cell(width, video_terminal::CELL_W);
    let printable: Vec<u8> = (0x20..=0x7e).collect();
    let (fg, bg) = video_terminal::Attr::DEFAULT.colours();
    let screen = board_console::screen::decode_exact(
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
    /// before the boot is released anyway. Each costs `screendump`'s own settle plus the poll
    /// interval below, about 650 ms measured, so four is under three seconds and comfortably inside
    /// the kernel's ten-second bound. Three retries is already more than an asynchronous
    /// `screendump` write has ever needed once the guest stopped spinning; see
    /// `console::hold_screen_for_host`, which is where that turned out to matter.
    ///
    /// Corrected 2026-10-12 (lane/scanout-referee-flake): `screendump` now waits for QEMU's answer,
    /// and QEMU answers only once the file is written, so a read can no longer land mid-write. The
    /// retries and the pause below were sized for that and are now margin; they are kept, not
    /// re-measured.
    const HELD_DUMPS: u32 = 4;
    let mut dumps_while_held = 0u32;
    while std::time::Instant::now() < deadline {
        // **The wire is read before the picture is taken, not after**, and getting that backwards
        // cost this lane its first green run. A dump taken before the hold line arrived is a dump
        // of whatever the screen showed *earlier*, so believing it because the line has since
        // appeared is the original race wearing the handshake's clothes.
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
        // A screen that decodes but does not hold what the stage wants is deliberately NOT stored:
        // an incomplete picture must not read as a pass. The dump file itself is the artefact,
        // and the caller prints its path on failure.
        let bytes = std::fs::read(shot).ok();
        let text = bytes
            .as_ref()
            .and_then(|bytes| board_console::screen::read(bytes).ok());
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
            // **Release the kernel once the tour has been read off the screen, or once enough
            // dumps taken during the hold have failed to show it.** Any byte does; the kernel
            // drains whatever arrives so the line editor that comes up on this same wire a moment
            // later does not find a keystroke nobody typed.
            //
            // The second condition is what keeps the verdict honest in both directions. The state
            // is stable while the kernel is held, so a dump that does not show the tour is a real
            // answer rather than bad luck; `HELD_DUMPS` retries are margin (see its note). And the boot must be released
            // either way: a held kernel is a stopped boot, and the verdict below can say more about
            // a boot that finished than about one this watcher wedged.
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
        // Before the release, the poll is fast because the kernel is stopped and every millisecond
        // is a millisecond of a bounded hold being spent. After it, the stages wait on a
        // person-speed shell.
        // The kernel is parked while it holds the screen, so there is no hurry and no window to
        // race: the pause is short enough to spend few of the hold's ten seconds.
        let pause = 250;
        std::thread::sleep(std::time::Duration::from_millis(pause));
    }
    seen
}

/// **Run the kernel suite under real firmware** (milestone 195), rather than the tour
/// `uefi_boot` boots.
///
/// The difference between the two is one ELF. A test build runs `test_main()` right after the
/// shared bring-up, so this boot prints the bring-up lines it shares with `uefi-boot` (the ACPI
/// walk, the PCIe window, the device interrupt) *and then* runs the suite, and the verdict it adds is the one thing the tour cannot say: that
/// the tests pass when the memory map, the ACPI root and the PCIe window came from firmware
/// instead of from a hypervisor. Until this existed, "it boots under real firmware" and "it passes
/// under real firmware" were different claims and only the first was made.
///
/// **The exit status is half the verdict and is checked as such.** The suite reports through
/// `isa-debug-exit`, which terminates QEMU with `(value << 1) | 1`, so a passing run is process
/// status 3 (`arch::x86_64::semihosting::EXIT_SUCCESS`) and a failing one is 1. A transcript scan
/// alone would pass a run whose harness printed its verdict and then faulted on the way out; the
/// status alone would pass a QEMU that never started the guest. Both, and neither is redundant.
///
/// # BUGS
///
/// - **It is a second firmware boot, not a replacement for the first.** `uefi-boot` still boots the
///   tour build, because the tour build is what `uefi-image` stages for the USB stick and what
///   calef carries to the bench; a regression that only the shipping image has would otherwise be
///   gated by nothing.
pub(crate) fn uefi_test() -> bool {
    // **A test boot is a test boot however it is reached.** The system_tests image's confinement
    // suite asserts the DMA-escape role in the archive's NVMe server, and fails by name without
    // it (that is 857654808's guard speaking: the attacker stays out of every boot that is not
    // `cargo xtask test`). But this boot is never a shipped image; it stages a test kernel and a
    // test archive under `target/esp-test`. Setting the switch here gives a bare
    // `cargo xtask uefi-test` the same archive `test`'s tail packs, which it has not had since
    // 857654808 (2026-10-04) and which every bare run between then and 2026-10-15 failed on.
    // When `test` is the caller the switch is already on and this is a no-op. The cost is the one
    // 857654808 already recorded: switching between a test and any other boot recompiles
    // `components`, because cargo fingerprints features. See `CONFINEMENT_ATTACKERS`.
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
    if !initial_ramdisk_x86() || !mkdisk() || !mknvmedisk() {
        return false;
    }
    // Both test images (milestone 609 (the system tests leave the kernel crate)): the kernel's own
    // unit tests and the whole-system suite are separate boots now, and "passes under real
    // firmware" is a claim about both.
    uefi_test_image("kernel") && uefi_test_image("system_tests")
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
        // **The devices the PVH runner attaches** (milestone 195), which the tour above needs none
        // of. They close the second of the three things milestone 215's BUGS listed as answerable
        // only on xenon: a PCI function whose BARs were placed by FIRMWARE rather than by this
        // kernel's own bus walk. Under `-kernel` the kernel assigns them itself, so "the driver can
        // reach an MSI-X table a bus walk found" and "the driver can reach one it put there" were
        // the same sentence.
        //
        // **One core here, two in the tour above.** The defect this split used to name is gone:
        // `every_secondary_runs_scheduled_work`'s half-the-time failure at two cores was
        // `boot_cpu_id` answering "which core am I" where every caller wanted "which core
        // booted", fixed by milestone 316 (which core booted), and the PVH runner has defaulted
        // to two since 2026-09-23, per DECISIONS §153 (how a two-core x86_64 test earns its
        // place), closed by milestone 315 (a port revoke that reaches every core). This leg
        // keeps the runner's one-core default because the whole suite at two cores under OVMF is
        // unmeasured since those fixes; the tour is where the second core is gated.
        //
        // `NIFE_DISK` and `NIFE_NVME` are already set by `test()` for the PVH leg that ran just
        // above, so this inherits them; they are named here only for a bare `cargo xtask uefi-test`.
        .env("NIFE_DISK", disk_path())
        .env("NIFE_NVME", nvme_disk_path());
    // **The GPU and keyboard ride the system image's boot, with the PVH leg's referee beside it**
    // (milestone 420 (x86_64 has one fixture of five and no NIC, GPU, keyboard or RNG)). The
    // display, compositor and keyboard tests live in that image; the kernel's own unit tests use
    // neither device, so its boot leaves them off rather than inherit `test()`'s `NIFE_GPU`. Constructed before the child, because it is what sets `NIFE_GPU_MON`.
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
    // **The bound is a hang detector, so it sits well above an honest run.** A passing boot exits
    // through `isa-debug-exit` as soon as the suite ends, so a higher bound costs nothing until
    // something hangs. In CI the `system_tests` boot takes about 156 s on main and about 190 s on
    // the 1120x624 scanout (milestone 142 (a text display good enough that people use it instead
    // of a GUI)), whose display and compositor tests compare 2.2 times the pixels; 300 s is the
    // bound `disk_throughput`'s boot already uses. A caller's own `NIFE_UEFI_TIMEOUT` still wins.
    if std::env::var_os("NIFE_UEFI_TIMEOUT").is_none() {
        command.env("NIFE_UEFI_TIMEOUT", "300");
    }
    // Timed and recorded (milestone 807 (the kernel suite reports what each test cost)). The
    // transcript used to be collected whole and printed at the end; it now streams as the other
    // legs' do, and stderr (QEMU's own, never the guest console) goes straight to ours rather
    // than being appended to what the checks below read.
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

/// **At a failed boot, say which exit path produced the status, and retire the VT-d theory.**
///
/// The number alone cannot: `1` is both the guest's failure write and QEMU's own error exit, and
/// neither the transcript nor QEMU's stderr rides on the line that prints the number. The shapes
/// below are the whole protocol. The guest writes `code >> 1` to the exit device's port and QEMU
/// reports `(written << 1) | 1`, so every status the device can produce is odd and 0 is not one of
/// them. What backs the readings: the summary print and the success write are adjacent statements
/// in `kernel/src/testing.rs`, and past that point the one in-tree writer of the failure code is
/// the panic arm, which prints `[PANIC]` first (`kernel/src/panic.rs`); QEMU exits 0 on a SIGTERM,
/// which is how both the `NIFE_UEFI_TIMEOUT` bound and the host watchdog end a boot (measured on
/// Homebrew QEMU 11.1.2, 2026-10-15); a QEMU that fails before or during the boot exits 1 with its
/// reason on stderr, which this boot streams to ours. Name provisional (this lane; calef names the
/// interfaces).
fn exit_verdict_lines(code: Option<i32>, exited: Duration, transcript: &str) -> Vec<String> {
    let mut lines = vec![
        // Milestone 215 (a PCI function's interrupt reaches nothing on x86_64) named this family of
        // QEMU lines the escape test's own expected faults: the masked fault-event interrupt (this
        // kernel polls, it does not take the interrupt) and the escape's translation failure. They are what a passing confinement test looks like from
        // outside, and they are the only thing in a red run's output that invites a wrong theory
        // about QEMU versions or firmware.
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

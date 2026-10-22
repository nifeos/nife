//! **The boot tour read back off a `ramfb`**, milestone 243 (a machine with no serial port), on the
//! two architectures whose firmware never lights a screen.
//!
//! [`crate::uefi`]'s `uefi_boot` twin, and deliberately the same shape: a QEMU monitor on a unix
//! socket, a poller asking it for a screendump, and `serial_console::screen` turning the picture back
//! into text. What differs is only where the framebuffer came from. On `x86_64` the firmware lit one
//! and the loader measured it; here the kernel allocates one and tells the emulator about it over
//! `fw_cfg` (`kernel/src/screen.rs`).
//!
//! *Module and subcommand names provisional (AGENTS.md: calef names these).*

use std::path::PathBuf;
use std::process::Command;

use crate::archive::{initial_ramdisk_path, initial_ramdisk_riscv, riscv_initial_ramdisk_path};
use crate::host::{cargo, workspace_root};
use crate::scanout::screendump;
use crate::{RISCV_TARGET, TARGET, profile_dir};

/// How long the watcher polls before giving up. Generous, because this is a TCG boot of a debug
/// kernel and the riscv64 leg is the slower of the two. The watch ends the moment the screen
/// answers, so the bound is only ever paid by a failure.
const WATCH_SECONDS: u64 = 300;

/// Boot one of the two board architectures with a `ramfb` and read the tour back off it.
///
/// **The claim is the one the serial transcript cannot make.** Both boots print the same tour on the
/// UART whether or not a single pixel was written, so the transcript would be identical with the
/// screen black. This asserts that `boot_ladder::SELF_TEST` is *on the screen*, decoded glyph by
/// glyph, which exercises the `fw_cfg` conversation, the physical address the kernel handed over,
/// the byte order, the stride and the font.
///
/// **No virtio-gpu on this machine**, which is a requirement rather than a tidiness: `ramfb` adds a
/// QEMU console and `screendump` with no device argument writes console 0, so a boot with both would
/// be photographing whichever QEMU happened to order first.
///
/// The boot is stopped as soon as the screen has answered; the kernel never exits on its own.
pub(crate) fn screen_boot(arch: &str) -> bool {
    let Some((runner, elf, archive)) = build_screen_leg(arch) else {
        return false;
    };

    eprintln!();
    eprintln!("--- the boot tour on a screen, {arch} (QEMU virt + ramfb) ---");

    // In /tmp rather than under target/, because a unix socket path is capped at 104 bytes by the
    // OS and a worktree checkout plus `target/` gets close. Same reason `gpu_mon_socket` does it.
    let sock = format!("/tmp/nife-ramfb-{arch}-{}.sock", std::process::id());
    let shot = PathBuf::from(format!("/tmp/nife-ramfb-{arch}-{}.ppm", std::process::id()));
    let _ = std::fs::remove_file(&sock);
    let _ = std::fs::remove_file(&shot);

    let Some(mut child) = start_ramfb_boot(runner, &elf, &sock, &archive) else {
        return false;
    };
    let collect = |mut from: Box<dyn std::io::Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = std::io::Read::read_to_end(&mut from, &mut bytes);
            String::from_utf8_lossy(&bytes).into_owned()
        })
    };
    let stdout = collect(Box::new(child.stdout.take().expect("piped stdout")));
    let stderr = collect(Box::new(child.stderr.take().expect("piped stderr")));

    let seen = {
        let (sock, shot) = (sock.clone(), shot.clone());
        std::thread::spawn(move || watch(&sock, &shot))
    }
    .join()
    .unwrap_or_default();

    // Stop the machine rather than waiting out its own bound: the kernel never exits, and what is
    // asserted below has already been painted by the time the marker appeared.
    let _ = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status();
    let _ = child.wait();
    let transcript = format!(
        "{}{}",
        stdout.join().unwrap_or_default(),
        stderr.join().unwrap_or_default()
    );

    let mut ok = screen_showed_the_tour(seen.as_deref(), arch, &shot);
    ok &= serial_control_holds(&transcript, ok);
    let _ = std::fs::remove_file(&sock);
    ok
}

/// **Build what `arch`'s screen leg boots**: its archive and its kernel. Returns the runner, the
/// kernel's path and the archive's, or `None` after saying why not.
fn build_screen_leg(arch: &str) -> Option<(&'static str, String, String)> {
    let (target, runner) = match arch {
        "aarch64" => (TARGET, "helpers/qemu-runner-aarch64.sh"),
        "riscv64" => (RISCV_TARGET, "helpers/qemu-runner-riscv64.sh"),
        other => {
            eprintln!(
                "screen-boot: no ramfb leg for {other}; x86_64's screen is `cargo xtask uefi-boot`"
            );
            return None;
        }
    };
    let prepared = if arch == "aarch64" {
        crate::user()
    } else {
        initial_ramdisk_riscv()
    };
    if !prepared || !cargo(&["build", "-p", "kernel", "--target", target]) {
        return None;
    }
    let elf = format!("target/{target}/{}/kernel", profile_dir());

    // **The archive this architecture's kernel was measured against**, and it has to be said here.
    //
    // [`cargo`] exports `NIFE_INITIAL_RAMDISK` pointing at the *aarch64* archive, because that is
    // what the aarch64 runner wants and it is the aarch64 path's helper. Every other riscv64 caller
    // in this crate overrides it for exactly this reason (`suite`'s riscv leg says so in its own
    // comment, and `swish_check` and the interactive boot both pick per architecture). This one did
    // not, and the failure had two faces: locally the aarch64 archive existed, so the riscv kernel
    // loaded it and printed `MEASURED BOOT REFUSED` and carried on; in a CI job that never built
    // one, QEMU refused to start at all with `could not load ramdisk`.
    let archive = if arch == "riscv64" {
        riscv_initial_ramdisk_path()
    } else {
        initial_ramdisk_path()
    };

    Some((runner, elf, archive))
}

/// Start the ramfb machine with its monitor on `sock`, its stdout and stderr piped. `None` after
/// saying why it did not start.
fn start_ramfb_boot(
    runner: &str,
    elf: &str,
    sock: &str,
    archive: &str,
) -> Option<std::process::Child> {
    match Command::new(runner)
        .arg(elf)
        .current_dir(workspace_root())
        .env("NIFE_SCREEN", "1")
        .env("NIFE_SCREEN_MON", sock)
        .env("NIFE_INITIAL_RAMDISK", archive)
        // The devices this boot must not have. The GPU and its monitor are the console-0 collision
        // above; the keyboard and the NVMe controller are suite devices that only lengthen a tour
        // nobody is reading here. Removed rather than simply not set, because `cargo` above and any
        // earlier `script/test` in the same shell set some of them in this process.
        .env_remove("NIFE_GPU")
        .env_remove("NIFE_GPU_MON")
        .env_remove("NIFE_KEYBOARD")
        .env_remove("NIFE_NVME")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => Some(c),
        Err(e) => {
            eprintln!("screen-boot: failed to run {runner}: {e}");
            None
        }
    }
}

/// **What the screen showed**, reported: true only when the tour's marker was on it.
fn screen_showed_the_tour(seen: Option<&str>, arch: &str, shot: &std::path::Path) -> bool {
    let mut ok = true;
    match seen {
        Some(text) if text.contains(boot_ladder::SELF_TEST) => {
            let rows: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
            eprintln!(
                "screen-boot: read {} non-blank row(s) of the {arch} tour back off a ramfb, ending",
                rows.len(),
            );
            for line in rows.iter().rev().take(4).rev() {
                eprintln!("screen-boot:   | {line}");
            }
        }
        Some(text) => {
            eprintln!(
                "screen-boot: the screen never showed `{}`. Last screen read ({}):",
                boot_ladder::SELF_TEST,
                shot.display()
            );
            for line in text.lines().filter(|l| !l.is_empty()) {
                eprintln!("screen-boot:   | {line}");
            }
            ok = false;
        }
        None => {
            eprintln!(
                "screen-boot: nothing decodable was ever on the screen (did QEMU get a ramfb and a monitor?)"
            );
            ok = false;
        }
    }
    ok
}

/// **The serial line, as the control**: false when the tour never reached it either. When
/// anything has failed (the screen, per `screen_ok`, or this), the transcript's tail is printed.
fn serial_control_holds(transcript: &str, screen_ok: bool) -> bool {
    let serial_ok = transcript.contains(boot_ladder::SELF_TEST);
    // **The serial line is the control, and this line is the point of having one.**
    //
    // A screen gate that only reports "the screen was blank" sends the next reader into the
    // framebuffer path, which is exactly where this milestone's own lane went and where nothing was
    // wrong. Asserting the same marker on the channel the screen is *replacing* costs one
    // `contains` and separates "this milestone's mechanism is broken" from "the machine did not
    // boot", which are different people's afternoons. It earned its keep on its first red CI run.
    if !serial_ok {
        eprintln!(
            "screen-boot: the tour never reached the SERIAL line either, so this is a boot failure and not a screen one"
        );
    }
    if !screen_ok || !serial_ok {
        let tail: Vec<&str> = transcript.lines().collect();
        for line in tail.iter().rev().take(20).rev() {
            eprintln!("screen-boot:   serial | {line}");
        }
    }
    serial_ok
}

/// Poll the monitor for a screendump until the tour's marker is on the screen.
///
/// Returns the decoded screen that carried the marker, or the last non-blank one seen, or `None` if
/// nothing ever decoded. A dump that fails to decode is never fatal; the retry is the answer.
/// (`screendump` waits for QEMU's answer since 2026-10-12, so the file is whole when it returns.)
fn watch(sock: &str, shot: &std::path::Path) -> Option<String> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(WATCH_SECONDS);
    let mut answered = false;
    let mut last = String::new();
    while std::time::Instant::now() < deadline {
        if !screendump(sock, shot) {
            // Once the monitor has answered and then stops, QEMU is gone and no later dump can be
            // better than the one already in hand.
            if answered {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
            continue;
        }
        answered = true;
        if let Some(text) = std::fs::read(shot)
            .ok()
            .and_then(|bytes| serial_console::screen::read(&bytes).ok())
        {
            if text.contains(boot_ladder::SELF_TEST) {
                return Some(text);
            }
            if !text.trim().is_empty() {
                last = text;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    if last.is_empty() { None } else { Some(last) }
}

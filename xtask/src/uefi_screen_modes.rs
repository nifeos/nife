//! **The screen handover at the modes a real monitor reports.**
//!
//! Name: provisional (lane lane/surface-map-budget, 2026-10-21 UTC).
//!
//! [`crate::uefi::uefi_boot`]'s tour runs at OVMF's default 1280x800, and every byte of the handover
//! between the kernel and `framebuffer_driver` scales with the screen: the aperture, the surface,
//! and the records the driver's address space keeps for each page it maps. A budget that leaves
//! the surface out still fits at 1280x800 and panics the handover at 1920x1080, xenon's HDMI
//! panel. So the gate boots the same image once more at each mode in [`MODES`] and asks the
//! serial wire whether both halves came up and the shell answered.
//!
//! Serial only, on purpose: reading the screen back is the tour's job, and its decoder is laid out
//! for 1280x800. What breaks here breaks before a pixel of the terminal is drawn, and the kernel
//! says so on COM1.

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::host::workspace_root;

/// One mode the gate boots, and the lines the kernel prints when the handover worked there.
struct Mode {
    /// The `VGA` device's preferred size, which OVMF's `QemuVideoDxe` reads from the EDID and
    /// offers as the GOP mode the loader takes.
    width: u32,
    height: u32,
    /// Every line the transcript must hold: the kernel's own report of what the driver serves.
    wanted: &'static [&'static str],
}

/// **xenon's mode, and one past the 8 MiB aperture.** 1920x1080 is the panel the panic was found
/// on, with a 2025-page surface. 2560x1440 is too large to map whole, so it takes
/// `screen_terminal_aperture`'s clip and maps as close to the 2048-page cap as a mode can, which
/// is the largest surface any screen hands over. A 4K mode is not here because OVMF does not
/// offer one on QEMU's `VGA`: it falls back to 1280x800, which the tour already boots.
const MODES: [Mode; 2] = [
    Mode {
        width: 1920,
        height: 1080,
        wanted: &[
            "1920x1080 pixels of it at scale 1 served by framebuffer_driver, a 137x41 terminal on it",
        ],
    },
    Mode {
        width: 2560,
        height: 1440,
        wanted: &[
            "too large to map whole; the terminal covers its top 819 rows",
            "2560x819 pixels of it at scale 1 served by framebuffer_driver, a 182x31 terminal on it",
        ],
    },
];

/// The shell's banner, printed once the progenitor has the console: the handover is over.
const SHELL_UP: &str = "nife capability shell.";

/// How long one mode may take to reach the shell. A release boot under TCG reaches it in well
/// under this; the bound is for a hang, which a panic is not (the panic line ends the wait).
const MODE_DEADLINE: Duration = Duration::from_secs(150);

/// Boot `esp` at every [`MODES`] entry. False after naming each mode that failed.
pub(crate) fn every_mode_hands_the_screen_over(esp: &std::path::Path) -> bool {
    let mut ok = true;
    for mode in &MODES {
        ok &= mode_hands_the_screen_over(esp, mode);
    }
    ok
}

fn mode_hands_the_screen_over(esp: &std::path::Path, mode: &Mode) -> bool {
    let name = format!("{}x{}", mode.width, mode.height);
    eprintln!();
    eprintln!("--- the screen handover at {name}, x86_64 (QEMU q35 + OVMF) ---");
    let Some(transcript) = boot_until_the_shell(esp, mode) else {
        return false;
    };
    let mut ok = true;
    for wanted in mode.wanted.iter().chain([&SHELL_UP]) {
        if !transcript.contains(wanted) {
            eprintln!("uefi-boot: at {name} the boot transcript is missing {wanted:?}");
            ok = false;
        }
    }
    if let Some(panic) = transcript.lines().find(|l| l.contains("[PANIC]")) {
        eprintln!("uefi-boot: at {name} the kernel panicked: {panic}");
        ok = false;
    }
    if ok {
        eprintln!("uefi-boot: at {name} the screen was handed over and the shell came up");
    } else {
        print!("{transcript}");
    }
    ok
}

/// Boot `esp` with the screen at `mode`, and return the serial transcript once the shell is up,
/// the kernel has panicked, or [`MODE_DEADLINE`] has passed. `None` after saying why it did not
/// start.
fn boot_until_the_shell(esp: &std::path::Path, mode: &Mode) -> Option<String> {
    let vars = workspace_root().join(format!(
        "target/ovmf-vars-{}x{}.fd",
        mode.width, mode.height
    ));
    let mut child = match Command::new("helpers/qemu-uefi-x86_64.sh")
        .arg(esp)
        .args(["-vga", "none", "-device"])
        .arg(format!("VGA,xres={},yres={}", mode.width, mode.height))
        .current_dir(workspace_root())
        // Its own variable store, so a mode's NVRAM never decides another boot's resolution.
        .env("NIFE_OVMF_VARS_OUT", &vars)
        .env("NIFE_UEFI_TIMEOUT", MODE_DEADLINE.as_secs().to_string())
        // The bare firmware machine, for the reason `start_uefi_screen_boot` removes these.
        .env_remove("NIFE_DISK")
        .env_remove("NIFE_NVME")
        .env_remove("NIFE_GPU")
        .env_remove("NIFE_GPU_MON")
        .env_remove("NIFE_KEYBOARD")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("uefi-boot: failed to run helpers/qemu-uefi-x86_64.sh: {e}");
            return None;
        }
    };
    let (stdout, stderr, so_far) = crate::uefi::collect_uefi_streams(&mut child);
    let mut serial = child.stdin.take().expect("piped stdin");
    let deadline = Instant::now() + MODE_DEADLINE;
    let mut released = false;
    while Instant::now() < deadline {
        let text = so_far.lock().map(|t| t.clone()).unwrap_or_default();
        // The gate's loader holds the screen for the tour's dump until a byte arrives on COM1;
        // nothing reads the screen here, so release it at once.
        if !released && text.contains(boot_ladder::SCREEN_HELD) {
            released = serial
                .write_all(b"\n")
                .and_then(|()| serial.flush())
                .is_ok();
        }
        if text.contains(SHELL_UP) || text.contains("[PANIC]") {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    // SIGTERM to the wrapper, which `qemu-bounded.sh` forwards to QEMU, as `uefi_boot_staged` does.
    let _ = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status();
    let _ = child.wait();
    let _ = std::fs::remove_file(&vars);
    Some(stdout.join().unwrap_or_default() + &stderr.join().unwrap_or_default())
}

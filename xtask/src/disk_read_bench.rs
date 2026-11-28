//! **The read-only NVMe bench boot, rehearsed** (milestone 163 (the JH7110's PCIe root complex)).
//!
//! `cargo xtask disk-read-bench` builds a `disk_read_bench` kernel for each architecture it is
//! given, boots it under QEMU with the suite's NVMe controller on a patterned image, and fails
//! unless the kernel gives the verdict below **and the image is byte for byte what it was before
//! the boot**. The second check is the point: the kernel claims it never writes the drive, and the
//! host reads the drive back to see. **The numbers are QEMU's and mean nothing about a disk**; this
//! proves the boot comes up, reads what is on the drive and says so in the shape the board run is
//! judged by. `script/board-image --extra-features disk_read_bench` builds the same kernel for the
//! board. See `kernel/src/disk_read_bench.rs` and nifeos/record:notes/jh7110-pci-express-bench.md.
//!
//! Name: provisional, minted 2026-11-28 (UTC) by lane/163-nvme-read-bench: the kernel feature's
//! name, hyphenated the way `disk-throughput` hyphenates its own. calef has not named it.

//! # Cases
//! | case | machine | the kernel must say |
//! |---|---|---|
//! | `read` | the suite's NVMe controller on an 8 MiB patterned image | the namespace's size, a landing check whose digest is the image's first 64 KiB, `READ-AT-RATE` |
//! | `no-controller` | no NVMe controller at all | `SKIPPED: no NVMe controller on the bus` |
//!
//! The second is the board's likeliest wrong answer, a slot whose link never trained, and a
//! verdict nobody has seen come out wrong is a verdict nobody has tested.

//! # Flags
//!
//! `--arch <isa>` runs one architecture (aarch64, riscv64 or x86_64; all three by default).
//! `--case <name>` runs one case. `--release` builds the release profile, which is what
//! `script/board-image` builds.

use std::process::ExitCode;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::host::workspace_root;
use crate::soak::{console_lines, kernel_leg, qemu_command, start_qemu, stop_qemu};
use crate::{RELEASE, cargo_profiled};

/// The image's size: 8 MiB, the suite's NVMe image size, so the timed phase reads all of it.
const IMAGE_BYTES: usize = 8 * 1024 * 1024;

/// The bytes the landing check reads: the kernel's one window, 16 blocks of 4096.
const LANDING_BYTES: usize = 16 * 4096;

/// How long a case may take before it is called a hang. A TCG boot tour on the slowest of the
/// three takes about a minute on a loaded machine.
const CASE_DEADLINE: Duration = Duration::from_secs(300);

/// The architectures, in the suite's order.
const ARCHITECTURES: &[&str] = &["aarch64", "riscv64", "x86_64"];

/// One machine shape and the verdict it must produce.
struct Case {
    name: &'static str,
    what: &'static str,
    /// Whether the machine carries the NVMe controller.
    nvme: bool,
}

const CASES: &[Case] = &[
    Case {
        name: "read",
        what: "the suite's NVMe controller on a patterned image",
        nvme: true,
    },
    Case {
        name: "no-controller",
        what: "no NVMe controller at all",
        nvme: false,
    },
];

/// The rehearsal's own image, one per architecture, so a run never contends with the suite's
/// `nife-nvme.img` lock.
fn image_path(arch: &str) -> std::path::PathBuf {
    workspace_root().join(format!("target/disk-read-bench-nvme-{arch}.img"))
}

/// The image's contents: every byte a function of its offset and its block's number, so a read
/// from the wrong block or the wrong offset changes the landing digest.
fn pattern() -> Vec<u8> {
    (0..IMAGE_BYTES)
        .map(|o| ((o >> 12) as u8).wrapping_mul(31) ^ (o as u8))
        .collect()
}

pub(crate) fn disk_read_bench() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let usage = format!(
        "usage: cargo xtask disk-read-bench [--arch {}] [--case {}] [--release]",
        ARCHITECTURES.join("|"),
        CASES.iter().map(|c| c.name).collect::<Vec<_>>().join("|"),
    );
    let mut arch: Option<String> = None;
    let mut only: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--arch" if i + 1 < args.len() => {
                arch = Some(args[i + 1].clone());
                i += 1;
            }
            "--case" if i + 1 < args.len() => {
                only = Some(args[i + 1].clone());
                i += 1;
            }
            "--release" => RELEASE.store(true, Ordering::Relaxed),
            other => {
                eprintln!("disk-read-bench: unknown argument {other}");
                eprintln!("{usage}");
                return ExitCode::from(4);
            }
        }
        i += 1;
    }
    if let Some(name) = &only
        && !CASES.iter().any(|c| c.name == name)
    {
        eprintln!("disk-read-bench: no case named {name}");
        return ExitCode::from(4);
    }
    let arches: Vec<&str> = match &arch {
        Some(a) if ARCHITECTURES.contains(&a.as_str()) => vec![a.as_str()],
        Some(a) => {
            eprintln!("disk-read-bench: unknown architecture {a}");
            eprintln!("{usage}");
            return ExitCode::from(4);
        }
        None => ARCHITECTURES.to_vec(),
    };

    let mut failed = Vec::new();
    for arch in arches {
        // The archive first and then the kernel, the measured-boot order: the kernel vouches for
        // the table the archive it was built beside carries.
        let Some((target, runner, initial_ramdisk)) = kernel_leg("disk-read-bench", arch) else {
            return ExitCode::from(4);
        };
        if !cargo_profiled(&[
            "build",
            "-p",
            "kernel",
            "--features",
            "disk_read_bench",
            "--target",
            target,
        ]) {
            return ExitCode::from(4);
        }
        for case in CASES
            .iter()
            .filter(|c| only.as_deref().is_none_or(|n| n == c.name))
        {
            if !run_case(case, arch, target, runner, &initial_ramdisk) {
                failed.push(format!("{arch} {}", case.name));
            }
        }
    }
    eprintln!();
    if failed.is_empty() {
        eprintln!(
            "disk-read-bench: every case produced its verdict and no image changed \
             (QEMU's numbers, not a disk's)"
        );
        ExitCode::SUCCESS
    } else {
        eprintln!("disk-read-bench: FAILED cases: {}", failed.join(", "));
        ExitCode::from(1)
    }
}

/// Boot `case` on `arch`, judge its transcript and, with a controller, the image after it.
fn run_case(case: &Case, arch: &str, target: &str, runner: &str, initial_ramdisk: &str) -> bool {
    eprintln!();
    eprintln!(
        "--- disk-read-bench: {arch} {} ({}) ---",
        case.name, case.what
    );
    let image = pattern();
    let path = image_path(arch);
    if case.nvme
        && let Err(e) = std::fs::write(&path, &image)
    {
        eprintln!("disk-read-bench: cannot write {}: {e}", path.display());
        return false;
    }
    let log_path =
        workspace_root().join(format!("target/disk-read-bench-{arch}-{}.log", case.name));
    let Ok(mut log) = std::fs::File::create(&log_path) else {
        eprintln!("disk-read-bench: cannot write {}", log_path.display());
        return false;
    };

    let mut cmd = qemu_command(runner, target, initial_ramdisk, arch, None);
    cmd.env_remove("NIFE_NVME");
    if case.nvme {
        cmd.env("NIFE_NVME", &path);
    }
    let Some((mut child, stdout)) = start_qemu("disk-read-bench", runner, &mut cmd) else {
        return false;
    };
    let transcript = read_transcript(stdout, &mut log);
    stop_qemu(&mut child, &mut log);

    let mut ok = judge(case, &image, &transcript);
    if case.nvme {
        ok &= image_unchanged(&path, &image);
    }
    eprintln!(
        "disk-read-bench: {arch} {}: {} (log {})",
        case.name,
        if ok { "as expected" } else { "WRONG" },
        log_path.display()
    );
    ok
}

/// The console, into `log` whole and onto this terminal where it is the kernel's bench lines, until
/// the kernel says it is done or [`CASE_DEADLINE`] passes.
fn read_transcript(stdout: std::process::ChildStdout, log: &mut impl std::io::Write) -> String {
    let lines = console_lines(stdout);
    let deadline = Instant::now() + CASE_DEADLINE;
    let mut transcript = String::new();
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        let Ok(line) = lines.recv_timeout(left) else {
            break;
        };
        let _ = writeln!(log, "{line}");
        if line.starts_with("disk-read-bench:") {
            println!("{line}");
        }
        transcript.push_str(&line);
        transcript.push('\n');
        if line.starts_with("disk-read-bench: done") {
            break;
        }
    }
    transcript
}

/// The lines `case` must print, given the image it booted on.
fn wanted(case: &Case, image: &[u8]) -> Vec<String> {
    if !case.nvme {
        return vec![
            "disk-read-bench: controller none".into(),
            "disk-read-bench: verdict SKIPPED: no NVMe controller on the bus. Not a pass.".into(),
        ];
    }
    let digest = measured_boot::hex(&measured_boot::sha256(&image[..LANDING_BYTES]));
    let prefix = String::from_utf8_lossy(&digest[..16]).into_owned();
    let reads = IMAGE_BYTES / LANDING_BYTES;
    vec![
        "disk-read-bench: controller nvme ".into(),
        format!("disk-read-bench: namespace  {IMAGE_BYTES} bytes"),
        format!("GET_SIZE_BYTES through the window answers {IMAGE_BYTES}: agrees"),
        format!("sha256 {prefix}.. both times"),
        format!("disk-read-bench: read       {reads} x {LANDING_BYTES} B"),
        "disk-read-bench: verdict READ-AT-RATE: ".into(),
        "nothing written".into(),
    ]
}

/// Whether the transcript holds every wanted line, exactly one verdict and the done line.
fn judge(case: &Case, image: &[u8], transcript: &str) -> bool {
    let mut ok = true;
    for want in wanted(case, image) {
        if !transcript.contains(&want) {
            eprintln!(
                "disk-read-bench: {}: the transcript is missing {want:?}",
                case.name
            );
            ok = false;
        }
    }
    if transcript.matches("disk-read-bench: verdict ").count() != 1 {
        eprintln!(
            "disk-read-bench: {}: want exactly one verdict line",
            case.name
        );
        ok = false;
    }
    if !transcript.contains("disk-read-bench: done, halting.") {
        eprintln!(
            "disk-read-bench: {}: the kernel never said it was done",
            case.name
        );
        ok = false;
    }
    ok
}

/// Whether the image on disk is still `image`, byte for byte: the host's check of the kernel's
/// claim that it never writes.
fn image_unchanged(path: &std::path::Path, image: &[u8]) -> bool {
    match std::fs::read(path) {
        Ok(after) if after == image => {
            eprintln!("disk-read-bench: the image is unchanged after the boot");
            true
        }
        Ok(after) => {
            let first = after
                .iter()
                .zip(image)
                .position(|(a, b)| a != b)
                .unwrap_or(image.len().min(after.len()));
            eprintln!(
                "disk-read-bench: the image CHANGED: {} bytes now, first difference at byte {first}",
                after.len()
            );
            false
        }
        Err(e) => {
            eprintln!("disk-read-bench: cannot read {} back: {e}", path.display());
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pattern changes from one block to the next at the same offset, so a read of the wrong
    /// block cannot give the landing digest a right one would.
    #[test]
    fn the_pattern_tells_blocks_apart() {
        let image = pattern();
        assert_eq!(image.len(), IMAGE_BYTES);
        for block in 0..IMAGE_BYTES / 4096 - 1 {
            assert_ne!(
                image[block * 4096..(block + 1) * 4096],
                image[(block + 1) * 4096..(block + 2) * 4096],
                "blocks {block} and {} are the same",
                block + 1
            );
        }
    }

    /// A transcript with every line passes, and one missing the verdict fails.
    #[test]
    fn judge_wants_the_verdict() {
        let case = &CASES[1];
        let image = pattern();
        let good = "disk-read-bench: controller none: no function on the bus is NVMe class\n\
                    disk-read-bench: verdict SKIPPED: no NVMe controller on the bus. Not a pass.\n\
                    disk-read-bench: done, halting.\n";
        assert!(judge(case, &image, good));
        let bad = "disk-read-bench: controller none: no function on the bus is NVMe class\n\
                   disk-read-bench: done, halting.\n";
        assert!(!judge(case, &image, bad));
    }
}

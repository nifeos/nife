//! **Fatal risk 6's bench boot, rehearsed** (milestone 261 (the NVMe driver leaves the kernel); notes/risk-6-bench-evening.md).
//!
//! `cargo xtask disk-throughput` builds the `disk_throughput` kernel **in release** (the number is
//! the claim, and a debug build's would understate it), stages it behind the UEFI loader at
//! `target/esp-disk-throughput` for a stick, and boots it under OVMF with `-device intel-iommu`
//! and an emulated NVMe in the four shapes below, failing if any gives another verdict. **The
//! numbers are QEMU's and mean nothing about a disk**; this proves the preflight and the verdict
//! read the way the bench will read them.

//! # Cases
//! | case | machine | the kernel must say |
//! |---|---|---|
//! | `root-port` | NVMe behind a PCIe root port, 512-byte LBAs (xenon's shape) | both preflights PASS, `CONFINED-AT-RATE` |
//! | `lba-4096` | NVMe on the root bus, 4096-byte LBAs | both PASS, `blocks_per 1`, `CONFINED-AT-RATE` |
//! | `lba-8192` | 8192-byte LBAs, which this driver must refuse | lba FAIL, `SKIPPED` |
//! | `bypass` | the root bus bypasses the IOMMU, so the DMAR names no unit for it | scope FAIL, `UNCONFINED` |
//!
//! The last two are the night-of conditions going the wrong way, which is the point: a verdict
//! nobody has seen come out red is a verdict nobody has tested.

//! # Flags
//!
//! `--stage-only` builds and stages without booting, which is the night-of use. `--case <name>`
//! runs one shape. `--debug` builds the debug profile instead, the fallback if a release image
//! misbehaves on the machine (every xenon boot before this one was debug). Names provisional.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, ExitCode, Stdio};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::archive::initial_ramdisk_x86;
use crate::host::workspace_root;
use crate::uefi::uefi_stage;
use crate::{RELEASE, X86_TARGET, cargo_profiled, profile_dir};

/// One machine shape and the verdict it must produce.
struct Case {
    name: &'static str,
    what: &'static str,
    env: &'static [(&'static str, &'static str)],
    qemu: &'static [&'static str],
    /// Every one of these must appear in the transcript.
    want: &'static [&'static str],
}

const CASES: &[Case] = &[
    Case {
        name: "root-port",
        what: "NVMe behind a PCIe root port, 512-byte LBAs: xenon's shape",
        env: &[("NIFE_NVME_ROOT_PORT", "1")],
        qemu: &[],
        want: &[
            "preflight 1/2 dmar scope : PASS  nvme 01:00.0",
            "preflight 2/2 lba size   : PASS  512-byte lbas, blocks_per 8",
            "verified    ",
            "verdict CONFINED-AT-RATE",
        ],
    },
    Case {
        name: "lba-4096",
        what: "4096-byte LBAs: blocks_per 1, the other end of the range",
        env: &[(
            "NIFE_NVME_DEVICE_OPTS",
            ",logical_block_size=4096,physical_block_size=4096",
        )],
        qemu: &[],
        want: &[
            "preflight 1/2 dmar scope : PASS",
            "preflight 2/2 lba size   : PASS  4096-byte lbas, blocks_per 1",
            "verdict CONFINED-AT-RATE",
        ],
    },
    Case {
        name: "lba-8192",
        what: "8192-byte LBAs, which this driver must refuse",
        env: &[(
            "NIFE_NVME_DEVICE_OPTS",
            ",logical_block_size=8192,physical_block_size=8192",
        )],
        qemu: &[],
        want: &[
            "preflight 1/2 dmar scope : PASS",
            "preflight 2/2 lba size   : FAIL  8192-byte lbas",
            "verdict SKIPPED",
        ],
    },
    Case {
        name: "bypass",
        what: "the root bus bypasses the IOMMU, so no unit owns the NVMe",
        env: &[],
        qemu: &["-machine", "default_bus_bypass_iommu=on"],
        want: &[
            "preflight 1/2 dmar scope : FAIL",
            "no drhd owns it",
            "verdict UNCONFINED",
        ],
    },
];

/// Where the stick image is staged. Its own directory, for `uefi_test_esp_dir`'s reason: this
/// kernel writes to the disk, and `target/esp` is what the ordinary bench procedure copies.
fn esp_dir() -> std::path::PathBuf {
    workspace_root().join("target/esp-disk-throughput")
}

/// The rehearsal's own disk image, so a run never contends with the suite's `nife-nvme.img` lock.
fn nvme_image() -> std::path::PathBuf {
    workspace_root().join("target/disk-throughput-nvme.img")
}

pub(crate) fn disk_throughput() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let mut stage_only = false;
    let mut debug = false;
    let mut only: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--stage-only" => stage_only = true,
            // The fallback if a release image misbehaves on the machine: every earlier xenon boot
            // was a debug build. Its figures are then a debug build's and must be labelled so.
            "--debug" => debug = true,
            "--case" if i + 1 < args.len() => {
                only = Some(args[i + 1].clone());
                i += 1;
            }
            other => {
                eprintln!("disk-throughput: unknown argument {other}");
                eprintln!(
                    "usage: cargo xtask disk-throughput [--stage-only] [--debug] [--case {}]",
                    CASES.iter().map(|c| c.name).collect::<Vec<_>>().join("|")
                );
                return ExitCode::from(4);
            }
        }
        i += 1;
    }
    if let Some(name) = &only
        && !CASES.iter().any(|c| c.name == name)
    {
        eprintln!("disk-throughput: no case named {name}");
        return ExitCode::from(4);
    }

    // Release, always: see the module doc. The archive first and then the kernel, for the
    // measured-boot order `uefi::uefi_kernel` documents.
    RELEASE.store(!debug, Ordering::Relaxed);
    if !initial_ramdisk_x86()
        || !cargo_profiled(&[
            "build",
            "-p",
            "kernel",
            "--features",
            "disk_throughput",
            "--target",
            X86_TARGET,
        ])
    {
        return ExitCode::from(4);
    }
    let kernel = workspace_root()
        .join(format!("target/{X86_TARGET}/{}/kernel", profile_dir()))
        .display()
        .to_string();
    if !uefi_stage(
        &kernel,
        &esp_dir(),
        "the loader, the disk_throughput kernel (it WRITES to the NVMe) and the archive",
        false,
    ) {
        return ExitCode::from(4);
    }
    if stage_only {
        return ExitCode::SUCCESS;
    }

    let mut failed = Vec::new();
    for case in CASES
        .iter()
        .filter(|c| only.as_deref().is_none_or(|n| n == c.name))
    {
        if !run_case(case) {
            failed.push(case.name);
        }
    }
    eprintln!();
    if failed.is_empty() {
        eprintln!(
            "disk-throughput: every case produced its verdict (QEMU's numbers, not a disk's)"
        );
        ExitCode::SUCCESS
    } else {
        eprintln!("disk-throughput: FAILED cases: {}", failed.join(", "));
        ExitCode::from(1)
    }
}

fn run_case(case: &Case) -> bool {
    eprintln!();
    eprintln!("--- disk-throughput: {} ({}) ---", case.name, case.what);
    if let Err(e) = std::fs::write(nvme_image(), vec![0u8; 16 * 1024 * 1024]) {
        eprintln!(
            "disk-throughput: cannot write {}: {e}",
            nvme_image().display()
        );
        return false;
    }
    let log_path = workspace_root().join(format!("target/disk-throughput-{}.log", case.name));
    let Ok(mut log) = std::fs::File::create(&log_path) else {
        eprintln!("disk-throughput: cannot write {}", log_path.display());
        return false;
    };
    let Some((mut child, stdout)) = start_case(case) else {
        return false;
    };
    let transcript = read_case_transcript(stdout, &mut log);
    stop_case(&mut child);
    let ok = judge_case(case, &transcript);
    eprintln!(
        "disk-throughput: {}: {} (log {})",
        case.name,
        if ok { "as expected" } else { "WRONG" },
        log_path.display()
    );
    ok
}

/// Start `case`'s OVMF machine on a fresh NVMe image, its console piped. `None` when it did not
/// start.
fn start_case(case: &Case) -> Option<(std::process::Child, std::process::ChildStdout)> {
    let mut cmd = Command::new("helpers/qemu-uefi-x86_64.sh");
    cmd.arg(esp_dir())
        .args(case.qemu)
        .current_dir(workspace_root())
        .env("NIFE_NVME", nvme_image())
        .env("NIFE_UEFI_TIMEOUT", "300")
        .env_remove("NIFE_DISK")
        .env_remove("NIFE_NVME_ROOT_PORT")
        .env_remove("NIFE_NVME_DEVICE_OPTS")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    for (k, v) in case.env {
        cmd.env(k, v);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("disk-throughput: cannot start the OVMF runner: {e}");
            return None;
        }
    };
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        return None;
    };
    Some((child, stdout))
}

/// The case's console, into `log` whole and onto this terminal where it is the kernel's verdict or
/// the IOMMU's, until the kernel says it is done.
fn read_case_transcript(stdout: std::process::ChildStdout, log: &mut std::fs::File) -> String {
    // Read until the kernel says it is done, then stop the machine: it halts rather than exiting.
    // The runner's own bound (300 s) is what ends a boot that never gets there.
    let started = Instant::now();
    let mut transcript = String::new();
    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        let _ = writeln!(log, "{line}");
        if line.starts_with("disk-throughput:") || line.contains("vt-d") {
            println!("{line}");
        }
        transcript.push_str(&line);
        transcript.push('\n');
        if line.starts_with("disk-throughput: done") || started.elapsed() > Duration::from_secs(300)
        {
            break;
        }
    }
    transcript
}

/// **Stop the case's machine and wait for its emulator to be gone.**
fn stop_case(child: &mut std::process::Child) {
    let pid = child.id();
    // The runner wraps QEMU (qemu-bounded.sh), so kill its children first and then it, the order
    // `soak::job_mix_sweep` uses and for its reason.
    //
    // **Then wait for those children to be gone**, not only for the runner. A SIGKILLed QEMU
    // releases the disk image's lock when it has finished exiting, which is after `pkill` returns,
    // and QEMU is the runner's child rather than ours, so `child.wait()` does not wait for it.
    // Without this the next case's QEMU can find the image still locked ("Failed to get "write"
    // lock"), which is how milestone 594 (every VT-d unit translates its own devices)'s rehearsal lost `lba-4096` once on a loaded machine.
    let children: Vec<String> = Command::new("pgrep")
        .args(["-P", &pid.to_string()])
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let _ = Command::new("pkill")
        .args(["-9", "-P", &pid.to_string()])
        .status();
    let _ = child.kill();
    let _ = child.wait();
    let gone_by = Instant::now() + Duration::from_secs(10);
    while Instant::now() < gone_by
        && children.iter().any(|c| {
            Command::new("kill")
                .args(["-0", c])
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success())
        })
    {
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Whether the transcript holds every line `case` wants, and exactly one verdict after the
/// preflight.
fn judge_case(case: &Case, transcript: &str) -> bool {
    let mut ok = true;
    for want in case.want {
        if !transcript.contains(want) {
            eprintln!(
                "disk-throughput: {}: the transcript is missing {want:?}",
                case.name
            );
            ok = false;
        }
    }
    // Exactly one verdict, and the preflight printed before it: a verdict with no preflight above
    // it is the failure this whole command exists to prevent.
    let verdict_at = transcript.find("disk-throughput: verdict ");
    let preflight_at = transcript.find("disk-throughput: preflight 1/2");
    if transcript.matches("disk-throughput: verdict ").count() != 1
        || preflight_at.is_none()
        || verdict_at < preflight_at
    {
        eprintln!(
            "disk-throughput: {}: want exactly one verdict line, after the preflight",
            case.name
        );
        ok = false;
    }
    ok
}

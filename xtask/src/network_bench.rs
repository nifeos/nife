//! **Milestone 494 (a driver for the network card a PC actually has)'s bench boot, rehearsed**
//! (notes/e1000e.md).
//!
//! `cargo xtask network-bench` builds the `network_bench` kernel in release, stages it behind the
//! UEFI loader at `target/esp-network-bench` for a stick, and boots it once under OVMF with
//! `-device intel-iommu` and QEMU's `e1000e`, where the peer is helpers/network-bench-peer behind a
//! slirp `guestfwd`. The rehearsal must print a lease in slirp's network, the full 8 MiB, the
//! preflight before the verdict, and `verdict LEASED-AND-MEASURED`; anything else fails the
//! command. **The rate it prints is QEMU's and means nothing about a NIC.**

//! # Flags
//!
//! `--stage-only` builds and stages without booting, the night-of use. `--peer a.b.c.d:port` bakes
//! the LAN peer into the image (it has no other way to learn one); without it the rehearsal's
//! `10.0.2.9:9494` is used, which on a real LAN reaches nothing. `--debug` builds the debug
//! profile. Names provisional.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, ExitCode, Stdio};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::archive::initial_ramdisk_x86;
use crate::host::workspace_root;
use crate::uefi::uefi_stage;
use crate::{RELEASE, X86_TARGET, cargo_profiled, profile_dir};

/// slirp's address for the rehearsal peer (helpers/qemu-uefi-x86_64.sh's `guestfwd`).
const REHEARSAL_PEER: &str = "10.0.2.9:9494";

/// What the rehearsal's transcript must contain.
const WANT: &[&str] = &[
    "network-bench: nic       : 8086:10d3 82574L (QEMU's e1000e), link up, mac 52:54:00:e1:00:0e",
    "network-bench: preflight dmar scope : PASS",
    "network-bench: dhcp      : leased 10.0.2.15",
    "network-bench: transfer  : 8388608 bytes",
    "network-bench: verdict LEASED-AND-MEASURED",
];

fn esp_dir() -> std::path::PathBuf {
    workspace_root().join("target/esp-network-bench")
}

pub(crate) fn network_bench() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let mut stage_only = false;
    let mut debug = false;
    let mut peer: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--stage-only" => stage_only = true,
            "--debug" => debug = true,
            "--peer" if i + 1 < args.len() => {
                peer = Some(args[i + 1].clone());
                i += 1;
            }
            other => {
                eprintln!("network-bench: unknown argument {other}");
                eprintln!(
                    "usage: cargo xtask network-bench [--stage-only] [--debug] [--peer a.b.c.d:port]"
                );
                return ExitCode::from(4);
            }
        }
        i += 1;
    }
    if peer.is_some() && !stage_only {
        eprintln!(
            "network-bench: --peer names a LAN host the rehearsal cannot reach; use it with \
             --stage-only"
        );
        return ExitCode::from(4);
    }
    let peer = peer.unwrap_or_else(|| REHEARSAL_PEER.to_owned());

    // The archive first and then the kernel, for the measured-boot order `uefi::uefi_kernel`
    // documents. The peer reaches the kernel as `option_env!`, which cargo tracks, so a different
    // peer rebuilds it.
    RELEASE.store(!debug, Ordering::Relaxed);
    // SAFETY: xtask is single-threaded here; nothing else reads the environment concurrently.
    unsafe { std::env::set_var("NIFE_NETWORK_BENCH_PEER", &peer) };
    if !initial_ramdisk_x86()
        || !cargo_profiled(&[
            "build",
            "-p",
            "kernel",
            "--features",
            "network_bench",
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
        "the loader, the network_bench kernel and the archive",
        false,
    ) {
        return ExitCode::from(4);
    }
    eprintln!("network-bench: staged for peer {peer}");
    if stage_only {
        return ExitCode::SUCCESS;
    }
    if rehearse() {
        eprintln!("network-bench: the rehearsal produced its verdict (QEMU's rate, not a NIC's)");
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn rehearse() -> bool {
    let log_path = workspace_root().join("target/network-bench.log");
    let Ok(mut log) = std::fs::File::create(&log_path) else {
        eprintln!("network-bench: cannot write {}", log_path.display());
        return false;
    };
    let mut child = match Command::new("helpers/qemu-uefi-x86_64.sh")
        .arg(esp_dir())
        .current_dir(workspace_root())
        .env("NIFE_UEFI_TIMEOUT", "300")
        .env_remove("NIFE_DISK")
        .env_remove("NIFE_NVME")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("network-bench: cannot start the OVMF runner: {e}");
            return false;
        }
    };
    let pid = child.id();
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        return false;
    };

    // Read until the kernel says it is done, then stop the machine: it halts rather than exiting.
    let started = Instant::now();
    let mut transcript = String::new();
    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        let _ = writeln!(log, "{line}");
        if line.starts_with("network-bench:") || line.contains("vt-d") {
            println!("{line}");
        }
        transcript.push_str(&line);
        transcript.push('\n');
        if line.starts_with("network-bench: done") || started.elapsed() > Duration::from_secs(300) {
            break;
        }
    }
    // The runner wraps QEMU, so its children go first and then it; `disk_throughput::run_case`
    // has the reasoning, including why the children are waited for.
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

    let mut ok = true;
    for want in WANT {
        if !transcript.contains(want) {
            eprintln!("network-bench: the transcript is missing {want:?}");
            ok = false;
        }
    }
    let verdict_at = transcript.find("network-bench: verdict ");
    let preflight_at = transcript.find("network-bench: preflight");
    if transcript.matches("network-bench: verdict ").count() != 1
        || preflight_at.is_none()
        || verdict_at < preflight_at
    {
        eprintln!("network-bench: want exactly one verdict line, after the preflight");
        ok = false;
    }
    eprintln!(
        "network-bench: {} (log {})",
        if ok { "as expected" } else { "WRONG" },
        log_path.display()
    );
    ok
}

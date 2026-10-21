//! **One leg, booted and typed at**: the per-architecture leg, the USB keyboard boot, and
//! `swish_check_boot`, which boots the system and types every script.
//!
//! Cut out of `swish_check.rs` unchanged by milestone 847 (the swish-check harness is split
//! along its seams).
//!
//! Name: provisional (milestone 847's lane, 2026-10-11 UTC; §280 (the swish-check harness
//! splits into submodules, with its script in one)).

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::*;

/// One architecture's leg of [`swish_check`]: `aarch64`, `riscv64` or `x86_64`.
///
/// # The `x86_64` leg boots under firmware (milestone 182)
///
/// **Under OVMF, from the same `\EFI\BOOT\BOOTX64.EFI` `cargo xtask uefi-image` stages for a
/// USB stick**, not through QEMU's PVH `-kernel` loader. That image is the thing a customer boots
/// (DECISIONS §157, milestone 198's rung 1), so it is the one worth typing at: the loader, the
/// firmware's memory map and ACPI tables, and the console server's tee onto the firmware's screen
/// (milestone 400) are all on the path, and the PVH boot has none of them. What it costs over PVH
/// is recorded in milestone 182's block, measured rather than asserted.
///
/// Two `x86_64` differences, each forced by the machine rather than chosen:
///
/// - **The default kernel, not `--features shell`.** `x86_64` has no early hand-over: every boot
///   runs the tour and then hands over (milestone 268 (every architecture boots the same way)),
///   and `uefi_image` builds exactly that.
/// - **No virtio-rng, so `RDSEED`.** Every entropy device the kernel can find is virtio-mmio and
///   `q35` has no mmio bus, so the kernel builds the progenitor's entropy service on the CPU's seed
///   instruction instead (milestone 595 (provisional)), which the runner's `-cpu max` implements.
///   The leg asserts the progenitor said so, because a boot that silently fell back to no entropy
///   would otherwise surface only as the `uuid` lines failing.
///
/// Falsification: replayable `xtask/falsifications/swish_check.boot.swish_check_leg.patch`
pub(super) fn swish_check_leg(arch: &str) -> bool {
    // **Where the graphical launch rides** (2026-10-03 UTC, calef's ruling folding the
    // `swish-check-graphical` job in; milestone 632 (provisional)). `graphical_terminal` is a
    // plain spawn the shell waits on, so a launch ends the boot it is typed into. The first boot
    // therefore stays GPU-less, which is what lets its `graphical_terminal` line require the
    // refusal sentence on every ISA at no extra boot. The second boot (after the reboot) attaches a
    // gpu and no keyboard and launches at the end of its short script: the keystrokes are the
    // guest's UART, the configuration all three target boards have. A third, scriptless boot
    // attaches a gpu and a keyboard for the other arm. aarch64 and riscv64 only: x86_64 has no
    // virtio-gpu in its runner (`design/roadmap/0632-*.md`'s BUGS has what it would take).
    let graphical = arch != "x86_64";
    // The panic probe halts the kernel and the input-race probe proves itself on the first
    // script, so neither boots again.
    let one_boot = matches!(probe(), Probe::Panic | Probe::InputRace);
    swish_check_boot(arch, &SWISH_CHECK_SCRIPT, true, None)
        && (one_boot
            || swish_check_boot(
                arch,
                SWISH_CHECK_AFTER_REBOOT,
                false,
                graphical.then_some(Keystrokes::Serial),
            ))
        && (one_boot
            || !graphical
            || swish_check_boot(
                arch,
                SWISH_CHECK_KEYBOARD_BOOT,
                false,
                Some(Keystrokes::Device),
            ))
        && (one_boot || usb_keyboard_boot(arch))
}

/// The boot line the kernel prints when the USB keyboard driver configured a keyboard. Spelled
/// here rather than imported, as [`JOB_DID_NOT_RUN`] is: a phrase is enough for a transcript.
const USB_KEYBOARD_FOUND: &str = "  usb       : a keyboard on port ";

/// What [`usb_keyboard_boot`] types, as QEMU's `sendkey` names the keys.
const USB_KEYBOARD_KEYS: [&str; 11] = ["e", "c", "h", "o", "spc", "h", "e", "l", "l", "o", "ret"];

/// **The USB keyboard boot** (milestone 242 (USB host and HID, because on commodity hardware the
/// keyboard is not a UART)): the image the first boot built, with an xHCI controller and a USB
/// keyboard attached and nothing else, `echo hello` typed on that keyboard through the QEMU
/// monitor's `sendkey`, and `hello` required back on the console. On all three architectures,
/// behind each one's IOMMU (`iommu=smmuv3`, `riscv-iommu-pci`, `intel-iommu`), which the kernel
/// refuses to hand the controller over without.
///
/// What it proves, end to end: the kernel found and confined the controller, the EL0 driver reset
/// it, enumerated the keyboard and configured its interrupt endpoint (the `usb       :` line says
/// so before the prompt), the progenitor delegated the line discipline's endpoint to it, and a
/// key pressed on the device reached the shell as the same bytes a serial line sends. **The serial
/// line types nothing in this boot**, so the echo can have come from nowhere else.
///
/// The keyboard enumerates at **full speed** (`usb_version=1`), the speed real keyboards use, so
/// the Evaluate Context step and the frame-based interval encoding run here rather than first on
/// the bench. What it does not prove: a real keyboard on real silicon, which is calef's bench step
/// in milestone 242's block; a hub; a low-speed device, which QEMU cannot model. The name is
/// provisional.
fn usb_keyboard_boot(arch: &str) -> bool {
    let x86 = arch == "x86_64";
    let kvm = x86 && kvm_is_usable();
    eprintln!();
    eprintln!(
        "--- swish-check ({arch}): a USB keyboard on an xHCI controller types `echo hello` ---"
    );
    let sock = gpu_mon_socket(&format!("{arch}-usb-keyboard"));
    let _ = std::fs::remove_file(&sock);
    let boot_secs = if x86 {
        SWISH_CHECK_BOOT_SECS * 2
    } else {
        SWISH_CHECK_BOOT_SECS
    };

    let mut cmd = usb_keyboard_runner(arch, kvm, boot_secs + 2 * SWISH_CHECK_X86_LINE_SECS);
    cmd.env("NIFE_SCREEN_MON", &sock);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("swish-check ({arch}): failed to start the runner: {e}");
            return false;
        }
    };
    let stdout = child.stdout.take().expect("piped stdout");
    let (seen, reader) = read_console_plain(stdout);
    let failed = type_on_the_usb_keyboard(&seen, &sock, boot_secs);

    // SIGTERM on x86_64, for `swish_check_boot`'s reason: that runner is `qemu-bounded.sh`.
    stop_runner(&mut child, x86);
    let _ = reader.join();
    let _ = std::fs::remove_file(&sock);

    match failed {
        None => {
            eprintln!(
                "swish-check ({arch}): `echo hello` typed on a USB keyboard answered `hello`, \
                 through the xHCI driver behind the IOMMU"
            );
            true
        }
        Some(why) => {
            eprintln!("{}", transcript_tail(&transcript_now(&seen), 3000));
            eprintln!("swish-check ({arch}): FAIL: {why}");
            false
        }
    }
}

/// The runner [`usb_keyboard_boot`] starts: [`leg_runner`] with the USB keyboard attached and
/// nothing else that could type.
fn usb_keyboard_runner(arch: &str, kvm: bool, uefi_timeout_secs: u64) -> Command {
    let mut cmd = leg_runner(arch, kvm, uefi_timeout_secs);
    // The keyboard and nothing that could also type: no virtio keyboard (which `sendkey` would
    // reach instead), no gpu, no NIC.
    for var in [
        "NIFE_KEYBOARD",
        "NIFE_GPU",
        "NIFE_GPU_MON",
        "NIFE_NET",
        "NIFE_DISK",
        "NIFE_NVME",
    ] {
        cmd.env_remove(var);
    }
    cmd.env("NIFE_USB_KEYBOARD", "1");
    // **Full speed**, the speed a real keyboard enumerates at. QEMU's `usb-kbd` defaults to high
    // speed on an xHCI, which takes a different packet size for endpoint 0 and a different
    // interval encoding; `usb_version=1` makes it the device xenon will actually see.
    cmd.env("NIFE_USB_KEYBOARD_OPTS", ",usb_version=1");
    // **And on x86_64, MSI with no MSI-X**, which is what an Intel PCH's xHCI offers (xenon's), so
    // the kernel's MSI fallback is what delivers the interrupt here rather than first on the bench.
    // QEMU drops the MSI capability as well unless `msi=on` says to keep it. The other two
    // architectures route the controller's INTx pin and ignore both.
    if arch == "x86_64" {
        cmd.env("NIFE_USB_CONTROLLER_OPTS", ",msix=off,msi=on");
    }
    cmd
}

/// The USB keyboard boot's three steps once its runner is up: a prompt, the driver's `usb` line,
/// and `echo hello` typed through `sendkey` coming back as `hello`. `None` when all three held,
/// otherwise the first that did not, in words for the failure line.
fn type_on_the_usb_keyboard(
    seen: &Arc<Mutex<String>>,
    sock: &str,
    boot_secs: u64,
) -> Option<String> {
    let wait = |from: usize, done: &dyn Fn(&str) -> bool, secs: u64| -> bool {
        wait_until(secs, || {
            done(&degauge(&seen.lock().expect("transcript lock")[from..]))
        })
    };
    if !wait(0, &|t| t.ends_with("$ "), boot_secs) {
        return Some("the boot never reached a prompt".to_string());
    }
    if !seen
        .lock()
        .expect("transcript lock")
        .contains(USB_KEYBOARD_FOUND)
    {
        return Some(format!(
            "the boot printed no `{}` line: the kernel or the driver did not configure the \
             keyboard (the `usb       :` line, if any, says why)",
            USB_KEYBOARD_FOUND.trim()
        ));
    }
    let mark = seen.lock().expect("transcript lock").len();
    for key in USB_KEYBOARD_KEYS {
        sendkey(sock, key);
        std::thread::sleep(Duration::from_millis(200));
    }
    if !wait(
        mark,
        &|t| t.contains("echo hello\nhello\n"),
        SWISH_CHECK_LINE_SECS,
    ) {
        return Some(
            "`echo hello` typed on the USB keyboard did not come back as `hello`".to_string(),
        );
    }
    None
}

/// The last `chars` characters of `transcript`, which is what a failed boot prints: the end is
/// where it stopped.
fn transcript_tail(transcript: &str, chars: usize) -> String {
    transcript
        .chars()
        .rev()
        .take(chars)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

/// **Poll `done` every 100 ms until it holds or `secs` pass**, and say which. Every wait in this
/// file is one of these, because a boot that hangs is exactly the failure the gate is for, and a
/// gate that hangs with it reports nothing.
fn wait_until(secs: u64, mut done: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < deadline {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

/// A reader thread over the runner's stdout that keeps one transcript, with the line editor's
/// carriage returns dropped. The USB keyboard boot reads it whole; [`Console::read`] is the
/// swish-check boot's, which also takes the gauges out.
fn read_console_plain(
    mut stdout: std::process::ChildStdout,
) -> (Arc<Mutex<String>>, std::thread::JoinHandle<()>) {
    let seen = Arc::new(Mutex::new(String::new()));
    let collector = Arc::clone(&seen);
    let reader = std::thread::spawn(move || {
        let mut buf = [0u8; 1024];
        let mut split = Vec::new();
        while let Ok(n) = stdout.read(&mut buf) {
            if n == 0 {
                return;
            }
            let text = whole_characters(&mut split, &buf[..n]).replace('\r', "");
            collector.lock().expect("transcript lock").push_str(&text);
        }
    });
    (seen, reader)
}

/// **The runner a leg boots under**, before the devices each boot adds: on `x86_64`, the UEFI
/// image under OVMF with `uefi_timeout_secs` as its own bound and KVM when [`kvm_is_usable`]; on
/// the other two, the architecture's runner script against the kernel this leg built and its
/// initial ramdisk.
///
/// The runner directly rather than through `cargo run`, so the process this owns **is** QEMU (the
/// runner script `exec`s it). A `cargo run` in between would leave the emulator alive when the kill
/// lands on cargo, which is the leak CLAUDE.md's QEMU rule exists about.
fn leg_runner(arch: &str, kvm: bool, uefi_timeout_secs: u64) -> Command {
    let riscv = arch == "riscv64";
    if arch == "x86_64" {
        // OVMF, one core (the runner's default; the tour is where the second core is gated). The
        // runner bounds itself
        // with `qemu-bounded.sh`; the bound the caller passes is every wait it makes added up, so
        // it only ever fires on a leg that has already failed.
        let mut c = Command::new("helpers/qemu-uefi-x86_64.sh");
        c.arg(esp_dir());
        c.env("NIFE_UEFI_TIMEOUT", uefi_timeout_secs.to_string());
        c.env_remove("NIFE_NVME");
        if kvm {
            c.env("NIFE_ACCEL", "kvm");
        }
        c
    } else {
        let target = if riscv { RISCV_TARGET } else { TARGET };
        // **An NVMe disk, for the second mounted tree** (milestone 834 (ioping on nife and Linux)): the disk tree serves it
        // at `/disk`, and the after-reboot script times reads of it with `ioping`. Zeros, made
        // fresh for each boot. `x86_64` attaches none, because a blank NVMe disk there is offered
        // the install (`verdict::swish_check_omits` says so where the lines are left out).
        let nvme = crate::disk::swish_check_nvme_path();
        if let Err(e) = std::fs::write(&nvme, std::vec![0u8; crate::disk::SWISH_CHECK_NVME_BYTES]) {
            eprintln!("swish-check: could not write {nvme}: {e}");
        }
        let mut c = Command::new(if riscv {
            "helpers/qemu-runner-riscv64.sh"
        } else {
            RUNNER
        });
        c.arg(format!("target/{target}/{}/kernel", profile_dir()));
        c.env("NIFE_NVME", &nvme);
        c.env(
            "NIFE_INITIAL_RAMDISK",
            if riscv {
                riscv_initial_ramdisk_path()
            } else {
                initial_ramdisk_path()
            },
        );
        c
    }
}

/// **Stop a leg's runner and reap it.** SIGTERM rather than `kill()`'s SIGKILL on x86_64: that
/// runner is `qemu-bounded.sh`, which forwards TERM to QEMU (the other two runners `exec` the
/// emulator, so the kill is QEMU's).
fn stop_runner(child: &mut std::process::Child, x86: bool) {
    if x86 {
        let _ = Command::new("kill")
            .args(["-TERM", &child.id().to_string()])
            .status();
    } else {
        let _ = child.kill();
    }
    let _ = child.wait();
}

/// **Whether the `x86_64` leg can run on the host's own cores**: an `x86_64` Linux host whose
/// `/dev/kvm` this process can open for writing, which is what QEMU's `-accel kvm` needs. Asked of
/// the device rather than of an environment variable, so a developer's exported `NIFE_ACCEL` cannot
/// change what this gate boots (`swish_check` clears it for that reason).
///
/// Only the `x86_64` leg asks. The aarch64 and riscv64 legs stay on TCG on purpose: on CI's arm64
/// runner, TCG is what puts their kernels in front of a weakly ordered host (ci.yml's header), and
/// KVM there would trade that for speed the two legs do not need (0.2 s a line). The `x86_64` guest
/// is TSO under either, so KVM gives up no ordering the leg was ever shown. Milestone 628 (the x86_64
/// swish-check leg costs what the others do) measured what it buys; notes/benchmarks/swish-check-x86-leg.md. The name is
/// provisional.
pub(crate) fn kvm_is_usable() -> bool {
    cfg!(all(target_os = "linux", target_arch = "x86_64"))
        && std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/kvm")
            .is_ok()
}

/// **One boot of [`swish_check_leg`]**: build (when `fresh`), boot, type `script`, read the answers.
/// `fresh` is false for the second boot, which runs against the disk the first one left behind and
/// builds nothing, because what it proves is that the disk is the only thing carried across
/// (milestone 198 (a package manager) rung 3a: an installed package survives a reboot).
///
/// `graphics` attaches a virtio-gpu (and, for [`Keystrokes::Device`], a virtio keyboard) and, once
/// `script` is done and the prompt is back, types `graphical_terminal` and reads the screen
/// ([`launch_graphical_terminal`]). `None` attaches no gpu, which is what the refusal line needs.
///
/// Falsification: replayable `xtask/falsifications/swish_check.boot.swish_check_boot.patch`
pub(super) fn swish_check_boot(
    arch: &str,
    script: &[Line],
    fresh: bool,
    graphics: Option<Keystrokes>,
) -> bool {
    let x86 = arch == "x86_64";
    let Some(programs) = OptionalPrograms::find(arch, fresh) else {
        return false;
    };
    let leg = Leg {
        arch,
        x86,
        // KVM when this host can give it, and only to the x86_64 leg: see [`kvm_is_usable`].
        kvm: x86 && kvm_is_usable(),
        script,
        fresh,
        graphics,
        programs,
    };
    eprintln!();
    eprintln!(
        "--- swish-check ({arch}): boot {} and type at the prompt ---",
        if leg.kvm {
            "the UEFI image under OVMF, on KVM"
        } else if x86 {
            "the UEFI image under OVMF"
        } else {
            "`--features shell`"
        }
    );
    if !build_leg(arch, fresh) {
        return false;
    }

    let gpu_sock = gpu_mon_socket(&format!("{arch}-swish-check"));
    let mut cmd = swish_check_runner(arch, leg.kvm, fresh, script.len(), graphics, &gpu_sock);
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("swish-check: failed to start the runner: {e}");
            return false;
        }
    };
    let mut stdin = child.stdin.take().expect("piped stdin");
    let stdout = child.stdout.take().expect("piped stdout");
    let (console, reader) = Console::read(stdout);

    // Everything below must reach the kill, so failures are recorded rather than returned.
    let mut failed: Vec<String> = Vec::new();
    if drive_the_prompt(&leg, &console, &mut stdin, &gpu_sock, &mut failed) == Driven::PanicProbe {
        // Nothing else to read: the kernel is halted. Same kill as the end of this function.
        stop_runner(&mut child, x86);
        let _ = reader.join();
        if !failed.is_empty() {
            eprintln!("--- swish-check ({arch}) panic probe FAILED ---");
            for f in &failed {
                eprintln!("  {f}");
            }
        }
        return failed.is_empty();
    }
    let whole = transcript_failures(&leg, &console, &mut failed);

    // **The reboot phase** (milestone 805 (`reboot` at the prompt); calef asked on #1783 whether
    // each swish-check could finish with the reboot, and it can). Last, on the first boot only, and
    // only once every scripted line passed, so a failure here is about the reboot and says so. The
    // checks above read a snapshot taken before it, so the second boot's lines cannot confuse them.
    if fresh && probe() == Probe::None && failed.is_empty() {
        match reboot_phase(arch, &mut stdin, &console.seen) {
            Ok(said) => eprintln!("{said}"),
            Err(why) => {
                eprintln!("--- swish-check ({arch}) reboot phase transcript ---");
                eprintln!("{}", console.seen.lock().expect("transcript lock"));
                failed.push(why);
            }
        }
    }

    stop_runner(&mut child, x86);
    let _ = reader.join();
    if graphics.is_some() {
        let _ = std::fs::remove_file(&gpu_sock);
    }

    if failed.is_empty() {
        report_leg_passed(&leg);
        return true;
    }
    eprintln!();
    eprintln!("--- swish-check ({arch}) transcript ---");
    eprintln!("{whole}");
    eprintln!("--- swish-check ({arch}) FAILED ---");
    for f in &failed {
        eprintln!("  {f}");
    }
    false
}

/// **One boot of a leg, as [`swish_check_boot`] was asked for it**: what it types, whether it is
/// the first, which keystrokes the graphical launch takes, and what this checkout built.
struct Leg<'s> {
    arch: &'s str,
    x86: bool,
    kvm: bool,
    script: &'s [Line],
    fresh: bool,
    graphics: Option<Keystrokes>,
    programs: OptionalPrograms,
}

impl Leg<'_> {
    /// Whether this boot leaves `line` out ([`OptionalPrograms::skips`]).
    fn skips(&self, line: &str) -> bool {
        self.programs.skips(self.arch, line)
    }
}

/// How [`drive_the_prompt`] left the boot: typed at as far as it would go, or ended by the panic
/// probe, after which the kernel is halted and there is nothing more to read.
#[derive(PartialEq, Eq)]
enum Driven {
    Typed,
    PanicProbe,
}

/// **Everything a boot is typed at**: the banner, the progenitor's claims, the script and its line
/// times, and the graphical launch where the leg has one. Failures go to `failed`.
fn drive_the_prompt(
    leg: &Leg,
    console: &Console,
    stdin: &mut std::process::ChildStdin,
    gpu_sock: &str,
    failed: &mut Vec<String>,
) -> Driven {
    let (arch, x86) = (leg.arch, leg.x86);
    // The banner is the first claim: the progenitor built the console, the line editor, the input driver and
    // the shell, and gave the shell every capability it needs to say hello. A boot that dies in any
    // of that prints nothing, which is the symptom all three of this milestone's bugs shared.
    if !console.wait_after(0, "nife capability shell", SWISH_CHECK_BOOT_SECS) {
        failed.push(format!(
            "no prompt banner within {SWISH_CHECK_BOOT_SECS}s: the `--features shell` boot never \
             reached a shell"
        ));
        return Driven::Typed;
    }
    failed.extend(boot_claim_failures(x86, &|| {
        after_hand_over(&transcript_now(&console.seen))
    }));
    // **How long each line took**, typed to prompt-back, so every run reports its own margin
    // against the per-line bound rather than leaving it to be guessed after a red one.
    let line_secs = if x86 && !leg.kvm {
        SWISH_CHECK_X86_LINE_SECS
    } else {
        SWISH_CHECK_LINE_SECS
    };
    // **The panic probe types nothing**: the kernel panics on its own once the log service has
    // drained thirty flood lines, and what is checked is that the panic reached the UART
    // with a drainer attached (§175's escape), and that the flood lines did too.
    if probe() == Probe::Panic {
        panic_probe_verdict(arch, console, failed);
        return Driven::PanicProbe;
    }
    let took = type_script(leg, line_secs, console, stdin, failed);
    // Each line's time into the job's record too (milestone 808 (every gate accounts for its
    // time)), so the daily record can read `swish-check` by line without a log scrape.
    crate::time_record::items(
        &format!(
            "{}{}",
            crate::time_record::leg(arch),
            if leg.kvm { " kvm" } else { "" }
        ),
        if leg.fresh {
            "first boot"
        } else {
            "second boot"
        },
        "line",
        &took,
    );
    report_line_times(arch, leg.kvm, line_secs, took, failed);
    // The launch ends the boot (the shell waits on the session), so it is the last thing typed.
    if failed.is_empty()
        && probe() != Probe::Panic
        && let Some(keystrokes) = leg.graphics
    {
        match launch_graphical_terminal(arch, keystrokes, stdin, &console.seen, gpu_sock) {
            Ok(said) => eprintln!("{said}"),
            Err(why) => failed.push(why),
        }
    }
    Driven::Typed
}

/// **Every check that reads the finished transcript**: each line's answer, `rg`'s walk, a killed
/// thread, both gauges, the job pool and the flood probe. Failures go to `failed`; what comes back
/// is the whole raw transcript, for a failure to print.
fn transcript_failures(leg: &Leg, console: &Console, failed: &mut Vec<String>) -> String {
    let arch = leg.arch;
    let whole = console.raw.lock().expect("transcript lock").clone();
    let filtered = without_redraws(
        &console.seen.lock().expect("transcript lock"),
        &console.gauges.lock().expect("gauge lock"),
    );
    let transcript = after_hand_over(&filtered);
    // The transcript is printed on failure, because that is when somebody needs it. This
    // prints it on success too, and it exists because the notes in this tree quote real prompt
    // sessions: `NIFE_SHOW_TRANSCRIPT=1 script/swish-check --arch aarch64` is where the EXAMPLES
    // in notes/swish-language.md and notes/pipes.md come from, rather than from somebody retyping
    // what they remember the shell saying.
    if std::env::var_os("NIFE_SHOW_TRANSCRIPT").is_some() {
        eprintln!("--- swish-check ({arch}) transcript ---");
        eprintln!("{whole}");
    }
    if failed.is_empty() {
        failed.extend(answer_failures(leg.script, &transcript, &|l| leg.skips(l)));
    }
    if failed.is_empty()
        && leg.script.iter().any(|l| l.typed == RG_WALK_LINE)
        && leg.programs.rg_built
        && !leg.skips(RG_WALK_LINE)
    {
        failed.extend(rg_walk_failure(arch, &transcript));
    }
    failed.extend(kernel_fault_failure(&transcript));
    failed.extend(resolver_refusal_failure(&transcript));
    let raw_transcript = console.raw.lock().expect("transcript lock").clone();
    failed.extend(slot_gauge_failure(arch, &after_hand_over(&raw_transcript)));
    failed.extend(stack_gauge_failures(arch, &console.gauges, &filtered));
    failed.extend(job_pool_failure(&filtered));
    failed.extend(flood_failure(arch, &console.raw));
    whole
}

/// **Which of the leg's optional programs this checkout built**, and so which script lines a boot
/// types and which it skips, saying so.
struct OptionalPrograms {
    std_built: bool,
    rg_built: bool,
    basalt: bool,
}

impl OptionalPrograms {
    /// Look for each program's image, and say on the first boot what is skipped and why. `None`
    /// when the leg must refuse instead: CI is missing one it always builds.
    fn find(arch: &str, fresh: bool) -> Option<Self> {
        // **`std_exerciser` is in this boot's archive only if it was built** (milestone 595
        // (provisional)): `cargo xtask std-exerciser` compiles it against this worktree's patched std farm, which
        // `script/test` runs and a bare `script/swish-check` does not. Without it the progenitor has no
        // image and the line would answer "could not spawn", which is a fact about this checkout rather
        // than the boot, so the line is skipped and says why. **Not in CI**, where `test` always builds
        // it first: a missing image there means the build broke, and skipping would hide exactly that.
        let std_built = crate::farm::std_exerciser_elf(&format!("{arch}-unknown-nife")).exists();
        // `jig` is not optional: every archive builds it for its own target and refuses to pack
        // without it (`crate::archive`'s `SHIPPED_STD_PROGRAMS`), so its lines are never skipped.
        // Said once per leg, on the first boot: the second boot types no `std` line.
        if fresh && !std_built {
            if std::env::var_os("CI").is_some() {
                eprintln!(
                    "swish-check ({arch}): no std_exerciser was built for this architecture, and in CI \
                     `test` builds it first; refusing to skip its line"
                );
                return None;
            }
            eprintln!(
                "swish-check ({arch}): skipping `std_exerciser`: it is not built here (`cargo xtask \
                 std-exerciser` builds it; `script/test` runs that)"
            );
        }
        // **`rg` and its package come from basalt** (milestone 810 (`ripgrep` is packaged in basalt
        // and installed with `jig`)): `cargo xtask basalt-fetch` fetches basalt's published package
        // by its pinned digest, writes the repository the test index host serves, and takes the
        // package's `rg` for the lines that run it by path. Every `swish-check` row of
        // `script/ci-build` runs it first, so in CI a missing one means the fetch broke and its
        // lines are refused rather than skipped, exactly as `std_exerciser`'s are: fatal risk 1's
        // claim rests on them. A local run without a network skips them and says so.
        let rg_built = crate::basalt::ripgrep_elf(&format!("{arch}-unknown-nife")).exists();
        let basalt = crate::basalt::repository_index().exists();
        if fresh && !(rg_built && basalt) {
            if std::env::var_os("CI").is_some() {
                eprintln!(
                    "swish-check ({arch}): no `rg` or no repository from basalt for this \
                     architecture, and in CI `cargo xtask basalt-fetch` runs first; refusing to \
                     skip their lines"
                );
                return None;
            }
            eprintln!(
                "swish-check ({arch}): skipping `{}`'s lines and the lines that install from \
                 basalt: nothing was fetched from basalt here (`cargo xtask basalt-fetch` fetches \
                 it, as CI does)",
                crate::disk::INSTALLED_RIPGREP
            );
        }
        Some(Self {
            std_built,
            rg_built,
            basalt,
        })
    }

    /// Whether `line` is left out of this leg: omitted on this architecture, or needing a program
    /// that was not built.
    fn skips(&self, arch: &str, line: &str) -> bool {
        swish_check_omits(arch, line).is_some()
            || (crate::swish_check::script::FROM_BASALT.contains(&line) && !self.basalt)
            || ((line.contains(crate::disk::INSTALLED_RIPGREP) || line == RG_WALK_CD)
                && !self.rg_built)
            || ((line.starts_with("std_exerciser")
                // The line after `std_exerciser redirected > args.txt` reads the file it wrote, so
                // it goes with it; until 2026-10-03 it stayed and failed every local run that had
                // not built the exerciser ("no such name in this directory").
                || line == "wc < args.txt"
                || line.contains(crate::disk::INSTALLED_STD_ECHO)
                || line.contains(crate::disk::INSTALLED_STD_GREP)
                // `reach`'s package is built from the same workspace (milestone 892).
                || crate::swish_check::script::REACH_LINES.contains(&line))
                && !self.std_built)
    }
}

/// **Build what the first boot of a leg boots**, or nothing when `fresh` is false.
///
/// The same build the interactive boot takes, because a gate that builds something else is
/// gating something else. The FS server first (`user()` packs the initial ramdisk by reading the
/// ELF off disk), then the RedoxFS image, because the runner attaches the disk only when the
/// file is there and `<` and `>` need one.
fn build_leg(arch: &str, fresh: bool) -> bool {
    let riscv = arch == "riscv64";
    let x86 = arch == "x86_64";
    let target = if riscv { RISCV_TARGET } else { TARGET };
    !fresh
        || if x86 {
        // `uefi_image` packs the archive, builds the kernel against it, and stages the loader;
        // the FS server has to exist first so the archive carries it.
        redoxfs_server_build(X86_TARGET)
            && mkdisk()
            && mkredoxfs_of(SWISH_CHECK_DISK_MIB)
            && if probe_features().is_empty() {
                uefi_image()
            } else {
                uefi_image_with(Some(probe_features()))
            }
    } else if riscv {
        redoxfs_server_build(RISCV_TARGET) && mkdisk() && mkredoxfs_of(SWISH_CHECK_DISK_MIB) && initial_ramdisk_riscv()
    } else {
        redoxfs_server_build(TARGET) && mkredoxfs_of(SWISH_CHECK_DISK_MIB) && mkdisk() && user()
    } // After the archive build, whose packages it copies: a package the image's catalog
    // vouches for, a tampered copy, and an unvouched program, on the disk for the installer's
    // lines in the script (milestone 198 rung 3a).
    && crate::disk::seed_installed(arch)
    && (x86
        || crate::cargo_profiled(&[
            "build",
            "-p",
            "kernel",
            "--features",
            &if probe_features().is_empty() {
                "shell".to_string()
            } else {
                format!("shell,{}", probe_features())
            },
            "--target",
            target,
        ]))
}

/// **The runner [`swish_check_boot`] starts**: [`leg_runner`], bounded on x86_64 by every wait
/// the boot makes for a script of `script_lines` lines, with the disk, the entropy device, and the
/// gpu, NIC and package source each boot's arm needs.
fn swish_check_runner(
    arch: &str,
    kvm: bool,
    fresh: bool,
    script_lines: usize,
    graphics: Option<Keystrokes>,
    gpu_sock: &str,
) -> Command {
    let x86 = arch == "x86_64";
    let mut cmd = leg_runner(
        arch,
        kvm,
        SWISH_CHECK_BOOT_SECS * 2 + SWISH_CHECK_X86_LINE_SECS * (script_lines as u64 + 2),
    );
    // The RedoxFS disk, which `>`, `<`, `ls` and `rm` need; opt-in on the x86_64 runner, and its
    // header says why.
    if x86 {
        cmd.env("NIFE_UEFI_REDOXFS", "1");
    }
    cmd.env("NIFE_DISK", disk_path());
    // **The first boot ends with `reboot`** (milestone 805, [`reboot_phase`]), so the x86_64 runners
    // must let a reset reset: they pass `-no-reboot` otherwise, so a triple fault exits rather than
    // loops. The phase is bounded and reads every line, so a loop costs a timeout here.
    if fresh {
        cmd.env("NIFE_ALLOW_REBOOT", "1");
    }
    // A virtio-rng device (DECISIONS §120 (a QEMU-only virtio-rng stopgap for the interactive boot)'s 2026-08-26 amendment: "grant the QEMU-only virtio-rng
    // stopgap"), unlike the GPU/keyboard/NVMe flags above `test()` sets: this is the interactive
    // boot itself, not the bench boot sharing its runner, so there is no icount-drift reason to
    // keep it test-leg only, and the whole point of the amendment is that this boot should have
    // one. `cmd.env`, not `test()`'s own `std::env::set_var`, because this function builds its own
    // `Command` directly (see `NIFE_INITIAL_RAMDISK`/`NIFE_DISK` just above) rather than spawning
    // through the global-env-inheriting path `test()` uses.
    cmd.env("NIFE_RNG", "1");
    // **The gpu, and the keyboard on the device arm** (milestone 632 (provisional)): read by
    // `helpers/qemu-runner-*.sh`. The boot ignores the devices and the shell holds their grants
    // until the launch line is typed. The monitor socket is what `screendump` reads.
    if graphics.is_some() {
        let _ = std::fs::remove_file(gpu_sock);
        cmd.env("NIFE_GPU", "1");
        cmd.env("NIFE_GPU_MON", gpu_sock);
        if graphics == Some(Keystrokes::Device) {
            cmd.env("NIFE_KEYBOARD", "1");
        }
    }
    // **And a NIC** (milestone 590 (provisional)), on the two legs whose runner can attach one:
    // the progenitor builds `net_stack` from it, and `network_echo_client` reaches the runners'
    // echo peer through that. The runner attaches two (an mmio NIC and a PCIe one behind the
    // IOMMU); the kernel grants the progenitor the mmio one and the other sits unclaimed.
    //
    // **Not on the scriptless keyboard boot**: it types nothing that reaches the network, and a
    // boot with a gpu, a keyboard, a virtio-rng and the NIC all attached peaks at 30 of 32
    // capability slots against the 28 `kernel/src/cap.rs` records (measured 2026-10-03 UTC, the
    // fold's first run), a configuration no gate booted before the fold. Dropping the NIC keeps
    // this boot the one milestone 632 measured (gpu, keyboard, rng: 28); the 30 is recorded in
    // milestone 632's BUGS rather than hidden by raising the constant in a lane about CI.
    //
    // **Correction (2026-10-04 UTC, milestone 753 (provisional)): the keyboard boot has a NIC
    // anyway.** `host::cargo` sets `NIFE_NET=1` in this process's own environment, and `cmd`
    // inherits it, so skipping the line below removes nothing. The trace in
    // notes/capability-peak-trace.md found `net_stack`'s endpoint among the keyboard boot's 31;
    // without it that boot reads 30. Removing the variable here would change what the gate boots
    // and what `CAPABILITY_TABLE_PEAK_MEASURED` records, so it is left for whoever owns that.
    if !x86 && graphics != Some(Keystrokes::Device) {
        cmd.env("NIFE_NET", "1");
    }
    // **What the package source serves this leg** (milestone 198 rung 3a's fetch). The runner
    // starts `helpers/package-http-peer` once per connection, and it inherits this through QEMU;
    // `disk::stage_installed` filled the directory. Every leg, x86_64's included: its runner
    // attaches the `e1000e` and the peer whether or not `NIFE_NET` is set.
    if graphics != Some(Keystrokes::Device) {
        cmd.env(
            "NIFE_PACKAGE_SOURCE",
            crate::disk::package_source_dir(arch).display().to_string(),
        );
    }
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd
}

/// **What a swish-check boot's runner printed, three ways**, filled by the reader thread
/// [`Console::read`] starts.
///
/// A reader thread rather than blocking reads on the boot's own, because every wait needs a
/// deadline: a boot that hangs is exactly the failure this gate is for, and a gate that hangs
/// with it reports nothing.
///
/// Three views of one stream: `raw` is every byte, for the transcript a person reads; `seen` is
/// the same with the kernel's progenitor stack gauge taken out, which is what every check reads;
/// `gauges` is what was taken out, and where. [`GaugeFilter`] says why.
struct Console {
    seen: Arc<Mutex<String>>,
    raw: Arc<Mutex<String>>,
    gauges: Arc<Mutex<Vec<(usize, String)>>>,
}

impl Console {
    /// Start the reader thread over `stdout`, and hand back the views it fills.
    fn read(mut stdout: std::process::ChildStdout) -> (Self, std::thread::JoinHandle<()>) {
        let console = Self {
            seen: Arc::new(Mutex::new(String::new())),
            raw: Arc::new(Mutex::new(String::new())),
            gauges: Arc::new(Mutex::new(Vec::new())),
        };
        let collector = Arc::clone(&console.seen);
        let raw_collector = Arc::clone(&console.raw);
        let gauge_collector = Arc::clone(&console.gauges);
        let reader = std::thread::spawn(move || {
            let mut filter = GaugeFilter::default();
            // A probe's flood lines come out first, by the same rule (whole lines only), so the
            // checks read what they would without the flood; a spliced one stays in and fails them.
            let mut flood = GaugeFilter::with_needles(&[FLOOD_NEEDLE]);
            let mut flood_out = String::new();
            let mut flood_lines = Vec::new();
            let mut buf = [0u8; 1024];
            let mut split = Vec::new();
            while let Ok(n) = stdout.read(&mut buf) {
                if n == 0 {
                    return;
                }
                // The terminal's own carriage returns are the line editor's, not content. Dropped so
                // the checks below can be about what a person reads.
                let text = whole_characters(&mut split, &buf[..n]).replace('\r', "");
                raw_collector
                    .lock()
                    .expect("transcript lock")
                    .push_str(&text);
                // `seen` before `gauges`, both held, so no reader sees a gauge's offset past the text.
                let mut out = collector.lock().expect("transcript lock");
                let mut removed = gauge_collector.lock().expect("gauge lock");
                let text = if probe() == Probe::None {
                    text
                } else {
                    let before = flood_out.len();
                    flood.feed(&text, &mut flood_out, &mut flood_lines);
                    flood_out[before..].to_string()
                };
                filter.feed(&text, &mut out, &mut removed);
            }
        });
        (console, reader)
    }

    /// Poll for a needle **after `from`** with a deadline, `from` being how long the transcript was
    /// when the thing we are waiting for was asked for.
    ///
    /// The position matters because this gate deliberately types `wc < gate.txt` twice. A
    /// whole-transcript search finds the first one's echo instantly and the wait returns before the
    /// second line has been read at all, which then types the line after it into a prompt that has
    /// not appeared. That is exactly what the first version of this did.
    fn wait_after(&self, from: usize, needle: &str, secs: u64) -> bool {
        // `degauge`: a gauge spliced character-by-character into the very bytes being waited
        // for must not defeat an exact search the way it did in #1371 and #1420. See its doc.
        wait_until(secs, || {
            degauge(&self.seen.lock().expect("transcript lock")[from..]).contains(needle)
        })
    }

    /// How long the transcript is now, for a later [`Console::wait_after`] to start from.
    fn mark(&self) -> usize {
        self.seen.lock().expect("transcript lock").len()
    }

    /// **Wait for the prompt to come back before typing the next line**, and this is not politeness.
    /// The line editor echoes a character the moment it arrives, whether or not the shell has asked
    /// for a line yet, so typing ahead produces a transcript in which a command's echo appears
    /// *before* the `$ ` that should introduce it. The first version of this gate typed ahead and
    /// then failed to find its own echo, which is a bug in the gate rather than in the shell.
    ///
    /// A transcript ending in the bare prompt is the unambiguous "ready": the prompt is out and
    /// nothing has been echoed since.
    fn wait_for_prompt(&self, secs: u64) -> bool {
        wait_until(secs, || {
            self.seen.lock().expect("transcript lock").ends_with("$ ")
        })
    }
}

/// **What the checks read, which starts at the hand-over.** Every architecture runs its self-test
/// before handing over, and its `userspace` check kills a program on purpose (`outlaw`, reading a
/// kernel address); `x86_64`'s tour also kills two hand-built threads under supervision. Those are
/// the kernel's fault path working, printed before any process this gate is about exists, so the
/// fault check and the "was the kernel writing during the boot" test both start where the
/// progenitor does. The cut is the same on all three, at the line `kernel/src/boot.rs`'s
/// `hand_over` prints.
fn after_hand_over(t: &str) -> String {
    t.find(X86_HAND_OVER_START)
        .map_or(t, |at| &t[at..])
        .to_string()
}

/// **The progenitor's claims about its own boot**, each read from `now()`, the transcript after
/// the hand-over as it stands when that claim is checked: one complaint per claim that did not
/// hold. They are all in the transcript by the time the banner is, because the banner comes from a
/// shell the progenitor starts afterwards; there is nothing to wait for.
fn boot_claim_failures(x86: bool, now: &dyn Fn() -> String) -> Vec<String> {
    let mut failed = Vec::new();
    // **The progenitor gave the construction budget away, and says so from the inside** (milestone 22
    // (trusted init: verify it, and shrink what a broken one can do), the interactive increment). The progenitor prints this one line after deleting the root untyped and
    // before starting the shell, and it prints it only when `RETYPE` and `RETYPE_OBJ` on that
    // slot both answered `NoSuchSlot`: the capability is gone, not narrowed. The other branch
    // says "NOT dropped", so a boot that kept its budget fails here rather than passing quietly.
    //
    // **The message says what was not found, and nothing about the kernel.** It used to name
    // two capability states ("it still holds the root untyped, or the delete did not take"),
    // which are the two reasons the progenitor would print the other branch, and which this check has no
    // evidence for: all it ever knows is whether a string is in a transcript. On milestone
    // 230's first CI run it said exactly that about a boot where the progenitor had dropped the budget
    // and had said so, and sent a maintainer looking for a capability bug that does not exist.
    // A missing marker means a missing marker. The transcript is printed below; that is the
    // evidence, and this line's job is to say which string was wanted and how close it came.
    //
    // **And the progenitor measured every program it loaded** (milestone 104 (the
    // measurement continues past init)), which is the line that
    // keeps the second link of the chain from evaporating. A kernel built without the
    // measurement step refuses to boot at all, but a *table* that stopped naming things would
    // leave a system that boots, prompts, and vouches for nothing, and it would look exactly
    // like a healthy one. So the progenitor says which way it went either way, and the affirmative
    // sentence is what this gate reads. The other branch names the programs it refused, so a
    // boot that quietly stopped spawning half the prompt's commands fails here.
    //
    // **And its directory grants go through a bound badge** (milestone 606 (a directory walk
    // costs what it does on Linux), calef's rulings D and T1 of 2026-09-27). Every leg's image carries `redoxfs_server`, whose bytes carry the
    // subtree-grants note, so the progenitor must have read it; the `rm` lines below then run
    // through the server's own narrowing rather than a caretaker. A progenitor that lost the
    // note, or stopped reading it, says the negative sentence and fails here rather than
    // passing quietly on the caretaker it falls back to.
    //
    // **And the progenitor has an entropy service, from the source this leg's machine has**
    // (milestone 595 (provisional)). The `uuid` and `std_exerciser` lines below would fail
    // without one, but as a refused draw several lines on, which says nothing about why. This
    // names the source: a virtio-rng on the two `virt` machines, the CPU's `RDSEED` on `q35`,
    // where the kernel builds the service and the progenitor is granted it. The negative is
    // the kernel's own refusal, which both of its no-entropy sentences end with.
    let claims: [(&str, &str, &str); 4] = [
        (
            "giving the construction budget away",
            "construction budget dropped; retype answers NoSuchSlot",
            "construction budget NOT dropped",
        ),
        (
            "measuring the programs it loads",
            "every program measured against the archive table",
            "measurement refused",
        ),
        (
            "how it delivers a directory grant",
            "directory grants get a bound badge",
            "directory grants get a caretaker",
        ),
        (
            "its entropy source",
            if x86 {
                "entropy service up; the kernel built it on the CPU's seed instruction"
            } else {
                "entropy service up; drew real bytes from a virtio-rng device"
            },
            "nothing at the prompt can draw random bytes",
        ),
    ];
    for (subject, positive, negative) in claims {
        failed.extend(boot_claim_complaint(&now(), subject, positive, negative));
    }
    // **And a boot with a network starts a name resolver on it** (milestone 892 (a std
    // program at the prompt holds the network and a resolver)), asking the name server the
    // lease named, which under slirp is always its own 10.0.2.3. A boot with no NIC (the
    // keyboard boot) says neither sentence and is not asked.
    let now = now();
    if now.contains("network stack up; DHCP leased")
        && let Some(complaint) = boot_claim_complaint(
            &now,
            "its name resolver",
            "name resolver up; asks 10.0.2.3, the lease's name server",
            "no name resolver",
        )
    {
        failed.push(complaint);
    }
    failed
}

/// **The panic probe's verdict**: the probe's message reached the UART, and the panic handler's
/// with it, counting the flood lines before it.
fn panic_probe_verdict(arch: &str, console: &Console, failed: &mut Vec<String>) {
    if console.wait_after(0, "kernel log panic probe", 120) {
        let raw_now = console.raw.lock().expect("transcript lock").clone();
        let (whole, spliced) = flood_counts(&raw_now);
        eprintln!(
            "swish-check ({arch}): panic probe: the panic reached the UART with the log \
             service attached; {whole} whole flood lines before it, {spliced} spliced"
        );
        if !raw_now.contains("[PANIC]") {
            failed.push("the probe's message arrived without the panic handler's".into());
        }
    } else {
        failed.push("the panic probe never reached the UART".to_string());
    }
}

/// **Type every line of `script` the leg does not skip**, each at a fresh prompt, and wait for its
/// echo; then wait for the last line's prompt. Returns each line typed with how long it took,
/// typed to prompt-back. Stops at the first line that fails, recording why.
fn type_script<'s>(
    leg: &Leg<'s>,
    line_secs: u64,
    console: &Console,
    stdin: &mut std::process::ChildStdin,
    failed: &mut Vec<String>,
) -> Vec<(&'s str, Duration)> {
    let mut took: Vec<(&str, Duration)> = Vec::new();
    let mut previous: Option<(&str, Instant)> = None;
    for &Line {
        typed: line,
        echoed,
        asks,
        ..
    } in leg.script
    {
        if leg.skips(line) {
            continue;
        }
        // **`rg`'s lines have bounds of their own** ([`RG_LINE_SECS`], [`RG_WALK_SECS`]): the
        // wait for this prompt is the previous line's run.
        let bound = match previous {
            Some((RG_WALK_LINE, _)) => RG_WALK_SECS,
            Some((prev, _))
                if prev.contains(crate::disk::INSTALLED_RIPGREP)
                    || crate::swish_check::script::FROM_BASALT.contains(&prev) =>
            {
                line_secs.max(RG_LINE_SECS)
            }
            _ => line_secs,
        };
        if !console.wait_for_prompt(bound) {
            failed.push(format!(
                "the prompt never came back to take `{line}`; the line before it did not finish"
            ));
            break;
        }
        if let Some((prev, typed)) = previous {
            took.push((prev, typed.elapsed()));
        }
        previous = Some((line, Instant::now()));
        if let Err(why) = type_one_line((line, echoed, asks), line_secs, console, stdin) {
            failed.push(why);
            break;
        }
    }
    // One more, for the last line: every other answer is bounded by the next line's wait, and
    // the last one has no next line. Without this the transcript is read while the final
    // command is still running.
    if failed.is_empty() && probe() != Probe::Panic && !console.wait_for_prompt(line_secs) {
        failed.push("the prompt never came back after the last line".to_string());
    }
    if let (true, Some((prev, typed))) = (failed.is_empty(), previous) {
        took.push((prev, typed.elapsed()));
    }
    took
}

/// **Type one script line at a prompt that is ready for it**, wait for its echo, and press `^C`
/// under a line [`SWISH_CHECK_INTERRUPTED`] names once it says it is running. `Err` says which
/// step failed.
fn type_one_line(
    (line, echoed, asks): (&str, &str, Option<(&str, &str)>),
    line_secs: u64,
    console: &Console,
    stdin: &mut std::process::ChildStdin,
) -> Result<(), String> {
    let at = console.mark();
    // A line abandoned with `^C` is sent without Enter: the shell has already discarded
    // it and painted a fresh prompt, which the next line's wait reads.
    let enter = if interrupted_at_prompt(line) {
        ""
    } else {
        "\n"
    };
    // **`NIFE_SWISH_ENTER_PAUSE_MS` types like a person**: the line, a pause, then Enter
    // (2026-10-04 UTC, name provisional). Past the system log service's 250 ms flush, any
    // kernel line queued at the prompt is drawn above a redraw of the whole typed line,
    // which is the window the noteless flake fell into once in hundreds of runs; this
    // opens it on every line, so [`without_redraws`] is tested by a boot and not by luck.
    // `--input-race` sets its own, so Enter lands inside the input driver's hold.
    let pause = std::env::var("NIFE_SWISH_ENTER_PAUSE_MS")
        .ok()
        .and_then(|ms| ms.parse().ok())
        .or((probe() == Probe::InputRace).then_some(INPUT_RACE_ENTER_PAUSE_MS))
        .map(Duration::from_millis);
    let typed = match pause {
        Some(p) if !enter.is_empty() => {
            let first = write!(stdin, "{line}").and_then(|()| stdin.flush());
            std::thread::sleep(p);
            first.and_then(|()| write!(stdin, "{enter}"))
        }
        _ => write!(stdin, "{line}{enter}"),
    };
    if typed.is_err() || stdin.flush().is_err() {
        return Err(format!("could not type `{line:?}` at the prompt"));
    }
    let echo_bound = if line.contains(crate::disk::INSTALLED_RIPGREP) {
        line_secs.max(RG_LINE_SECS)
    } else {
        line_secs
    };
    if !console.wait_after(at, &format!("{echoed}\n"), echo_bound) {
        return Err(format!("the prompt never echoed `{echoed}` for {line:?}"));
    }
    // **A question, answered once it is asked** (milestone 892): the reply goes to the running
    // program, whose read the terminal's line discipline answers with what was typed.
    if let Some((cue, reply)) = asks {
        if !console.wait_after(at, cue, line_secs) {
            return Err(format!("`{line}` never asked {cue:?}"));
        }
        if writeln!(stdin, "{reply}").is_err() || stdin.flush().is_err() {
            return Err(format!("could not answer {cue:?} under `{line}`"));
        }
    }
    if SWISH_CHECK_INTERRUPTED.contains(&line) {
        if !console.wait_after(at, SWISH_CHECK_RUNNING, line_secs) {
            return Err(format!(
                "`{line}` never said it was running under supervision ({SWISH_CHECK_RUNNING:?})"
            ));
        }
        // ETX, the byte a terminal sends for `^C`, with no newline: the line editor acts on
        // it the moment it arrives.
        if stdin.write_all(&[0x03]).is_err() || stdin.flush().is_err() {
            return Err(format!("could not press ^C under `{line}`"));
        }
    }
    if SWISH_CHECK_SUSPENDED.contains(&line) {
        if !console.wait_after(at, SWISH_CHECK_RUNNING, line_secs) {
            return Err(format!(
                "`{line}` never said it was running under supervision ({SWISH_CHECK_RUNNING:?})"
            ));
        }
        // SUB, the byte a terminal sends for `^Z`.
        if stdin.write_all(&[0x1a]).is_err() || stdin.flush().is_err() {
            return Err(format!("could not press ^Z under `{line}`"));
        }
    }
    Ok(())
}

/// **Report what each typed line cost**, and fail a leg whose median line is far off its own
/// baseline.
fn report_line_times(
    arch: &str,
    kvm: bool,
    line_secs: u64,
    mut took: Vec<(&str, Duration)>,
    failed: &mut Vec<String>,
) {
    // Every line's time, in script order, when that was asked for. `NIFE_SHOW_LINE_TIMES`
    // prints the table alone and `NIFE_SHOW_TRANSCRIPT` prints it beside the whole transcript
    // (which is what a person reading a session wants and far too much text to diff a
    // before/after measurement out of). The split exists for exactly that: the paint path's
    // legs are priced by this table (`SWISH_CHECK_X86_LINE_SECS`'s own doc), and a lane that
    // changes the paint path needs the table from two runs, not two transcripts.
    if std::env::var_os("NIFE_SHOW_TRANSCRIPT").is_some()
        || std::env::var_os("NIFE_SHOW_LINE_TIMES").is_some()
    {
        for (l, d) in &took {
            eprintln!("swish-check ({arch}): {:6.2}s  {l}", d.as_secs_f64());
        }
    }
    // **The leg's median line against its own baseline** (milestone 722 (swish-check
    // fails a leg that costs five times the others per line)): a cost defect
    // shows as a leg far off its own history, which a per-line bound sized to hide a real hang
    // cannot see. See [`LEG_COST_BASELINE`] for why this is not a comparison between legs.
    let accel = if kvm { "kvm" } else { "tcg" };
    let secs: Vec<f64> = took.iter().map(|(_, d)| d.as_secs_f64()).collect();
    let cost = leg_cost_verdict(arch, accel, &secs);
    eprintln!(
        "swish-check ({arch}): median {:.2}s a line under {accel}{}",
        cost.median,
        match cost.baseline {
            Some(b) => format!(
                ", baseline {b:.2}s, limit {:.2}s ({LEG_COST_FACTOR}x)",
                b * LEG_COST_FACTOR
            ),
            None => ", no baseline row".to_string(),
        }
    );
    if cost.over {
        failed.push(format!(
            "{arch} under {accel} costs a median {:.2}s a line, over {LEG_COST_FACTOR}x its \
             baseline {:.2}s. A leg this far off its own history is a cost defect rather than \
             a hang (milestone 628 was a runnable boot thread halting until the next tick; \
             notes/benchmarks/swish-check-x86-leg.md). If this host is simply slower, measure \
             it and add a row to `LEG_COST_BASELINE` with the source.",
            cost.median,
            cost.baseline.unwrap_or(0.0)
        ));
    }
    took.sort_by_key(|t| std::cmp::Reverse(t.1));
    let total: Duration = took.iter().map(|(_, d)| *d).sum();
    eprintln!(
        "swish-check ({arch}): {} lines in {:.1}s; slowest, against a {line_secs}s \
         bound per line: {}",
        took.len(),
        total.as_secs_f64(),
        took.iter()
            .take(3)
            .map(|(l, d)| format!("`{l}` {:.1}s", d.as_secs_f64()))
            .collect::<Vec<_>>()
            .join(", "),
    );
}

/// **Read each typed line's answer out of the transcript and check it**: one complaint per wanted
/// phrase missing, per `^C`'d line that answered, and per tagged job that did not run.
///
/// Walked in order with a moving cursor, not searched. The script types `wc < gate.txt`
/// twice on purpose and the two answers are the whole point of the append arm, so a search
/// that found either one would read the same answer for both lines and pass a `>>` that had
/// truncated.
fn answer_failures(
    script: &[Line],
    transcript: &str,
    skipped: &dyn Fn(&str) -> bool,
) -> Vec<String> {
    let mut failed = Vec::new();
    let mut cursor = 0usize;
    for (i, l) in script.iter().enumerate() {
        if skipped(l.typed) {
            continue;
        }
        let Some((answer, next)) = swish_check_answer(transcript, cursor, l.echoed) else {
            failed.push(format!("`{}` produced no answer at all", l.echoed));
            continue;
        };
        cursor = next;
        // Everything between this line's answer and the next typed line's echo, for a
        // failure to say whether the wanted text was written and the answer cut short.
        let next_echo = script[i + 1..]
            .iter()
            .find(|l| !skipped(l.typed))
            .map(|l| format!("$ {}\n", l.echoed));
        let begun = next - answer.len();
        let until = next_echo
            .and_then(|e| transcript[next..].find(&e).map(|at| next + at))
            .unwrap_or(transcript.len());
        failed.extend(one_answer_failures(l, answer, &transcript[begun..until]));
    }
    failed
}

/// The complaints about one line's `answer`; `before_next` is the transcript from that answer up
/// to the next typed line's echo.
fn one_answer_failures(l: &Line, answer: &str, before_next: &str) -> Vec<String> {
    let Line {
        typed,
        echoed: line,
        jobs,
        answer: want,
        ..
    } = *l;
    let mut failed = Vec::new();
    // **A line abandoned with `^C` ran nothing** (DECISIONS §227 option D): the
    // shell edits its own line now, so `^C` at the prompt is a byte its editor
    // turns into a discard. Anything between the echo and the next prompt means
    // the line ran anyway.
    if interrupted_at_prompt(typed) && !answer.trim().is_empty() {
        failed.push(format!(
            "{typed:?} was abandoned with ^C and still answered {:?}",
            answer.trim()
        ));
    }
    // **Every wanted phrase, not the first**, because one answer can carry several
    // independent claims and checking one of them makes the rest decoration. `caps`
    // is the case that forced it: it prints the shell's whole endowment, and a gate
    // that read only the clock row would pass a boot that had stopped granting the
    // shell a directory, which is the wiring the headline of milestone 31 (a capability
    // shell: designation is authorization) rests on.
    // **And whether the program said it anyway** (the noteless flake, 2026-10-04
    // UTC): an answer cut short by something the gate read as a prompt is a
    // misread transcript, not a silent program, and the line says which.
    for want in want {
        if !answer.contains(want) {
            let misread = if before_next.contains(want) {
                "; the wanted text is in the transcript before the next line's \
                 echo, so the program wrote it and the gate cut the answer short \
                 (see `without_redraws`)"
            } else {
                ""
            };
            failed.push(format!(
                "`{line}` answered {:?}, wanted {want:?}{misread}",
                answer.trim()
            ));
        }
    }
    // **And a line that runs jobs ran them** ([`Line`]'s doc): the summary's job
    // count is summed from these tags, so a tagged line whose job was never built
    // must fail here, including the ones whose answer is checked by a later line.
    if jobs > 0
        && let Some(said) = JOB_DID_NOT_RUN.iter().find(|s| answer.contains(**s))
    {
        failed.push(format!(
            "`{line}` runs {jobs} job(s) and one did not run: its answer carries \
             {said:?} ({:?})",
            answer.trim()
        ));
    }
    failed
}

/// **And `rg`'s walk found exactly what the host's found** (milestone 121 (`ripgrep` on nife:
/// enumeration as a capability, and what the walk costs)), which no list of
/// wanted phrases can say: a phrase list passes an answer that dropped a directory.
fn rg_walk_failure(arch: &str, transcript: &str) -> Option<String> {
    // Only a boot whose script types the walk asks, so an answer that cannot be found is a failure:
    // passing it would let the one check no phrase list can make go quiet.
    let Some((answer, _)) = swish_check_answer(transcript, 0, RG_WALK_LINE) else {
        return Some(format!(
            "`{RG_WALK_LINE}` was typed and its answer is not in the transcript"
        ));
    };
    let expected = crate::disk::ripgrep_walk_expected();
    match std::fs::read_to_string(&expected)
        .map_err(|e| format!("{}: {e}", expected.display()))
        .and_then(|want| rg_walk_verdict(answer, &want))
    {
        Ok(said) => {
            eprintln!("swish-check ({arch}): {said}");
            None
        }
        Err(why) => Some(why),
    }
}

/// **A name asked at the prompt must reach the name server** (milestone 892 (a std program at the
/// prompt holds the network and a name resolver)). [`RESOLVING_LINE`]'s listed location is the one
/// name a program resolves at this prompt. Under slirp the lease's 10.0.2.3 forwards to the host's
/// resolver, which knows no `.test` name, so the answer is `NotFound` with a network and
/// `TimedOut` without one; neither is asserted. What is refused is `NetworkUnreachable`: the
/// resolver's own socket was refused a step, and no query left the guest. Nothing else here
/// notices, because `jig` falls back to the address the sources give, so every other line passes
/// on a boot where no name resolves.
///
/// Falsification: replayable `xtask/falsifications/swish_check.boot.resolver_refusal_failure.patch`
fn resolver_refusal_failure(transcript: &str) -> Option<String> {
    let (answer, _) = swish_check_answer(transcript, 0, RESOLVING_LINE)?;
    let refused = answer
        .lines()
        .find(|l| l.contains(": NetworkUnreachable"))?;
    Some(format!(
        "`{RESOLVING_LINE}` asked a name that never reached the name server: {:?}. The resolver \
         the progenitor started was refused a network step (the parentheses name it), so no \
         program at this prompt can resolve a name; `build_domain_name_resolver`'s endowment is \
         the first place to look.",
        refused.trim()
    ))
}

/// **Nothing may have died** (milestone 233 (`login` dies on every boot, and the boot says it is
/// ready)), which is the ratchet the lane of milestone 230 (`script/shell-check` is red on `main`)
/// identified and deliberately left, because it would have been red on both architectures until
/// `login` was fixed. It was: `login` faulted at `_start` on every interactive boot, on both
/// ISAs, for an unknown length of time, while every check above passed and the progenitor went on printing
/// a line about a login service.
///
/// **The whole transcript, not the boot**, because the typed script is where a death would be
/// most surprising. Nothing in `SWISH_CHECK_SCRIPT` traps on purpose: the three lines that fail
/// (`wc` and `doc` with nothing named, `least_authority_demo` with no argument) are all refusals, two at the
/// prompt before anything is spawned and one an ordinary non-zero exit, and `rm gate.txt`'s
/// refusal is an answer rather than a fault. `echo $?` reading `2` right after `least_authority_demo` is this
/// gate's own proof of that distinction: a thread the kernel killed does not get to set a status.
/// A trap in any of them would be a real regression rather than a false positive here.
///
/// **What a deliberate trap does was measured rather than assumed** (milestone 233), because
/// milestone 230's lane named it as the thing it could not cheaply find out. `least_authority_demo` was
/// patched to `supervision_protocol::fail()` on `least_authority_demo 5` and this gate run against it. Two
/// results, and the second is the more interesting one:
///
///   1. This check fires, naming the thread and the reason, so it is a check that can fail
///      rather than one that only ever passes. That mattered: it was written against a tree
///      where `login` had just stopped dying, so nothing else would have exercised it.
///   2. **The prompt never comes back.** The run also failed with "the prompt never came back
///      to take `least_authority_demo 6`", because the shell waits on the result endpoint of a job that
///      faulted instead of sending, and nothing wakes that wait. A spawned command that traps
///      hangs the shell rather than returning a status. That is a real limitation this gate now
///      makes visible, and it is `components/src/swish.rs`'s to carry rather than this file's.
///
/// The first two of `KERNEL_FAULT_TOKENS` rather than one string, and that constant's own doc
/// carries why. The same pair is what `kernel_wrote_during_boot` reads, which is the other half
/// of this: a fault during the boot is both a failure here and the one thing that can shuffle a
/// console line, so the two checks are looking at one fact from two sides.
fn kernel_fault_failure(transcript: &str) -> Option<String> {
    let at = transcript.find(KERNEL_FAULT_TOKENS[0])?;
    if !transcript[at..].contains(KERNEL_FAULT_TOKENS[1]) {
        return None;
    }
    let line = transcript[at..].lines().next().unwrap_or("").trim_end();
    Some(format!(
        "the kernel reported killing a user thread during this run: {line:?}. Every \
         program this boot starts is supposed to survive it, and one that does not is \
         invisible everywhere else: the progenitor's own report says what the progenitor measured, not what \
         stayed alive. The transcript below has the fault's registers, and `llvm-objdump -d` \
         on the program at that `pc` names the function."
    ))
}

/// **And the capability-slot gauge** (milestone 231 (nothing counts how many capability slots a
/// boot actually uses)), which is the other half of this pair of milestones. Two claims, and neither is a margin picked out of the air.
///
/// The line must be *there*, because a gauge that stopped printing is a gauge nobody would miss
/// until the wall arrived again, which is exactly how `CAPABILITY_TABLE_SLOTS` came to be raised
/// three times reactively. And it must not say `ABOVE`, which is the kernel's own word for a
/// boot that went past the peak `kernel::cap::CAPABILITY_TABLE_PEAK_MEASURED` records. That
/// constant is a measurement rather than a target, so what this fails on is a recorded fact
/// going stale, not a boot getting close to something. The fix when it fires is to measure,
/// update the constant, and re-read the headroom arithmetic beside `CAPABILITY_TABLE_SLOTS`.
/// Read from the raw transcript: [`GaugeFilter`] takes this line out of the one the waits read.
fn slot_gauge_failure(arch: &str, raw_after_hand_over: &str) -> Option<String> {
    match raw_after_hand_over
        .lines()
        .rfind(|l| l.contains(SLOT_GAUGE))
    {
        Some(line) => {
            eprintln!("swish-check ({arch}):{}", line.trim_end());
            line.contains("ABOVE").then(|| {
                format!(
                    "this boot used more capability slots than the tree records: {:?}. The \
                     number beside CAPABILITY_TABLE_SLOTS in kernel/src/cap.rs is now stale; \
                     measure, update CAPABILITY_TABLE_PEAK_MEASURED, and re-read that constant's \
                     headroom arithmetic rather than raising the ceiling reflexively.",
                    line.trim()
                )
            })
        }
        None => Some(format!(
            "the boot never printed {SLOT_GAUGE:?}. The kernel says this from the scheduler's \
             idle loop once the mark has settled (kernel::cap::report_peak), so either the boot \
             never idled or the gauge stopped being printed; the second is the one that matters, \
             because it is the only thing standing between this tree and a fourth reactive raise \
             of CAPABILITY_TABLE_SLOTS."
        )),
    }
}

/// **And the progenitor's stack gauge** (milestone progenitor-stack (provisional)), the same
/// shape as the slot gauge for the same reason: its stack was raised three times, each time by
/// overflowing it. The line must be there, and it must not say `BELOW`, which is the kernel's word
/// for a boot that left less than `kernel::progenitor_stack::HEADROOM_FLOOR` of the stack unused.
/// Every line is echoed with the prompt line it followed, which makes the transcript a per-command
/// measurement: a line after `jig install greeting@0.1.0` is that path.
fn stack_gauge_failures(
    arch: &str,
    gauges: &Mutex<Vec<(usize, String)>>,
    filtered: &str,
) -> Vec<String> {
    let gauges: Vec<(String, &str)> = gauges
        .lock()
        .expect("gauge lock")
        .iter()
        .filter(|(_, line)| !line.contains(SLOT_GAUGE))
        .map(|(at, line)| (line.clone(), gauge_follows(filtered, *at)))
        .collect();
    let mut failed = Vec::new();
    for (gauge, after) in &gauges {
        eprintln!("swish-check ({arch}): {gauge}  [after {after:?}]");
        if gauge.contains("BELOW") {
            failed.push(format!(
                "the progenitor's stack came within its headroom floor: {gauge:?}, after \
                 {after:?}. Something on that path grew a frame; notes/stack/progenitor-stack.md says how \
                 to list the progenitor's largest frames. Trim the frame, or raise \
                 PROGENITOR_STACK_PAGES in kernel/src/user/progenitor_boot.rs, measurement beside it."
            ));
        }
    }
    if gauges.is_empty() {
        failed.push(format!(
            "the boot never printed {:?}. The kernel says this from the scheduler's idle loop, and \
             on every architecture (kernel::progenitor_stack); a gauge that stopped \
             printing is how the stack got raised three times by overflowing it.",
            GaugeFilter::NEEDLE.trim()
        ));
    }
    failed
}

/// **And the job pool came back** (lane fix/riscv64-progenitor-oom, 2026-10-03 UTC). `free`'s
/// `Yours:` line is the progenitor's job pool as `MemoryRegion::USAGE` sees it, and a pool whose
/// regions all came back holds `free` itself and little else. Until that date a region returned
/// out of order stayed a hole for the rest of the boot, so the two pipelines before this line
/// left two regions behind on every run, every architecture, and the races that sometimes added
/// more were what ran the pool out under `std_exerciser`. That leak was in the transcript all
/// along, as 576 used, and nothing read it: this is the read that turns it from intermittent
/// into a failure on every run.
fn job_pool_failure(filtered: &str) -> Option<String> {
    let used = yours_used_kib(filtered)?;
    (used > YOURS_CEILING_KIB).then(|| {
        format!(
            "`free` reports {used} KiB of the progenitor's job pool in use, above the \
             {YOURS_CEILING_KIB} KiB that `free` itself and one job the reaper has not reached yet \
             account for. Something left a region behind: memory_regions' \
             `RegionTable::return_to_parent` is where a returned job's pages go back, and \
             notes/swish-check-flake.md has how this was traced."
        )
    })
}

/// **Milestone 342's flood verdict** (milestone 342 (the kernel and the `console` server drive one
/// UART from two address spaces, and nothing arbitrates)): every kernel flood line on a line of its
/// own, or, for the detached control, how many were not. `None` when no flood probe is running.
fn flood_failure(arch: &str, raw: &Mutex<String>) -> Option<String> {
    if !matches!(probe(), Probe::Flood | Probe::FloodDetached) {
        return None;
    }
    let (whole, spliced) = flood_counts(&raw.lock().expect("transcript lock"));
    eprintln!(
        "swish-check ({arch}): flood: {whole} kernel lines whole, {spliced} spliced into \
         other output{}",
        if probe() == Probe::FloodDetached {
            " (the detached control: the kernel printing for itself, as before milestone 342)"
        } else {
            ""
        }
    );
    (probe() == Probe::Flood && (spliced > 0 || whole == 0)).then(|| {
        format!(
            "the flood spliced {spliced} kernel lines into other output ({whole} whole); with \
             the log service attached it must splice none"
        )
    })
}

/// **Say what a passing boot proved**: the scriptless keyboard boot's launch, the second boot's
/// packages, or the first boot's whole script.
fn report_leg_passed(leg: &Leg) {
    let Leg {
        arch,
        x86,
        script,
        fresh,
        graphics,
        ref programs,
        ..
    } = *leg;
    if !fresh && script.is_empty() {
        eprintln!(
            "swish-check ({arch}): rebooted against the same disk with a gpu and a keyboard and \
             launched `graphical_terminal` from the prompt"
        );
        return;
    }
    if !fresh {
        eprintln!(
            "swish-check ({arch}): rebooted against the same disk, ran the two packages installed \
             before the reboot, removed one and saw its vouch gone while the other still ran, \
             rolled back, and ran it again, then fetched the distribution's index over pinned TLS, \
             refused a lying copy by its digest and installed a package only the index vouches \
             for{}",
            if graphics.is_some() {
                ", then launched `graphical_terminal` from the prompt with a gpu and no keyboard"
            } else {
                ""
            }
        );
        return;
    }
    // Summed from the lines this leg typed, so an omitted line (see [`swish_check_omits`]) and
    // a skipped `std_exerciser` take their jobs with them; every tagged line was checked above
    // for a job that did not run. Until 2026-09-26 this was a hand-kept word per leg (27 on
    // aarch64 with `std_exerciser`), which undercounted the real 45 by eighteen.
    let jobs: u32 = script
        .iter()
        .filter(|l| !leg.skips(l.typed))
        .map(|l| u32::from(l.jobs))
        .sum();
    if x86 {
        let omitted: Vec<&str> = script
            .iter()
            .map(|l| l.typed)
            .filter(|line| swish_check_omits(arch, line).is_some())
            .collect();
        eprintln!(
            "swish-check (x86_64): booted under OVMF from \\EFI\\BOOT\\BOOTX64.EFI; ran {} \
             of {} lines, omitting {}: {:?}",
            script.len() - omitted.len(),
            script.len(),
            omitted.len(),
            omitted,
        );
    }
    let std_ran = if programs.std_built {
        ", one of them a `std` program built in the layout nife's `std` reads"
    } else {
        ""
    };
    let network = if x86 {
        "reached the network twice through the stack the kernel built on the e1000e and \
         refused it to a program that did not declare it, ran bytes nobody vouched for holding \
         nothing the line did not grant but the two pages, vouched for them as the owner and \
         rolled the vouch back, "
    } else {
        "reached the network twice through the stack the progenitor built and refused it to a \
         program that did not declare it, ran bytes nobody vouched for holding nothing the line \
         did not grant but the two pages, vouched for them as the owner and rolled the vouch \
         back, "
    };
    eprintln!(
        "swish-check ({arch}): the prompt booted, piped, redirected, appended, named a \
         file to a reader, read the clock, timed a command with a clock of its own, kept \
         a declared second stream off the redirection, previewed a directory grant and \
         then removed exactly the name it designated through a caretaker the progenitor built for \
         that one command, swept a \
         match too large to hand over in batches whose authority is exactly what each was \
         designated, named a file whose name has a space in it, searched an installed \
         documentation store and got back pages a following line could then designate, \
         rendered one of those pages straight at the prompt with no `| wc` in front of it, ran \
         a && past a command that succeeded and not past one it refused, {network}stopped a \
         supervised job with ^C and tore down one that ignored it, and ran \
         {jobs} jobs through the progenitor's bounded job pool after the progenitor gave its \
         construction budget away{std_ran}"
    );
}

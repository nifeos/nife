//! **The `cargo xtask swish-check` verb**: its flags, the probes of milestone 342, and the
//! loop over the legs. `main.rs` reaches [`swish_check`] through the parent's re-export.
//!
//! Cut out of `swish_check.rs` unchanged by milestone 847 (the swish-check harness is split
//! along its seams).
//!
//! Name: provisional (milestone 847's lane, 2026-10-11 UTC; §280 (the swish-check harness
//! splits into submodules, with its script in one)).

use super::*;

/// **The probes of milestone 342 (the kernel and the `console` server drive one UART from two
/// address spaces)** (names provisional): what the kernel is built to do on top of the
/// ordinary leg. `--flood` prints a kernel line every ten ticks once the log service attaches, and
/// the leg fails if any of them landed anywhere but on a line of its own. `--flood-detached` is
/// the same flood with the kernel never trusting its drainer: every line direct, which is the
/// kernel before milestone 342, and the leg reports how many splices that made. `--panic-probe`
/// panics on the thirtieth flood line and checks the panic reached the UART.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Probe {
    None,
    Flood,
    FloodDetached,
    Panic,
    /// `--input-race`: the input driver holds after every drain (`components/input_race_probe`)
    /// and Enter follows each line by [`INPUT_RACE_ENTER_PAUSE_MS`], so Enter lands inside the
    /// hold on every line. That is the window where clearing the PL011's interrupt after the
    /// drain would strand a byte in its FIFO and stop all input. One boot, the first script.
    InputRace,
}

/// The pause before Enter under `--input-race`, in ms: well inside the driver's 200 ms hold.
pub(super) const INPUT_RACE_ENTER_PAUSE_MS: u64 = 50;

static PROBE: std::sync::OnceLock<Probe> = std::sync::OnceLock::new();

pub(super) fn probe() -> Probe {
    *PROBE.get().unwrap_or(&Probe::None)
}

/// The kernel features a probe adds, comma-separated, or empty.
pub(super) fn probe_features() -> &'static str {
    match probe() {
        Probe::None => "",
        Probe::Flood => "console_flood",
        Probe::FloodDetached => "console_flood,kernel_log_detached",
        Probe::Panic => "kernel_log_panic_probe",
        Probe::InputRace => "",
    }
}

/// The `components` feature a probe adds, or `None`. Read by `archive::build_programs`.
pub(crate) fn probe_program_feature() -> Option<&'static str> {
    (probe() == Probe::InputRace).then_some("components/input_race_probe")
}

/// What a flood line looks like: `kernel::kernel_log::flood_tick`'s sentence, indent included.
pub(super) const FLOOD_NEEDLE: &str = "  kernel flood:";

/// Count the flood lines in a transcript: `(whole, spliced)`, a whole one being a line that is
/// exactly the sentence, and a spliced one any other place its text appears.
pub(super) fn flood_counts(raw: &str) -> (usize, usize) {
    let whole = raw
        .split('\n')
        .filter(|l| {
            l.strip_prefix(FLOOD_NEEDLE)
                .is_some_and(|n| n.trim_start().parse::<u64>().is_ok() && n.starts_with(' '))
        })
        .count();
    let all = raw.matches(FLOOD_NEEDLE.trim_start()).count();
    (whole, all - whole)
}

/// **Boot the system and type at it** (milestone 50 (pipes and redirection), notes/pipes.md).
///
/// Everything else that exercises the shell wires it from **the kernel**, which serves the spawn
/// protocol in place of `components/src/progenitor.rs`, so a change that broke the real
/// progenitor's spawn path would fail nothing else. Such breaks present as a boot that prints
/// nothing at all (a virtual-address collision, a full capability table, a stack one call short).
///
/// The lines answer each other rather than a constant:
///
/// ```text
/// echo hello world | wc      -> 1 2 12   the bytes went through a real spawned process
/// echo hello world > gate    -> nothing  the same bytes into a file the shell backs
/// wc < gate                  -> 1 2 12   ... and they are the same bytes
/// echo hello world >> gate   -> nothing
/// wc < gate                  -> 2 4 24   ... exactly twice, so `>>` kept the first line
/// wc gate                    -> 2 4 24   milestone 31: the name IS the grant, same bytes
/// wc                         -> refused  ... and with no name there is nothing to read
/// caps wc gate               -> input     ... and the preview says which file, and how
/// date                       -> ...UTC   a real wall clock, wired through the real progenitor
/// caps date                  -> cap 1    ... and `caps` names the capability that made it real
/// ```
///
/// That walks the whole endowment: a spawn through the real progenitor, the FS service it narrowed
/// into the shell, and both redirection operators.
pub(crate) fn swish_check() -> bool {
    let probe = if std::env::args().any(|a| a == "--flood") {
        Probe::Flood
    } else if std::env::args().any(|a| a == "--flood-detached") {
        Probe::FloodDetached
    } else if std::env::args().any(|a| a == "--panic-probe") {
        Probe::Panic
    } else if std::env::args().any(|a| a == "--input-race") {
        Probe::InputRace
    } else {
        Probe::None
    };
    let _ = PROBE.set(probe);
    let legs = match flag_value("--arch").as_deref() {
        None => ArchLegs::ALL,
        Some("aarch64") => ArchLegs::AARCH64,
        Some("riscv64") => ArchLegs::RISCV64,
        // The third leg (milestone 182), since milestone 299 gave x86_64 a userspace console to
        // reach a prompt through. It boots under OVMF rather than PVH; see `swish_check_leg`.
        Some("x86_64") => ArchLegs::X86_64,
        Some(other) => {
            eprintln!(
                "swish-check: --arch {other} is not an architecture (aarch64, riscv64 or x86_64)"
            );
            return false;
        }
    };
    // **The two graphical flags are gone, and saying so beats ignoring them** (2026-10-03 UTC):
    // every flag here is read with `any`, so a stale `--graphical` in somebody's muscle memory or a
    // script would otherwise run the plain legs and print green for a boot it never made.
    if std::env::args().any(|a| a == "--graphical" || a == "--graphical-serial") {
        eprintln!(
            "swish-check: --graphical and --graphical-serial no longer exist; the aarch64 and riscv64 \
             legs launch `graphical_terminal` themselves (see `swish_check_leg`)"
        );
        return false;
    }
    // `--release` builds and boots the optimized kernel and programs, which is what a customer's
    // stick carries (`xtask stick` is release-only). Added for the progenitor stack's measurement
    // (milestone progenitor-stack (provisional)): the gauge's numbers differ by profile, and the
    // debug build is the deeper one. Not in CI. Flag name provisional.
    if std::env::args().any(|a| a == "--release") {
        crate::RELEASE.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    // TCG only. This boot never exits (the shell loops on its prompt), so it is killed rather than
    // waited on, and there is nothing HVF would buy a gate that spends its time in QEMU's serial.
    // SAFETY: `set_var`/`remove_var` became unsafe in edition 2024 because they race other
    // threads. xtask is single-threaded here: this runs on the main thread before the child
    // that reads it is spawned, and the only thread xtask ever starts (the transcript reader
    // in swish_check_boot) copies pipe bytes or polls a socket and never touches the
    // environment.
    unsafe { std::env::remove_var("NIFE_ACCEL") };
    if legs.aarch64() && !swish_check_leg("aarch64") {
        return false;
    }
    if legs.riscv64() && !swish_check_leg("riscv64") {
        return false;
    }
    if legs.x86_64() && !swish_check_leg("x86_64") {
        return false;
    }
    true
}

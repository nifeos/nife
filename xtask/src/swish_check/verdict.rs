//! **What each leg omits, how long it waits, and whether it ran slow**: the per-architecture
//! omissions, the x86_64 hand-over markers, the timeouts and the leg-cost verdict.
//!
//! Cut out of `swish_check.rs` unchanged by milestone 847 (the swish-check harness is split
//! along its seams).
//!
//! Name: provisional (milestone 847's lane, 2026-10-11 UTC; §280 (the swish-check harness
//! splits into submodules, with its script in one)).

// Read only by the intra-doc links below, which name the script and the leg.
#[allow(unused_imports)]
use super::*;

/// **The lines a leg does not type, each with the reason** (milestone 182 (`x86_64`'s own
/// interactive-boot entry point), under the rule
/// milestone 150 added: an omitted line carries a stated reason, or it is a gap nobody can see).
///
/// A function over the line rather than a second table, so a line added to [`SWISH_CHECK_SCRIPT`]
/// is typed on every leg by default and an omission is the thing that has to be argued for. `None`
/// means the line runs, and since 2026-10-05 every line runs on every leg.
pub(super) fn swish_check_omits(arch: &str, line: &str) -> Option<&'static str> {
    // The four `uuid` lines and `std_exerciser` used to be here, for want of an entropy service at
    // the x86_64 prompt: the progenitor built one only from a virtio-rng, and `q35` has no mmio bus
    // to find one on. Milestone 595 (provisional) gave the progenitor the kernel's service on
    // `RDSEED` instead (`kernel::user::boot_instruction_entropy`), so they run. The network lines
    // followed on 2026-10-05: milestone 198 (a package manager) granted the progenitor the stack
    // the kernel builds on x86_64's `e1000e` (`kernel::user::boot_e1000e_network`), and the
    // disk install that stood in for the fetch there went with them. Nothing is omitted today;
    // the function stays because the next leg-specific gap needs somewhere to say why.
    //
    // **The disk tree's lines** (milestone 834 (ioping on nife and Linux)): `x86_64`'s leg attaches no NVMe disk, because the
    // kernel offers a blank one the install and waits for an answer (`install_service::offer`),
    // so that boot has no second mounted tree. `cargo xtask install-boot` reads the tree on
    // `x86_64`, on the installed machine.
    if arch == "x86_64" && crate::swish_check::script::ON_THE_DISK_TREE.contains(&line) {
        return Some("x86_64's leg attaches no NVMe disk, so it mounts no disk tree");
    }
    None
}

/// The first thing the hand-over prints (`hand_over` in `kernel/src/boot.rs`), where every leg
/// starts reading for faults; see `after_hand_over` in [`swish_check_leg`]. The same sentence
/// `uefi_boot` requires. Named for the leg that read it first.
pub(super) const X86_HAND_OVER_START: &str =
    "nife: handing the system to the userspace progenitor.";

/// How long to wait for the banner, for one line's echo, and for the whole transcript. Generous:
/// under TCG on a loaded machine a cold boot to the prompt is seconds, and a gate that flakes on a
/// busy laptop is a gate people learn to ignore.
///
/// # BUGS
///
/// **It flakes anyway, and the thirty seconds is a *per-echo* budget rather than a per-line one.**
/// On 2026-08-18 the riscv64 leg failed with "the prompt never echoed `caps date 2> err.txt`" after
/// echoing thirteen characters of it, on a machine running four other lanes; the same commit passed
/// on a rerun with nothing else changed. So a failure of this shape is a load report and not a
/// finding, and the way to tell them apart is to rerun the one leg before reading the transcript.
/// Raising the number is not obviously the fix: a real hang would then take proportionally longer
/// to report, and the honest measurement (how long an echo actually takes under load, versus the
/// budget) has not been made.
pub(super) const SWISH_CHECK_BOOT_SECS: u64 = 120;
pub(super) const SWISH_CHECK_LINE_SECS: u64 = 30;

/// **The `x86_64` leg's per-line bound under TCG, half again the others'**, and measured rather
/// than chosen (milestone 182 set it at 90 s on 2026-09-19; milestone 628 (provisional) cut it to 45 s
/// on 2026-10-03).
///
/// Under KVM the leg holds the other legs' [`SWISH_CHECK_LINE_SECS`], because it costs what they do;
/// this bound is for the leg emulated, which is every Mac and any host without `/dev/kvm`. Measured
/// typed to prompt-back, one run each, after 628 stopped the boot thread halting on the run queue:
///
/// | where | lines | per line | slowest line |
/// |---|---|---|---|
/// | patagonia, TCG | 118 in 125.7 s | 1.07 s | `caps installed/unvouched` 10.8 s |
/// | CI arm64, TCG (run 37089120726) | 128 in 364.5 s | 2.85 s | `caps installed/unvouched` 14.2 s |
/// | CI `x86_64`, KVM (same run) | 128 in 33.4 s | 0.26 s | `interrupt_ignorer` 7.0 s |
///
/// **Which part is the emulator's.** The same shell over TCG answers every line in under a second
/// on the other two legs, so the whole difference is the screen path: `display_terminal` paints
/// the damaged cells (the whole 924x344 surface on every scroll) and `framebuffer_driver` copies
/// them into an uncacheable aperture one word at a time, both as unoptimised debug builds, each
/// store through TCG. A real PC pays the same copy in native stores at uncacheable speed, which is
/// milliseconds per scroll rather than seconds and is not measured on silicon
/// (`framebuffer_driver`'s BUGS). Milestone 400's BUGS records the design half: the console
/// blocks on the screen (`components/src/console.rs`'s batcher is off).
///
/// **Milestone 624 (the paint path), 2026-10-03 UTC, measured before 628's boot-thread fix and why
/// that is not the bound's evidence.** Same machine, the script grown to 128 lines on the first boot:
///
/// | tree | lines | total | per line | slowest line |
/// |---|---|---|---|---|
/// | `main` at 4db8c13bf | 119 | 753.5 s | 6.3 s | `caps std_exerciser` 27.5 s |
/// | milestone 624 at 4124d6390 | 128 | 665.6 s | 5.2 s | `caps /installed/std-grep needle docs` 22.9 s |
///
/// Both runs shared patagonia with another session's `x86_64` leg, and both still had the boot
/// thread halting on the run queue, so the leg was paced by 10 ms ticks (7.7 s a line on CI with a
/// 35 s slowest line, notes/benchmarks/swish-check-x86-leg.md), which is what 90 s had covered and
/// what 624's rule (2x the slowest local line at CI's worst 1.8x ratio, 82 s) was measuring. With
/// the pacing gone the slowest emulated line is 10.8 s locally and 14.2 s on CI, so 45 s is 3.2x
/// CI's slowest.
pub(super) const SWISH_CHECK_X86_LINE_SECS: u64 = 45;

/// **What one leg's median line costs when it is healthy, per architecture and accelerator**
/// (milestone 722 (swish-check fails a leg that costs five times the others per line),
/// provisional): the committed baseline [`leg_cost_verdict`] compares a leg against.
///
/// Seconds a line, taken from CI's `swish-check` jobs on 2026-10-03 UTC (runs 37141733965 and
/// 37141058488), as the job printed the mean before this check reported a median. A median sits at
/// or under the mean, so these are loose, and loose is the right side to err on for a tripwire.
///
/// | leg | accelerator | source | seconds a line |
/// |---|---|---|---|
/// | aarch64 | TCG | CI arm64, 145 lines in 27.5 s | 0.20 |
/// | riscv64 | TCG | CI arm64, 145 lines in 30.7 s | 0.21 |
/// | x86_64 | KVM | CI x86_64, 142 lines in 36.8 s | 0.26 |
/// | x86_64 | TCG | CI arm64, 128 lines in 364.5 s (milestone 628's note) | 2.85 |
///
/// **Why against its own history and not against the other legs.** The proposal asked for five
/// times the median of the legs that share an accelerator. In CI no two legs share KVM, and the
/// legs that share TCG are not alike: `x86_64` under TCG costs about fourteen times aarch64 under
/// TCG, legitimately, because every screen store is emulated (the table above
/// [`SWISH_CHECK_X86_LINE_SECS`]). A peer rule would fail that leg on every Mac. So a leg is held to
/// its own row, and a leg with no row is reported and not failed.
///
/// **What it catches and what it does not.** The defect this exists for, a runnable boot thread
/// halting until the next 10 ms tick (milestone 628), ran the KVM leg at 7.7 s a line against a
/// 0.26 baseline, about 30 times, and fails here. The same defect under TCG was a factor of 2.7
/// against its row and passes, because 45 s a line is the per-line bound's job there. A host far
/// slower than CI's arm64 runner would also fail; the answer is a measurement and a new row.
const LEG_COST_BASELINE: &[(&str, &str, f64)] = &[
    ("aarch64", "tcg", 0.20),
    ("riscv64", "tcg", 0.21),
    ("x86_64", "kvm", 0.26),
    ("x86_64", "tcg", 2.85),
];

/// How many times its own baseline a leg's median line may cost before the leg fails.
pub(super) const LEG_COST_FACTOR: f64 = 5.0;

/// A boot that typed fewer lines than this reports its median and is not judged: the second boot's
/// eight lines are mostly a fixed cost of reaching a prompt, and eight samples is one slow line from
/// any median at all.
const LEG_COST_MIN_LINES: usize = 20;

/// The leg's median seconds a line, its baseline row if it has one, and whether it is over.
#[derive(Debug, PartialEq)]
pub(super) struct LegCost {
    pub(super) median: f64,
    pub(super) baseline: Option<f64>,
    pub(super) over: bool,
}

/// Judge one boot's per-line times against [`LEG_COST_BASELINE`]. Pure, so the rule is tested on
/// the host without a boot. `accel` is `"kvm"` or `"tcg"`.
pub(super) fn leg_cost_verdict(arch: &str, accel: &str, secs: &[f64]) -> LegCost {
    let mut sorted = secs.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("a duration is a number"));
    let median = match sorted.len() {
        0 => 0.0,
        n if n % 2 == 1 => sorted[n / 2],
        n => (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0,
    };
    let baseline = LEG_COST_BASELINE
        .iter()
        .find(|(a, x, _)| *a == arch && *x == accel)
        .map(|(_, _, s)| *s);
    let over =
        secs.len() >= LEG_COST_MIN_LINES && baseline.is_some_and(|b| median > b * LEG_COST_FACTOR);
    LegCost {
        median,
        baseline,
        over,
    }
}

#[cfg(test)]
mod tests;

//! **What each leg omits, how long it waits, and whether it ran slow**: the per-architecture
//! omissions, the x86_64 hand-over markers, the timeouts and the leg-cost verdict. See
//! notes/swish-check-harness.md, "Timeouts and what a leg costs".
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
/// means the line runs.
pub(super) fn swish_check_omits(arch: &str, line: &str) -> Option<&'static str> {
    // The disk tree's lines (milestone 834 (ioping on nife and Linux)): the kernel offers a blank
    // NVMe disk to the install and waits for an answer (`install_service::offer`), so `x86_64`'s
    // leg attaches none. `cargo xtask install-boot` reads the tree on the installed machine.
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

/// How long to wait for the banner. Generous, because a gate that flakes on a busy laptop is a
/// gate people learn to ignore.
pub(super) const SWISH_CHECK_BOOT_SECS: u64 = 120;
/// How long to wait for one line's echo, on the legs that are not emulated `x86_64`.
///
/// # BUGS
///
/// - It is a per-echo budget, not a per-line one, and it flakes under load: a failure after a
///   partial echo is a load report until the one leg is rerun. How long an echo takes under load
///   is not measured. See notes/swish-check-harness.md, "Timeouts and what a leg costs".
pub(super) const SWISH_CHECK_LINE_SECS: u64 = 30;

/// **The `x86_64` leg's per-line bound under TCG, half again the others'**, measured rather than
/// chosen (milestone 628 (provisional), 2026-10-03 UTC). Under KVM the leg keeps
/// [`SWISH_CHECK_LINE_SECS`]; this bound is for every Mac and any host without `/dev/kvm`.
///
/// The slowest emulated line was 10.8 s on patagonia and 14.2 s on CI's arm64 runner (run
/// 37089120726), so 45 s is 3.2 times CI's slowest. The difference from the other legs is the
/// screen path, each store through TCG. The tables, and the earlier tick-paced runs that are not
/// this bound's evidence, are in notes/swish-check-harness.md, "Timeouts and what a leg costs".
pub(super) const SWISH_CHECK_X86_LINE_SECS: u64 = 45;

/// **What one leg's median line costs when it is healthy, per architecture and accelerator**
/// (milestone 722 (swish-check fails a leg that costs five times the others per line),
/// provisional): the committed baseline [`leg_cost_verdict`] compares a leg against.
///
/// Seconds a line, the mean CI's `swish-check` jobs printed on 2026-10-03 UTC (runs 37141733965
/// and 37141058488); a median sits at or under it, so the rows are loose on purpose. A leg is held
/// to its own row, not its peers', because emulated `x86_64` legitimately costs about fourteen
/// times aarch64. A leg with no row is reported and not failed. The sources, and what the check
/// catches and misses, are in notes/swish-check-harness.md, "Timeouts and what a leg costs".
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

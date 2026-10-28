//! **A hundred spawn-and-reap cycles of a job the shell holds** (milestone 933 (every job is built
//! the way a supervised job is)): `free`'s `Yours:` row before and after, which `boot.rs` requires
//! to be the same. A region the shell split off its budget and did not get back, or a window the
//! progenitor did not free, would show as pages used that a hundred jobs only grow. The shell's
//! budget could not hold a hundred leaked regions at all, so a leak of a whole region fails the run
//! with "could not spawn" before it fails the comparison, and a smaller one fails the comparison.
//!
//! Name: provisional (milestone 933's lane, 2026-10-27 UTC).

use super::*;

/// How many jobs run between the two readings.
pub(super) const CYCLES: usize = 100;

/// What is typed before the first of the jobs and after the last: `free`'s row, with a marker the
/// gate finds it by (`CYCLES_BEFORE`, `CYCLES_AFTER`).
pub(in crate::swish_check) const CYCLES_BEFORE: &str = "echo cycles-before; free";
pub(in crate::swish_check) const CYCLES_AFTER: &str = "echo cycles-after; free";

/// The lines of the cycle, in table order.
pub(super) fn lines() -> Vec<Line> {
    let mut all = vec![line(0, CYCLES_BEFORE, &["cycles-before", "Yours:"])];
    all.extend(std::iter::repeat_n(
        line(0, "job_sleeper 1 --mem 4", &["the sleeper woke"]),
        CYCLES,
    ));
    all.push(line(0, CYCLES_AFTER, &["cycles-after", "Yours:"]));
    all
}

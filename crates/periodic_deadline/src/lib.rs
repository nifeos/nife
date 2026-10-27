//! **`periodic_deadline`: the next deadline on a fixed beat.** Given the deadline that fired, the
//! period and the current reading, [`next_after`] answers the next deadline strictly after now that
//! sits on the same beat. A deadline that arrives late comes back onto its grid and skips the periods
//! it missed, rather than restarting from now or firing once per missed period.
//!
//! Pure integer arithmetic over `u64` counter readings, so it serves both the scheduler
//! (`timetable`, in nanoseconds) and the kernel's software timer re-arm (aarch64 and riscv64, in
//! counter ticks). No IO, no syscalls, no allocation. The function is Kani-proved (`src/proofs.rs`),
//! and the kernel calls the same function, so the proof binds the code that runs (§178 (where the
//! timer re-arm seam goes), milestone 360 (`crates/timetable`'s proved `next_after` is not what the
//! timer calls)). See notes/periodic-deadline-proofs.md.
//!
//! Name: provisional (milestone 360). A noun for a deadline that repeats at a fixed period. Refused
//! `timetable_arithmetic` (names the first caller, not the thing), `rearm` (a verb, and it names the
//! kernel's use only), and `beat` (a metaphor with no `deadline` in it).

#![no_std]

/// The "never fires" deadline: `u64::MAX`. [`next_after`] answers it for a zero period.
///
/// Name: provisional (milestone 360).
pub const NEVER: u64 = u64::MAX;

/// **The next fire after `now`, given the one that was scheduled at `prev` and a period.**
///
/// The result is **strictly after `now`**, so a fire never repeats within a polling pass; keeps the
/// **phase** (congruent to `prev` modulo `period`), so a late entry returns to its beat; and
/// **skips rather than catches up**, as Vixie cron does: an entry that missed two hundred fires
/// fires once, because catching up turns a stall into a stampede. Skipped occurrences are lost;
/// work that must not be wants a durable queue. `period == 0` answers [`u64::MAX`] (never), so the
/// function is total, though the `timetable` parser refuses a zero interval. All but the phase are
/// machine-checked (`src/proofs.rs`). See notes/periodic-deadline-proofs.md.
///
/// Name: provisional (milestone 360).
///
/// # Examples
///
/// ```
/// # use periodic_deadline::next_after;
/// // The ordinary case: one period on.
/// assert_eq!(next_after(100, 10, 100), 110);
///
/// // Late by eight periods, and it does not fire eight times to make up.
/// assert_eq!(next_after(100, 10, 185), 190);
/// assert_eq!(next_after(100, 10, 190), 200);
///
/// // The phase survives the skip: every result is congruent to `prev` mod `period`.
/// assert_eq!(next_after(103, 10, 1_000) % 10, 3);
/// ```
pub const fn next_after(prev: u64, period: u64, now: u64) -> u64 {
    if period == 0 {
        return NEVER;
    }
    let first = prev.saturating_add(period);
    if first > now {
        return first;
    }
    // Snap `now` back onto the beat and add one period, rather than counting the periods that went
    // by: no multiplication, which keeps the Kani harnesses to seconds
    // (notes/periodic-deadline-proofs.md).
    // `now >= first > prev`, so the subtraction cannot wrap, and `r < period`, so `now - r` cannot.
    let r = (now - prev) % period;
    (now - r).saturating_add(period)
}

// Machine-checked proofs (Kani). Behind `#[cfg(kani)]`, so an ordinary build or test never sees
// them; `script/verify` runs them.
#[cfg(kani)]
mod proofs;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_after_is_strictly_in_the_future_and_keeps_its_phase() {
        // The ordinary case.
        assert_eq!(next_after(100, 10, 100), 110);
        // Exactly on a boundary: strictly greater, so a polling loop cannot fire twice.
        assert_eq!(next_after(100, 10, 110), 120);
        // Late, by a whole number of periods and by a fraction of one.
        assert_eq!(next_after(100, 10, 185), 190);
        assert_eq!(next_after(100, 10, 190), 200);
        // The phase survives.
        for prev in [0u64, 3, 97, 1_000_003] {
            for period in [1u64, 7, 1_000, 999_983] {
                for now in [0u64, 1, 50, 1_000_000, u64::MAX / 4] {
                    let next = next_after(prev, period, now);
                    assert!(next > now, "{prev} {period} {now} -> {next}");
                    assert!(next >= prev.saturating_add(period));
                    assert_eq!(next % period, prev % period);
                    if now >= prev {
                        assert!(next <= now.saturating_add(period));
                    }
                }
            }
        }
        // Total rather than panicking, for a period `parse` will never produce.
        assert_eq!(next_after(1, 0, 5), u64::MAX);
    }
}

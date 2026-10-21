/// `invoke(cap, ARM, deadline, notification_slot, bits)` -> 0. Arm this timer: when the counter
/// reaches `deadline`, OR `bits` into the notification named by `notification_slot`, exactly as
/// a [`SIGNAL`](super::notification::SIGNAL) would. **Replaces any pending deadline**, which then
/// never fires. A deadline already reached signals at once and leaves nothing armed. `bits` of
/// zero is accepted and delivers nothing when it fires, as `SIGNAL(0)` does.
///
/// `WrongObject` if the slot is not a notification; `NotPermitted` without `WRITE` on either.
/// A notification destroyed while the timer is armed makes the expiry a no-op, not an error:
/// there is nobody left to tell.
pub const ARM: u64 = 0;

/// `invoke(cap, CANCEL, _, _, _)` -> 1 or 0. Disarm. `1` if a deadline was pending and now never
/// fires; `0` if nothing was armed (never armed, already fired, or already cancelled), which
/// tells the caller any signal has already been sent. Bits a timer already signalled stay in
/// the notification's word: a notification never loses a bit, and a cancel does not reach into
/// one.
pub const CANCEL: u64 = 1;

/// **The scheduler tick, in hertz, on every architecture, and so the resolution of every timed
/// wait.** The kernel checks for due deadlines only from the tick (DECISIONS §147), so an [`ARM`]
/// fires at the first tick at or after its deadline: a wait asked for 1 ms lasts up to
/// `1 / TICK_HZ` s. A caller that cannot afford that latency must not sleep through it.
///
/// Name: provisional, minted 2026-10-21 (UTC) by milestone 53's receive-rate lane, after the
/// kernel's own `arch::timer::TICK_HZ`, which takes its value from here.
pub const TICK_HZ: u64 = 100;

/// **Counter ticks in a duration, rounded up**, for a counter running at `hz`: the `n` that
/// makes `now() + n` a deadline that is never early. Saturates at `u64::MAX` rather than
/// wrapping, and a saturated deadline is one that never fires, which is the right reading of a
/// duration too long to represent.
///
/// Rounding up is the point. `std::thread::sleep` promises to sleep *at least* its duration, and
/// truncating (what the PAL's yield-spin did) could make it one tick short.
///
/// Split into whole seconds and a sub-second part so every product fits in 64 bits for any
/// counter up to 2^34 Hz: `nanos * hz` is under `10^9 * 2^34`, which is under `2^64`. That keeps
/// the arithmetic in the width the solver can check, and in the width a 64-bit core divides in.
pub const fn counter_ticks_for(secs: u64, nanos: u32, hz: u64) -> u64 {
    const NS: u64 = 1_000_000_000;
    let Some(whole) = secs.checked_mul(hz) else {
        return u64::MAX;
    };
    let Some(part) = (nanos as u64).checked_mul(hz) else {
        return u64::MAX;
    };
    whole.saturating_add(part.div_ceil(NS))
}

/// **Tested, not proved, and the reason is a measurement.** A Kani harness over this function
/// did not finish in ten minutes with a symbolic rate, nor in five with five concrete rates
/// (2026-09-26): a symbolic 64-bit divide is the expensive thing for CBMC whatever the
/// divisor. The deadline arithmetic that *is* proved is the kernel's
/// (`inter_process_communication::timer`), where a wrong answer is a missed wake for everyone;
/// a wrong answer here is one sleep one tick short, and the tests below pin the boundaries
/// where that could happen.
#[cfg(test)]
mod tests {
    use super::counter_ticks_for;

    const RATES: [u64; 5] = [62_500_000, 10_000_000, 4_000_000, 31_250_000, 4_200_000_000];

    fn check(secs: u64, nanos: u32, hz: u64) {
        let n = counter_ticks_for(secs, nanos, hz) as u128;
        let exact = (secs as u128 * 1_000_000_000 + nanos as u128) * hz as u128;
        assert!(
            n * 1_000_000_000 >= exact,
            "early: {secs}s {nanos}ns at {hz} Hz"
        );
        assert!(
            n == 0 || (n - 1) * 1_000_000_000 < exact,
            "late: {secs}s {nanos}ns at {hz} Hz"
        );
    }

    #[test]
    fn a_duration_is_never_short_and_at_most_one_tick_long() {
        for hz in RATES {
            for secs in [0, 1, 59, 86_400, 1 << 30] {
                for nanos in [
                    0,
                    1,
                    15,
                    16,
                    17,
                    99,
                    100,
                    101,
                    999_999_999,
                    500_000_000,
                    238,
                ] {
                    check(secs, nanos, hz);
                }
            }
            // Every nanosecond count below one tick, and just past it, at this rate.
            let tick_ns = (1_000_000_000 / hz).max(1) as u32;
            for nanos in 0..(3 * tick_ns + 3) {
                check(0, nanos, hz);
            }
        }
    }

    #[test]
    fn a_duration_too_long_to_represent_saturates_rather_than_wraps() {
        assert_eq!(counter_ticks_for(u64::MAX, 0, 62_500_000), u64::MAX);
        assert_eq!(
            counter_ticks_for(u64::MAX / 62_500_000, 999_999_999, 62_500_000),
            u64::MAX
        );
        assert_eq!(counter_ticks_for(0, 0, 62_500_000), 0);
    }
}

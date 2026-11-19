//! Which local port the network stack gives an outbound socket: the private ephemeral range, walked
//! by a cursor that never falls behind the boot clock. An allocation takes the later of the
//! cursor and a clock advancing [`PER_SECOND`] ports a second, so a fresh stack starts past the
//! ports an earlier one used, which a peer may still hold in `TIME_WAIT`. Here so its claim is
//! a host test; only the network stack uses it.
//!
//! Name: provisional, minted 2026-11-19 (UTC) by lane 1108-renames-scope.

//! # BUGS
//!
//! - The bound is a rate, not a proof: a stack replaced quickly can hand its successor a port.
//! - The range wraps every 64 s, inside a longer `TIME_WAIT` such as Windows' 240 s.
//! - The clock is boot-relative, so a rebooted machine's first ports repeat the last boot's.
//! - The sequence-number half stays smoltcp's: a reused 4-tuple still fails half the time.

/// The lowest port of the private ephemeral range.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane 1108-renames-scope.
pub const LO: u16 = 49152;
/// The highest.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane 1108-renames-scope.
pub const HI: u16 = 65535;
/// How many ports the range holds.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane 1108-renames-scope.
pub const SPAN: u64 = (HI - LO) as u64 + 1;
/// How far the clock moves the cursor each second. 256 makes one tick about 4 ms, so a stack
/// started a few milliseconds after another's last open already starts past it, and keeps the
/// wrap at 64 s, above the 60 s `TIME_WAIT` of Linux and the BSDs.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane 1108-renames-scope.
pub const PER_SECOND: u64 = 256;

// The wrap stays at or above the 60 s `TIME_WAIT` the BUGS section names, checked at compile
// time.
const _: () = assert!(SPAN * 1000 / PER_SECOND >= 60_000);

/// The cursor. It counts positions without wrapping; a position's port is its remainder in the
/// range. A new one has handed out nothing, so its first allocation starts at the clock.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane 1108-renames-scope.
pub struct Ports {
    next: u64,
}

impl Ports {
    /// A cursor that has handed out nothing.
    ///
    /// Name: provisional, minted 2026-11-19 (UTC) by lane 1108-renames-scope.
    pub const fn new() -> Self {
        Self { next: 0 }
    }

    /// The next port at `milliseconds` on the boot clock that `held` does not say a live socket
    /// holds, or `None` when `held` says all [`SPAN`] ports of the range are held. Bounded by
    /// the range, so it always returns; a stack holds far fewer sockets than the range has
    /// ports, so in practice the first or second candidate is free. The cursor moves past
    /// every port it tried, so a caller that gets `None` has used the whole range.
    ///
    /// Name: provisional, minted 2026-11-19 (UTC) by lane 1108-renames-scope.
    pub fn next(&mut self, milliseconds: u64, held: impl Fn(u16) -> bool) -> Option<u16> {
        self.next_within(milliseconds, held, SPAN)
    }

    /// [`Ports::next`] with the number of candidates it will try as a parameter, so a proof can
    /// bound the loop at a few steps where the whole range is sixteen thousand.
    fn next_within(
        &mut self,
        milliseconds: u64,
        held: impl Fn(u16) -> bool,
        tries: u64,
    ) -> Option<u16> {
        self.next = self.next.max(clock(milliseconds));
        for _ in 0..tries {
            let port = port_at(self.next);
            self.next += 1;
            if !held(port) {
                return Some(port);
            }
        }
        None
    }
}

impl Default for Ports {
    fn default() -> Self {
        Self::new()
    }
}

/// The clock's position at `milliseconds`.
fn clock(milliseconds: u64) -> u64 {
    milliseconds.saturating_mul(PER_SECOND) / 1000
}

fn port_at(position: u64) -> u16 {
    LO + (position % SPAN) as u16
}

/// Machine-checked proofs, run with `cargo kani`: the clock and the position-to-port map hold
/// for every input, which the sampled tests below cannot say.
#[cfg(kani)]
mod verification {
    use super::*;

    /// **A port is always in the range, and the clock never runs backwards**: for any position
    /// the port lies in `LO..=HI`, and for any two times the later one's clock is not earlier.
    /// Falsification: replayable `crates/network_stack/falsifications/ephemeral.verification.a_position_is_a_port_in_range_and_the_clock_never_runs_backwards.patch`
    #[kani::proof]
    fn a_position_is_a_port_in_range_and_the_clock_never_runs_backwards() {
        let position: u64 = kani::any();
        let port = port_at(position);
        // `HI` is the port type's maximum, so only the lower bound can fail.
        assert!(port >= LO);
        let earlier: u64 = kani::any();
        let later: u64 = kani::any();
        kani::assume(earlier <= later);
        assert!(clock(earlier) <= clock(later));
    }

    /// **The cursor never moves backwards, and what it hands out is in range and not held.**
    /// For any cursor position, any time and any answers from `held`, one allocation leaves
    /// the cursor at or past where it stood and past the clock. A port it returns is in
    /// `LO..=HI` and the last one `held` was asked about; when it returns none, it asked about
    /// every candidate it was allowed and the cursor moved that many. The loop is bounded at
    /// four candidates (`next_within`), because a proof over the whole [`SPAN`] of 16384 did not
    /// finish in over an hour at 6 GiB; that the real bound behaves like four is an argument
    /// (the loop body does not depend on the count), not a proof. The position is assumed below
    /// 2^62, which the clock reaches after about 500 million years.
    ///
    /// Falsification: replayable `crates/network_stack/falsifications/ephemeral.verification.an_allocation_never_moves_the_cursor_back_or_returns_a_held_port.patch`
    #[kani::proof]
    #[kani::unwind(6)]
    fn an_allocation_never_moves_the_cursor_back_or_returns_a_held_port() {
        let start: u64 = kani::any();
        kani::assume(start < 1 << 62);
        let milliseconds: u64 = kani::any();
        let mut ports = Ports { next: start };
        let answer = core::cell::Cell::new(false);
        let got = ports.next_within(
            milliseconds,
            |_| {
                let held: bool = kani::any();
                answer.set(held);
                held
            },
            4,
        );
        let began = start.max(clock(milliseconds));
        assert!(ports.next >= start);
        assert!(ports.next > began);
        match got {
            Some(port) => {
                assert!(port >= LO);
                assert!(!answer.get());
            }
            None => assert_eq!(ports.next, began + 4),
        }
    }

    /// **A stack started four milliseconds or more after another's last open starts past every
    /// port the other used.** The no-reuse claim as arithmetic, where the sampled test below
    /// covers five boot times. At a gap of at least 4 ms (`PER_SECOND` is 256, so one tick) the
    /// clock gains a whole position, so each open of a stack that opens that far apart takes
    /// the clock's own position and the cursor never needs its one-more fallback. So after
    /// its open at `last` the stack has used no position past `clock(last)`, and a stack whose
    /// first open comes at `last + gap` starts at `clock(last + gap)`, which is past it.
    ///
    ///
    /// The formula is small and its hard part is three 64-bit divisions by 1000. kissat proves
    /// it in a little over half the time CaDiCaL, Kani's default, takes; minisat takes longer
    /// than either. The solver changes how long the proof takes and not what it proves.
    ///
    /// Falsification: replayable `crates/network_stack/falsifications/ephemeral.verification.a_stack_started_four_milliseconds_later_starts_past_every_port_used.patch`
    #[kani::proof]
    #[kani::solver(kissat)]
    fn a_stack_started_four_milliseconds_later_starts_past_every_port_used() {
        let last: u64 = kani::any();
        let gap: u64 = kani::any();
        kani::assume(last < 1 << 50);
        kani::assume((4..1 << 50).contains(&gap));
        // A stack that opened at `last - gap` and at `last`, 4 ms or more apart, took the
        // clock's position both times: the step that keeps it at or below `clock(last)`.
        kani::assume(last >= gap);
        let mut first = Ports { next: 0 };
        let _ = first.next(last - gap, |_| false);
        let _ = first.next(last, |_| false);
        assert_eq!(first.next, clock(last) + 1);
        // The second stack's first open lands past every position the first used.
        let mut second = Ports::new();
        let _ = second.next(last + gap, |_| false);
        assert_eq!(second.next, clock(last + gap) + 1);
        assert!(clock(last + gap) > clock(last));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hand out `n` ports from a fresh cursor, `gap_milliseconds` apart, starting at `start_milliseconds`. Returns
    /// the ports and the time of the last open.
    fn run(start_milliseconds: u64, n: u64, gap_milliseconds: u64) -> (std::vec::Vec<u16>, u64) {
        let mut ports = Ports::new();
        let mut at = start_milliseconds;
        let mut got = std::vec::Vec::new();
        for _ in 0..n {
            got.push(ports.next(at, |_| false).expect("no port is held"));
            at += gap_milliseconds;
        }
        (got, at - gap_milliseconds)
    }

    /// A stack started after another hands out none of its ports. The shapes are a name
    /// resolver's: fourteen opens 50 ms apart, then a new stack whose first open comes
    /// every whole number of milliseconds from one tick (4 ms) to 2 s later (the measured gap
    /// is about 230 ms on riscv64), at a spread of boot times. Seeding the cursor once from
    /// the counter fails this.
    ///
    /// Falsification: replayable `crates/network_stack/falsifications/ephemeral.tests.a_stack_started_after_another_reuses_none_of_its_ports.patch`
    #[test]
    fn a_stack_started_after_another_reuses_none_of_its_ports() {
        for start in [0, 1, 7_919, 123_457, 3_600_001] {
            let (first, last) = run(start, 14, 50);
            for gap in 4..=2000 {
                let (second, _) = run(last + gap, 14, 50);
                for p in &second {
                    assert!(
                        !first.contains(p),
                        "boot time {start} ms, gap {gap} ms: port {p} reused from {first:?}",
                    );
                }
            }
        }
    }

    /// One stack never hands out a port twice before the range has cycled, however fast it
    /// opens: a burst runs ahead of the clock rather than repeating it.
    #[test]
    fn a_burst_runs_ahead_of_the_clock_and_repeats_nothing() {
        let mut ports = Ports::new();
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..SPAN {
            assert!(seen.insert(ports.next(1_000, |_| false).expect("no port is held")));
        }
    }

    /// The cursor walks the whole private range and nothing outside it (the
    /// range 49152 to 65535), in order, and wraps to its bottom. Spelled as numbers, because the
    /// other tests compare ports only with each other and would pass on any range.
    #[test]
    fn the_cursor_walks_49152_to_65535_and_wraps() {
        let mut ports = Ports::new();
        for want in 49152..=65535u16 {
            assert_eq!(ports.next(0, |_| false), Some(want));
        }
        assert_eq!(ports.next(0, |_| false), Some(49152));
    }

    /// A port a live socket holds is skipped, and the cursor moves past it.
    #[test]
    fn a_held_port_is_skipped() {
        let mut ports = Ports::new();
        let at = clock(10_000);
        let held = port_at(at);
        assert_eq!(ports.next(10_000, |p| p == held), Some(port_at(at + 1)));
        assert_eq!(ports.next(10_000, |p| p == held), Some(port_at(at + 2)));
    }

    /// A range with every port held yields nothing, and the cursor has moved past the whole
    /// range, so the next allocation starts a full cycle later.
    #[test]
    fn a_range_with_every_port_held_yields_none() {
        let mut ports = Ports::new();
        assert_eq!(ports.next(0, |_| true), None);
        assert_eq!(ports.next, SPAN);
        assert_eq!(ports.next(0, |_| false), Some(LO));
    }
}

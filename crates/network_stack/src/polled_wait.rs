//! Whether the network stack may sleep while it waits on a network interface card (NIC) that
//! raises no interrupt (the `e1000e` and a `DesignWare` port). For one scheduler
//! tick after the last frame the stack yields and looks at the ring again, and it sleeps only
//! once a whole tick has passed with nothing, because a timed wait ends at a tick and a sleep
//! would add up to a tick to every round trip. Here so its claim is a host test; only
//! the network stack uses it.
//!
//! Name: provisional, minted 2026-11-19 (UTC) by lane 1108-renames-scope.

//! # BUGS
//!
//! - A core is busy while a stream flows, and for a tick after it ends: each look yields, but
//!   an idle core does not idle. A NIC interrupt or a finer timed wait would fix it.

/// What a polled NIC's wait does next.
///
/// Name: provisional, minted 2026-10-21 (UTC) by milestone 53's receive-rate lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    /// Yield, then look at the ring again: a frame came within the last tick, so another is
    /// likely sooner than a sleep could end.
    Look,
    /// Sleep this many microseconds (the stack's own next timer or the poll interval, whichever
    /// is sooner), then look. Zero means look now.
    SleepMicroseconds(u64),
}

/// The wait at counter time `now_ticks`, when the last frame was taken at `last_frame_ticks` and a
/// scheduler tick is `ticks_per_scheduler_tick` counter ticks. `stack_due_microseconds` is when the stack next needs
/// polling for its own timers, if ever; `poll_microseconds` is the longest a polled NIC goes
/// unlooked-at.
///
/// Name: provisional, minted 2026-10-21 (UTC) by milestone 53's receive-rate lane.
pub const fn next(
    now_ticks: u64,
    last_frame_ticks: u64,
    ticks_per_scheduler_tick: u64,
    stack_due_microseconds: Option<u64>,
    poll_microseconds: u64,
) -> Next {
    if now_ticks.wrapping_sub(last_frame_ticks) < ticks_per_scheduler_tick {
        return Next::Look;
    }
    match stack_due_microseconds {
        Some(due) if due < poll_microseconds => Next::SleepMicroseconds(due),
        _ => Next::SleepMicroseconds(poll_microseconds),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TICK: u64 = 40_000; // a 4 MHz counter at 100 Hz, as on a VisionFive 2

    /// The regression: any wait inside a tick of the last frame sleeps through a tick instead.
    #[test]
    fn within_a_tick_of_a_frame_the_stack_never_sleeps() {
        let last = 1_000_000;
        for since in [0, 1, TICK / 2, TICK - 1] {
            assert_eq!(next(last + since, last, TICK, None, 1000), Next::Look);
        }
    }

    /// The cost side: an idle link must reach a sleep, or the stack spins a core forever while
    /// a client waits on a listener nobody connects to.
    #[test]
    fn a_tick_after_the_last_frame_the_stack_sleeps_no_longer_than_its_timers_allow() {
        let last = 1_000_000;
        assert_eq!(
            next(last + TICK, last, TICK, None, 1000),
            Next::SleepMicroseconds(1000)
        );
        assert_eq!(
            next(last + TICK, last, TICK, Some(300), 1000),
            Next::SleepMicroseconds(300)
        );
        assert_eq!(
            next(last + TICK, last, TICK, Some(0), 1000),
            Next::SleepMicroseconds(0)
        );
        // Before any frame at all, a stack that has been up a tick sleeps too.
        assert_eq!(
            next(5 * TICK, 0, TICK, Some(5000), 1000),
            Next::SleepMicroseconds(1000)
        );
    }
}

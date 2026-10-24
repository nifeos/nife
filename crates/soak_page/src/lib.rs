//! **The one page the soak workload and the kernel share**, and the arithmetic they both do to it
//! (milestone 219).
//!
//! The workload's progress is a fact only the workload knows, and the kernel reads it every
//! heartbeat without asking, at the cost of one store per round trip. The kernel's `soak` module
//! only reads and each worker writes only its own slots, so each `u64` has one writer, and an
//! aligned `u64` store does not tear on any of the three architectures: no lock is needed. The
//! wakes array (milestone 221) is kept apart from the round-trip total because the two are
//! different quantities. The anomaly counters are the kernel's and stay in `sched`'s per-core trace
//! counters, out of the subject's reach. See notes/soak.md, "The shape".

//! # BUGS
//!
//! - A worker that dies stops writing and looks exactly like one that wedged: both fail the same
//!   stall check, and only the thread dump the kernel prints on failure tells them apart.
//!

//! Name: ratified 2026-09-13 (calef, working the unratified worklist). Coined by milestone 219's
//! lane on 2026-09-01. A noun for the thing it describes: the one page the soak workload and the
//! kernel share.
//!
//! **The refusal is the valuable half, and it draws a line the rest of `crates/` follows without
//! writing down.** `soak_proto` was refused because **nothing here is a wire protocol: it is a
//! memory layout**, and the `_proto` suffix in this tree names request/reply vocabularies. That
//! puts this crate in a family with `block_roster` and `grant_plan::job_page_frame`, against
//! `clock_contract`, `credential_contract` and `supervision_protocol` on the other side of the line.
//!
//! **The list this block used to give was wrong on its own terms**, and the correction is recorded
//! rather than quietly swapped: it named `clock_contract` and `counter_frequency_contract` as examples of "the
//! shape the tree already uses for a layout", when both are `_proto` crates and therefore the
//! counter-examples the very next sentence distinguishes. `counter_frequency_contract` also became
//! `counter_frequency_proto` by calef's ruling the same day, so the citation was about to dangle as
//! well as contradict.
//!
//! Refused `soak_counters` for naming half the content: the page carries `rounds`, `mismatches` and
//! `wakes` per worker, and the arithmetic both sides do to find them. Refused
//! `soak_heartbeat_page`: the kernel reads this every heartbeat, but the page is not the heartbeat,
//! it is what the heartbeat reads.

#![no_std]

/// Where the kernel maps the page in every worker's address space.
///
/// A pair page on the address-space map (`crates/address_space_map`), the same neighbourhood
/// `grant_plan::job_page_frame`'s window uses.
pub const VA: u64 = address_space_map::pair_page(0x0020_0000);

/// The page's size, and the ceiling on everything below.
pub const PAGE: u64 = 4096;

/// How many workers the page has room for.
///
/// Three `u64` per worker costs 1.5 KiB of the 4 KiB page at this ceiling, which is the headroom
/// the host test below checks rather than assumes. It is far above any machine this project has:
/// the kernel builds one group of six per online core, so 64 covers a ten-core board with room to
/// spare, and the cap exists so that the kernel's own fixed-size arrays have a bound rather than
/// because anything is near it.
pub const MAX_WORKERS: usize = 64;

/// Byte offset of worker `i`'s completed-round-trip counter.
///
/// Written by the worker, read by the kernel. Monotone, and never reset: the whole point is that a
/// later run can be compared against an earlier one, which needs a total rather than a rate.
#[must_use]
pub const fn rounds(i: usize) -> u64 {
    (i as u64) * 8
}

/// Byte offset of worker `i`'s wrong-answer counter.
///
/// A caller that gets back something other than [`answer`] of what it sent has caught the IPC path
/// delivering the wrong message, which is a stronger finding than a hang and the one this workload
/// would most like to produce. Nonzero here fails the run.
#[must_use]
pub const fn mismatches(i: usize) -> u64 {
    ((MAX_WORKERS + i) as u64) * 8
}

/// Byte offset of worker `i`'s tick-wake counter (milestone 221).
///
/// Written by a waiter, read by the kernel, and **separate from [`rounds`] on purpose**: a waiter
/// completes no IPC round trip, so folding its wakes into the round-trip total would make the one
/// number a later run is compared against mean two different things depending on which build
/// produced it. It is its own field in the heartbeat for the same reason.
///
/// A waiter leaves [`rounds`] at zero and a caller, responder or grinder leaves this at zero, so
/// the kernel's stall check can ask one question of every worker (did either counter move?)
/// without holding a table of who plays which role.
#[must_use]
pub const fn wakes(i: usize) -> u64 {
    ((2 * MAX_WORKERS + i) as u64) * 8
}

/// The transform a responder applies to the word a caller sent.
///
/// It exists so that a reply carrying *some* value is not mistaken for a reply carrying the *right*
/// value. A stale mailbox, a wake completed against the wrong parked receive, or a reply delivered
/// to the wrong caller all produce a word; only the correct rendezvous produces this one. The
/// constant is arbitrary and the multiply-xor is chosen so that neighbouring sequence numbers give
/// answers that share no low bits, which a truncated or half-written word would not survive.
#[must_use]
pub const fn answer(sent: u64) -> u64 {
    (sent.wrapping_mul(0x9E37_79B9_7F4A_7C15)) ^ 0x5851_F42D_4C95_7F2D
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Every offset the page defines is inside the page.** One test, because the failure it
    /// prevents is a worker writing past its mapping and taking a fault the kernel would report as
    /// a soak failure with a misleading cause.
    #[test]
    fn every_offset_fits_in_the_page() {
        for i in 0..MAX_WORKERS {
            assert!(rounds(i) + 8 <= PAGE, "rounds({i}) escapes the page");
            assert!(
                mismatches(i) + 8 <= PAGE,
                "mismatches({i}) escapes the page"
            );
            assert!(wakes(i) + 8 <= PAGE, "wakes({i}) escapes the page");
        }
    }

    /// The three counter arrays do not overlap, which a hand-written offset scheme gets wrong
    /// exactly once and then reports as a workload that never makes progress.
    #[test]
    fn the_three_counter_arrays_are_disjoint() {
        assert!(rounds(MAX_WORKERS - 1) + 8 <= mismatches(0));
        assert!(mismatches(MAX_WORKERS - 1) + 8 <= wakes(0));
    }

    /// **Every one of the 192 slots is its own aligned word**, which is the contract the two
    /// binaries actually rely on and the one the two tests above only approximate.
    ///
    /// An offset function that collapses (`rounds` returning a constant), strides wrong or counts
    /// backwards passes them while giving two workers one word, which breaks the single-writer
    /// argument, and a `u64` store is untearable only on a `u64` boundary. The assertion is about
    /// distinctness rather than values, so it does not test the arithmetic against itself.
    /// Milestone 326 (nobody has been assigned to turn a mutation score upward): five of this
    /// crate's seven mutation survivors were offsets that collide.
    #[test]
    fn every_slot_is_its_own_aligned_word() {
        let mut offset = [0u64; 3 * MAX_WORKERS];
        for i in 0..MAX_WORKERS {
            offset[i] = rounds(i);
            offset[MAX_WORKERS + i] = mismatches(i);
            offset[2 * MAX_WORKERS + i] = wakes(i);
        }

        for a in 0..offset.len() {
            assert_eq!(
                offset[a] % 8,
                0,
                "slot {a} is at {}, which is not a u64 boundary",
                offset[a]
            );
            for b in (a + 1)..offset.len() {
                assert_ne!(
                    offset[a], offset[b],
                    "slots {a} and {b} are both at byte {}, so two writers own one word",
                    offset[a]
                );
            }
        }
    }

    /// **Neighbouring sequence numbers give unrelated answers**, which is the property that makes a
    /// wrong reply detectable rather than plausible. Checked against the two failures worth
    /// catching: an off-by-one delivery and a zeroed word.
    #[test]
    fn a_neighbouring_sequence_number_does_not_give_a_similar_answer() {
        for seq in [0u64, 1, 2, 1000, u64::MAX / 2] {
            assert_ne!(answer(seq), answer(seq.wrapping_add(1)));
            assert_ne!(answer(seq), 0, "a zeroed reply word must never be correct");
            assert_ne!(answer(seq), seq, "an echoed reply must never be correct");
        }
    }

    /// **No bit is set in every answer**, which is how this test states the property the whole
    /// transform rests on: it is a bijection, so two sequence numbers never share a reply word.
    ///
    /// Injectivity is the thing that makes a wrong reply *detectable*, and it is not something a
    /// handful of sampled pairs can show: a transform that only ever sets bits (`|` where this
    /// writes `^`) passes every assertion above, because neighbouring answers still differ and
    /// none of them is zero, while quietly mapping many sequence numbers onto one word. The tell
    /// is cheap and total rather than sampled. A mask leaves its own bits standing in every
    /// output; a xor clears a bit as often as it sets one, so intersecting enough answers leaves
    /// nothing. Milestone 326: the `^`-to-`|` mutant survived the three assertions above.
    #[test]
    fn the_answers_share_no_bit_in_common() {
        let mut common = u64::MAX;
        for seq in 0..64u64 {
            common &= answer(seq);
        }
        assert_eq!(
            common, 0,
            "every answer carries the bits {common:#x}, so the transform masks rather than \
             permutes and two sequence numbers can produce one reply"
        );
    }
}

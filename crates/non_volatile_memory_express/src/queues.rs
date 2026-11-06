//! **Ring arithmetic**: where the next command lands, where the next completion appears and which
//! phase marks it fresh.
//!
//! Name: ratified 2026-11-06 (UTC). The module holding [`SubmissionQueue`] and [`CompletionQueue`].

use crate::completion::Completion;

/// A submission ring's driver-side state: where the next command lands and what tail value to
/// ring. The controller's consumption (via [`Completion::submission_queue_head`]) is recorded by
/// [`SubmissionQueue::record_head`], and [`SubmissionQueue::push`] answers `None` when the ring is
/// full, keeping one slot empty so a full ring is distinguishable from an empty one.
///
/// Name: ratified 2026-10-25 (#2720). Refused `SqState` (an abbreviation, and the type is the
/// queue, not a state beside it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubmissionQueue {
    pub(crate) entries: u16,
    pub(crate) tail: u16,
    /// The controller's head, updated from each completion's [`Completion::submission_queue_head`].
    pub(crate) head: u16,
}

impl SubmissionQueue {
    /// A fresh ring of `entries` slots (at least 2: a one-slot ring cannot distinguish full from
    /// empty in tail/head form, and the specification's minimum queue size is 2).
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub fn new(entries: u16) -> Self {
        assert!(entries >= 2);
        SubmissionQueue {
            entries,
            tail: 0,
            head: 0,
        }
    }

    /// Claim the slot for one command: returns the slot index to write, advancing the tail. The
    /// caller then rings [`crate::doorbell`] with [`SubmissionQueue::tail`]. Returns `None`, and
    /// changes nothing, when the ring is full: the controller has not yet consumed enough
    /// commands, and the caller decides whether that is a bug or a reason to wait.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub fn push(&mut self) -> Option<u16> {
        let next = (self.tail + 1) % self.entries;
        if next == self.head {
            return None;
        }
        let slot = self.tail;
        self.tail = next;
        Some(slot)
    }

    /// The tail value to write to the doorbell after [`SubmissionQueue::push`].
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub fn tail(&self) -> u16 {
        self.tail
    }

    /// Record the controller's consumption, from a completion's `submission_queue_head`.
    ///
    /// Name: ratified 2026-10-25 (#2720). Refused `note_head` (it records the head the controller
    /// reported).
    pub fn record_head(&mut self, head: u16) {
        assert!(
            head < self.entries,
            "controller reported an impossible head"
        );
        self.head = head;
    }
}

/// A completion ring's driver-side state: where the next completion will appear and which phase
/// value marks it as fresh. The phase starts true (the ring starts zeroed, so the controller's
/// first lap writes 1s) and flips every time the head wraps.
///
/// # Examples
///
/// A zeroed ring reads as phase 0, the first lap writes 1s, and every lap flips:
///
/// ```
/// use non_volatile_memory_express::{Completion, CompletionQueue};
///
/// let mut queue = CompletionQueue::new(2); // a two-slot ring, so it laps quickly
///
/// // The initial zeros. Phase 0 against an expected phase of 1: not ours, do not read the result.
/// let stale = Completion::from_dwords([0, 0, 0, 0]);
/// assert!(!queue.is_owned(&stale));
///
/// // The controller's first completion, phase bit set, for command id 7.
/// let fresh = Completion::from_dwords([0, 0, 0, 7 | (1 << 16)]);
/// assert!(queue.is_owned(&fresh));
/// assert_eq!(fresh.cid, 7);
/// assert_eq!(fresh.status, 0); // 0 is success
/// assert_eq!(queue.pop(), 1); // the value the driver writes to the completion head doorbell
///
/// // Slot 1, same lap, still phase 1.
/// assert!(queue.is_owned(&fresh));
/// assert_eq!(queue.pop(), 0); // wrapped, so the expected phase flipped
///
/// // Now the ring has lapped, and last lap's entries are the stale ones. This is the whole trick:
/// // the bytes in slot 0 have not changed, and their meaning has.
/// assert!(!queue.is_owned(&fresh));
/// assert!(queue.is_owned(&stale));
/// ```
///
/// Name: ratified 2026-10-25 (#2720). Refused `CqState` (the same reason as `SqState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompletionQueue {
    pub(crate) entries: u16,
    pub(crate) head: u16,
    pub(crate) phase: bool,
}

impl CompletionQueue {
    /// A fresh ring of `entries` slots, phase starting true (see the struct docs).
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub fn new(entries: u16) -> Self {
        assert!(entries >= 2);
        CompletionQueue {
            entries,
            head: 0,
            phase: true,
        }
    }

    /// The slot to poll for the next completion.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub fn head(&self) -> u16 {
        self.head
    }

    /// Does this completion belong to the driver yet? True when its phase tag matches the phase
    /// this lap expects; a stale entry (last lap's, or the initial zeros) fails the match.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub fn is_owned(&self, c: &Completion) -> bool {
        c.phase == self.phase
    }

    /// Consume the entry at the head: advance, flipping the expected phase on wrap. Returns the
    /// new head, which the caller writes to this queue's [`crate::doorbell`].
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub fn pop(&mut self) -> u16 {
        self.head += 1;
        if self.head == self.entries {
            self.head = 0;
            self.phase = !self.phase;
        }
        self.head
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_phase_flips_every_lap_and_only_at_the_wrap() {
        let mut queue = CompletionQueue::new(3);
        let mut flips = 0;
        let mut last = true;
        for _ in 0..9 {
            assert!(queue.head() < 3);
            queue.pop();
            let phase_now = queue.is_owned(&Completion::from_dwords([0, 0, 0, 1 << 16]));
            if phase_now != last {
                flips += 1;
                last = phase_now;
            }
        }
        assert_eq!(flips, 3, "nine pops of a 3-ring wrap exactly three times");
    }

    #[test]
    fn head_reports_the_actual_next_slot_not_a_constant() {
        // A bound check alone is satisfied by a constant 0 or 1. Assert the actual progression.
        let mut queue = CompletionQueue::new(4);
        assert_eq!(queue.head(), 0);
        queue.pop();
        assert_eq!(queue.head(), 1);
        queue.pop();
        assert_eq!(queue.head(), 2);
    }

    #[test]
    fn the_submission_ring_walks_its_slots_in_order() {
        let mut queue = SubmissionQueue::new(4);
        // With completions consuming as we go (head following), the ring cycles forever.
        for i in 0..12u16 {
            assert_eq!(queue.push(), Some(i % 4));
            assert_eq!(queue.tail(), (i + 1) % 4);
            queue.record_head(queue.tail()); // the controller caught up
        }
    }

    #[test]
    fn a_full_submission_ring_refuses_a_push_and_changes_nothing() {
        let mut queue = SubmissionQueue::new(2);
        // Tail 1, head 0: one free slot left is the full condition.
        assert_eq!(queue.push(), Some(0));
        let full = queue;
        assert_eq!(queue.push(), None);
        assert_eq!(queue, full, "a refused push leaves the ring as it was");
        // The controller consumes the command, and the ring accepts another.
        queue.record_head(1);
        assert_eq!(queue.push(), Some(1));
    }
}

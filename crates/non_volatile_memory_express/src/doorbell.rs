//! **Doorbell addressing**: which BAR0 offset rings which queue.
//!
//! Name: ratified 2026-11-06 (UTC). The module holding [`Doorbell`] and [`doorbell`].

use crate::registers;

/// A queue's role, which picks its doorbell: submission tails are even doorbells, completion
/// heads odd (sections 3.1.24 and 3.1.25).
///
/// Name: ratified 2026-10-25 (#2720).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Doorbell {
    /// The submission queue's tail doorbell: even index.
    SubmissionTail,
    /// The completion queue's head doorbell: odd index.
    CompletionHead,
}

/// The BAR0 offset of queue `qid`'s doorbell. `dstrd` is [`crate::Capabilities::doorbell_stride`].
///
/// The stride scales the whole doorbell file: `CAP.DSTRD` is four bits, so a controller may space
/// its doorbells as far as `4 << 15` bytes apart, and queue 1's doorbells then sit hundreds of
/// kilobytes past `DOORBELL_BASE`. A caller that maps less of BAR0 than the doorbells it rings
/// refuses the strides that reach past its mapping.
///
/// # Examples
///
/// ```
/// use non_volatile_memory_express::{Capabilities, Doorbell, doorbell};
///
/// // A controller reporting DSTRD 0, so doorbells are 4 bytes apart.
/// let capabilities = Capabilities(0);
/// assert_eq!(capabilities.doorbell_stride(), 0);
///
/// // Admin queue (qid 0), then the one I/O queue pair this driver creates.
/// assert_eq!(doorbell(0, Doorbell::SubmissionTail, 0), 0x1000);
/// assert_eq!(doorbell(0, Doorbell::CompletionHead, 0), 0x1004);
/// assert_eq!(doorbell(1, Doorbell::SubmissionTail, 0), 0x1008);
/// assert_eq!(doorbell(1, Doorbell::CompletionHead, 0), 0x100c);
/// ```
///
/// Name: ratified 2026-10-25 (#2720).
pub fn doorbell(qid: u16, which: Doorbell, dstrd: u32) -> u64 {
    let index = 2 * qid as u64
        + match which {
            Doorbell::SubmissionTail => 0,
            Doorbell::CompletionHead => 1,
        };
    registers::DOORBELL_BASE + index * (4u64 << (dstrd & 0xf))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doorbells_land_where_the_spec_puts_them() {
        // Sections 3.1.24 and 3.1.25 worked example, stride 0: SQ0 tail at 0x1000, CQ0 head at
        // 0x1004, SQ1 tail 0x1008, CQ1 head 0x100c.
        assert_eq!(doorbell(0, Doorbell::SubmissionTail, 0), 0x1000);
        assert_eq!(doorbell(0, Doorbell::CompletionHead, 0), 0x1004);
        assert_eq!(doorbell(1, Doorbell::SubmissionTail, 0), 0x1008);
        assert_eq!(doorbell(1, Doorbell::CompletionHead, 0), 0x100c);
        // A stride spreads them: with DSTRD=2 each doorbell is 16 bytes apart.
        assert_eq!(doorbell(1, Doorbell::SubmissionTail, 2), 0x1020);
    }
}

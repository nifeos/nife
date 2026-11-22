//! **Raw-mode byte delivery**: [`RawQueue`] and its bound.
//!
//! Its fields are `pub(super)` because `handoff` saves and restores them.
//!
//! Name: provisional (milestone 878's lane, 2026-10-12 UTC; §295 (the line editor's crate root
//! splits into module files)), for the module and the file.

/// How many raw bytes [`RawQueue`] holds for a parked `OPERATION_READRAW`, when bytes arrive faster than
/// the raw reader drains them. Sized like [`MAXIMUM_LINE_BYTES`](crate::MAXIMUM_LINE_BYTES)'s bell-on-overflow
/// precedent: a burst past this is dropped, audibly, rather than grown without bound. 64 is
/// generous for a keystroke-at-a time reader (`kilo`'s own event loop reads one key, redraws, reads
/// the next) and still covers a terminal-paste dumped in one go.
///
/// Name: provisional, minted 2026-11-22 (UTC) by milestone/1098-finish-review; the units rule ends a limit with its unit.
pub const MAXIMUM_RAW_QUEUE_BYTES: usize = 64;

/// **Raw-mode byte delivery** (milestone 169 (`kilo`, the smallest real text editor, as the forcing function for raw terminal input), `OPERATION_READRAW`): a plain FIFO of wire bytes, with no
/// interpretation of any of them. Kept separate from [`LineEditor`](crate::LineEditor) rather than a mode bit on it,
/// because raw mode's whole point is that nothing here ever looks at what a byte means; a
/// `LineEditor` exists to do exactly that.
pub struct RawQueue {
    pub(super) buffer: [u8; MAXIMUM_RAW_QUEUE_BYTES],
    pub(super) head: usize,
    pub(super) length_bytes: usize,
}

impl Default for RawQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl RawQueue {
    /// An empty queue. `const` for the same reason [`LineEditor::new`](crate::LineEditor::new) is: so a holder can be a
    /// `static` instead of a stack local.
    pub const fn new() -> Self {
        RawQueue {
            buffer: [0; MAXIMUM_RAW_QUEUE_BYTES],
            head: 0,
            length_bytes: 0,
        }
    }

    /// True once nothing is queued.
    pub fn is_empty(&self) -> bool {
        self.length_bytes == 0
    }

    /// Push bytes onto the queue. Returns the number actually pushed; once the queue is full the
    /// rest are dropped, and the caller (which holds the [`Sink`](crate::Sink) this crate deliberately does
    /// not) is the one that rings the bell, the same split [`LineEditor::feed`](crate::LineEditor::feed) keeps between
    /// interpretation and echo.
    pub fn push(&mut self, bytes: &[u8]) -> usize {
        let room = MAXIMUM_RAW_QUEUE_BYTES - self.length_bytes;
        let n = bytes.len().min(room);
        for &b in &bytes[..n] {
            self.buffer[(self.head + self.length_bytes) % MAXIMUM_RAW_QUEUE_BYTES] = b;
            self.length_bytes += 1;
        }
        n
    }

    /// Pop up to 8 bytes (the register-only reply's own limit), packed little-endian. `None` if
    /// the queue is empty.
    ///
    /// Name: provisional, minted 2026-11-22 (UTC) by milestone/1098-finish-review; the eight is spelled out.
    pub fn pop_up_to_eight(&mut self) -> Option<(usize, u64)> {
        if self.length_bytes == 0 {
            return None;
        }
        let n = self.length_bytes.min(8);
        let mut packed = 0u64;
        for i in 0..n {
            packed |= (self.buffer[(self.head + i) % MAXIMUM_RAW_QUEUE_BYTES] as u64) << (8 * i);
        }
        self.head = (self.head + n) % MAXIMUM_RAW_QUEUE_BYTES;
        self.length_bytes -= n;
        Some((n, packed))
    }
}

#[cfg(test)]
mod tests;

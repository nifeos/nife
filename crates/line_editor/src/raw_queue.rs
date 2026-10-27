//! **Raw-mode byte delivery**: [`RawQueue`] and its bound.
//!
//! Its fields are `pub(super)` because `handoff` saves and restores them.
//!
//! Name: provisional (milestone 878's lane, 2026-10-12 UTC; §295 (the line editor's crate root
//! splits into module files)), for the module and the file.

/// How many raw bytes [`RawQueue`] holds for a parked `OPERATION_READRAW`, when bytes arrive faster than
/// the raw reader drains them. Sized like [`LINE_MAX`](crate::LINE_MAX)'s bell-on-overflow precedent: a burst past
/// this is dropped, audibly, rather than grown without bound. 64 is generous for a keystroke-at-a
/// time reader (`kilo`'s own event loop reads one key, redraws, reads the next) and still covers a
/// terminal-paste dumped in one go.
pub const RAW_QUEUE_MAX: usize = 64;

/// **Raw-mode byte delivery** (milestone 169 (`kilo`, the smallest real text editor, as the forcing function for raw terminal input), `OPERATION_READRAW`): a plain FIFO of wire bytes, with no
/// interpretation of any of them. Kept separate from [`LineDisc`](crate::LineDisc) rather than a mode bit on it,
/// because raw mode's whole point is that nothing here ever looks at what a byte means; a
/// `LineDisc` exists to do exactly that.
pub struct RawQueue {
    pub(super) buf: [u8; RAW_QUEUE_MAX],
    pub(super) head: usize,
    pub(super) len: usize,
}

impl Default for RawQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl RawQueue {
    /// An empty queue. `const` for the same reason [`LineDisc::new`](crate::LineDisc::new) is: so a holder can be a
    /// `static` instead of a stack local.
    pub const fn new() -> Self {
        RawQueue {
            buf: [0; RAW_QUEUE_MAX],
            head: 0,
            len: 0,
        }
    }

    /// True once nothing is queued.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Push bytes onto the queue. Returns the number actually pushed; once the queue is full the
    /// rest are dropped, and the caller (which holds the [`Sink`](crate::Sink) this crate deliberately does
    /// not) is the one that rings the bell, the same split [`LineDisc::feed`](crate::LineDisc::feed) keeps between
    /// interpretation and echo.
    pub fn push(&mut self, bytes: &[u8]) -> usize {
        let room = RAW_QUEUE_MAX - self.len;
        let n = bytes.len().min(room);
        for &b in &bytes[..n] {
            self.buf[(self.head + self.len) % RAW_QUEUE_MAX] = b;
            self.len += 1;
        }
        n
    }

    /// Pop up to 8 bytes (the register-only reply's own limit), packed little-endian. `None` if
    /// the queue is empty.
    pub fn pop8(&mut self) -> Option<(usize, u64)> {
        if self.len == 0 {
            return None;
        }
        let n = self.len.min(8);
        let mut packed = 0u64;
        for i in 0..n {
            packed |= (self.buf[(self.head + i) % RAW_QUEUE_MAX] as u64) << (8 * i);
        }
        self.head = (self.head + n) % RAW_QUEUE_MAX;
        self.len -= n;
        Some((n, packed))
    }
}

#[cfg(test)]
mod tests;

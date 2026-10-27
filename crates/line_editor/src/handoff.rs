// This module's docs are on its `pub mod` line in `lib.rs`.

use super::{HIST, LINE_MAX, LineDisc, PROMPT_MAX, RAW_QUEUE_MAX, RawQueue};

/// "LEDT", then the layout, in the blob's first eight bytes.
pub const MAGIC: u32 = 0x5444_454c;
/// The one layout this build writes and reads.
pub const LAYOUT: u32 = 1;

/// A cursor writing into a blob. Refuses to run past its end rather than truncating, because a
/// truncated blob that parsed would be state silently lost.
pub struct Writer<'a> {
    buf: &'a mut [u8],
    at: usize,
    full: bool,
}

impl<'a> Writer<'a> {
    /// Start a blob: the magic and layout go in first.
    pub fn new(buf: &'a mut [u8]) -> Self {
        let mut w = Writer {
            buf,
            at: 0,
            full: false,
        };
        w.u32(MAGIC);
        w.u32(LAYOUT);
        w
    }
    /// Append bytes.
    pub fn bytes(&mut self, b: &[u8]) {
        if self.at + b.len() > self.buf.len() {
            self.full = true;
            return;
        }
        self.buf[self.at..self.at + b.len()].copy_from_slice(b);
        self.at += b.len();
    }
    /// Append a little-endian word.
    pub fn u32(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes());
    }
    /// Append a length, then that many bytes.
    pub fn field(&mut self, b: &[u8]) {
        self.u32(b.len() as u32);
        self.bytes(b);
    }
    /// How many bytes were written, or `None` if the blob did not fit.
    pub fn finish(self) -> Option<usize> {
        if self.full { None } else { Some(self.at) }
    }
}

/// A cursor reading a blob.
pub struct Reader<'a> {
    buf: &'a [u8],
    at: usize,
}

/// Why a blob was not absorbed.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Refusal {
    /// The blob does not start with [`MAGIC`]: there is no blob at all.
    NoBlob,
    /// The blob is in a layout this build does not understand; carries that layout.
    Layout(u32),
    /// The blob ends early or a field is longer than its buffer.
    Malformed,
}

impl<'a> Reader<'a> {
    /// Open a blob, checking the magic and the layout.
    pub fn new(buf: &'a [u8]) -> Result<Self, Refusal> {
        let mut r = Reader { buf, at: 0 };
        if r.u32().ok_or(Refusal::NoBlob)? != MAGIC {
            return Err(Refusal::NoBlob);
        }
        let layout = r.u32().ok_or(Refusal::Malformed)?;
        if layout != LAYOUT {
            return Err(Refusal::Layout(layout));
        }
        Ok(r)
    }
    /// Read `n` bytes.
    pub fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let out = self.buf.get(self.at..self.at.checked_add(n)?)?;
        self.at += n;
        Some(out)
    }
    /// Read a little-endian word.
    pub fn u32(&mut self) -> Option<u32> {
        let b = self.bytes(4)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    /// Read a length-prefixed field into `out`, returning its length. `None` when it would
    /// not fit, which is a malformed blob rather than a truncation.
    pub fn field(&mut self, out: &mut [u8]) -> Option<usize> {
        let n = self.u32()? as usize;
        if n > out.len() {
            return None;
        }
        out[..n].copy_from_slice(self.bytes(n)?);
        Some(n)
    }
}

impl LineDisc {
    /// Write this discipline's carried state (see the module docs) into `w`.
    pub fn save(&self, w: &mut Writer<'_>) {
        w.field(&self.buf[..self.len]);
        w.u32(self.cur as u32);
        w.field(&self.kill[..self.kill_len]);
        w.field(&self.prompt[..self.prompt_len]);
        w.u32(self.hist_count as u32);
        w.u32(self.hist_next as u32);
        for (line, n) in &self.hist {
            w.field(&line[..*n]);
        }
    }

    /// Replace this discipline's carried state with what `r` holds. On `None` the discipline
    /// may be half written, and the caller refuses the whole blob rather than serve from it.
    pub fn restore(&mut self, r: &mut Reader<'_>) -> Option<()> {
        self.len = r.field(&mut self.buf)?;
        let cur = r.u32()? as usize;
        if cur > self.len {
            return None;
        }
        self.cur = cur;
        self.kill_len = r.field(&mut self.kill)?;
        self.prompt_len = r.field(&mut self.prompt[..PROMPT_MAX])?;
        self.hist_count = (r.u32()? as usize).min(HIST);
        self.hist_next = (r.u32()? as usize) % HIST;
        for entry in &mut self.hist {
            let mut line = [0u8; LINE_MAX];
            let n = r.field(&mut line)?;
            *entry = (line, n);
        }
        self.browse = None;
        Some(())
    }
}

impl RawQueue {
    /// Write the unread raw bytes into `w`, oldest first.
    pub fn save(&self, w: &mut Writer<'_>) {
        let mut out = [0u8; RAW_QUEUE_MAX];
        for (i, b) in out.iter_mut().take(self.len).enumerate() {
            *b = self.buf[(self.head + i) % RAW_QUEUE_MAX];
        }
        w.field(&out[..self.len]);
    }

    /// Replace the queue with what `r` holds.
    pub fn restore(&mut self, r: &mut Reader<'_>) -> Option<()> {
        let mut buf = [0u8; RAW_QUEUE_MAX];
        let n = r.field(&mut buf)?;
        *self = RawQueue::new();
        self.push(&buf[..n]);
        Some(())
    }
}

#[cfg(test)]
mod tests;

/// The entry is a directory, so [`super::fs::OPENDIR`] is the verb for it rather than
/// [`super::fs::OPEN`]. Carried because a listing whose reader has to open every name to find
/// out what it is turns one enumeration into N opens, and because completion wants to know.
///
/// Name: provisional, minted 2026-11-18 (UTC) by the lane of milestone 1090 (an outside review of `filesystem_contract`).
pub const IS_DIRECTORY: u8 = 1 << 0;

/// Bytes one record takes: two for the header, plus the name.
pub const fn record_len(name_len: usize) -> usize {
    2 + name_len
}

/// Write one record at the start of `out`, returning its length, or `None` if it does not fit or
/// the name is too long to encode.
pub fn encode(out: &mut [u8], name: &[u8], is_directory: bool) -> Option<usize> {
    let n = record_len(name.len());
    if name.len() > u8::MAX as usize || out.len() < n {
        return None;
    }
    out[0] = if is_directory { IS_DIRECTORY } else { 0 };
    out[1] = name.len() as u8;
    out[2..n].copy_from_slice(name);
    Some(n)
}

/// Walk the records in `buf` (exactly the `r0` bytes a [`fs::READDIR`] reply filled). Stops at
/// the first record that runs off the end, which is what a truncated or corrupt reply looks like
/// and which must not be read as a name.
///
/// [`fs::READDIR`]: super::fs::READDIR
pub fn iter(buf: &[u8]) -> Entries<'_> {
    Entries { buf, at: 0 }
}

/// The iterator [`iter`] returns: `(name, is_directory)` per entry.
pub struct Entries<'a> {
    buf: &'a [u8],
    at: usize,
}

impl<'a> Iterator for Entries<'a> {
    type Item = (&'a [u8], bool);

    fn next(&mut self) -> Option<Self::Item> {
        let head = self.buf.get(self.at..self.at + 2)?;
        let (flags, len) = (head[0], head[1] as usize);
        let name = self.buf.get(self.at + 2..self.at + 2 + len)?;
        self.at += record_len(len);
        Some((name, flags & IS_DIRECTORY != 0))
    }
}

#[cfg(test)]
mod tests;

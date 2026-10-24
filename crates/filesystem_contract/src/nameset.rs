use super::grant;

/// **The most names one set grant carries**, where `ARG_MAX` becomes a capability limit.
///
/// Measured, not chosen: at sixteen names the shell ran off its stack planning one grant, because a
/// set travels by value through four frames a debug build does not collapse. Eight names of sixteen
/// bytes is 152 bytes per copy. Exceeding it is a refusal at the prompt
/// (`grant_plan::Refusal::TooManyNames`), never a truncation, because `caps rm *.txt` prints the
/// set before anything runs and a grant that quietly took a prefix would be one nobody checked.
pub const MAX_NAMES: usize = 8;

/// Bytes an encoded set occupies at most: a header and a name each, plus the terminator.
pub const BYTES: usize = MAX_NAMES * (1 + grant::MAX_NAME) + 1;

/// Bit 7 of a record's header: this name was a directory when the grant was planned.
pub const IS_DIR: u8 = 1 << 7;

/// The low bits of a record's header: the name's length. Zero terminates the set.
const LEN: u8 = 0x7f;

/// Write `names` into `out`, returning the bytes used, or `None` if the set does not fit, a name
/// is empty or too long, or there are more than [`MAX_NAMES`] of them.
///
/// **`None` rather than a short write**, for [`MAX_NAMES`]'s reason: the caller is building a
/// capability, and a truncated capability is a different capability.
pub fn encode(names: &[(&[u8], bool)], out: &mut [u8]) -> Option<usize> {
    if names.len() > MAX_NAMES {
        return None;
    }
    let mut n = 0;
    for &(name, is_dir) in names {
        if name.is_empty() || name.len() > grant::MAX_NAME {
            return None;
        }
        let end = n + 1 + name.len();
        if end + 1 > out.len() {
            return None;
        }
        out[n] = name.len() as u8 | if is_dir { IS_DIR } else { 0 };
        out[n + 1..end].copy_from_slice(name);
        n = end;
    }
    *out.get_mut(n)? = 0;
    Some(n + 1)
}

/// Walk an encoded set: `(name, is_dir)` per record. Stops at the terminator, and at the first
/// record that runs off the end, which is what a truncated buffer looks like and must not be
/// read as a name.
pub fn iter(buf: &[u8]) -> Names<'_> {
    Names { buf, at: 0 }
}

/// Whether `name` is in the set. **This is the whole of the attenuation**: the caretaker asks
/// it once per name-taking request against its granted directory, and a name that is not in the
/// set is `ENOENT`, because in that scope there is no such name.
pub fn contains(buf: &[u8], name: &[u8]) -> bool {
    iter(buf).any(|(n, _)| n == name)
}

/// How many names the set holds.
pub fn count(buf: &[u8]) -> usize {
    iter(buf).count()
}

/// The iterator [`iter`] returns.
pub struct Names<'a> {
    buf: &'a [u8],
    at: usize,
}

impl<'a> Iterator for Names<'a> {
    type Item = (&'a [u8], bool);

    fn next(&mut self) -> Option<Self::Item> {
        let head = *self.buf.get(self.at)?;
        let len = (head & LEN) as usize;
        if len == 0 {
            return None;
        }
        let name = self.buf.get(self.at + 1..self.at + 1 + len)?;
        self.at += 1 + len;
        Some((name, head & IS_DIR != 0))
    }
}

#[cfg(test)]
mod tests;

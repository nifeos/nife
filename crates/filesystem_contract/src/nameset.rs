use super::grant;

/// **The most names one set grant carries**, where `ARG_MAX` becomes a capability limit.
///
/// Measured, not chosen: at sixteen names the shell ran off its stack planning one grant, because a
/// set travels by value through four frames a debug build does not collapse. Exceeding it is a
/// refusal at the prompt (`grant_plan::Refusal::TooManyNames`), never a truncation, because `caps rm
/// *.txt` prints the set before anything runs and a grant that quietly took a prefix would be one
/// nobody checked. See notes/glob-grant/xargs.md ("Why the bound is eight").
pub const MAX_NAMES: usize = 8;

/// **The most bytes of name one set carries, all its names together.** A second bound beside
/// [`MAX_NAMES`], for the same stack: a set travels by value through the shell's frames, so what it
/// costs is its bytes, and holding eight names of [`grant::MAX_NAME`] each would cost eight times
/// what a set of ordinary names needs. Two of the longest names fit, or eight of 64 bytes. Measured
/// with milestone 946 (a granted file name can be longer than sixteen bytes); the measurement is in notes/glob-grant/xargs.md ("The bytes a set carries").
/// Over it is the same refusal as a ninth name, never a truncation.
///
/// Name: provisional (milestone 946).
pub const NAME_BYTES: usize = 512;

/// Bytes an encoded set occupies at most: a two-byte header per name, the names, and the
/// terminator.
pub const BYTES: usize = 2 * MAX_NAMES + NAME_BYTES + 1;

// A set arrives in one page, and the longest name has to fit in one set.
const _: () = assert!(BYTES <= super::PAGE);
const _: () = assert!(NAME_BYTES >= 2 * grant::MAX_NAME);

/// Bit 0 of a record's flags byte: this name was a directory when the grant was planned.
pub const IS_DIR: u8 = 1 << 0;

/// Write `names` into `out`, returning the bytes used, or `None` if the set does not fit, a name
/// is empty or too long, there are more than [`MAX_NAMES`] of them, or together they are over
/// [`NAME_BYTES`].
///
/// A record is a length byte, a flags byte ([`IS_DIR`]), then the name; a zero length ends the set,
/// so a name may not be empty.
///
/// **`None` rather than a short write**, for [`MAX_NAMES`]'s reason: the caller is building a
/// capability, and a truncated capability is a different capability.
pub fn encode(names: &[(&[u8], bool)], out: &mut [u8]) -> Option<usize> {
    if names.len() > MAX_NAMES {
        return None;
    }
    let mut n = 0;
    let mut name_bytes = 0;
    for &(name, is_dir) in names {
        if !grant::fits(name) {
            return None;
        }
        name_bytes += name.len();
        if name_bytes > NAME_BYTES {
            return None;
        }
        let end = n + 2 + name.len();
        if end + 1 > out.len() {
            return None;
        }
        out[n] = name.len() as u8;
        out[n + 1] = if is_dir { IS_DIR } else { 0 };
        out[n + 2..end].copy_from_slice(name);
        n = end;
    }
    *out.get_mut(n)? = 0;
    Some(n + 1)
}

/// Walk an encoded set: `(name, is_dir)` per record. Stops at the terminator, at the first
/// record that runs off the end, which is what a truncated buffer looks like and must not be
/// read as a name, and after [`MAX_NAMES`] records.
///
/// **The last stop is the contract, not a guess.** [`encode`] never writes more than
/// [`MAX_NAMES`] records, so a buffer holding more was not made by it. Reading only the first
/// [`MAX_NAMES`] of them can only make a set narrower, never wider. It also bounds every walk a
/// caretaker makes against a buffer it was handed.
pub fn iter(buf: &[u8]) -> core::iter::Take<Names<'_>> {
    Names { buf }.take(MAX_NAMES)
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
    /// What is left to read, starting at the next record.
    buf: &'a [u8],
}

impl<'a> Iterator for Names<'a> {
    type Item = (&'a [u8], bool);

    fn next(&mut self) -> Option<Self::Item> {
        let (&len, rest) = self.buf.split_first()?;
        if len == 0 {
            return None;
        }
        let (&flags, rest) = rest.split_first()?;
        let (name, rest) = rest.split_at_checked(len as usize)?;
        self.buf = rest;
        Some((name, flags & IS_DIR != 0))
    }
}

#[cfg(test)]
mod tests;

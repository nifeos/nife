/// The granted directions, packed into the caretaker's spec word. Read alone is the common case
/// (`run wc report.txt`); write implies read, since a writer that cannot read back is a shape
/// nothing has asked for.
pub const READ: u64 = 1 << 0;
/// Implies [`READ`]: a writer that cannot read back is a shape nothing has asked for.
pub const WRITE: u64 = 1 << 1;

/// **The longest granted name, in bytes**: what the shell lets a line name as one file, what a
/// name set ([`super::nameset`]) carries per name, and what a caretaker compares against. 255, which
/// is Linux's `NAME_MAX` and [`super::xattr::MAX_NAME`], so a file a customer brings is not refused
/// for its name. It is **provisional** (milestone 946 (a granted file name can be longer than sixteen bytes)): the bound is shared by the filesystem and the
/// loader, so it is an architect's call, and it lives in this one constant so the call is one edit.
/// RedoxFS itself stores at most 252 bytes of name (`DIR_ENTRY_MAX_LENGTH`), so a create of a longer
/// name is the server's refusal, not this contract's.
///
/// A name this long does not fit in a caretaker's `START` words. Those carry at most
/// [`WORDS_NAME`], and a name over that travels on a page: a name set's frame, or the bytes the
/// shell opens itself. [`pack_name`] refuses rather than truncates, so nothing can put a long name in
/// two words by mistake.
pub const MAX_NAME: usize = 255;

/// **The longest name two `START` argument words carry**: sixteen bytes, eight per word. A
/// caretaker's granted directory, `rm`'s operand, a `login` identity's subtree and an activation
/// request's package name ride this way, so they cost no extra page and the process needs nothing
/// mapped before it runs. Anything longer is refused where it would be packed ([`pack_name`]
/// answers `None`), never truncated, because a truncated name is a different file.
///
/// Name: provisional (milestone 946), for "what two words can hold" beside [`MAX_NAME`]'s "what a
/// grant can name".
pub const WORDS_NAME: usize = 16;

/// `EROFS`, the reply to a write through a read-only grant. Chosen over `EACCES` on purpose:
/// `EACCES` is the Unix answer, "you were denied", which implies a policy that could have said
/// yes. There is no policy here. The capability carries one direction, so the honest statement
/// is about the thing itself, and `std::io::ErrorKind::ReadOnlyFilesystem` is what a caller sees.
pub const EROFS: i32 = 30;

/// `ENOTDIR`, the reply to a [`super::fs::CREATE`] through a file grant. "This is a file, not a
/// directory" is a fact about what the holder has, not a refusal of what it asked.
pub const ENOTDIR: i32 = 20;

/// The handle the caretaker mints for the one file it serves. Fixed, because there is exactly
/// one: a holder that guesses a different number gets `EBADF` from the same check every other
/// handle goes through.
pub const HANDLE: u64 = 0;

/// **Pack a name into the two argument words a process is started with**, or `None` when it is
/// empty or longer than [`WORDS_NAME`]. Names shorter than that are zero-padded. `None` rather than
/// a prefix, so a caller that forgot to check cannot start a caretaker on a different name.
pub const fn pack_name(name: &[u8]) -> Option<(u64, u64)> {
    if !fits_words(name) {
        return None;
    }
    let mut lo = [0u8; 8];
    let mut hi = [0u8; 8];
    let mut i = 0;
    while i < name.len() && i < 8 {
        lo[i] = name[i];
        i += 1;
    }
    while i < name.len() {
        hi[i - 8] = name[i];
        i += 1;
    }
    Some((u64::from_le_bytes(lo), u64::from_le_bytes(hi)))
}

/// Unpack a name from two argument words into `buf` and return its length, at most
/// [`WORDS_NAME`]: a length word claiming more is clamped to what two words can hold.
pub fn unpack_name(lo: u64, hi: u64, len: usize, buf: &mut [u8; WORDS_NAME]) -> usize {
    buf[..8].copy_from_slice(&lo.to_le_bytes());
    buf[8..].copy_from_slice(&hi.to_le_bytes());
    len.min(WORDS_NAME)
}

/// Whether a name can be granted at all: one to [`MAX_NAME`] bytes.
pub const fn fits(name: &[u8]) -> bool {
    !name.is_empty() && name.len() <= MAX_NAME
}

/// Whether a name fits in two `START` argument words: one to [`WORDS_NAME`] bytes.
///
/// Name: provisional (milestone 946), after [`fits`] and [`WORDS_NAME`].
pub const fn fits_words(name: &[u8]) -> bool {
    !name.is_empty() && name.len() <= WORDS_NAME
}

/// Pack the name length and the granted rights into the caretaker's third argument word.
///
/// # Examples
///
/// `wc` starts holding one file, read-only, with the name in two argument words.
///
/// ```
/// use filesystem_contract::grant;
///
/// assert!(grant::fits_words(b"report.txt"));
/// let (lo, hi) = grant::pack_name(b"report.txt").unwrap();
/// let spec = grant::spec(b"report.txt".len(), grant::READ);
///
/// // What the caretaker unpacks from its three argument words.
/// let mut buf = [0u8; grant::WORDS_NAME];
/// let n = grant::unpack_name(lo, hi, grant::spec_len(spec), &mut buf);
/// assert_eq!(&buf[..n], b"report.txt");
///
/// // A read-only grant is not a policy that could have said yes, so a write gets EROFS.
/// assert!(!grant::is_writable(spec));
/// assert_eq!(grant::EROFS, 30);
///
/// // A name too long for two words can still be granted, on a page, but is refused by the packer
/// // rather than truncated on the wire.
/// assert!(grant::fits(b"quarterly-report.txt"));
/// assert!(grant::pack_name(b"quarterly-report.txt").is_none());
/// ```
pub const fn spec(len: usize, rights: u64) -> u64 {
    ((len as u64) & 0xff) | (rights << 8)
}

/// The granted name's length from a spec word.
pub const fn spec_len(w: u64) -> usize {
    (w & 0xff) as usize
}

/// The granted rights from a spec word.
pub const fn spec_rights(w: u64) -> u64 {
    w >> 8
}

/// Whether a spec word grants writing.
pub const fn is_writable(w: u64) -> bool {
    spec_rights(w) & WRITE != 0
}

/// **A grant whose operand is the holder's whole namespace**, spelled as a name of zero length.
///
/// A per-file or per-directory grant names one thing, and [`fits`] refuses an empty name, so a
/// length of zero cannot mean a name and is free to mean something else. It means the grant
/// carries a **set** ([`super::nameset`]), which does not fit in two argument words, so the
/// program is told "act on everything you can see" and reads the set by enumerating the
/// capability it was handed. That is not a loophole: the namespace of a set capability *is* the
/// set, so enumerating it reveals exactly what the command line already showed.
pub const WHOLE_NAMESPACE: usize = 0;

#[cfg(test)]
mod tests;

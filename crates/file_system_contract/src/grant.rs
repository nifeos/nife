/// **The directions a file grant can carry**, packed into the caretaker's spec word: `READ` and `WRITE`. Paths read `grant::rights::READ`.
///
/// Name: provisional, minted 2026-11-22 (UTC) by the file-system-contract-rename lane (ruled 2026-11-22).
pub mod rights {
    /// The granted directions, packed into the caretaker's spec word. Read alone is the common case
    /// (`run wc report.txt`); write implies read, since a writer that cannot read back is a shape
    /// nothing has asked for.
    pub const READ: u64 = 1 << 0;
    /// Implies [`READ`]: a writer that cannot read back is a shape nothing has asked for.
    pub const WRITE: u64 = 1 << 1;
}

/// **The longest granted name, in bytes**: what the shell lets a line name as one file, what a
/// name set ([`super::nameset`]) carries per name, and what a caretaker compares against. 252, which
/// is the bound for a program, a package and the files they travel in:
/// RedoxFS's `DIR_ENTRY_MAX_LENGTH`, the smallest name limit of a filesystem this stack installs onto.
/// Chosen over 255, Linux's `NAME_MAX`, which would make a 253-byte name grantable and unstorable
/// on the stack's own disk.
///
/// A name this long does not fit in a caretaker's `START` words. Those carry at most
/// [`MAXIMUM_WORDS_NAME_BYTES`], and a name over that travels on a page: a name set's frame, or the bytes the
/// shell opens itself. [`pack_name`] refuses rather than truncates, so nothing can put a long name in
/// two words by mistake.
///
/// Name: ratified 2026-10-27 (#2797).
pub const MAXIMUM_NAME_BYTES: usize = 252;

/// **The longest name two `START` argument words carry**: sixteen bytes, eight per word. A
/// caretaker's granted directory, `rm`'s operand, a `login` identity's subtree and an activation
/// request's package name ride this way, so they cost no extra page and the process needs nothing
/// mapped before it runs. Anything longer is refused where it would be packed ([`pack_name`]
/// answers `None`), never truncated, because a truncated name is a different file.
///
/// Name: provisional (milestone 946 (a granted file name longer than sixteen bytes)), for "what two words can hold" beside [`MAXIMUM_NAME_BYTES`]'s "what
/// a grant can name".
pub const MAXIMUM_WORDS_NAME_BYTES: usize = 16;

/// `EROFS`, the reply to a write through a read-only grant. Chosen over `EACCES` on purpose:
/// `EACCES` is the Unix answer, "you were denied", which implies a policy that could have said
/// yes. There is no policy here. The capability carries one direction, so the honest statement
/// is about the thing itself, and `std::io::ErrorKind::ReadOnlyFilesystem` is what a caller sees.
///
/// The POSIX error number, spelled as `errno.h` spells it.
#[doc(alias = "read-only file system")]
pub const EROFS: i32 = 30;

/// `ENOTDIR`, the reply to a [`super::file_system::CREATE`] through a file grant. "This is a file, not a
/// directory" is a fact about what the holder has, not a refusal of what it asked.
///
/// The POSIX error number, spelled as `errno.h` spells it.
#[doc(alias = "not a directory")]
pub const ENOTDIR: i32 = 20;

/// The handle the caretaker mints for the one file it serves. Fixed, because there is exactly
/// one: a holder that guesses a different number gets `EBADF` from the same check every other
/// handle goes through.
pub const HANDLE: u64 = 0;

/// **Pack a name into the two argument words a process is started with**, or `None` when it is
/// empty or longer than [`MAXIMUM_WORDS_NAME_BYTES`]. Names shorter than that are zero-padded. `None` rather than
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
/// [`MAXIMUM_WORDS_NAME_BYTES`]: a length word claiming more is clamped to what two words can hold.
pub fn unpack_name(
    lo: u64,
    hi: u64,
    len: usize,
    buf: &mut [u8; MAXIMUM_WORDS_NAME_BYTES],
) -> usize {
    buf[..8].copy_from_slice(&lo.to_le_bytes());
    buf[8..].copy_from_slice(&hi.to_le_bytes());
    len.min(MAXIMUM_WORDS_NAME_BYTES)
}

/// Whether a name can be granted at all: one to [`MAXIMUM_NAME_BYTES`] bytes.
pub const fn fits(name: &[u8]) -> bool {
    !name.is_empty() && name.len() <= MAXIMUM_NAME_BYTES
}

/// Whether a name fits in two `START` argument words: one to [`MAXIMUM_WORDS_NAME_BYTES`] bytes.
///
/// Name: provisional (milestone 946), after [`fits`] and [`MAXIMUM_WORDS_NAME_BYTES`].
pub const fn fits_words(name: &[u8]) -> bool {
    !name.is_empty() && name.len() <= MAXIMUM_WORDS_NAME_BYTES
}

/// Pack the name length and the granted rights into the caretaker's third argument word.
///
/// # Examples
///
/// `wc` starts holding one file, read-only, with the name in two argument words.
///
/// ```
/// use file_system_contract::grant;
///
/// assert!(grant::fits_words(b"report.txt"));
/// let (lo, hi) = grant::pack_name(b"report.txt").unwrap();
/// let spec = grant::specification(b"report.txt".len(), grant::rights::READ);
///
/// // What the caretaker unpacks from its three argument words.
/// let mut buf = [0u8; grant::MAXIMUM_WORDS_NAME_BYTES];
/// let n = grant::unpack_name(lo, hi, grant::specification_len(spec), &mut buf);
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
///
/// Name: provisional, minted 2026-11-18 (UTC) by the lane of milestone 1090 (an outside review of `file_system_contract`).
pub const fn specification(len: usize, rights: u64) -> u64 {
    ((len as u64) & 0xff) | (rights << 8)
}

/// The granted name's length from a spec word.
///
/// Name: provisional, minted 2026-11-18 (UTC) by the lane of milestone 1090 (an outside review of `file_system_contract`).
pub const fn specification_len(w: u64) -> usize {
    (w & 0xff) as usize
}

/// The granted rights from a spec word.
///
/// Name: provisional, minted 2026-11-18 (UTC) by the lane of milestone 1090 (an outside review of `file_system_contract`).
pub const fn specification_rights(w: u64) -> u64 {
    w >> 8
}

/// Whether a spec word grants writing.
pub const fn is_writable(w: u64) -> bool {
    specification_rights(w) & rights::WRITE != 0
}

/// **Where a caretaker's grant page is mapped**, read-only, in the caretaker's address space. Its
/// builder maps the page here and the caretaker reads it here, both through this one constant: the
/// progenitor, `login` and the kernel's test wiring on one side, `fs_subtree_caretaker`,
/// `fs_nameset_caretaker` and `fs_file_caretaker` on the other. A page of the address space map's
/// pair-page window; the progenitor checks it against the map at compile time
/// (`address_space_layout::pair_page`), so this crate keeps no dependency of its own.
///
/// Name: provisional.
pub const PAGE_VIRTUAL_ADDRESS: u64 = 0x0000_0000_0070_0000;

/// **Where a caretaker's grant page holds its name set**, in bytes from the start of the page.
///
/// A caretaker is started with one read-only page, its grant page, and every name it is granted
/// travels there rather than in its `START` words. The granted name (the directory a subtree or
/// name-set caretaker descends into, or the one file a file caretaker answers for) is at offset 0
/// as a set of one name in [`super::nameset`]'s encoding, or as an empty set for a grant with no
/// name to descend by. A name-set caretaker's set follows here. 256 is the first offset past the
/// longest one-name set, a two-byte header, [`MAXIMUM_NAME_BYTES`] and the terminator, so the two
/// never overlap whatever the name.
///
/// Name: provisional.
pub const PAGE_NAME_SET_OFFSET: usize = 256;

const _: () = assert!(PAGE_NAME_SET_OFFSET > 2 + MAXIMUM_NAME_BYTES);

/// **How many bytes of its grant page a caretaker reads**: the granted name and the longest set
/// after it. Far under a page, so one frame carries the whole grant.
///
/// Name: provisional.
pub const PAGE_USED_BYTES: usize = PAGE_NAME_SET_OFFSET + super::nameset::MAXIMUM_BYTES;

const _: () = assert!(PAGE_USED_BYTES <= super::PAGE_BYTES);

/// **Write a caretaker's grant page**: `name` at offset 0 as a set of one name, flagged
/// [`super::nameset::IS_DIRECTORY`] when `is_directory`, and `set`, already encoded by
/// [`super::nameset::encode`], at [`PAGE_NAME_SET_OFFSET`]. An empty `name` is written as an empty
/// set, which is how a grant with nothing to descend by travels (the root of the shell's namespace,
/// or a channel already bound to its directory). An empty `set` is written as an empty set too.
///
/// `None`, with `out` partly written, when `name` is longer than [`MAXIMUM_NAME_BYTES`], `set` is
/// longer than [`super::nameset::MAXIMUM_BYTES`] or `out` is shorter than [`PAGE_USED_BYTES`].
/// Never a truncation, because a truncated name is a different file.
///
/// # Examples
///
/// `rm` is granted `notes`, and inside it one name too long for any two words.
///
/// ```
/// use file_system_contract::{grant, nameset};
///
/// let long = [b'x'; 200];
/// let mut set = [0u8; nameset::MAXIMUM_BYTES];
/// let n = nameset::encode(&[(&long[..], false)], &mut set).unwrap();
/// let mut page = [0u8; grant::PAGE_USED_BYTES];
/// grant::write_page(b"notes", true, &set[..n], &mut page).unwrap();
///
/// // What the caretaker reads back from its read-only page.
/// assert_eq!(grant::page_name(&page), Some(&b"notes"[..]));
/// assert!(nameset::contains(grant::page_name_set(&page), &long));
/// ```
///
/// Name: provisional.
pub fn write_page(name: &[u8], is_directory: bool, set: &[u8], out: &mut [u8]) -> Option<()> {
    if out.len() < PAGE_USED_BYTES
        || name.len() > MAXIMUM_NAME_BYTES
        || set.len() > super::nameset::MAXIMUM_BYTES
    {
        return None;
    }
    if name.is_empty() {
        out[0] = 0;
    } else {
        super::nameset::encode(&[(name, is_directory)], &mut out[..PAGE_NAME_SET_OFFSET])?;
    }
    let tail = &mut out[PAGE_NAME_SET_OFFSET..PAGE_USED_BYTES];
    if set.is_empty() {
        tail[0] = 0;
    } else {
        tail[..set.len()].copy_from_slice(set);
    }
    Some(())
}

/// **The granted name on a caretaker's grant page**: empty for a grant with nothing to descend by,
/// or `None` when offset 0 does not hold exactly one name or none.
///
/// Read strictly, not with [`super::nameset::iter`], which stops quietly at a record that runs
/// off its buffer. Here a quiet stop would read as the empty name, and an empty name is the whole
/// namespace the caretaker was meant to narrow. So a record that runs past [`MAXIMUM_NAME_BYTES`],
/// a second record, or a flags byte with a bit [`super::nameset::IS_DIRECTORY`] does not define is
/// `None`, and the caretaker refuses to start.
///
/// Name: provisional.
pub fn page_name(page: &[u8]) -> Option<&[u8]> {
    let len = *page.first()? as usize;
    if len == 0 {
        return Some(&[]);
    }
    if len > MAXIMUM_NAME_BYTES || *page.get(1)? & !super::nameset::IS_DIRECTORY != 0 {
        return None;
    }
    let name = page.get(2..2 + len)?;
    (*page.get(2 + len)? == 0).then_some(name)
}

/// **The name set on a caretaker's grant page**, in [`super::nameset`]'s encoding: the bytes from
/// [`PAGE_NAME_SET_OFFSET`] to [`PAGE_USED_BYTES`], or nothing when `page` is shorter. A subtree
/// caretaker's is empty.
///
/// Name: provisional.
pub fn page_name_set(page: &[u8]) -> &[u8] {
    page.get(PAGE_NAME_SET_OFFSET..PAGE_USED_BYTES)
        .unwrap_or(&[])
}

/// **Whether a caretaker's `START` words are the ones a grant page goes with**: both name words
/// zero and no length in the spec word, so the words carry only the rights. Restrictive first: a
/// caretaker refuses words that still carry a name, rather than reading one of two places.
///
/// Name: provisional.
pub const fn words_carry_no_name(name_lo: u64, name_hi: u64, spec: u64) -> bool {
    name_lo == 0 && name_hi == 0 && specification_len(spec) == 0
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

//! **nifefs**: a filesystem so simple it fits in a comment. Read-only, flat, fixed-size
//! everything: it exists to be parsed, as `crates/elf` does, and it is the boot archive. The disk
//! tool writes an image with [`write_image`] and the userspace filesystem server reads it with
//! [`Fs::parse`], so one definition of the format serves both. Pure logic, host-tested, no `std`.
//!
//! The directory is [`DIR_BLOCKS`] blocks, holding up to [`MAX_FILES`] entries. `start_block` is
//! absolute, so a reader never needs `DIR_BLOCKS` to find data; only the writer places it. A name
//! is at most [`NAME_LEN`] bytes and unterminated when full, so readers compare `NAME_LEN` bytes and
//! stop at the first NUL. See notes/nifefs.md.

//! ```text
//!   blocks 0..DIR_BLOCKS   the superblock and directory
//!     magic   "CRKR0002"   (8 bytes)
//!     count   u32 LE       how many files          <- HEADER_LEN = 12 bytes so far
//!     ...then `count` directory entries, each ENTRY_LEN = 40 bytes:
//!       name        NAME_LEN = 32 bytes, NUL-padded
//!       start_block u32 LE  where the file's data begins (an absolute block number)
//!       len         u32 LE  the file's length in bytes
//!   blocks DIR_BLOCKS..     file data, each file block-aligned
//! ```

//! # BUGS
//! - A name over [`NAME_LEN`] ([`Error::NameTooLong`]) or holding a NUL ([`Error::NameHasNul`]) is
//!   refused, never truncated: two names agreeing up to the cut would become one.
//! - A duplicate name is not an error, and the first wins; only the disk tool keeps names unique.
//! - [`MAX_FILES`] grows with the suite, not the system, so a resize lands on whoever merges.
//! - `a_short_image_is_refused_not_indexed`'s falsification takes tens of minutes and gigabytes of
//!   solver memory, the same shape as `package_archive`'s `a_short_file_is_refused`.
//! - The EL0 blk driver (`crates/virtio`) buffers block 0 only, so it sees the first
//!   [`ENTRIES_IN_FIRST_BLOCK`] files, and nothing in the format says so.
//! - No directories, writes or permissions; the read-write filesystem is `redoxfs_server/`.

//! Name: ratified 2026-08-01 as `crickerfs` (calef), renamed `nifefs` with the OS. Refused
//! `cricker_fs` (`procfs` is the shape of a filesystem name; nobody writes `proc_fs`).

#![no_std]

/// The block size, and the alignment of everything.
pub const BLOCK: usize = 512;

/// Superblock magic. Version in the last four bytes, so a format change is legible.
///
/// Bump it when a reader can tell. A wider entry is such a change: a reader striding the old width
/// would return the wrong file instead of [`Error::BadMagic`], and the bump also makes a stale image
/// left in `target/` fail loudly. A move of [`DIR_BLOCKS`] is not, since `start_block` is absolute.
/// See notes/nifefs.md.
pub const MAGIC: [u8; 8] = *b"CRKR0002";

/// How many blocks the superblock-and-directory occupies. File data starts after it.
///
/// 14, which puts [`MAX_FILES`] at 178: fifty entries clear of the x86_64 kernel-suite archive,
/// because lanes that cannot see each other cross this ceiling. A raise needs no magic bump. Its
/// one cost besides image bytes is `a_short_image_is_refused_not_indexed`, which proves over a
/// symbolic array of `DIR_BLOCKS * BLOCK - 1` bytes; watch that harness's own time on a raise. See
/// notes/nifefs.md.
pub const DIR_BLOCKS: usize = 14;

/// The magic plus the count, before the first entry.
pub const HEADER_LEN: usize = 12;

/// The longest archive name, in bytes. A name that uses all of them has no NUL terminator.
///
/// 32, which clears the longest settled program name (`os_primitives_benchmarker`, 25) by 7 bytes.
/// Not larger, because every extra byte is a byte off every directory entry in every image, and a
/// raise costs no data migration: every image regenerates from this crate. See notes/nifefs.md.
pub const NAME_LEN: usize = 32;

/// One directory entry: the name, then `start_block` and `len` as `u32`s.
pub const ENTRY_LEN: usize = NAME_LEN + 8;

/// How many entries fit **entirely inside block 0**, which is the bound for a reader that has only
/// one block buffered. 12 at `NAME_LEN = 32`.
///
/// Entries pack from [`HEADER_LEN`] with no per-block padding, so one may straddle a block
/// boundary. The EL0 blk driver walks the directory inside a 512-byte DMA buffer, and without this
/// bound it would read the entry after the last complete one out of whatever follows the buffer.
pub const ENTRIES_IN_FIRST_BLOCK: usize = (BLOCK - HEADER_LEN) / ENTRY_LEN;

/// The most files an archive can hold: the directory blocks, past the header, in whole entries.
/// **178 at `DIR_BLOCKS = 14` and `ENTRY_LEN = 40`.** It costs image bytes and no kernel stack,
/// because [`Fs`] holds no entry array.
pub const MAX_FILES: usize = (DIR_BLOCKS * BLOCK - HEADER_LEN) / ENTRY_LEN;

/// Why [`Fs::parse`] refused an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The first eight bytes are not the archive magic.
    BadMagic,
    /// The directory claims more entries than [`MAX_FILES`] allows.
    TooManyFiles,
    /// A name longer than [`NAME_LEN`] bytes. Refused rather than truncated: two names agreeing in
    /// their first `NAME_LEN` bytes would otherwise become one entry, and the loader would fetch
    /// whichever program was packed first. See the crate's BUGS section.
    NameTooLong,
    /// A name containing a NUL byte, which the format cannot represent.
    ///
    /// Names are NUL-padded and every reader stops at the first NUL, so `"a\0b"` would read back as
    /// `"a"` and `read("a\0b")` would find nothing: the same collision as [`Error::NameTooLong`],
    /// refused for the same reason. `fuzz/fuzz_targets/nifefs_roundtrip` holds the property.
    NameHasNul,
    /// A file's data runs past the end of the image.
    OutOfBounds,
    /// The image is smaller than the directory span (`DIR_BLOCKS` blocks).
    Truncated,
}

/// One file: a name, and where its bytes are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    /// NUL-padded on disk; use [`name_str`](Self::name_str) or [`name_eq`](Self::name_eq) rather
    /// than reading this directly.
    pub name: [u8; NAME_LEN],
    /// The block where this file's data begins.
    pub start_block: u32,
    /// The file's length in bytes.
    pub len: u32,
}

impl Entry {
    /// The name as a `&str` up to the first NUL, if it is valid UTF-8.
    pub fn name_str(&self) -> Option<&str> {
        let end = self.name.iter().position(|&b| b == 0).unwrap_or(NAME_LEN);
        core::str::from_utf8(&self.name[..end]).ok()
    }

    /// Whether this entry's name is exactly `name`.
    pub fn name_eq(&self, name: &str) -> bool {
        self.name_str() == Some(name)
    }
}

/// A parsed superblock. Borrows the whole image so file lookups can return slices into it.
///
/// Holds no entry array: entries decode from the borrowed image on demand, so [`MAX_FILES`] costs
/// no kernel stack, where `Fs` is a local on the boot and spawn paths. `fs_does_not_grow_with_max_files`
/// keeps it that way. See notes/nifefs.md.
pub struct Fs<'a> {
    image: &'a [u8],
    count: usize,
}

impl<'a> Fs<'a> {
    /// Validate the whole directory up front: magic, entry count, and every entry's data bounds.
    /// A returned `Fs` has nothing left to check; decoding an entry and every lookup built on it
    /// trust this pass completely.
    pub fn parse(image: &'a [u8]) -> Result<Self, Error> {
        // The directory blocks must be present before any entry offset (up to HEADER_LEN +
        // MAX_FILES*ENTRY_LEN, which sits inside DIR_BLOCKS) is read; this guard is what makes
        // those reads sound.
        if image.len() < DIR_BLOCKS * BLOCK {
            return Err(Error::Truncated);
        }
        if image[0..8] != MAGIC {
            return Err(Error::BadMagic);
        }

        let count = u32le(image, 8) as usize;
        if count > MAX_FILES {
            return Err(Error::TooManyFiles);
        }

        // Validate every entry now, so a server rejects a corrupt image once rather than mid-request.
        for i in 0..count {
            let e = Self::entry_at(image, i);
            Self::entry_bounds(image.len(), e.start_block, e.len)?;
        }

        Ok(Fs { image, count })
    }

    /// How many files the directory holds.
    pub fn len(&self) -> usize {
        self.count
    }

    /// Whether the directory holds no files.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Decode entry `i` out of the directory. Caller guarantees `i < count`, which every caller here
    /// does by construction; `parse` has already bounds-checked the directory span.
    fn entry_at(image: &[u8], i: usize) -> Entry {
        let off = HEADER_LEN + i * ENTRY_LEN;
        let mut name = [0u8; NAME_LEN];
        name.copy_from_slice(&image[off..off + NAME_LEN]);
        Entry {
            name,
            start_block: u32le(image, off + NAME_LEN),
            len: u32le(image, off + NAME_LEN + 4),
        }
    }

    /// The directory, decoded lazily. One `Entry` exists at a time rather than [`MAX_FILES`] of them.
    pub fn entries(&self) -> impl Iterator<Item = Entry> + '_ {
        (0..self.count).map(|i| Self::entry_at(self.image, i))
    }

    /// Where entry `(start_block, len)` lies in an image of `image_len` bytes, as `start..end`,
    /// or [`Error::OutOfBounds`] if it does not fit.
    ///
    /// [`Fs::parse`] validates every entry with this and [`Fs::read`] slices with it, so the check
    /// and the indexing cannot drift apart, and a model checker can quantify over it without a
    /// symbolic directory (as `elf`'s `check_segment_bounds` does). See notes/nifefs.md.
    ///
    /// The name is **provisional**: calef names functions (AGENTS.md, milestone 160).
    fn entry_bounds(image_len: usize, start_block: u32, len: u32) -> Result<(usize, usize), Error> {
        let start = start_block as usize * BLOCK;
        let end = start.checked_add(len as usize).ok_or(Error::OutOfBounds)?;
        if end > image_len {
            return Err(Error::OutOfBounds);
        }
        Ok((start, end))
    }

    /// The bytes of a file, by name.
    pub fn read(&self, name: &str) -> Option<&'a [u8]> {
        let e = self.entries().find(|e| e.name_eq(name))?;
        // Cannot fail: `parse` accepted this entry through this same function, and an `Fs`
        // exists only if `parse` returned. Stated as an `expect` rather than an `ok()?` on
        // purpose: a `None` here would hide the impossible case as a missing file.
        let (start, end) = Self::entry_bounds(self.image.len(), e.start_block, e.len)
            .expect("parse validated every entry with this same function");
        Some(&self.image[start..end])
    }
}

fn u32le(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// Write a nifefs image containing `files` (name, contents) into `out`, returning the number
/// of bytes written. `out` must be large enough; the disk tool sizes it from [`image_size`].
///
/// Not `no_std`-hostile, but only the disk-building tool calls it, on the host.

/// # Examples
///
/// Pack two files and read one back:
///
/// ```
/// # use nifefs::{Fs, image_size, write_image};
/// let files: [(&str, &[u8]); 2] = [("motd", b"welcome\n"), ("os_primitives_benchmarker", b"\x7fELF")];
/// let mut img = vec![0u8; image_size(&files)];
/// write_image(&files, &mut img).unwrap();
///
/// let fs = Fs::parse(&img).unwrap();
/// assert_eq!(fs.read("motd"), Some(&b"welcome\n"[..]));
/// assert_eq!(fs.len(), 2);
/// ```
pub fn write_image(files: &[(&str, &[u8])], out: &mut [u8]) -> Result<usize, Error> {
    if files.len() > MAX_FILES {
        return Err(Error::TooManyFiles);
    }
    // Every name is checked before a byte is written, so a rejected archive is not a half-written
    // one.
    for (name, _) in files {
        if name.len() > NAME_LEN {
            return Err(Error::NameTooLong);
        }
        // The padding is a NUL and every reader stops at the first one, so a name holding one would
        // be an entry `read` could never match.
        if name.as_bytes().contains(&0) {
            return Err(Error::NameHasNul);
        }
    }

    for b in out.iter_mut() {
        *b = 0;
    }
    out[0..8].copy_from_slice(&MAGIC);
    out[8..HEADER_LEN].copy_from_slice(&(files.len() as u32).to_le_bytes());

    let mut block = DIR_BLOCKS as u32; // data starts after the superblock and directory
    for (i, (name, data)) in files.iter().enumerate() {
        let off = HEADER_LEN + i * ENTRY_LEN;
        let n = name.len(); // checked against NAME_LEN above
        out[off..off + n].copy_from_slice(&name.as_bytes()[..n]);
        out[off + NAME_LEN..off + NAME_LEN + 4].copy_from_slice(&block.to_le_bytes());
        out[off + NAME_LEN + 4..off + NAME_LEN + 8]
            .copy_from_slice(&(data.len() as u32).to_le_bytes());

        let start = block as usize * BLOCK;
        let end = start + data.len();
        if end > out.len() {
            return Err(Error::OutOfBounds);
        }
        out[start..end].copy_from_slice(data);

        let blocks = data.len().div_ceil(BLOCK).max(1) as u32;
        block += blocks;
    }

    Ok(block as usize * BLOCK)
}

/// How many bytes an image holding `files` needs.
pub fn image_size(files: &[(&str, &[u8])]) -> usize {
    let mut blocks = DIR_BLOCKS;
    for (_, data) in files {
        blocks += data.len().div_ceil(BLOCK).max(1);
    }
    blocks * BLOCK
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;
    use std::vec;

    #[test]
    fn write_then_read_round_trips() {
        let files: [(&str, &[u8]); 2] = [("motd", b"welcome to nife\n"), ("empty", b"")];
        let mut img = vec![0u8; image_size(&files)];
        let n = write_image(&files, &mut img).unwrap();
        assert_eq!(n, img.len());

        let fs = Fs::parse(&img).unwrap();
        assert_eq!(fs.len(), 2);
        assert_eq!(fs.read("motd"), Some(&b"welcome to nife\n"[..]));
        assert_eq!(fs.read("empty"), Some(&b""[..]));
        assert_eq!(fs.read("nope"), None);
    }

    /// A file longer than one block pushes the next file to a later block.
    #[test]
    fn files_are_block_aligned() {
        let big = vec![0x41u8; 600]; // > 512, so 2 blocks
        let files: [(&str, &[u8]); 2] = [("big", &big), ("after", b"x")];
        let mut img = vec![0u8; image_size(&files)];
        write_image(&files, &mut img).unwrap();

        let fs = Fs::parse(&img).unwrap();
        let after = fs.entries().find(|e| e.name_eq("after")).unwrap();
        assert_eq!(
            after.start_block as usize,
            DIR_BLOCKS + 2,
            "big took the two blocks after the directory, after should follow"
        );
        assert_eq!(fs.read("big").unwrap().len(), 600);
    }

    /// **The two derived capacity constants are the values their docs claim.** Nothing else pins
    /// them: every other test compares an image against the same constant it was built from, so an
    /// operator slip in the formula moves both sides together and no round trip notices. The values
    /// are load-bearing outside this crate (the EL0 blk driver stops at `ENTRIES_IN_FIRST_BLOCK`;
    /// the initial ramdisk build fails at `MAX_FILES`), so a wrong one is a wrong bound in another
    /// program. A deliberate resize of `DIR_BLOCKS` or `ENTRY_LEN` updates this test in the same
    /// commit. The values: `(512 - 12) / 40 = 12` and `(7168 - 12) / 40 = 178`.
    #[test]
    fn the_capacity_constants_are_the_documented_values() {
        assert_eq!(ENTRIES_IN_FIRST_BLOCK, 12);
        assert_eq!(MAX_FILES, 178);
    }

    /// A full-directory-sized image, so the truncation guard passes and the magic check refuses it.
    #[test]
    fn bad_magic_is_refused() {
        let img = vec![0u8; DIR_BLOCKS * BLOCK];
        assert_eq!(Fs::parse(&img).err(), Some(Error::BadMagic));
    }

    #[test]
    fn a_truncated_image_is_refused() {
        assert_eq!(Fs::parse(&[0u8; 10]).err(), Some(Error::Truncated));
    }

    /// **One byte under the directory span is `Truncated`, at the exact boundary.** The guard must
    /// compare against the full `DIR_BLOCKS * BLOCK` = 7168 bytes; the 10-byte test above is under
    /// every plausible mis-computation of that span (a `+` slip gives 526), so only a
    /// just-under-the-line image proves the multiplication. A guard that let 7167 bytes through
    /// would report `BadMagic` here instead, and would let entry reads run off a short image.
    #[test]
    fn an_image_one_byte_under_the_directory_span_is_truncated() {
        let img = vec![0u8; DIR_BLOCKS * BLOCK - 1];
        assert_eq!(Fs::parse(&img).err(), Some(Error::Truncated));
    }

    /// **A file whose last byte is the image's last byte is legal, in the writer and the reader.**
    /// Both bounds checks are `end > len`, and every other test ends its data short of a block
    /// boundary, so the accept side of the exact-fit case was untested in both directions: a check
    /// hardened to `>=` would refuse a valid archive whose final file is block-sized. One block of
    /// data after a 14-block directory ends at byte 7680 == image length exactly.
    #[test]
    fn a_file_ending_exactly_at_the_image_end_round_trips() {
        let body = vec![0x5au8; BLOCK];
        let files: [(&str, &[u8]); 1] = [("exact", &body)];
        let mut img = vec![0u8; image_size(&files)];
        let n = write_image(&files, &mut img).unwrap();
        assert_eq!(n, img.len());
        let fs = Fs::parse(&img).unwrap();
        assert_eq!(fs.read("exact"), Some(&body[..]));
    }

    /// `is_empty` had no caller in the tests at all, so all three of its mutations (constant true,
    /// constant false, inverted comparison) were invisible. Both sides pin it to `len() == 0`.
    #[test]
    fn an_empty_archive_is_empty_and_a_packed_one_is_not() {
        let none: [(&str, &[u8]); 0] = [];
        let mut img = vec![0u8; image_size(&none)];
        write_image(&none, &mut img).unwrap();
        let fs = Fs::parse(&img).unwrap();
        assert!(fs.is_empty());
        assert_eq!(fs.len(), 0);

        let files: [(&str, &[u8]); 1] = [("motd", b"hi")];
        let mut img = vec![0u8; image_size(&files)];
        write_image(&files, &mut img).unwrap();
        assert!(!Fs::parse(&img).unwrap().is_empty());
    }

    /// One entry whose `start_block` (100) lies far past the image.
    #[test]
    fn a_file_pointing_past_the_end_is_refused() {
        let mut img = vec![0u8; DIR_BLOCKS * BLOCK];
        img[0..8].copy_from_slice(&MAGIC);
        img[8..12].copy_from_slice(&1u32.to_le_bytes());
        let off = HEADER_LEN;
        img[off + NAME_LEN..off + NAME_LEN + 4].copy_from_slice(&100u32.to_le_bytes());
        img[off + NAME_LEN + 4..off + NAME_LEN + 8].copy_from_slice(&1u32.to_le_bytes());
        assert_eq!(Fs::parse(&img).err(), Some(Error::OutOfBounds));
    }

    /// **A full directory is an archive, not an error**: exactly [`MAX_FILES`] entries write,
    /// parse, and read back by name, the last one included. With `too_many_files_is_refused` below
    /// it pins the ceiling from both sides, so a `DIR_BLOCKS` change that miscounts fails here.
    #[test]
    fn exactly_max_files_is_written_and_every_entry_reads_back() {
        let names: vec::Vec<std::string::String> =
            (0..MAX_FILES).map(|i| std::format!("f{i}")).collect();
        let files: vec::Vec<(&str, &[u8])> =
            names.iter().map(|n| (n.as_str(), n.as_bytes())).collect();
        let mut img = vec![0u8; image_size(&files)];
        write_image(&files, &mut img).unwrap();
        let fs = Fs::parse(&img).unwrap();
        assert_eq!(fs.len(), MAX_FILES);
        for n in &names {
            assert_eq!(fs.read(n), Some(n.as_bytes()), "{n} did not read back");
        }
    }

    #[test]
    fn too_many_files_is_refused() {
        let data: &[u8] = b"x";
        let files: vec::Vec<(&str, &[u8])> = (0..MAX_FILES + 1).map(|_| ("f", data)).collect();
        let mut img = vec![0u8; image_size(&files) + BLOCK];
        assert_eq!(
            write_image(&files, &mut img).err(),
            Some(Error::TooManyFiles)
        );
    }

    /// **The longest settled program name fits, and one byte more is refused.** The limit exists to
    /// bound a name, and a name is what the reader will actually collide with, so the test names the
    /// real one: `os_primitives_benchmarker` (25 bytes) is what forced the raise from 24. A name
    /// using every byte has no NUL terminator, the case `name_str` must handle, and one byte over
    /// is an error rather than a truncation that would merge two names.
    #[test]
    fn a_name_is_bounded_by_name_len_and_a_longer_one_is_refused() {
        let data: &[u8] = b"y";
        let longest = "os_primitives_benchmarker";
        assert!(longest.len() <= NAME_LEN, "the name that forced the raise");

        let full = "x".repeat(NAME_LEN);
        let files: [(&str, &[u8]); 2] = [(longest, data), (full.as_str(), data)];
        let mut img = vec![0u8; image_size(&files)];
        write_image(&files, &mut img).unwrap();
        let fs = Fs::parse(&img).unwrap();
        assert_eq!(fs.read(longest), Some(data));
        assert_eq!(fs.read(&full), Some(data));

        let over = "x".repeat(NAME_LEN + 1);
        let files: [(&str, &[u8]); 1] = [(over.as_str(), data)];
        let mut img = vec![0u8; image_size(&files)];
        assert_eq!(
            write_image(&files, &mut img).err(),
            Some(Error::NameTooLong)
        );
    }

    /// **A name with a NUL in it is refused, not stored and then unfindable**, the regression for
    /// what `fuzz/fuzz_targets/nifefs_roundtrip` found (notes/nifefs.md). The empty name stays
    /// legal: it is representable (an all-NUL name field) and reads back as itself.
    #[test]
    fn a_name_with_a_nul_in_it_is_refused() {
        let data: &[u8] = b"z";
        for name in ["\0", "a\0b", "init\0", "\0\0"] {
            let files: [(&str, &[u8]); 1] = [(name, data)];
            let mut img = vec![0u8; image_size(&files)];
            assert_eq!(
                write_image(&files, &mut img).err(),
                Some(Error::NameHasNul),
                "{name:?} cannot be stored, so it must not be accepted"
            );
        }

        let files: [(&str, &[u8]); 1] = [("", data)];
        let mut img = vec![0u8; image_size(&files)];
        write_image(&files, &mut img).unwrap();
        assert_eq!(Fs::parse(&img).unwrap().read(""), Some(data));
    }

    /// **`MAX_FILES` costs no kernel stack, and this is what keeps it that way.** `Fs` is a stack
    /// local on the kernel's boot and spawn paths; when it held a `[Entry; MAX_FILES]`, raising the
    /// limit overflowed a 4-page kernel stack the same day. Reintroducing the array would make this
    /// fail rather than making a boot fault on a guard page.
    #[test]
    fn fs_does_not_grow_with_max_files() {
        assert_eq!(
            core::mem::size_of::<Fs>(),
            core::mem::size_of::<&[u8]>() + core::mem::size_of::<usize>(),
            "Fs is a borrowed image and a count, nothing per-entry"
        );
        assert_eq!(core::mem::size_of::<Entry>(), ENTRY_LEN);
    }

    /// Exactly [`MAX_FILES`] files write and parse back, so the directory's last entry (offset
    /// `12 + (MAX_FILES - 1) * 40`) is honored, not silently past the header's reach.
    #[test]
    fn a_full_directory_round_trips() {
        let data: &[u8] = b"z";
        let names: vec::Vec<std::string::String> =
            (0..MAX_FILES).map(|i| std::format!("f{i}")).collect();
        let files: vec::Vec<(&str, &[u8])> = names.iter().map(|n| (n.as_str(), data)).collect();
        let mut img = vec![0u8; image_size(&files)];
        write_image(&files, &mut img).unwrap();
        let fs = Fs::parse(&img).unwrap();
        assert_eq!(fs.len(), MAX_FILES);
        assert_eq!(fs.read(&names[MAX_FILES - 1]), Some(data));
    }

    /// **A directory that spills past one block round-trips, every file with distinct contents.**
    /// `a_full_directory_round_trips` above proves the capacity boundary but writes identical bytes
    /// to every file, so a directory that mixed up `start_block`s would still read the same byte back
    /// and pass. This one gives every file its own contents and name and checks each reads back
    /// byte-for-byte, the integrity guard for the multi-block directory. Twenty files, each longer
    /// than a name, so a shifted or truncated directory could not accidentally match.
    #[test]
    fn a_directory_spanning_two_blocks_round_trips() {
        let bodies: vec::Vec<vec::Vec<u8>> = (0..20u8)
            .map(|i| vec![i; (BLOCK + i as usize) % (2 * BLOCK) + 1])
            .collect();
        let names: vec::Vec<vec::Vec<u8>> = (0..20u8)
            .map(|i| std::format!("file{i}").into_bytes())
            .collect();
        let files: vec::Vec<(&str, &[u8])> = (0..20)
            .map(|i| {
                (
                    core::str::from_utf8(&names[i]).unwrap(),
                    bodies[i].as_slice(),
                )
            })
            .collect();
        assert!(
            files.len() > ENTRIES_IN_FIRST_BLOCK,
            "test must exceed one block"
        );

        let mut img = vec![0u8; image_size(&files)];
        let n = write_image(&files, &mut img).expect("write");
        assert_eq!(n, img.len());

        let fs = Fs::parse(&img).expect("parse");
        assert_eq!(fs.len(), 20);
        for (i, body) in bodies.iter().enumerate() {
            let name = core::str::from_utf8(&names[i]).unwrap();
            assert_eq!(
                fs.read(name),
                Some(body.as_slice()),
                "file {name} mismatched"
            );
        }
    }
}

/// Machine-checked proofs (`script/verify`; notes/verification.md).
///
/// The kernel parses the initial ramdisk (`kernel::user::program`), so this parser is in the TCB
/// on externally supplied bytes. A whole-parse proof did not converge, so two harnesses carry the
/// weight: the validation-implies-safe-read arithmetic for every entry value and image length, and
/// the truncation guard for every image under the directory span. Whole-parse totality and
/// `name_str` are deliberately not proved; notes/nifefs.md says why.
#[cfg(kani)]
mod verification {
    use super::*;

    /// **Parse's validation is sufficient for `read`: proved on the function both of them call.**
    /// For ANY `start_block`, `len`, and image length, an accepted entry yields
    /// `start <= end <= image_len`, so `read`'s indexing cannot panic and the bytes it returns
    /// lie inside the image. It calls [`Fs::entry_bounds`] rather than restating it, because a
    /// proof of a copy is a proof of neither caller (milestone 213). The two equalities are the
    /// format, not the implementation: an entry names an absolute block of [`BLOCK`] bytes. See
    /// notes/nifefs.md.
    /// Falsification: replayable `crates/nifefs/falsifications/verification.the_validation_implies_reads_slice_is_in_bounds.patch`
    #[kani::proof]
    fn the_validation_implies_reads_slice_is_in_bounds() {
        let start_block: u32 = kani::any();
        let len: u32 = kani::any();
        let image_len: usize = kani::any();

        let Ok((start, end)) = Fs::entry_bounds(image_len, start_block, len) else {
            return; // refused, and `parse` refuses exactly here
        };
        assert!(start <= end);
        assert!(end <= image_len, "read could index past the image");
        assert_eq!(
            start,
            start_block as usize * BLOCK,
            "an entry begins at its own block"
        );
        assert_eq!(
            end - start,
            len as usize,
            "an entry spans exactly its own length"
        );
    }

    /// **A short image is always `Truncated`, never indexed**: for any image under the directory
    /// span, parse refuses before touching a byte past the length check (which is what keeps the
    /// entry reads, up to offset [`HEADER_LEN`] + [`MAX_FILES`] * [`ENTRY_LEN`] inside
    /// [`DIR_BLOCKS`], in bounds).
    ///
    /// `#[kani::unwind(9)]` lets a weakened guard reach a verdict instead of unrolling the entry
    /// loop without end, and removes no path on the correct tree. 9 is the least bound that lets
    /// the 8-byte magic compare finish. Measurements are in notes/nifefs.md.

    /// # BUGS
    /// - One record per harness (§134 (a harness carries a machine-replayable falsification
    ///   record)), so this weakened guard replaced the magic-before-length reordering #1217
    ///   recorded, which still goes red under this bound in 1.1 s and is the cheaper replay.
    /// Falsification: replayable `crates/nifefs/falsifications/verification.a_short_image_is_refused_not_indexed.patch`
    #[kani::proof]
    #[kani::unwind(9)]
    fn a_short_image_is_refused_not_indexed() {
        const SHORT: usize = DIR_BLOCKS * BLOCK - 1;
        let image: [u8; SHORT] = kani::any();
        assert_eq!(Fs::parse(&image).err(), Some(Error::Truncated));
    }
}

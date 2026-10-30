//! **`activation_store`**: the composed image's first installed-programs table, carried as one
//! small fixed-shape file beside the initial ramdisk (milestone 909 (basalt composes the bootable
//! image from its packages), item 3). The container holds the generations and the `current`
//! pointer and nothing else; a generation's body is byte for byte what
//! [`installed_programs`] writes for a generation, so the table's grammar, its writers and its
//! readers are unchanged and this crate is only the wrapping.
//!
//! It exists because a composed image carries no writable filesystem: the table the installer
//! keeps under `activation/` on the machine's disk has no home in an image that boots with no
//! disk. The container is the home. **Whether the installer's later writes unify with it is
//! follow-on work, and nothing here assumes it**: the reader and writer below are the functions a
//! unified writer would call, and where a store lives on a machine's own filesystem, how it grows
//! there and what renames over what, is that work's decision, not this crate's.
//!
//! # The byte layout
//!
//! ```text
//!   bytes 0..8     magic     b"ACTSTORE"
//!         8..12    version   u32 LE, 1
//!        12..16    count     u32 LE, generations held, 1 to MAXIMUM_GENERATIONS
//!        16..20    current   u32 LE, the live generation's number, 1 to count
//!        20..24    reserved  u32 LE, 0
//!        24..      count descriptors, each 8 bytes: offset u32 LE (absolute), len u32 LE
//!        then the generation bodies, sequential, each at an 8-byte-aligned offset
//! ```
//!
//! Offsets are absolute, as `nifefs`' and `package_archive`'s are, so a reader never has to know
//! where the descriptor table ends to find data. The layout has exactly one shape: descriptor 0's
//! offset is the descriptor table's end, each body begins where the last one ended rounded up to
//! the alignment, and the file ends at the last body's end. A reader refuses anything else, so the
//! container's bytes are the writer's output or they are nothing.
//!
//! # The write is whole, and the flip is a rename
//!
//! A generation is immutable once written: the container has no edit. A new generation is a new
//! file written whole and renamed over the old one, which is also what moves `current` atomically:
//! a reader meets the old store or the new one, never a half-written mixture. A torn write leaves
//! a file whose length is not the layout's end, and [`Store::parse`] refuses it whole, so
//! `current` can never name a half-written generation. This is the commit order
//! notes/packages.md records for the installer's generations ("the new generation is created and
//! written whole, `current` is written to `current.next` and renamed over `current`"), applied to
//! the container as a whole; the rename is the caller's, since this crate does no input or output.
//!
//! # EXAMPLES
//!
//! ```
//! use activation_store::{Store, write, write_size};
//!
//! // One generation: the text `installed_programs` writes, wrapped.
//! let first = "sha256:0000000000000000000000000000000000000000000000000000000000000000 \
//!              progenitor 0.1.0 init owner\n";
//! let mut out = vec![0u8; write_size(&[first.len()])];
//! let n = write(&[first.as_bytes()], 1, &mut out).unwrap();
//!
//! let store = Store::parse(&out[..n]).unwrap();
//! assert_eq!(store.count(), 1);
//! assert_eq!(store.current(), 1);
//! assert_eq!(store.live(), first.as_bytes());
//! ```
//!
//! # BUGS
//!
//! - The container is written whole; a store that grows on a running system is the caller's
//!   discipline, and no gate proves the rename on the target, because nothing on the target writes
//!   a store yet.
//! - No signature over the container. The seed's measurement is the kernel's trust root
//!   (`measured_boot::ACTIVATION_STORE`); signing the store itself is milestone 917 (basalt's
//!   packages build reproducibly) and 607's item 6, as it is for every boot artifact.
//! - A `default` pointer naming a digest no row holds passes [`Store::parse`], as it passes the
//!   table's own line reader; the table's lookups refuse it, and no writer this tree owns writes
//!   one.
//!
//! Name: provisional, minted 2026-10-29 (UTC) by lane
//! milestone/909-item-3-composition-checks-the-approval, for the store the architect's ruling of
//! that day describes as "a nife-native activation store". calef names crates. Considered
//! `seed_store` (names the composed image's first use, not the container), `generations` (says
//! nothing about what the generations are of) and `activation` (one word short of what it holds:
//! the generations and the pointer, not the directory the installer keeps).

#![no_std]

use installed_programs::Error as TableError;

/// The container's magic. The version is the field after it, not the magic's last bytes, so a
/// shape change and a spelling change are separately legible.
///
/// Name: provisional, minted 2026-10-29 (UTC) by lane
/// milestone/909-item-3-composition-checks-the-approval. calef names constants.
pub const MAGIC: [u8; 8] = *b"ACTSTORE";

/// The layout this crate reads and writes. A reader refuses any other, so a container's bytes
/// name their layout by carrying it.
pub const VERSION: u32 = 1;

/// The magic, the version, the count, `current` and the reserved word, before the descriptors.
pub const HEADER_LEN: usize = 24;

/// One descriptor: a body's absolute offset and its length, as `u32`s.
pub const DESCRIPTOR_LEN: usize = 8;

/// The bodies' alignment, the one `package_archive` uses and for its reason: the cheapest way to
/// be wrong later is to hand a parser an odd address.
pub const BODY_ALIGN: usize = 8;

/// The most generations a container holds. An install is one generation, and nothing collects old
/// ones yet ([`installed_programs`]' BUGS), so the bound is generous rather than tight; a reader
/// refuses a `count` past it before it reads anything else, which is what keeps a hostile count
/// from naming a descriptor table no file carries.
///
/// Name: provisional, minted 2026-10-29 (UTC) by lane
/// milestone/909-item-3-composition-checks-the-approval. calef names constants.
pub const MAXIMUM_GENERATIONS: usize = 1024;

/// Why a container was refused, or a write could not produce one.
///
/// Name: provisional, minted 2026-10-29 (UTC) by lane
/// milestone/909-item-3-composition-checks-the-approval. calef names public items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The first eight bytes are not [`MAGIC`].
    BadMagic,
    /// The version is not [`VERSION`].
    UnknownVersion,
    /// The reserved word is not zero, which no writer this tree owns leaves.
    Reserved,
    /// The container holds no generation. A store always has a live one to name.
    NoGenerations,
    /// The count is past [`MAXIMUM_GENERATIONS`].
    TooManyGenerations,
    /// `current` is zero or names a generation the container does not hold. The pointer names one
    /// whole generation or the container is refused; it never names nothing.
    BadCurrent,
    /// The file is shorter than its own header and descriptor table.
    Truncated,
    /// A body runs past the end of the file.
    OutOfBounds,
    /// The bodies are not sequential and 8-aligned from the descriptor table's end, or the file
    /// does not end at the last body's end. The layout has one shape, and this is not it.
    BadLayout,
    /// A generation's body is not a table [`installed_programs`] reads.
    NotATable,
    /// `write`'s output buffer is not exactly [`write_size`] bytes.
    TooSmall,
}

impl From<TableError> for Error {
    fn from(_: TableError) -> Self {
        Error::NotATable
    }
}

/// **A parsed container**: the generations it holds, borrowed from the caller's bytes, and which
/// one is live. [`Store::parse`] checked every field and every body; nothing here re-checks.
///
/// Name: provisional, minted 2026-10-29 (UTC) by lane
/// milestone/909-item-3-composition-checks-the-approval. calef names public items.
pub struct Store<'a> {
    image: &'a [u8],
    count: usize,
    current: u32,
}

impl<'a> Store<'a> {
    /// The container `image` spells, or the refusal. The whole container is validated before any
    /// of it is trusted, so a refused container has named no generation and vouched for nothing.
    ///
    /// Name: provisional, minted 2026-10-29 (UTC) by lane
    /// milestone/909-item-3-composition-checks-the-approval. calef names public items.
    pub fn parse(image: &'a [u8]) -> Result<Self, Error> {
        if image.len() < 8 || image[0..8] != MAGIC {
            return Err(Error::BadMagic);
        }
        if image.len() < HEADER_LEN {
            return Err(Error::Truncated);
        }
        if u32le(image, 8) != VERSION {
            return Err(Error::UnknownVersion);
        }
        if u32le(image, 20) != 0 {
            return Err(Error::Reserved);
        }
        let count = u32le(image, 12) as usize;
        if count == 0 {
            return Err(Error::NoGenerations);
        }
        if count > MAXIMUM_GENERATIONS {
            return Err(Error::TooManyGenerations);
        }
        let descriptors_end = HEADER_LEN + count * DESCRIPTOR_LEN;
        if image.len() < descriptors_end {
            return Err(Error::Truncated);
        }
        let current = u32le(image, 16);
        if current == 0 || current as usize > count {
            return Err(Error::BadCurrent);
        }
        check_layout(image, count)?;
        // Every generation's body is a table, in the one grammar this tree owns. A container that
        // wraps anything else is refused whole: half a table would vouch for whatever survived.
        for number in 1..=count as u32 {
            validate_body(body_text(image, number, count)?)?;
        }
        Ok(Store {
            image,
            count,
            current,
        })
    }

    /// How many generations the container holds.
    ///
    /// Name: provisional, minted 2026-10-29 (UTC) by lane
    /// milestone/909-item-3-composition-checks-the-approval. calef names public items.
    pub fn count(&self) -> usize {
        self.count
    }

    /// The live generation's number, one-based, as `installed_programs::parse_current` reads the
    /// installer's `current` file.
    ///
    /// Name: provisional, minted 2026-10-29 (UTC) by lane
    /// milestone/909-item-3-composition-checks-the-approval. calef names public items.
    pub fn current(&self) -> u32 {
        self.current
    }

    /// Generation `number`'s bytes, the table text exactly as it was written, or `None` when the
    /// container does not hold it.
    ///
    /// Name: provisional, minted 2026-10-29 (UTC) by lane
    /// milestone/909-item-3-composition-checks-the-approval. calef names public items.
    pub fn generation(&self, number: u32) -> Option<&'a [u8]> {
        if number == 0 || number as usize > self.count {
            return None;
        }
        // Parse checked the layout and every descriptor's bounds; this cannot refuse now.
        body_text(self.image, number, self.count).ok()
    }

    /// The live generation's bytes.
    ///
    /// Name: provisional, minted 2026-10-29 (UTC) by lane
    /// milestone/909-item-3-composition-checks-the-approval. calef names public items.
    pub fn live(&self) -> &'a [u8] {
        // Parse checked `current` against `count`; this cannot refuse.
        self.generation(self.current).unwrap_or(&[])
    }
}

/// The layout's one check, split from [`Store::parse`]: the bodies are sequential and 8-aligned
/// from the descriptor table's end, each descriptor's body lies inside the file, and the file ends
/// exactly at the last body's end.
fn check_layout(image: &[u8], count: usize) -> Result<(), Error> {
    let descriptors_end = HEADER_LEN + count * DESCRIPTOR_LEN;
    // `at` is where the next body must begin, aligned; `end_of_last` is the previous body's end,
    // which is the file's end once the loop has run out of bodies.
    let mut at = descriptors_end;
    let mut end_of_last = descriptors_end;
    for k in 0..count {
        let descriptor = HEADER_LEN + k * DESCRIPTOR_LEN;
        let offset = u32le(image, descriptor) as usize;
        let len = u32le(image, descriptor + 4) as usize;
        if offset != at {
            return Err(Error::BadLayout);
        }
        let end = offset.checked_add(len).ok_or(Error::OutOfBounds)?;
        if end > image.len() {
            return Err(Error::OutOfBounds);
        }
        end_of_last = end;
        at = end.next_multiple_of(BODY_ALIGN);
    }
    if image.len() != end_of_last {
        return Err(Error::BadLayout);
    }
    Ok(())
}

/// Body `number`'s bytes out of `image`, whose header holds `count` generations, or the refusal.
/// One-based, as `current` is.
fn body_text<'a>(image: &'a [u8], number: u32, count: usize) -> Result<&'a [u8], Error> {
    let k = (number as usize).checked_sub(1).ok_or(Error::BadCurrent)?;
    if k >= count {
        return Err(Error::BadCurrent);
    }
    let at = HEADER_LEN + k * DESCRIPTOR_LEN;
    let offset = u32le(image, at) as usize;
    let len = u32le(image, at + 4) as usize;
    let end = offset.checked_add(len).ok_or(Error::OutOfBounds)?;
    image.get(offset..end).ok_or(Error::OutOfBounds)
}

/// `b`'s little-endian `u32` at `at`, the one `nifefs` reads with.
fn u32le(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// How many bytes a container holding bodies of these lengths needs.
///
/// Name: provisional, minted 2026-10-29 (UTC) by lane
/// milestone/909-item-3-composition-checks-the-approval. calef names public items.
pub fn write_size(generation_lens: &[usize]) -> usize {
    let mut at = HEADER_LEN + generation_lens.len() * DESCRIPTOR_LEN;
    for len in generation_lens {
        at = at.next_multiple_of(BODY_ALIGN) + len;
    }
    at
}

/// **Write the container**: `generations` in order, `current` naming the live one, into `out`,
/// which must be exactly [`write_size`] bytes. Every body is checked to be a table before a byte
/// is written, and nothing is written on a refusal, so a refused write leaves no half-container.
/// Returns the number of bytes written, which is `out`'s length: the container fills it exactly.
///
/// Name: provisional, minted 2026-10-29 (UTC) by lane
/// milestone/909-item-3-composition-checks-the-approval. calef names public items.
pub fn write(generations: &[&[u8]], current: u32, out: &mut [u8]) -> Result<usize, Error> {
    if generations.is_empty() {
        return Err(Error::NoGenerations);
    }
    if generations.len() > MAXIMUM_GENERATIONS {
        return Err(Error::TooManyGenerations);
    }
    if current == 0 || current as usize > generations.len() {
        return Err(Error::BadCurrent);
    }
    let mut size = HEADER_LEN + generations.len() * DESCRIPTOR_LEN;
    for body in generations {
        size = size.next_multiple_of(BODY_ALIGN) + body.len();
    }
    if out.len() != size {
        return Err(Error::TooSmall);
    }
    for body in generations {
        validate_body(body)?;
    }
    let count = generations.len() as u32;
    out[0..8].copy_from_slice(&MAGIC);
    out[8..12].copy_from_slice(&VERSION.to_le_bytes());
    out[12..16].copy_from_slice(&count.to_le_bytes());
    out[16..20].copy_from_slice(&current.to_le_bytes());
    out[20..24].copy_from_slice(&0u32.to_le_bytes());
    // One walk writes each descriptor and then the body it names, so the descriptor's offset is
    // the running end and the layout the checker demands is the layout this produces.
    let mut body_at = HEADER_LEN + generations.len() * DESCRIPTOR_LEN;
    for (index, body) in generations.iter().enumerate() {
        body_at = body_at.next_multiple_of(BODY_ALIGN);
        let descriptor = HEADER_LEN + index * DESCRIPTOR_LEN;
        out[descriptor..descriptor + 4].copy_from_slice(&(body_at as u32).to_le_bytes());
        out[descriptor + 4..descriptor + 8].copy_from_slice(&(body.len() as u32).to_le_bytes());
        out[body_at..body_at + body.len()].copy_from_slice(body);
        body_at += body.len();
    }
    Ok(out.len())
}

/// Whether `body` is a table this tree's readers accept: every line well-formed, and the grant
/// lines sorted, each once, and naming a row. The checks are [`installed_programs`]' own readers,
/// the same ones that refuse a malformed table at spawn time; the store adds no second grammar.
fn validate_body(body: &[u8]) -> Result<(), Error> {
    let text = core::str::from_utf8(body).map_err(|_| Error::NotATable)?;
    for line in installed_programs::entries(text) {
        line?;
    }
    for grant in installed_programs::grants(text) {
        grant?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;

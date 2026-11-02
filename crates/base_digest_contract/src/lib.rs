//! **The base digest page**: the SHA-256 digest of the base set's measurement table, handed from a
//! kernel to its first process as a read-only page. The kernel holds the digest from its loader and
//! writes the page once, before the first process exists; the first process maps it, reads the
//! digest and checks the table against it before it reads a line of the table. This crate is the
//! page's one definition for the writer and the reader.
//!
//! The page carries no address. The reader maps it wherever it chooses, because a capability
//! names the page and no other program ever sees it.

//! # BUGS
//!
//! - The digest is SHA-256 alone. A second algorithm is a new [`VERSION`] and a new [`MAGIC`].
//! - [`read_page`] refuses an all-zero digest as a frame nobody wrote. A table whose real SHA-256
//!   is all zeroes would be refused too; finding one is a preimage attack on SHA-256.
//!
//! Name: provisional, minted 2026-11-02 (UTC) by milestone/695-part-1-base-digest-capability.
//! `base` is the loader's word for the same digest; `digest` says what the page carries;
//! `_contract` is the family suffix. Refused `base_set_page` (names the page's shape, not its
//! content) and `measurement_table_digest_contract` (accurate, and longer than the loader's word
//! for it).

#![no_std]

/// The page's first eight bytes: ASCII, unpadded, the shape the sibling pages' magics use. The last
/// byte is the layout's [`VERSION`] as a digit, so [`read_page`] can tell another layout of this
/// page from a page that is not this one.
///
/// Name: provisional, minted 2026-11-02 (UTC) by milestone/695-part-1-base-digest-capability. Its
/// value `BASEDIG1` is a wire value.
pub const MAGIC: [u8; 8] = *b"BASEDIG1";

/// The page layout's version, the digit [`MAGIC`] carries. A layout break bumps this and re-mints
/// the magic in one change.
///
/// Name: provisional, minted 2026-11-02 (UTC) by milestone/695-part-1-base-digest-capability. The
/// name every contract crate uses for its layout version.
pub const VERSION: u32 = 1;

/// This interface's identity in a program's `INTERFACES` note: 22, naming the base digest page.
/// The number is this interface's for good: no other interface takes it, and a format change is a
/// new version, never a new number. A program declares the pair of this id and [`VERSION`].
///
/// Name: provisional, minted 2026-11-02 (UTC) by milestone/695-part-1-base-digest-capability. The
/// name every contract crate uses for its interface identity.
pub const INTERFACE_ID: u64 = 22;

/// The length of a SHA-256 digest, in bytes.
///
/// Name: provisional, minted 2026-11-02 (UTC) by milestone/695-part-1-base-digest-capability.
pub const DIGEST_BYTES: usize = 32;

/// A SHA-256 digest: the 32 bytes of the hash, with no algorithm label.
///
/// Name: provisional, minted 2026-11-02 (UTC) by milestone/695-part-1-base-digest-capability.
pub type Digest = [u8; DIGEST_BYTES];

const OFF_MAGIC: usize = 0;
const OFF_DIGEST: usize = OFF_MAGIC + MAGIC.len();

/// **The size of the page's content** in bytes: the magic, then the digest. This is not a page
/// size. The page is one whole frame, and this is how much of it carries data; the rest is zeroes.
///
/// Name: provisional, minted 2026-11-02 (UTC) by milestone/695-part-1-base-digest-capability. The
/// page-crate family's name for the content length (design/naming/crates.md).
pub const PAGE_CONTENT_BYTES: usize = OFF_DIGEST + DIGEST_BYTES;

/// **Build a base digest page's content** for `digest`. The writer copies the result into a zeroed
/// frame before any reader can map it; nothing else constructs one.
///
/// # Examples
///
/// ```
/// use base_digest_contract::{build_page, read_page};
///
/// let digest = [7u8; 32];
/// assert_eq!(read_page(&build_page(&digest)), Ok(digest));
/// ```
///
/// Name: provisional, minted 2026-11-02 (UTC) by milestone/695-part-1-base-digest-capability.
/// `counter_frequency_contract`'s name for the same function.
pub fn build_page(digest: &Digest) -> [u8; PAGE_CONTENT_BYTES] {
    let mut page = [0u8; PAGE_CONTENT_BYTES];
    page[OFF_MAGIC..OFF_DIGEST].copy_from_slice(&MAGIC);
    page[OFF_DIGEST..PAGE_CONTENT_BYTES].copy_from_slice(digest);
    page
}

/// **Why [`read_page`] refused a page.** Each refusal names a different fault, so a reader that
/// refuses its boot can say which one it met.
///
/// Name: provisional, minted 2026-11-02 (UTC) by milestone/695-part-1-base-digest-capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Fewer than [`PAGE_CONTENT_BYTES`] bytes: `length` is how many there were.
    ShortPage {
        /// How many bytes the reader handed over.
        length: usize,
    },
    /// The first seven bytes are not [`MAGIC`]'s: a zeroed frame, or a page that is not this one.
    NotThisPage,
    /// [`MAGIC`]'s first seven bytes with another version byte: a layout this reader does not know.
    UnsupportedVersion {
        /// The version byte the page carries, as written (an ASCII digit when the writer was this
        /// crate).
        byte: u8,
    },
    /// The magic and version are right and every digest byte is zero: a page whose digest nobody
    /// wrote. [`is_plausible`] is the one definition.
    ZeroDigest,
}

impl Refusal {
    /// One line that names the refusal, for a console. Fixed text, because the reader that prints
    /// it may have no formatter.
    ///
    /// # Examples
    ///
    /// ```
    /// use base_digest_contract::{Refusal, read_page};
    ///
    /// assert_eq!(read_page(&[0u8; 40]).unwrap_err().reason(), Refusal::NotThisPage.reason());
    /// ```
    ///
    /// Name: provisional, minted 2026-11-02 (UTC) by milestone/695-part-1-base-digest-capability.
    pub fn reason(&self) -> &'static str {
        match self {
            Refusal::ShortPage { .. } => "the base digest page is shorter than its content",
            Refusal::NotThisPage => "the base digest page does not carry its magic",
            Refusal::UnsupportedVersion { .. } => {
                "the base digest page carries a layout version this reader does not know"
            }
            Refusal::ZeroDigest => "the base digest page carries an all-zero digest",
        }
    }
}

/// **Is `digest` one a writer could have meant?** An all-zero digest is a frame nobody wrote, not
/// a measurement. The check exists to catch a page left blank, not to judge a hash.
///
/// # Examples
///
/// ```
/// use base_digest_contract::is_plausible;
///
/// assert!(is_plausible(&[1u8; 32]));
/// assert!(!is_plausible(&[0u8; 32]));
/// ```
///
/// Name: provisional, minted 2026-11-02 (UTC) by milestone/695-part-1-base-digest-capability.
/// `counter_frequency_contract`'s name for its band check.
pub fn is_plausible(digest: &Digest) -> bool {
    digest.iter().any(|&b| b != 0)
}

/// **Read the digest out of a base digest page's content**, or say why not. `bytes` is the page as
/// the reader mapped it; bytes past [`PAGE_CONTENT_BYTES`] are ignored, so a whole frame reads the
/// same as its content alone.
///
/// Refuses a short buffer, a page without the magic, another layout version and an all-zero
/// digest ([`Refusal`]), in that order.
///
/// # Examples
///
/// ```
/// use base_digest_contract::{Refusal, build_page, read_page};
///
/// let mut page = build_page(&[9u8; 32]);
/// page[7] = b'2';
/// assert_eq!(read_page(&page), Err(Refusal::UnsupportedVersion { byte: b'2' }));
/// ```
///
/// Name: provisional, minted 2026-11-02 (UTC) by milestone/695-part-1-base-digest-capability.
pub fn read_page(bytes: &[u8]) -> Result<Digest, Refusal> {
    let Some(content) = bytes.get(..PAGE_CONTENT_BYTES) else {
        return Err(Refusal::ShortPage {
            length: bytes.len(),
        });
    };
    let (magic, rest) = content.split_at(OFF_DIGEST);
    let version_at = MAGIC.len() - 1;
    if magic[..version_at] != MAGIC[..version_at] {
        return Err(Refusal::NotThisPage);
    }
    if magic[version_at] != MAGIC[version_at] {
        return Err(Refusal::UnsupportedVersion {
            byte: magic[version_at],
        });
    }
    let mut digest = [0u8; DIGEST_BYTES];
    digest.copy_from_slice(rest);
    if !is_plausible(&digest) {
        return Err(Refusal::ZeroDigest);
    }
    Ok(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A digest that is not all one byte, so a test that swapped two halves would see it.
    fn sample() -> Digest {
        core::array::from_fn(|i| i as u8 + 1)
    }

    #[test]
    fn a_built_page_round_trips_its_digest() {
        assert_eq!(read_page(&build_page(&sample())), Ok(sample()));
    }

    /// The reader maps a whole frame; the zeroes past the content change nothing.
    #[test]
    fn a_whole_frame_reads_as_its_content() {
        let mut frame = [0u8; 4096];
        frame[..PAGE_CONTENT_BYTES].copy_from_slice(&build_page(&sample()));
        assert_eq!(read_page(&frame), Ok(sample()));
    }

    #[test]
    fn a_short_buffer_is_refused_with_its_length() {
        let page = build_page(&sample());
        assert_eq!(
            read_page(&page[..PAGE_CONTENT_BYTES - 1]),
            Err(Refusal::ShortPage {
                length: PAGE_CONTENT_BYTES - 1
            })
        );
        assert_eq!(read_page(&[]), Err(Refusal::ShortPage { length: 0 }));
    }

    #[test]
    fn a_zeroed_frame_is_not_this_page() {
        assert_eq!(
            read_page(&[0u8; PAGE_CONTENT_BYTES]),
            Err(Refusal::NotThisPage)
        );
    }

    /// Every byte of the magic but the version digit is checked, the first and the last of them.
    #[test]
    fn any_wrong_magic_byte_is_not_this_page() {
        for at in 0..MAGIC.len() - 1 {
            let mut page = build_page(&sample());
            page[at] ^= 0x20;
            assert_eq!(read_page(&page), Err(Refusal::NotThisPage), "byte {at}");
        }
    }

    #[test]
    fn another_version_digit_is_an_unsupported_version() {
        let mut page = build_page(&sample());
        page[MAGIC.len() - 1] = b'2';
        assert_eq!(
            read_page(&page),
            Err(Refusal::UnsupportedVersion { byte: b'2' })
        );
    }

    #[test]
    fn an_all_zero_digest_is_refused() {
        assert_eq!(
            read_page(&build_page(&[0u8; DIGEST_BYTES])),
            Err(Refusal::ZeroDigest)
        );
    }

    /// One nonzero byte anywhere makes a digest plausible; the check is not a prefix test.
    #[test]
    fn one_nonzero_byte_anywhere_is_plausible() {
        for at in 0..DIGEST_BYTES {
            let mut digest = [0u8; DIGEST_BYTES];
            digest[at] = 1;
            assert!(is_plausible(&digest), "byte {at}");
        }
    }

    /// Each refusal says something different, so a console line tells them apart.
    #[test]
    fn every_refusal_has_its_own_reason() {
        let all = [
            Refusal::ShortPage { length: 0 },
            Refusal::NotThisPage,
            Refusal::UnsupportedVersion { byte: b'2' },
            Refusal::ZeroDigest,
        ];
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a.reason(), b.reason());
            }
        }
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn the_layout_is_the_magic_then_the_digest() {
        assert_eq!(OFF_DIGEST, 8);
        assert_eq!(PAGE_CONTENT_BYTES, 40);
        assert!(PAGE_CONTENT_BYTES < 4096, "the page must fit in one frame");
        assert_eq!(MAGIC[7], b'0' + VERSION as u8);
    }
}

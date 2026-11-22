//! **The argument page.** One definition of the read-only page that carries a program's argv as
//! plain bytes, shared by whoever assembles it (a shell, a test harness) and the runtime that reads
//! it, so the writer and the reader cannot drift on the layout.
//!
//! **A program written by somebody else hears its arguments as bytes, and the bytes carry no
//! authority.** Whether `rg foo src` may read `src` is decided by the access the line granted and
//! the program's own manifest, so nothing on this page is validated for meaning. The page is
//! length-prefixed, not delimited, so any byte (a NUL, a newline, `0xff`) may appear in an argument.
//! It holds `argv[0]` too, since programs such as `ripgrep` skip the first element and `clap` treats
//! it as the program's name. It is one page because an argument line that does not fit one frame is
//! refused whole rather than truncated.
//!
//! # The layout
//! ```text
//!   offset  size  field
//!   0       8     MAGIC, b"PROGARG1"
//!   8       4     count, little-endian u32: how many arguments
//!   12      4     total, little-endian u32: how many bytes of records follow
//!   16      ...   count records, each a little-endian u32 length and then that many bytes
//! ```
//! Any byte may appear in a record, nothing past the records is read, and a line longer than
//! [`MAXIMUM_RECORDS_BYTES`] is refused ([`PushRefused::TooLong`]).

//! # A page that does not parse reads as no arguments
//!
//! [`MAGIC`] is checked first, and then every length against what is left. A zeroed frame, a page
//! belonging to something else, or a count that disagrees with the records all answer an empty argv
//! rather than a partial one: a program that sees half its arguments would act on a line nobody
//! typed. That holds for damage to any one field. The layout has no checksum, so a header whose
//! `count` and `total` both describe a prefix of the records is a shorter, consistent argv and reads
//! as one (see BUGS below).

//! # BUGS
//!
//! - A secret on a command line is plain bytes on this page. The layout accepts that cost, because
//!   the argument line is what the program was asked to run.
//! - Damage to two header fields at once can read as a shorter argv. A `count` and a `total` that
//!   both describe a prefix of the records parse as exactly that prefix, since nothing in the
//!   layout can tell a consistently shortened line from a typed one. The page has one writer and
//!   maps read-only, so this is a property of the format under corruption and not an attack
//!   surface; a checksum would be a wire-format change.
//! - [`ArgumentPage::from_virtual_address`] reads through a `usize` pointer, so on a target
//!   narrower than 64 bits an address above 4 GiB would read a different page. No target built here
//!   is that narrow.
//! - This crate is one file, `src/lib.rs`: the nife repository's `std` port copies it by name, so a
//!   module split, or an inner attribute the port cannot carry, breaks that build and not this one.
//!   A test checks that `src/` holds no other file.
//! - The environment does not ride on this page.
//!
//! Name: ratified 2026-10-18 (UTC). `argument` alone collides with the argument register and the
//! argument word, and `argument_contract` collides the same way, so the crate names the program's
//! arguments in full. The layout is a separate wire format two programs agree on.

#![cfg_attr(not(test), no_std)]

/// The page this layout fills. One frame.
///
/// Name: ratified 2026-11-21 (UTC).
pub const PAGE_BYTES: usize = 4096;

/// The header: [`MAGIC`], the count and the total.
///
/// Name: ratified 2026-11-21 (UTC).
pub const HEADER_BYTES: usize = 16;

/// The bytes of records one page holds, each argument's four-byte length prefix included.
///
/// Name: provisional, renamed from `CAPACITY` so the name carries its unit. `PAGE_CONTENT_BYTES`
/// is not used: elsewhere in the contract family it counts the header too.
pub const MAXIMUM_RECORDS_BYTES: usize = PAGE_BYTES - HEADER_BYTES;

/// The bytes one argument of `argument_bytes` bytes takes in the records: its four-byte length
/// prefix and then the bytes. A line fits the page when the sum of these over its arguments is at
/// most [`MAXIMUM_RECORDS_BYTES`], so a caller that checks a line before it has a page counts with
/// this function and not with a copy of the prefix width.
///
/// Name: ratified 2026-11-21 (UTC).
#[must_use]
pub const fn record_bytes(argument_bytes: usize) -> usize {
    LENGTH_PREFIX_BYTES + argument_bytes
}

/// The width of the length that starts each record.
const LENGTH_PREFIX_BYTES: usize = 4;

// The header stores the record total in a `u32`, and every length in one, so the page must be small
// enough that no stored number truncates.
const _: () = assert!(MAXIMUM_RECORDS_BYTES <= u32::MAX as usize);

/// The first eight bytes of an assembled page: the page's name and then the digit of its
/// [`VERSION`], so a reader that meets a later layout refuses it at the first comparison.
///
/// Name: provisional, awaiting a ruling on the spelling.
pub const MAGIC: [u8; 8] = *b"PROGARG1";

// The magic's last byte is the version's digit, so the two cannot drift apart.
#[allow(clippy::cast_possible_truncation)]
const _: () = assert!(MAGIC[7] == b'0' + VERSION as u8);

/// The page layout's version, the digit that ends [`MAGIC`]: a break to the layout is a new
/// version, and the magic's digit changes in the same coordinated change.
///
/// Name: ratified 2026-11-21 (UTC).
pub const VERSION: u32 = 1;

/// This interface's identity in a program's `INTERFACES` note: 18, naming the argument page. The
/// number is this interface's for good: no other interface takes it, and a format change is a new
/// version, never a new number. A program declares the pair of this id and [`VERSION`].
///
/// Name: ratified 2026-11-21 (UTC).
pub const INTERFACE_ID: u64 = 18;

/// Why [`PageBuilder::push`] refused an argument.
///
/// Name: ratified 2026-11-21 (UTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushRefused {
    /// The argument does not fit in what is left of the page. The line is too long to run, and
    /// nothing is truncated to make it fit.
    TooLong,
}

/// Why [`PageBuilder::new`] refused a buffer: its length is not exactly [`PAGE_BYTES`]. A shorter
/// buffer cannot hold a page, and a longer one is refused too, so a caller that hands over the
/// wrong region learns of it at once.
///
/// Name: ratified 2026-11-21 (UTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WrongPageLength;

/// Assembles one argument page, in place.
///
/// It writes into a page the caller owns rather than returning one, because the one real caller
/// (the shell) writes straight into the frame it is about to send and has no stack to spare for a
/// second 4 KiB copy. A refused argument leaves the page as it was, and nothing is valid until
/// [`PageBuilder::finish`] writes the header: a page abandoned half-built reads as no arguments.
///
/// Name: ratified 2026-11-21 (UTC).
///
/// # Examples
///
/// ```
/// use program_argument_contract::{ArgumentPage, PAGE_BYTES, PageBuilder};
///
/// let mut page = [0u8; PAGE_BYTES];
/// let mut b = PageBuilder::new(&mut page).unwrap();
/// for argument in [&b"rg"[..], b"ab\xffcd", b"src"] {
///     b.push(argument).unwrap();
/// }
/// b.finish();
/// let arguments: Vec<&[u8]> = ArgumentPage::parse(&page).iter().collect();
/// assert_eq!(arguments, [&b"rg"[..], b"ab\xffcd", b"src"]);
/// ```
pub struct PageBuilder<'p> {
    page: &'p mut [u8],
    count: u32,
    used: usize,
}

impl core::fmt::Debug for PageBuilder<'_> {
    /// The counts only, since the page itself is 4 KiB of bytes.
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PageBuilder")
            .field("count", &self.count)
            .field("used", &self.used)
            .finish_non_exhaustive()
    }
}

impl<'p> PageBuilder<'p> {
    /// Name: ratified 2026-11-21 (UTC).
    ///
    /// An empty argv, written over `page`. The magic is cleared first, so the page is not a valid
    /// argv again until [`Self::finish`].
    ///
    /// # Errors
    ///
    /// [`WrongPageLength`] when `page` is not exactly [`PAGE_BYTES`] long. The buffer is left
    /// untouched.
    pub fn new(page: &'p mut [u8]) -> Result<Self, WrongPageLength> {
        if page.len() != PAGE_BYTES {
            return Err(WrongPageLength);
        }
        page[..HEADER_BYTES].fill(0);
        Ok(PageBuilder {
            page,
            count: 0,
            used: 0,
        })
    }

    /// Append one argument, or refuse it if the page cannot hold it.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    ///
    /// # Errors
    ///
    /// [`PushRefused::TooLong`] when the argument's record is longer than [`Self::remaining_bytes`].
    /// The page and the count are left as they were.
    pub fn push(&mut self, argument: &[u8]) -> Result<(), PushRefused> {
        let need = record_bytes(argument.len());
        if need > self.remaining_bytes() {
            return Err(PushRefused::TooLong);
        }
        // `need` fits the page, so the length fits a `u32`; a failure here is the same refusal.
        let length = u32::try_from(argument.len()).map_err(|_| PushRefused::TooLong)?;
        let at = HEADER_BYTES + self.used;
        self.page[at..at + LENGTH_PREFIX_BYTES].copy_from_slice(&length.to_le_bytes());
        self.page[at + LENGTH_PREFIX_BYTES..at + need].copy_from_slice(argument);
        self.used += need;
        self.count += 1;
        Ok(())
    }

    /// How many bytes of records the page still has room for, so an argument fits when its
    /// [`record_bytes`] are no more than this.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    #[must_use]
    pub const fn remaining_bytes(&self) -> usize {
        MAXIMUM_RECORDS_BYTES - self.used
    }

    /// How many arguments have been pushed.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    #[must_use]
    pub const fn argument_count(&self) -> u32 {
        self.count
    }

    /// Write the header, which is what makes the page an argv. Returns the count.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    // The count is a convenience: a caller that has been counting its own pushes ignores it.
    #[allow(clippy::must_use_candidate)]
    pub fn finish(self) -> u32 {
        // `used` never passes `MAXIMUM_RECORDS_BYTES`, which the assertion above keeps inside a `u32`.
        #[allow(clippy::cast_possible_truncation)]
        let total = self.used as u32;
        self.page[8..12].copy_from_slice(&self.count.to_le_bytes());
        self.page[12..16].copy_from_slice(&total.to_le_bytes());
        self.page[..8].copy_from_slice(&MAGIC);
        self.count
    }
}

/// A parsed argument page: an argv that has already been checked whole.
///
/// Name: ratified 2026-11-21 (UTC).
#[derive(Clone, Copy, Debug)]
pub struct ArgumentPage<'a> {
    records: &'a [u8],
    count: u32,
}

impl<'a> ArgumentPage<'a> {
    /// No arguments.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub const EMPTY: ArgumentPage<'static> = ArgumentPage {
        records: &[],
        count: 0,
    };

    /// Read `page`.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    ///
    /// Anything that is not a whole, consistent page is [`Self::EMPTY`]: see the
    /// crate's documentation for why a partial argv is never returned.
    #[must_use]
    pub fn parse(page: &'a [u8]) -> Self {
        Self::check(page).unwrap_or(ArgumentPage::EMPTY)
    }

    fn check(page: &'a [u8]) -> Option<Self> {
        if page.len() < HEADER_BYTES || page[..8] != MAGIC {
            return None;
        }
        let count = u32::from_le_bytes(page[8..12].try_into().ok()?);
        let total = u32::from_le_bytes(page[12..16].try_into().ok()?) as usize;
        let room = (page.len() - HEADER_BYTES).min(MAXIMUM_RECORDS_BYTES);
        if total > room {
            return None;
        }
        let records = &page[HEADER_BYTES..HEADER_BYTES + total];
        // Walk every record now, so the iterator below can never meet a length that overruns. The
        // walk runs until the records are used up, so its bound is the slice (each record takes at
        // least four bytes) and not a count the page chose, and the number walked must then equal
        // the count the header states.
        let mut rest = records;
        let mut walked: u32 = 0;
        while !rest.is_empty() {
            let (_, tail) = split_record(rest)?;
            rest = tail;
            walked = walked.checked_add(1)?;
        }
        (walked == count).then_some(ArgumentPage { records, count })
    }

    /// Read the page mapped at `virtual_address`.
    ///
    /// Name: provisional, renamed from `from_va` so the address is spelled out.
    ///
    /// # Safety
    ///
    /// `virtual_address` must be the start of [`PAGE_BYTES`] of readable memory that stays mapped and unwritten
    /// for `'a`. The loader maps this page read-only and nothing writes it after assembly.
    #[must_use]
    pub unsafe fn from_virtual_address(virtual_address: u64) -> Self {
        // SAFETY: the caller's contract, exactly.
        Self::parse(unsafe {
            core::slice::from_raw_parts(virtual_address as *const u8, PAGE_BYTES)
        })
    }

    /// How many arguments.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    #[must_use]
    pub const fn len(&self) -> usize {
        self.count as usize
    }

    /// Whether there are none.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The arguments, in order.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    #[must_use]
    pub fn iter(&self) -> Iter<'a> {
        Iter {
            rest: self.records,
            left: self.count,
        }
    }
}

/// The arguments of an [`ArgumentPage`], in order.
///
/// Name: ratified 2026-11-21 (UTC).
#[derive(Clone, Debug)]
pub struct Iter<'a> {
    rest: &'a [u8],
    left: u32,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<&'a [u8]> {
        if self.left == 0 {
            return None;
        }
        // `ArgumentPage::check` walked these records already, so this cannot fail; answering `None`
        // rather than panicking keeps that a property of the parser and not of this line.
        let (argument, tail) = split_record(self.rest)?;
        self.rest = tail;
        self.left -= 1;
        Some(argument)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.left as usize, Some(self.left as usize))
    }
}

impl ExactSizeIterator for Iter<'_> {}

impl core::iter::FusedIterator for Iter<'_> {}

/// One record off the front of `rest`: its bytes and what follows it.
fn split_record(rest: &[u8]) -> Option<(&[u8], &[u8])> {
    let len = u32::from_le_bytes(rest.get(..4)?.try_into().ok()?) as usize;
    let body = rest.get(4..)?;
    (len <= body.len()).then(|| body.split_at(len))
}

/// Proofs, run by Kani (the file stays one file, so they sit here).
#[cfg(kani)]
mod verification {
    use super::*;

    /// A symbolic page of 48 bytes whose magic matches, so the proofs reach the records. A page
    /// whose magic differs is refused by one comparison before anything else is read.
    fn any_page() -> [u8; 48] {
        let mut page: [u8; 48] = kani::any();
        page[..8].copy_from_slice(&MAGIC);
        page
    }

    /// **`parse` never panics, and its iterator yields exactly `len()` items**, with `is_empty`
    /// agreeing, for any page that carries the magic.
    /// Falsification: replayable `crates/program_argument_contract/falsifications/verification.parse_never_panics_and_iterates_len_items.patch`
    #[kani::proof]
    #[kani::unwind(14)]
    fn parse_never_panics_and_iterates_len_items() {
        let page = any_page();
        let parsed = ArgumentPage::parse(&page);
        assert_eq!(parsed.iter().count(), parsed.len());
        assert_eq!(parsed.is_empty(), parsed.iter().next().is_none());
    }

    /// **A page that parses holds exactly the records its header states**: the arguments number
    /// `count`, and their four-byte prefixes and bytes add up to `total` with nothing left over.
    /// Anything else is an empty argv.
    /// Falsification: replayable `crates/program_argument_contract/falsifications/verification.a_parsed_page_holds_exactly_what_its_header_states.patch`
    #[kani::proof]
    #[kani::unwind(14)]
    fn a_parsed_page_holds_exactly_what_its_header_states() {
        let page = any_page();
        let count = u32::from_le_bytes([page[8], page[9], page[10], page[11]]);
        let total = u32::from_le_bytes([page[12], page[13], page[14], page[15]]) as usize;
        let parsed = ArgumentPage::parse(&page);
        if parsed.is_empty() {
            return;
        }
        assert_eq!(parsed.len() as u32, count);
        let mut sum = 0;
        for argument in parsed.iter() {
            sum += 4 + argument.len();
        }
        assert_eq!(sum, total);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(arguments: &[&[u8]]) -> [u8; PAGE_BYTES] {
        let mut page = [0xa5; PAGE_BYTES];
        let mut b = PageBuilder::new(&mut page).unwrap();
        for argument in arguments {
            b.push(argument).unwrap();
        }
        b.finish();
        page
    }

    fn read(page: &[u8]) -> Vec<Vec<u8>> {
        ArgumentPage::parse(page)
            .iter()
            .map(<[u8]>::to_vec)
            .collect()
    }

    /// The builder prints its counts and not the page, so the output is short and names both.
    #[test]
    fn builder_debug_shows_counts_only() {
        let mut page = [0u8; PAGE_BYTES];
        let mut b = PageBuilder::new(&mut page).unwrap();
        b.push(b"rg").unwrap();
        let text = format!("{b:?}");
        assert!(text.starts_with("PageBuilder"));
        assert!(text.contains("count: 1"));
        assert!(text.contains("used: "));
    }

    /// Any byte survives, which is the reason the layout is length-prefixed: a NUL and a `0xff`
    /// in the middle of an argument, and an argument that is empty.
    #[test]
    fn every_byte_round_trips() {
        let all: Vec<u8> = (0..=255).collect();
        let page = build(&[b"rg", b"a\0b\xffc", b"", &all]);
        assert_eq!(read(&page), [&b"rg"[..], b"a\0b\xffc", b"", &all]);
    }

    /// **The raw-pointer reader gives what the slice parser gives**, and a zeroed frame through it
    /// is no arguments. This is the one path through [`ArgumentPage::from_virtual_address`], so the test argues its
    /// contract: the page is a live, 4 KiB array, borrowed for the whole of the reads.
    #[test]
    fn a_page_read_through_its_virtual_address_matches_the_slice_reader() {
        let page = build(&[b"rg", b"a\0b", b""]);
        // SAFETY: `page` is a live array of `PAGE_BYTES`, readable and unwritten while `parsed` is used.
        let parsed = unsafe { ArgumentPage::from_virtual_address(page.as_ptr() as u64) };
        assert_eq!(
            parsed.iter().map(<[u8]>::to_vec).collect::<Vec<_>>(),
            read(&page)
        );
        assert_eq!(parsed.len(), 3);

        let zeroed = [0u8; PAGE_BYTES];
        // SAFETY: as above, for `zeroed`.
        let none = unsafe { ArgumentPage::from_virtual_address(zeroed.as_ptr() as u64) };
        assert!(none.is_empty() && none.iter().next().is_none());
    }

    /// **Two header fields damaged together can read as a shorter argv**, which the crate's BUGS
    /// records: `count` and `total` that both describe the first record of two parse as that one.
    #[test]
    fn a_consistently_shortened_header_reads_as_the_shorter_arguments() {
        let mut page = build(&[b"rg", b"src"]);
        page[8..12].copy_from_slice(&1u32.to_le_bytes());
        page[12..16].copy_from_slice(&6u32.to_le_bytes());
        assert_eq!(read(&page), [&b"rg"[..]]);
    }

    /// **This crate is one file**, because the `std` port copies `src/lib.rs` by name: a second
    /// file here would break that build, so it fails this test first.
    #[test]
    fn source_holds_exactly_one_file() {
        let names: Vec<_> = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/src"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["lib.rs"]);
    }

    /// **The cost of a record and the room left agree with what the page holds.** An argument
    /// costs its bytes and a four-byte prefix, `remaining_bytes` falls by that cost per push, and an
    /// argument whose cost equals what is left fits.
    #[test]
    fn record_cost_and_remaining_room_track_the_pushes() {
        assert_eq!(record_bytes(0), 4);
        assert_eq!(record_bytes(10), 14);
        let mut page = [0; PAGE_BYTES];
        let mut b = PageBuilder::new(&mut page).unwrap();
        assert_eq!(b.remaining_bytes(), MAXIMUM_RECORDS_BYTES);
        b.push(b"abc").unwrap();
        assert_eq!(b.remaining_bytes(), MAXIMUM_RECORDS_BYTES - 7);
        let rest = b.remaining_bytes();
        b.push(&vec![b'x'; rest - 4]).unwrap();
        assert_eq!(b.remaining_bytes(), 0);
        assert_eq!(b.push(b""), Err(PushRefused::TooLong));
    }

    /// Iterating a borrowed page with `for` gives the same arguments as `iter`, and an iterator that
    /// has ended stays ended.
    #[test]
    fn a_borrowed_page_iterates_and_a_finished_iterator_stays_finished() {
        let page = build(&[b"a", b"bb"]);
        let parsed = ArgumentPage::parse(&page);
        let mut seen = Vec::new();
        for argument in parsed.iter() {
            seen.push(argument.to_vec());
        }
        assert_eq!(seen, [b"a".to_vec(), b"bb".to_vec()]);
        let mut it = parsed.iter();
        it.next();
        it.next();
        assert_eq!(it.next(), None);
        assert_eq!(it.next(), None);
    }

    /// A frame nobody assembled a page into is no arguments, not a garbage argv.
    #[test]
    fn a_zeroed_page_is_no_arguments() {
        assert!(ArgumentPage::parse(&[0; PAGE_BYTES]).is_empty());
        assert!(ArgumentPage::parse(&[]).is_empty());
    }

    /// The page holds exactly `MAXIMUM_RECORDS_BYTES` bytes of records, and one more is refused whole rather
    /// than truncated. The builder is unchanged by the refusal.
    #[test]
    fn the_page_fills_to_capacity_and_refuses_one_byte_more() {
        let mut page = [0; PAGE_BYTES];
        let mut b = PageBuilder::new(&mut page).unwrap();
        let big = vec![b'x'; MAXIMUM_RECORDS_BYTES - 4];
        b.push(&big).unwrap();
        assert_eq!(b.push(b""), Err(PushRefused::TooLong));
        assert_eq!(b.argument_count(), 1);
        b.finish();
        assert_eq!(read(&page), [big]);

        let mut b = PageBuilder::new(&mut page).unwrap();
        assert_eq!(
            b.push(&[b'x'; MAXIMUM_RECORDS_BYTES - 3]),
            Err(PushRefused::TooLong)
        );
        b.finish();
        assert!(ArgumentPage::parse(&page).is_empty());
    }

    /// **Only a buffer of exactly one page is taken.** A shorter or longer one is refused, and the
    /// refused buffer is not written.
    #[test]
    fn a_buffer_that_is_not_one_page_is_refused_untouched() {
        for length in [0, 1, HEADER_BYTES, PAGE_BYTES - 1, PAGE_BYTES + 1] {
            let mut buffer = vec![0xa5; length];
            assert_eq!(PageBuilder::new(&mut buffer).unwrap_err(), WrongPageLength);
            assert!(buffer.iter().all(|&byte| byte == 0xa5), "length {length}");
        }
        let mut buffer = vec![0xa5; PAGE_BYTES];
        assert!(PageBuilder::new(&mut buffer).is_ok());
    }

    /// **A page reused for a second line is not the first line's argv until it is finished.** The
    /// shell writes each line's words over a frame, so a builder abandoned partway (a word refused
    /// as too long) must not leave the previous header describing the new bytes.
    #[test]
    fn an_unfinished_page_is_no_arguments() {
        let mut page = build(&[b"rg", b"old"]);
        {
            let mut b = PageBuilder::new(&mut page).unwrap();
            b.push(b"new").unwrap();
        }
        assert!(ArgumentPage::parse(&page).is_empty());
    }

    /// **No corruption yields a partial argv.** Every single-byte change to the header or to a
    /// length prefix, and every truncation of the page, either parses to the original argv or to
    /// nothing at all. A program that saw its first two arguments and not its third would run a
    /// line nobody typed. (A change inside an argument's own bytes is a different argv, validly.)
    #[test]
    fn a_damaged_page_is_all_or_nothing() {
        let want: Vec<Vec<u8>> = vec![b"rg".to_vec(), b"pattern".to_vec(), b"src".to_vec()];
        let page = build(&[b"rg", b"pattern", b"src"]);
        // The header, and the three prefixes: `rg` at 16, `pattern` at 22, `src` at 33.
        let framing = (0..HEADER_BYTES).chain(16..20).chain(22..26).chain(33..37);
        for at in framing {
            for v in [0u8, 1, 0x7f, 0xff] {
                let mut p = page;
                p[at] ^= v;
                let got = read(&p);
                assert!(
                    got.is_empty() || got == want,
                    "byte {at} ^ {v:#x} gave {got:?}"
                );
            }
        }
        for n in 0..=PAGE_BYTES {
            let got = read(&page[..n]);
            assert!(got.is_empty() || got == want, "truncated to {n}");
        }
    }

    /// A header that claims more records than the page holds is refused before any record is read.
    #[test]
    fn a_total_past_the_page_is_refused() {
        let mut p = build(&[b"rg"]);
        p[12..16]
            .copy_from_slice(&(u32::try_from(MAXIMUM_RECORDS_BYTES).unwrap() + 1).to_le_bytes());
        assert!(ArgumentPage::parse(&p).is_empty());
    }

    /// **The counts are numbers, not flags.** `argument_count`, `len`, `is_empty` and `size_hint` each have a
    /// two-value wrong answer (0 or 1) that a one-argument fixture cannot tell from the right one,
    /// so this uses three arguments and checks the hint shrinks as the iterator is drained.
    #[test]
    fn counts_track_three_arguments_and_the_hint_shrinks() {
        let mut page = [0; PAGE_BYTES];
        let mut b = PageBuilder::new(&mut page).unwrap();
        assert_eq!(b.argument_count(), 0);
        for (i, argument) in [&b"a"[..], b"bb", b"ccc"].into_iter().enumerate() {
            b.push(argument).unwrap();
            assert_eq!(b.argument_count(), u32::try_from(i).unwrap() + 1);
        }
        assert_eq!(b.finish(), 3);

        let arguments = ArgumentPage::parse(&page);
        assert_eq!(arguments.len(), 3);
        assert!(!arguments.is_empty());
        let mut it = arguments.iter();
        assert_eq!(it.size_hint(), (3, Some(3)));
        it.next();
        assert_eq!(it.size_hint(), (2, Some(2)));
        assert_eq!(it.len(), 2);
        it.next();
        it.next();
        assert_eq!(it.size_hint(), (0, Some(0)));
        assert_eq!(it.next(), None);
    }
}

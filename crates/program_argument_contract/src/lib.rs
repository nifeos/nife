//! **The argument page** (milestone 205 (how a foreign program is told what to do), DECISIONS
//! §170 (how a foreign program is told what to do)). One definition of the read-only page that
//! carries a `std` program's argv, shared by whoever assembles it (the shell, the kernel test
//! harness) and the `std` PAL's `sys/args`, as `environment_contract` is for the configuration page.
//!
//! **A program written by somebody else hears its arguments as bytes, and the bytes carry no
//! authority** (§170). Whether `rg foo src` may read `src` is decided by the directory the line
//! granted and the program's manifest, so nothing on this page is validated for meaning.
//!

//! # The layout
//! ```text
//!   offset  size  field
//!   0       8     MAGIC, b"nifeargv"
//!   8       4     count, little-endian u32: how many arguments
//!   12      4     total, little-endian u32: how many bytes of records follow
//!   16      ...   count records, each a little-endian u32 length and then that many bytes
//! ```
//! Any byte may appear in a record, nothing past the records is read, and a line longer than
//! [`CAPACITY`] is refused ([`Refused::TooLong`]). See notes/foreign-program-arguments.md.

//! # A page that does not parse reads as no arguments
//!
//! [`MAGIC`] is checked first, and then every length against what is left, the same
//! default-honest shape `environment_contract::ConfigPage` uses. A zeroed frame, a page belonging
//! to something else, or a count that disagrees with the records all answer an empty argv rather
//! than a partial one: a program that sees half its arguments would act on a line nobody typed.

//! # BUGS
//!
//! - A secret on a command line is plain bytes on this page, the ruling's known cost; §111 (inert
//!   configuration is a read-only page) refused free-form strings on its page for this reason.
//! - The environment does not ride here: §170 is silent on it, so it stays on §111's page.
//!
//! Name: ratified 2026-10-18 (calef), asked "Or should it be `program_argument_contract`?" and
//! answering yes: `argument` alone collides with the argument register and the argument word.
//! Refused `argument_contract` (that collision). The layout is a separate wire format two programs
//! agree on; `design/roadmap/0672-the-argument-page-layout.md` is its proposal.

#![cfg_attr(not(test), no_std)]

/// The page this layout fills. One frame.
pub const PAGE_BYTES: usize = 4096;

/// The header: [`MAGIC`], the count and the total.
pub const HEADER_BYTES: usize = 16;

/// The bytes of records one page holds, each argument's four-byte length prefix included.
pub const CAPACITY: usize = PAGE_BYTES - HEADER_BYTES;

/// The first eight bytes of an assembled page. Provisional, with the rest of the layout.
pub const MAGIC: [u8; 8] = *b"nifeargv";

/// Why [`PageBuilder::push`] refused an argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// The argument does not fit in what is left of the page. The line is too long to run, and
    /// nothing is truncated to make it fit.
    TooLong,
}

/// Assembles one argument page, in place.
///
/// It writes into a page the caller owns rather than returning one, because the one real caller
/// (the shell) writes straight into the frame it is about to send and has no stack to spare for a
/// second 4 KiB copy. A refused argument leaves the page as it was, and nothing is valid until
/// [`PageBuilder::finish`] writes the header: a page abandoned half-built reads as no arguments.
///
/// # Examples
///
/// ```
/// use program_argument_contract::{ArgPage, PAGE_BYTES, PageBuilder};
///
/// let mut page = [0u8; PAGE_BYTES];
/// let mut b = PageBuilder::new(&mut page);
/// for a in [&b"rg"[..], b"ab\xffcd", b"src"] {
///     b.push(a).unwrap();
/// }
/// b.finish();
/// let args: Vec<&[u8]> = ArgPage::parse(&page).iter().collect();
/// assert_eq!(args, [&b"rg"[..], b"ab\xffcd", b"src"]);
/// ```
pub struct PageBuilder<'p> {
    page: &'p mut [u8; PAGE_BYTES],
    count: u32,
    used: usize,
}

impl<'p> PageBuilder<'p> {
    /// An empty argv, written over `page`. The magic is cleared first, so the page is not a valid
    /// argv again until [`Self::finish`].
    pub fn new(page: &'p mut [u8; PAGE_BYTES]) -> Self {
        page[..HEADER_BYTES].fill(0);
        PageBuilder {
            page,
            count: 0,
            used: 0,
        }
    }

    /// Append one argument, or refuse it if the page cannot hold it.
    pub fn push(&mut self, arg: &[u8]) -> Result<(), Refused> {
        let need = 4 + arg.len();
        if need > CAPACITY - self.used {
            return Err(Refused::TooLong);
        }
        let at = HEADER_BYTES + self.used;
        self.page[at..at + 4].copy_from_slice(&(arg.len() as u32).to_le_bytes());
        self.page[at + 4..at + need].copy_from_slice(arg);
        self.used += need;
        self.count += 1;
        Ok(())
    }

    /// How many arguments have been pushed.
    pub const fn count(&self) -> u32 {
        self.count
    }

    /// Write the header, which is what makes the page an argv. Returns the count.
    pub fn finish(self) -> u32 {
        self.page[8..12].copy_from_slice(&self.count.to_le_bytes());
        self.page[12..16].copy_from_slice(&(self.used as u32).to_le_bytes());
        self.page[..8].copy_from_slice(&MAGIC);
        self.count
    }
}

/// A parsed argument page: an argv that has already been checked whole.
#[derive(Clone, Copy)]
pub struct ArgPage<'a> {
    records: &'a [u8],
    count: u32,
}

impl<'a> ArgPage<'a> {
    /// No arguments.
    pub const EMPTY: ArgPage<'static> = ArgPage {
        records: &[],
        count: 0,
    };

    /// Read `page`. Anything that is not a whole, consistent page is [`Self::EMPTY`]: see the
    /// crate's documentation for why a partial argv is never returned.
    pub fn parse(page: &'a [u8]) -> Self {
        Self::check(page).unwrap_or(ArgPage {
            records: &[],
            count: 0,
        })
    }

    fn check(page: &'a [u8]) -> Option<Self> {
        if page.len() < HEADER_BYTES || page[..8] != MAGIC {
            return None;
        }
        let count = u32::from_le_bytes(page[8..12].try_into().ok()?);
        let total = u32::from_le_bytes(page[12..16].try_into().ok()?) as usize;
        let room = (page.len() - HEADER_BYTES).min(CAPACITY);
        if total > room {
            return None;
        }
        let records = &page[HEADER_BYTES..HEADER_BYTES + total];
        // Walk every record now, so the iterator below can never meet a length that overruns.
        let mut rest = records;
        for _ in 0..count {
            let (_, tail) = split_record(rest)?;
            rest = tail;
        }
        rest.is_empty().then_some(ArgPage { records, count })
    }

    /// Read the page mapped at `va`.
    ///
    /// # Safety
    ///
    /// `va` must be the start of [`PAGE_BYTES`] of readable memory that stays mapped and unwritten
    /// for `'a`. The loader maps this page read-only and nothing writes it after assembly.
    pub unsafe fn from_va(va: u64) -> Self {
        // SAFETY: the caller's contract, exactly.
        Self::parse(unsafe { core::slice::from_raw_parts(va as *const u8, PAGE_BYTES) })
    }

    /// How many arguments.
    pub const fn len(&self) -> usize {
        self.count as usize
    }

    /// Whether there are none.
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The arguments, in order.
    pub fn iter(&self) -> Iter<'a> {
        Iter {
            rest: self.records,
            left: self.count,
        }
    }
}

/// The arguments of an [`ArgPage`], in order.
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
        // `ArgPage::check` walked these records already, so this cannot fail; answering `None`
        // rather than panicking keeps that a property of the parser and not of this line.
        let (arg, tail) = split_record(self.rest)?;
        self.rest = tail;
        self.left -= 1;
        Some(arg)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.left as usize, Some(self.left as usize))
    }
}

impl ExactSizeIterator for Iter<'_> {}

/// One record off the front of `rest`: its bytes and what follows it.
fn split_record(rest: &[u8]) -> Option<(&[u8], &[u8])> {
    let len = u32::from_le_bytes(rest.get(..4)?.try_into().ok()?) as usize;
    let body = rest.get(4..)?;
    (len <= body.len()).then(|| body.split_at(len))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(args: &[&[u8]]) -> [u8; PAGE_BYTES] {
        let mut page = [0xa5; PAGE_BYTES];
        let mut b = PageBuilder::new(&mut page);
        for a in args {
            b.push(a).unwrap();
        }
        b.finish();
        page
    }

    fn read(page: &[u8]) -> Vec<Vec<u8>> {
        ArgPage::parse(page).iter().map(<[u8]>::to_vec).collect()
    }

    /// Any byte survives, which is the reason the layout is length-prefixed: a NUL and a `0xff`
    /// in the middle of an argument, and an argument that is empty.
    #[test]
    fn every_byte_round_trips() {
        let all: Vec<u8> = (0..=255).collect();
        let page = build(&[b"rg", b"a\0b\xffc", b"", &all]);
        assert_eq!(read(&page), [&b"rg"[..], b"a\0b\xffc", b"", &all]);
    }

    /// A frame nobody assembled a page into is no arguments, not a garbage argv.
    #[test]
    fn a_zeroed_page_is_no_arguments() {
        assert!(ArgPage::parse(&[0; PAGE_BYTES]).is_empty());
        assert!(ArgPage::parse(&[]).is_empty());
    }

    /// The page holds exactly `CAPACITY` bytes of records, and one more is refused whole rather
    /// than truncated. The builder is unchanged by the refusal.
    #[test]
    fn the_page_fills_to_capacity_and_refuses_one_byte_more() {
        let mut page = [0; PAGE_BYTES];
        let mut b = PageBuilder::new(&mut page);
        let big = vec![b'x'; CAPACITY - 4];
        b.push(&big).unwrap();
        assert_eq!(b.push(b""), Err(Refused::TooLong));
        assert_eq!(b.count(), 1);
        b.finish();
        assert_eq!(read(&page), [big]);

        let mut b = PageBuilder::new(&mut page);
        assert_eq!(b.push(&[b'x'; CAPACITY - 3]), Err(Refused::TooLong));
        b.finish();
        assert!(ArgPage::parse(&page).is_empty());
    }

    /// **A page reused for a second line is not the first line's argv until it is finished.** The
    /// shell writes each line's words over a frame, so a builder abandoned partway (a word refused
    /// as too long) must not leave the previous header describing the new bytes.
    #[test]
    fn an_unfinished_page_is_no_arguments() {
        let mut page = build(&[b"rg", b"old"]);
        {
            let mut b = PageBuilder::new(&mut page);
            b.push(b"new").unwrap();
        }
        assert!(ArgPage::parse(&page).is_empty());
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
        p[12..16].copy_from_slice(&(CAPACITY as u32 + 1).to_le_bytes());
        assert!(ArgPage::parse(&p).is_empty());
    }

    /// **The counts are numbers, not flags.** `count`, `len`, `is_empty` and `size_hint` each have a
    /// two-value wrong answer (0 or 1) that a one-argument fixture cannot tell from the right one,
    /// so this uses three arguments and checks the hint shrinks as the iterator is drained.
    /// Mutation survivor triage, milestone 326 (turn a mutation score upward), batch 3.
    #[test]
    fn counts_track_three_arguments_and_the_hint_shrinks() {
        let mut page = [0; PAGE_BYTES];
        let mut b = PageBuilder::new(&mut page);
        assert_eq!(b.count(), 0);
        for (i, a) in [&b"a"[..], b"bb", b"ccc"].into_iter().enumerate() {
            b.push(a).unwrap();
            assert_eq!(b.count(), i as u32 + 1);
        }
        assert_eq!(b.finish(), 3);

        let args = ArgPage::parse(&page);
        assert_eq!(args.len(), 3);
        assert!(!args.is_empty());
        let mut it = args.iter();
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

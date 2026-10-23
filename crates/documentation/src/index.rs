//! The search index: one byte layout shared by the host builder and the confined reader. A reader
//! holds one 4 KiB page and never more, so every section starts on a page boundary and every record
//! divides 4096 evenly. The worked example is on `build`. See notes/documentation.md, "Searchable".
//!
//! ```text
//!   page 0            header (below), rest zero
//!   page_off          page records, 128 bytes each, 32 per page
//!   term_off          term records, 32 bytes each, 128 per page, sorted by term bytes
//!   post_off          postings, 4 bytes each, 1024 per page, grouped by term
//! ```

/// The page size the whole layout is built around: the frame a client shares with the FS server.
pub const PAGE: usize = 4096;

/// The magic that starts a well-formed index.
pub const MAGIC: [u8; 8] = *b"CRKRMAN1";

/// **The layout version, in bytes 8..12 of the header.**
///
/// A shard whose version is not this one is [`Error::BadVersion`], and the reader stops rather
/// than read a record layout it does not know. Version 2's page record carries [`Page::tokens`],
/// which ranking divides by. Every shard is a build artifact of `cargo xtask manual`, so a
/// mismatch is refused, not migrated.
pub const VERSION: u32 = 2;

/// **The directory the store is installed in**, relative to the root of whatever directory
/// capability the searcher holds. `doc/bundles` lists the bundles; `doc/<bundle>/index` is one
/// bundle's shard, and `doc/<bundle>/<page>.md` are its pages.
///
/// Here rather than in the builder or in the shell because it is the definition of the store, and
/// **a store layout is a thing two programs agree on** (AGENTS.md rule 7): the host writes these
/// names and the guest opens them. Spelling them twice is the drift `c_seam` is the scar for.
pub const STORE_DIR: &str = "doc";

/// The manifest naming the installed bundles, one per line, inside [`STORE_DIR`].
///
/// **A file rather than a directory listing**, which is the whole milestone in one constant: there
/// is no directory iteration in this system, and adding one would be adding authority.
pub const MANIFEST: &str = "bundles";

/// One bundle's index shard, inside `<STORE_DIR>/<bundle>/`.
pub const SHARD: &str = "index";

/// Bytes in a page record.
pub const PAGE_REC: usize = 128;
/// Bytes in a term record.
pub const TERM_REC: usize = 32;
/// Bytes in a posting.
pub const POST_REC: usize = 4;

/// The longest page path an index can name.
pub const PATH_MAX: usize = 72;
/// The longest page title an index can carry.
pub const TITLE_MAX: usize = 48;
/// The longest term an index stores; longer ones are truncated to it, so two identifiers that
/// agree in their first 24 bytes share a posting list.
pub const TERM_MAX: usize = 24;

/// What went wrong reading an index.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    /// The first eight bytes are not [`MAGIC`].
    BadMagic,
    /// The header is shorter than a header.
    Truncated,
    /// The magic is right and the layout version is not [`VERSION`]. A separate variant from
    /// [`BadMagic`](Error::BadMagic) because the two are different facts about the world: one file
    /// is not an index at all, and the other is an index this reader is too old or too new for.
    BadVersion {
        /// What the file claims to be.
        found: u32,
    },
}

/// The header, parsed. Every offset is a byte offset from the start of the file and is a multiple
/// of [`PAGE`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Header {
    /// The layout version, which [`Header::parse`] has already checked is [`VERSION`]. Carried
    /// rather than dropped so a caller printing what it opened can say which one it read.
    pub version: u32,
    /// How many pages the index describes.
    pub pages: u32,
    /// How many distinct terms it holds.
    pub terms: u32,
    /// How many postings, across all terms.
    pub postings: u32,
    /// Byte offset of the page records.
    pub page_off: u64,
    /// Byte offset of the term records.
    pub term_off: u64,
    /// Byte offset of the postings.
    pub post_off: u64,
    /// The index's total size in bytes, which is what a caller checks a file against.
    pub total: u64,
}

impl Header {
    /// Read a header out of the index's first page.
    pub fn parse(first: &[u8]) -> Result<Header, Error> {
        if first.len() < 64 {
            return Err(Error::Truncated);
        }
        if first[..8] != MAGIC {
            return Err(Error::BadMagic);
        }
        let version = u32(first, 8);
        if version != VERSION {
            return Err(Error::BadVersion { found: version });
        }
        Ok(Header {
            version,
            pages: u32(first, 12),
            terms: u32(first, 16),
            postings: u32(first, 20),
            page_off: u64v(first, 24),
            term_off: u64v(first, 32),
            post_off: u64v(first, 40),
            total: u64v(first, 48),
        })
    }
}

/// A source of index pages.
///
/// The reader asks for a page by number and gets it or does not; it never seeks, never reads a byte
/// range, and never holds two pages at once. That is the whole interface, and it is this small so
/// that the guest implementation is a single `filesystem_contract` `READ` and the host implementation is a
/// slice.
pub trait Pages {
    /// Fill `out` with page `index` of the file. `false` means the read failed or ran off the end.
    fn page(&mut self, index: u64, out: &mut [u8; PAGE]) -> bool;
}

/// A [`Pages`] over bytes already in memory, which is what the host and the tests use.
pub struct Slice<'a>(pub &'a [u8]);

impl Pages for Slice<'_> {
    fn page(&mut self, index: u64, out: &mut [u8; PAGE]) -> bool {
        let start = index as usize * PAGE;
        if start >= self.0.len() {
            return false;
        }
        let end = (start + PAGE).min(self.0.len());
        out[..end - start].copy_from_slice(&self.0[start..end]);
        out[end - start..].fill(0);
        true
    }
}

/// A term that was found: where its postings are and how many there are.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Hit {
    /// Index of this term's first posting.
    pub first: u32,
    /// How many pages mention the term.
    pub count: u16,
}

/// One page's mention of a term.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Posting {
    /// Which page record, by index.
    pub page: u16,
    /// How many times the term occurs on it.
    pub count: u16,
}

/// A page the index names.
#[derive(Clone, Copy)]
pub struct Page {
    path: [u8; PATH_MAX],
    path_len: u8,
    title: [u8; TITLE_MAX],
    title_len: u8,
    tokens: u32,
}

impl Page {
    /// The page's path, as the store names it.
    pub fn path(&self) -> &[u8] {
        &self.path[..self.path_len as usize]
    }
    /// The page's title: its first level-one heading, or its path if it has none.
    pub fn title(&self) -> &[u8] {
        &self.title[..self.title_len as usize]
    }
    /// **How many tokens [`tokens`] emitted over this page's text**, the denominator ranking
    /// divides by. Not a byte count and not a word count in the ordinary sense: it is the same
    /// count the builder used to fill the postings, so a page's density for a term is exactly
    /// `count / this`.
    pub fn tokens(&self) -> u32 {
        self.tokens
    }
}

/// Fold a query word the way the builder folded the text, so the two can be compared.
///
/// Returns the length written into `out`. A word that folds to nothing (punctuation alone) returns
/// zero and is not a term.
pub fn normalize(word: &[u8], out: &mut [u8; TERM_MAX]) -> usize {
    let mut n = 0;
    for &b in word {
        if n == TERM_MAX {
            break;
        }
        if b.is_ascii_alphanumeric() {
            out[n] = b.to_ascii_lowercase();
            n += 1;
        }
    }
    n
}

/// Split text into terms the way the builder does: runs of ASCII letters and digits, folded to
/// lower case, truncated to [`TERM_MAX`], and dropped if shorter than two characters.
///
/// An underscore-joined name is also one term: `line_editor` yields `line`, `editor` and
/// `lineeditor`, which is what [`normalize`] makes of the query. Only the underscore joins. See
/// notes/documentation/underscore-terms.md.
///
/// BUGS: folding is ASCII only and every other byte splits, so a search for `filesystem` also
/// finds every page that says `filesystem_contract`.
pub fn tokens(text: &[u8], mut f: impl FnMut(&[u8])) {
    let mut buf = [0u8; TERM_MAX];
    let mut n = 0;
    // The compound: this run and every run underscore-joined ahead of it, with the underscores
    // taken out, which is exactly what `normalize` will do to the query.
    let mut joined = [0u8; TERM_MAX];
    let mut jn = 0;
    let mut parts = 0usize;
    for &b in text {
        if b.is_ascii_alphanumeric() {
            if n < TERM_MAX {
                buf[n] = b.to_ascii_lowercase();
                n += 1;
            }
            if jn < TERM_MAX {
                joined[jn] = b.to_ascii_lowercase();
                jn += 1;
            }
            continue;
        }
        if n >= 2 {
            f(&buf[..n]);
        }
        if n > 0 {
            parts += 1;
        }
        n = 0;
        if b != b'_' {
            // Emitted only when there was something to join, so an ordinary word is not offered
            // twice and the term table does not double for prose.
            if parts > 1 && jn >= 2 {
                f(&joined[..jn]);
            }
            jn = 0;
            parts = 0;
        }
    }
    if n >= 2 {
        f(&buf[..n]);
    }
    if n > 0 {
        parts += 1;
    }
    if parts > 1 && jn >= 2 {
        f(&joined[..jn]);
    }
}

/// **Read the bundle manifest**, calling `f` once per installed bundle name.
///
/// The format is one name per line, which is the whole of it: the manifest exists because nothing
/// can list a directory, so it is the smallest thing that can answer "what is installed" and it
/// should stay readable by a person holding `cat`. Blank lines are skipped and `\r` is trimmed, so
/// a manifest that has been through a text editor still reads.
pub fn bundles(text: &[u8], mut f: impl FnMut(&[u8])) {
    for line in text.split(|&b| b == b'\n') {
        let line = match line.split_last() {
            Some((b'\r', head)) => head,
            _ => line,
        };
        if !line.is_empty() {
            f(line);
        }
    }
}

/// Look a term up, reading at most `ceil(log2(term pages)) + 1` pages.
///
/// The search is over *pages*, not over records: each probe reads one page and compares the term it
/// starts with, which narrows the range by half for one read. The final page is searched in memory,
/// where it costs nothing.
pub fn lookup(h: &Header, term: &[u8], src: &mut impl Pages) -> Option<Hit> {
    let mut key = [0u8; TERM_MAX];
    let n = normalize(term, &mut key);
    if n == 0 || h.terms == 0 {
        return None;
    }
    let key = &key[..n];

    let per = PAGE / TERM_REC;
    let first = h.term_off / PAGE as u64;
    let last = (h.terms as u64 - 1) / per as u64;

    let mut buf = [0u8; PAGE];
    let mut lo = 0u64;
    let mut hi = last;
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        if !src.page(first + mid, &mut buf) {
            return None;
        }
        if cmp(&buf[..TERM_REC], key) == core::cmp::Ordering::Greater {
            hi = mid - 1;
        } else {
            lo = mid;
        }
    }
    if !src.page(first + lo, &mut buf) {
        return None;
    }
    let here = (h.terms as u64 - lo * per as u64).min(per as u64) as usize;
    for i in 0..here {
        let rec = &buf[i * TERM_REC..(i + 1) * TERM_REC];
        if cmp(rec, key) == core::cmp::Ordering::Equal {
            return Some(Hit {
                first: u32(rec, TERM_MAX),
                count: u16v(rec, TERM_MAX + 4),
            });
        }
    }
    None
}

/// Read up to `out.len()` of a hit's postings, returning how many were read.
pub fn postings(h: &Header, hit: &Hit, src: &mut impl Pages, out: &mut [Posting]) -> usize {
    let mut buf = [0u8; PAGE];
    let per = PAGE / POST_REC;
    let want = (hit.count as usize).min(out.len());
    let mut done = 0;
    let mut loaded = u64::MAX;
    while done < want {
        let at = hit.first as usize + done;
        let page = h.post_off / PAGE as u64 + (at / per) as u64;
        if page != loaded {
            if !src.page(page, &mut buf) {
                return done;
            }
            loaded = page;
        }
        let off = (at % per) * POST_REC;
        out[done] = Posting {
            page: u16v(&buf, off),
            count: u16v(&buf, off + 2),
        };
        done += 1;
    }
    done
}

/// Read one page record.
pub fn page_record(h: &Header, i: u16, src: &mut impl Pages) -> Option<Page> {
    if i as u32 >= h.pages {
        return None;
    }
    let per = PAGE / PAGE_REC;
    let mut buf = [0u8; PAGE];
    if !src.page(
        h.page_off / PAGE as u64 + (i as usize / per) as u64,
        &mut buf,
    ) {
        return None;
    }
    let rec = &buf[(i as usize % per) * PAGE_REC..][..PAGE_REC];
    let mut p = Page {
        path: [0; PATH_MAX],
        path_len: rec[PATH_MAX + TITLE_MAX].min(PATH_MAX as u8),
        title: [0; TITLE_MAX],
        title_len: rec[PATH_MAX + TITLE_MAX + 1].min(TITLE_MAX as u8),
        tokens: u32(rec, PATH_MAX + TITLE_MAX + 2),
    };
    p.path.copy_from_slice(&rec[..PATH_MAX]);
    p.title
        .copy_from_slice(&rec[PATH_MAX..PATH_MAX + TITLE_MAX]);
    Some(p)
}

// ---- the answer: searching a store of shards, with no allocator ------------------------------

/// The longest store location a result can name: `<STORE_DIR>/<bundle>/<page>`.
///
/// Ninety-six rather than [`PATH_MAX`] plus slack, because a location is not a path in the index:
/// it is three short components, and the widest real one today (`doc/swish/line-discipline.md`) is
/// twenty-eight bytes. A location longer than this is truncated and [`Found::is_truncated`] says
/// so, for the reason the renderer gives about losing text silently.
pub const LOCATION_MAX: usize = 96;

/// **How many results one search keeps.**
///
/// Sixteen, and the number is the reader's rather than the index's: a search answer is something a
/// person reads at a prompt before deciding what to open, and a screen of a terminal this system
/// has is 16 rows (`video_terminal`) or a serial window. Every result past the sixteenth is counted
/// and not kept, and [`Ranked::offered`] is what lets the caller say so instead of implying the
/// answer was complete.
pub const RESULTS_MAX: usize = 16;

/// **The fixed-point scale `score` multiplies by**, so `count / tokens` survives as an integer
/// worth comparing rather than rounding to zero every time a term is not the whole page.
///
/// A million rather than something smaller because the numerator, `count as u64 * SCORE_SCALE`,
/// has to fit `u64` for the largest `count` this layout can carry (`u16::MAX`) and the smallest
/// meaningful `tokens` (one): `65535 * 1_000_000` is about 6.55e10, nowhere near `u64::MAX`. The
/// result then fits `u32` because a term cannot occur more times than the page holds tokens, so
/// `count <= tokens` in the honest case and the ratio never exceeds [`SCORE_SCALE`] itself.
const SCORE_SCALE: u64 = 1_000_000;

/// **Term frequency over document length.** `page_tokens` is clamped to at least one, so a page
/// recorded as empty cannot divide by zero in a `no_std` reader.
///
/// BUGS: the index stores no positions, because the reader has no memory to spend on them, so
/// there is no phrase search and a heading mention ranks like a passing one.
fn score(count: u16, page_tokens: u32) -> u32 {
    let denom = (page_tokens as u64).max(1);
    ((count as u64 * SCORE_SCALE) / denom).min(u32::MAX as u64) as u32
}

/// One line of a search answer: a page, where a reader can find it, and how strongly it matched.
///
/// It holds **copies** rather than borrows, and that is what makes a merge across shards possible
/// at all: each shard is opened, searched and closed in turn, and a result borrowed from the page
/// buffer would name bytes that the next shard's first read overwrites. It names a page and
/// grants nothing: reading the page is a separate act, backed by whoever holds the store.
#[derive(Clone, Copy)]
pub struct Found {
    location: [u8; LOCATION_MAX],
    location_len: u8,
    truncated: bool,
    title: [u8; TITLE_MAX],
    title_len: u8,
    /// The page's path as the index records it, which is where the page came from in the source
    /// tree rather than where it is installed. See [`Found::location`] for the difference.
    origin: [u8; PATH_MAX],
    origin_len: u8,
    /// How many times the term occurs on this page. Printed as-is: what a reader sees is the raw
    /// occurrence count, never the score below, because the count is the fact and the score is
    /// only how results are ordered.
    pub count: u16,
    /// **The ranking key**: `count` scaled by [`SCORE_SCALE`] and divided by the page's token
    /// count, so a short page where a term is dense outranks a long page where it is sparse even
    /// with a smaller raw count. Not exposed: nothing outside [`Ranked::offer`] needs to compare
    /// it, and printing a fixed-point ratio would need explaining that `count` does not.
    score: u32,
}

impl Found {
    /// **Where a reader can open this page**, as a name they can type: `<STORE_DIR>/<bundle>/<page>`.
    ///
    /// Computed by the searcher from the shard it opened, not read out of the index: the searcher
    /// already knows its bundle, and a fact stored twice can disagree with itself.
    pub fn location(&self) -> &[u8] {
        &self.location[..self.location_len as usize]
    }

    /// Whether the location above lost its tail to [`LOCATION_MAX`].
    pub fn is_truncated(&self) -> bool {
        self.truncated
    }

    /// The page's title: its first level-one heading, or its path if it has none.
    pub fn title(&self) -> &[u8] {
        &self.title[..self.title_len as usize]
    }

    /// **Where the page came from**, as the index records it: a path in the source tree
    /// (`notes/pipes.md`), which is provenance rather than something to open. [`Found::location`]
    /// is the openable name.
    pub fn origin(&self) -> &[u8] {
        &self.origin[..self.origin_len as usize]
    }
}

/// **The merge across shards**: the best [`RESULTS_MAX`] results offered, strongest first.
///
/// The roadmap's "the index is a merge of shards" lives here, and it is a fixed array rather than a
/// heap because the merge runs in a shell holding one 4 KiB stack page and no allocator. Ties keep
/// the order they were offered in, so a store whose bundles are searched in manifest order gives
/// the same answer twice.
pub struct Ranked {
    found: [Found; RESULTS_MAX],
    n: usize,
    offered: usize,
}

impl Default for Ranked {
    fn default() -> Self {
        Ranked::new()
    }
}

impl Ranked {
    /// An empty table. `const` so a caller with no allocator can put one in `.bss`, which is what
    /// the shell does: [`RESULTS_MAX`] of these is about 2.4 KiB and a stack page is four.
    pub const fn new() -> Self {
        Ranked {
            found: [Found {
                location: [0; LOCATION_MAX],
                location_len: 0,
                truncated: false,
                title: [0; TITLE_MAX],
                title_len: 0,
                origin: [0; PATH_MAX],
                origin_len: 0,
                count: 0,
                score: 0,
            }; RESULTS_MAX],
            n: 0,
            offered: 0,
        }
    }

    /// Offer one page that mentioned the term. Kept if it beats the weakest result held.
    ///
    /// It takes the record's fields rather than a [`Page`], so the merge is independent of how
    /// the record was read: a caller with a path, a title and a token count in hand can offer
    /// one, which is what lets the shell's rendering be host-tested without a shard to open.
    ///
    /// `page_tokens` is the page's own [`Page::tokens`], the denominator `score` divides by. A
    /// page whose stored token count is zero (only possible for a version-1 layout this reader
    /// never opens, or an empty page) is treated as one, so a term cannot score as infinitely
    /// dense.
    pub fn offer(
        &mut self,
        bundle: &[u8],
        path: &[u8],
        title: &[u8],
        count: u16,
        page_tokens: u32,
    ) {
        self.offered += 1;
        let s = score(count, page_tokens);
        // Where it goes: past everything that scored at least as strongly, which keeps ties in
        // offer order because the test is strict.
        let mut at = 0;
        while at < self.n && self.found[at].score >= s {
            at += 1;
        }
        if at == RESULTS_MAX {
            return;
        }
        let end = (self.n + 1).min(RESULTS_MAX);
        for i in (at + 1..end).rev() {
            self.found[i] = self.found[i - 1];
        }
        self.n = end;

        let f = &mut self.found[at];
        f.count = count;
        f.score = s;
        (f.location_len, f.truncated) = location(bundle, path, &mut f.location);
        f.title_len = copy_into(&mut f.title, title) as u8;
        f.origin_len = copy_into(&mut f.origin, path) as u8;
    }

    /// The results, strongest first.
    pub fn results(&self) -> &[Found] {
        &self.found[..self.n]
    }

    /// **How many results were offered in total**, including the ones that did not fit. A caller
    /// that prints [`results`](Ranked::results) without this is implying its answer was complete.
    pub fn offered(&self) -> usize {
        self.offered
    }
}

/// **Search one shard**, offering every page that mentions `term` into `into`.
///
/// `cargo xtask manual <term>` on the host and the shell's `apropos` both run this function over
/// the same bytes, through the same one-page-at-a-time [`Pages`], so it is where the writer and
/// the reader are proved to agree.
///
/// Returns `Ok(())` with nothing offered for a term nothing mentions, and an [`Err`] when the
/// shard is not one. Memory is one page buffer at a time inside [`lookup`], [`postings`] and
/// [`page_record`], plus a sixteen-posting batch on this frame.
pub fn search(
    bundle: &[u8],
    term: &[u8],
    src: &mut impl Pages,
    into: &mut Ranked,
) -> Result<(), Error> {
    let mut first = [0u8; PAGE];
    if !src.page(0, &mut first) {
        return Err(Error::Truncated);
    }
    let h = Header::parse(&first)?;
    let Some(hit) = lookup(&h, term, src) else {
        return Ok(());
    };

    let mut batch = [Posting { page: 0, count: 0 }; 16];
    let mut done = 0u32;
    while done < hit.count as u32 {
        let rest = Hit {
            first: hit.first + done,
            count: (hit.count as u32 - done) as u16,
        };
        let n = postings(&h, &rest, src, &mut batch);
        if n == 0 {
            break;
        }
        for p in &batch[..n] {
            if let Some(rec) = page_record(&h, p.page, src) {
                into.offer(bundle, rec.path(), rec.title(), p.count, rec.tokens());
            }
        }
        done += n as u32;
    }
    Ok(())
}

/// Build `<STORE_DIR>/<bundle>/<basename of path>` into `out`, returning its length and whether it
/// lost its tail.
fn location(bundle: &[u8], path: &[u8], out: &mut [u8; LOCATION_MAX]) -> (u8, bool) {
    let base = match path.iter().rposition(|&b| b == b'/') {
        Some(i) => &path[i + 1..],
        None => path,
    };
    let mut n = 0usize;
    let mut put = |bytes: &[u8]| {
        for &b in bytes {
            if n < LOCATION_MAX {
                out[n] = b;
                n += 1;
            }
        }
    };
    put(STORE_DIR.as_bytes());
    put(b"/");
    put(bundle);
    put(b"/");
    put(base);
    let want = STORE_DIR.len() + 1 + bundle.len() + 1 + base.len();
    (n as u8, want > LOCATION_MAX)
}

/// Copy as much of `src` as fits, returning how much that was.
fn copy_into(dst: &mut [u8], src: &[u8]) -> usize {
    let n = src.len().min(dst.len());
    dst[..n].copy_from_slice(&src[..n]);
    dst[n..].fill(0);
    n
}

/// Compare a term record's term against a key.
fn cmp(rec: &[u8], key: &[u8]) -> core::cmp::Ordering {
    let n = rec[TERM_MAX + 6] as usize;
    rec[..n.min(TERM_MAX)].cmp(key)
}

fn u16v(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn u64v(b: &[u8], at: usize) -> u64 {
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[at..at + 8]);
    u64::from_le_bytes(v)
}

// ---- the builder, host only ----------------------------------------------------------------

#[cfg(feature = "builder")]
mod builder {
    use alloc::collections::BTreeMap;
    use alloc::vec;
    use alloc::vec::Vec;

    use super::*;

    /// One page offered to the builder.
    pub struct Source<'a> {
        /// The path the store will know the page by. Truncated to [`PATH_MAX`].
        pub path: &'a str,
        /// The page's title. Truncated to [`TITLE_MAX`].
        pub title: &'a str,
        /// The page's markdown source, which is what gets tokenised.
        pub text: &'a [u8],
    }

    /// Build an index over these pages.
    ///
    /// This is the only allocating function in the crate and it is behind the `builder` feature for
    /// that reason: it runs in `xtask` on a host with a heap, never on the guest.
    ///
    /// # EXAMPLES
    ///
    /// ```
    /// use documentation::index;
    ///
    /// let bytes = index::build(&[
    ///     index::Source { path: "notes/glob.md", title: "Globbing", text: b"a pattern and a name" },
    ///     index::Source { path: "notes/ipc.md", title: "IPC", text: b"a name and an endpoint" },
    /// ]);
    ///
    /// let mut src = index::Slice(&bytes);
    /// let h = index::Header::parse(&bytes[..index::PAGE]).unwrap();
    ///
    /// // Both pages say "name", so the term has two postings.
    /// let hit = index::lookup(&h, b"name", &mut src).unwrap();
    /// assert_eq!(hit.count, 2);
    ///
    /// // And one of them is the one whose title says so.
    /// let mut post = [index::Posting { page: 0, count: 0 }; 4];
    /// let n = index::postings(&h, &hit, &mut src, &mut post);
    /// let first = index::page_record(&h, post[0].page, &mut src).unwrap();
    /// assert_eq!(n, 2);
    /// assert_eq!(first.title(), b"Globbing");
    /// ```
    pub fn build(pages: &[Source<'_>]) -> Vec<u8> {
        // term -> (page index -> occurrences). A BTreeMap because the term table must come out
        // sorted and this way it is sorted by construction rather than by a pass nobody can see.
        let mut terms: BTreeMap<Vec<u8>, BTreeMap<u16, u32>> = BTreeMap::new();
        // Every page's total token count, the ranking denominator: not the distinct-term count
        // above, the count of every token `tokens` ever called `f` with for that page, underscore
        // joins included, because that is the same population `count` is drawn from.
        let mut page_tokens = vec![0u32; pages.len()];
        for (i, p) in pages.iter().enumerate() {
            tokens(p.text, |t| {
                page_tokens[i] = page_tokens[i].saturating_add(1);
                *terms
                    .entry(t.to_vec())
                    .or_default()
                    .entry(i as u16)
                    .or_default() += 1;
            });
        }

        let post_count: usize = terms.values().map(|m| m.len()).sum();
        let page_off = PAGE;
        let term_off = page_off + round_up(pages.len() * PAGE_REC);
        let post_off = term_off + round_up(terms.len() * TERM_REC);
        let total = post_off + round_up(post_count * POST_REC);
        let mut out = vec![0u8; total];

        out[..8].copy_from_slice(&MAGIC);
        out[8..12].copy_from_slice(&VERSION.to_le_bytes());
        out[12..16].copy_from_slice(&(pages.len() as u32).to_le_bytes());
        out[16..20].copy_from_slice(&(terms.len() as u32).to_le_bytes());
        out[20..24].copy_from_slice(&(post_count as u32).to_le_bytes());
        out[24..32].copy_from_slice(&(page_off as u64).to_le_bytes());
        out[32..40].copy_from_slice(&(term_off as u64).to_le_bytes());
        out[40..48].copy_from_slice(&(post_off as u64).to_le_bytes());
        out[48..56].copy_from_slice(&(total as u64).to_le_bytes());

        for (i, p) in pages.iter().enumerate() {
            let rec = page_off + i * PAGE_REC;
            let path = p.path.as_bytes();
            let n = path.len().min(PATH_MAX);
            out[rec..rec + n].copy_from_slice(&path[..n]);
            let title = p.title.as_bytes();
            let m = title.len().min(TITLE_MAX);
            out[rec + PATH_MAX..rec + PATH_MAX + m].copy_from_slice(&title[..m]);
            out[rec + PATH_MAX + TITLE_MAX] = n as u8;
            out[rec + PATH_MAX + TITLE_MAX + 1] = m as u8;
            out[rec + PATH_MAX + TITLE_MAX + 2..rec + PATH_MAX + TITLE_MAX + 6]
                .copy_from_slice(&page_tokens[i].to_le_bytes());
        }

        let mut post = 0usize;
        for (i, (term, byname)) in terms.iter().enumerate() {
            let rec = term_off + i * TERM_REC;
            let n = term.len().min(TERM_MAX);
            out[rec..rec + n].copy_from_slice(&term[..n]);
            out[rec + TERM_MAX..rec + TERM_MAX + 4].copy_from_slice(&(post as u32).to_le_bytes());
            out[rec + TERM_MAX + 4..rec + TERM_MAX + 6]
                .copy_from_slice(&(byname.len() as u16).to_le_bytes());
            out[rec + TERM_MAX + 6] = n as u8;
            for (page, count) in byname {
                let at = post_off + post * POST_REC;
                out[at..at + 2].copy_from_slice(&page.to_le_bytes());
                out[at + 2..at + 4]
                    .copy_from_slice(&(*count).min(u16::MAX as u32).to_le_bytes()[..2]);
                post += 1;
            }
        }

        out
    }

    /// The first level-one heading in a markdown document, which is what a page is called.
    pub fn title_of(text: &[u8]) -> Option<&str> {
        let mut start = 0;
        while start < text.len() {
            let end = text[start..]
                .iter()
                .position(|&b| b == b'\n')
                .map(|k| start + k)
                .unwrap_or(text.len());
            let line = &text[start..end];
            if line.starts_with(b"# ") {
                return core::str::from_utf8(&line[2..]).ok().map(str::trim);
            }
            start = end + 1;
        }
        None
    }

    fn round_up(n: usize) -> usize {
        n.div_ceil(PAGE) * PAGE
    }
}

#[cfg(feature = "builder")]
pub use builder::{Source, build, title_of};

#[cfg(test)]
mod tests;

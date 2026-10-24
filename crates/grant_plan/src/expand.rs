//! **Expansion: what a pattern on the command line designates** (milestone 47's globbing lane).
//!
//! `crates/glob` answers whether a pattern matches a name; this module turns that into an
//! authority. The expansion you see is the grant: `echo *.txt` prints the authority `rm *.txt`
//! would transfer, because both drive one [`Expander`] over the same listing. [`Name`] and
//! [`NameSet`] own their bytes, since a matched name comes out of a listing read into a reused
//! buffer, not out of the line. Only a token's last component expands; `**` and qualifiers are
//! out (notes/glob.md). See notes/glob-grant.md.

use crate::Refusal;
use crate::nav::{self, Step};

/// The longest name a grant can carry, which is [`crate::MAX_FILE_NAME`].
pub const MAX_NAME: usize = crate::MAX_FILE_NAME;

/// **The most names one expansion may grant**: Unix's `ARG_MAX` as a capability limit. The set
/// travels by value in a frame the caretaker keeps whole and through four frames of the shell's
/// stack, and sixteen ran the shell off its stack. It matches
/// `filesystem_contract::nameset::MAX_NAMES`, pinned by a host test. Exceeding it is
/// [`Refusal::TooManyNames`] at the prompt, never a truncation. See notes/glob-grant/xargs.md
/// ("Why the bound is eight").
pub const MAX_NAMES: usize = 8;

/// **The most bytes of name one expansion may grant, all its names together**, for
/// [`MAX_NAMES`]'s reason: a [`NameSet`] is copied through the shell's deepest frames, so it holds
/// its names packed in this many bytes rather than [`MAX_NAMES`] slots of [`MAX_NAME`] each. It
/// matches `filesystem_contract::nameset::NAME_BYTES`, pinned by a host test, and exceeding it is
/// the same [`Refusal::TooManyNames`]. See notes/glob-grant/xargs.md ("The bytes a set carries").
///
/// Name: provisional (milestone 946).
pub const NAME_BYTES: usize = 2 * (MAX_NAME + 1);

/// One name, owned. See the module note: a name a pattern produced is not a slice of the line.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Name {
    bytes: [u8; MAX_NAME],
    len: u8,
}

/// Written as the name it holds, because a `Debug` full of zero padding is one nobody reads, and
/// [`crate::Endowment`] carries these into every failing assertion.
impl core::fmt::Debug for Name {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match core::str::from_utf8(self.as_bytes()) {
            Ok(s) => write!(f, "{s:?}"),
            Err(_) => write!(f, "{:?}", self.as_bytes()),
        }
    }
}

impl Name {
    /// A name, or `None` if it cannot travel in a grant at all. **`None` rather than a truncation**,
    /// for [`crate::file_name_fits`]'s reason: a truncated filename is a different file, and this is
    /// the path that decides what gets granted.
    pub fn new(name: &[u8]) -> Option<Name> {
        if !crate::file_name_fits(name) {
            return None;
        }
        let mut bytes = [0u8; MAX_NAME];
        bytes[..name.len()].copy_from_slice(name);
        Some(Name {
            bytes,
            len: name.len() as u8,
        })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }
}

/// **The namespace a grant designates**: one name for a plain token, the matched names for a
/// pattern.
///
/// This is the type that makes the milestone's finding true in the code rather than only in the
/// prose. `fs_file_caretaker` serves "a namespace of exactly one name"; globbing generalizes it to
/// a set, and a literal operand is simply the set of one. Same caretaker shape, same `filesystem_contract`
/// protocol above and below, wider namespace, nothing new in the kernel.
///
/// Each name carries what the directory said it was, because the caretaker answers `READDIR` from
/// the set itself (`filesystem_contract::nameset`), and because a listing is a rendering of authority.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct NameSet {
    /// The names, packed end to end in order: name `i` is `bytes[start(i)..ends[i]]`. Packed rather
    /// than [`MAX_NAMES`] slots of [`MAX_NAME`], because this is copied by value through the
    /// shell's deepest frames ([`NAME_BYTES`]).
    bytes: [u8; NAME_BYTES],
    /// Where each name ends in `bytes`, in step with `is_directory`.
    ends: [u16; MAX_NAMES],
    /// Whether each name is a directory, in step with `ends`.
    ///
    /// Name: ratified 2026-10-18 (calef), for this field and for the `is_directory` parameters
    /// that feed it.
    is_directory: [bool; MAX_NAMES],
    n: usize,
}

impl core::fmt::Debug for NameSet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list()
            .entries(self.iter().map(|(name, _)| name))
            .finish()
    }
}

impl Default for NameSet {
    fn default() -> Self {
        NameSet::empty()
    }
}

impl NameSet {
    pub const fn empty() -> Self {
        NameSet {
            bytes: [0; NAME_BYTES],
            ends: [0; MAX_NAMES],
            is_directory: [false; MAX_NAMES],
            n: 0,
        }
    }

    /// The set of exactly one name: what a token with no pattern in it designates.
    pub fn one(name: &[u8], is_directory: bool) -> Option<NameSet> {
        let mut set = NameSet::empty();
        set.push(name, is_directory).then_some(set)
    }

    /// Where name `i` starts in `bytes`.
    fn start(&self, i: usize) -> usize {
        if i == 0 { 0 } else { self.ends[i - 1] as usize }
    }

    /// Name `i`.
    fn name(&self, i: usize) -> &[u8] {
        &self.bytes[self.start(i)..self.ends[i] as usize]
    }

    /// The bytes of name the set holds, all its names together.
    ///
    /// Name: provisional, minted 2026-10-24 (UTC) by lane
    /// `milestone/946-granted-file-name-longer-than-sixteen-bytes`.
    pub fn name_bytes(&self) -> usize {
        self.start(self.n)
    }

    /// Whether `len` more bytes of name fit beside what is held.
    fn has_room_for(&self, len: usize) -> bool {
        self.n < MAX_NAMES && self.name_bytes() + len <= NAME_BYTES
    }

    /// Add a name. `false` if the set is full, by count or by bytes, or the name cannot travel in a
    /// grant; the caller must turn that into a refusal rather than carrying on with a smaller set.
    pub fn push(&mut self, name: &[u8], is_directory: bool) -> bool {
        if !crate::file_name_fits(name) || self.contains(name) || !self.has_room_for(name.len()) {
            return false;
        }
        self.insert(self.n, name, is_directory);
        true
    }

    /// Put `name` at index `at`, moving the names from `at` on up by one. The caller has checked
    /// [`NameSet::has_room_for`].
    fn insert(&mut self, at: usize, name: &[u8], is_directory: bool) {
        let from = self.start(at);
        let used = self.name_bytes();
        let len = name.len();
        self.bytes.copy_within(from..used, from + len);
        self.bytes[from..from + len].copy_from_slice(name);
        let mut i = self.n;
        while i > at {
            self.ends[i] = self.ends[i - 1] + len as u16;
            self.is_directory[i] = self.is_directory[i - 1];
            i -= 1;
        }
        self.ends[at] = (from + len) as u16;
        self.is_directory[at] = is_directory;
        self.n += 1;
    }

    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    pub fn contains(&self, name: &[u8]) -> bool {
        self.iter().any(|(n, _)| n == name)
    }

    /// **Keep the smallest names offered that fit**, in ascending byte order, which is how one
    /// [`Batch`] is selected out of a match too large to hand over at once.
    ///
    /// Returns whether nothing was left out: `false` when `name` itself, or names it displaced, did
    /// not fit. The name goes in when it sorts below a name held, displacing the largest ones until
    /// the set fits [`MAX_NAMES`] and [`NAME_BYTES`] again, so what is left out always sorts after
    /// what is kept. The caller then refuses every later name after the set's last
    /// ([`Expander::offer`]), or a short name could slip in behind a long one that was left out and
    /// the next batch would skip it. It takes a name the caller has established can travel in a
    /// grant.
    fn keep_smallest(&mut self, name: &[u8], is_directory: bool) -> bool {
        // Where it belongs: the first slot holding a name that sorts after it.
        let at = (0..self.n)
            .find(|&i| self.name(i) > name)
            .unwrap_or(self.n);
        let mut kept_all = true;
        while !self.has_room_for(name.len()) && self.n > at {
            self.n -= 1;
            kept_all = false;
        }
        if !self.has_room_for(name.len()) {
            // Everything after it is gone and it still does not fit: it belongs to a later batch.
            return false;
        }
        self.insert(at, name, is_directory);
        kept_all
    }

    /// The **last** name in the set, which is a batch's watermark: the exclusive lower bound the
    /// next batch resumes from. `None` for an empty set, which is a sweep that is over.
    pub fn last(&self) -> Option<&[u8]> {
        (self.n > 0).then(|| self.name(self.n - 1))
    }

    /// `(name, is_directory)` per name, in the order the directory yielded them, or in **byte order** for
    /// a set [`Expander::batch`] selected.
    ///
    /// The two differ because a batch's last name is its successor's boundary, so batching needs a
    /// total order and one expansion needs none. Sorting the unbatched case too would be a change to
    /// what `echo *.txt` prints, bought for nothing.
    pub fn iter(&self) -> impl Iterator<Item = (&[u8], bool)> {
        (0..self.n).map(|i| (self.name(i), self.is_directory[i]))
    }

    /// The one name, when the set holds exactly one. A grant that must designate a single thing (a
    /// per-file grant) asks this and refuses when the answer is `None`.
    pub fn only(&self) -> Option<&[u8]> {
        (self.n == 1).then(|| self.name(0))
    }
}

/// **The one expander both `echo` and the grant planner drive.**
///
/// The caller enumerates a directory (which needs `ENUMERATE` on it, and nothing more) and offers
/// every entry; this decides membership and refuses at the two edges. Keeping it a fed collector
/// rather than a function over a slice is what lets the shell read a directory in rounds out of one
/// reused page without ever holding the whole listing.
pub struct Expander<'a> {
    pattern: &'a [u8],
    set: NameSet,
    /// A matched name the set could not take. Kept as a refusal rather than dropped, because the
    /// two ways it happens are the two ways a grant could silently become a prefix of what the
    /// command matched.
    refused: Option<Refusal>,
    /// Where this batch resumes from, or `None` for the unbatched expansion that refuses at the
    /// bound. See [`Expander::batch`].
    resume: Option<Resume>,
    /// A match beyond this batch was seen. Only ever set in batching mode, where being over the
    /// bound is a fact to report rather than a refusal. Once it is, a match after the set's last
    /// name is left out too, so the batch stays a prefix of the ordering
    /// ([`NameSet::keep_smallest`]).
    more: bool,
}

/// **Where a batch starts**: at the beginning, or immediately after the last name of the batch
/// before it.
///
/// The resume point is a **name**, not a directory cursor, which `rm` taking names away would
/// shift. A name is stable under removal and insertion, so "the smallest matches strictly greater
/// than this" terminates whatever the command does to what it was handed, and the shell, whose
/// heap is capped at 32 KiB, never materializes a whole match. See notes/glob-grant/xargs.md ("The
/// resume point is a name, not a cursor").
///
/// [`Resume::After`] holds a whole [`Name`], up to 255 bytes, and a `Box` is not an option in a
/// crate with no allocator; there is one of these per sweep.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Resume {
    #[default]
    Start,
    After(Name),
}

/// **One batch of a match too large to hand over at once**: the names this invocation designates,
/// and whether the pattern still matches something beyond them.
///
/// `more` is what makes a sweep terminate without anyone counting: the shell asks for a batch, runs
/// it, and asks again from [`NameSet::last`] until a batch says there is nothing after it. Nobody
/// ever holds the whole match, which is the point rather than an accident of the implementation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Batch {
    pub names: NameSet,
    pub more: bool,
}

impl<'a> Expander<'a> {
    /// Expand `pattern`, which is one component: it is matched against a name, never against a path.
    pub fn new(pattern: &'a [u8]) -> Self {
        Expander {
            pattern,
            set: NameSet::empty(),
            refused: None,
            resume: None,
            more: false,
        }
    }

    /// **One batch of an expansion, resumed from a name** (milestone 109).
    ///
    /// The same membership decision as [`Expander::new`] and a different collection policy: instead
    /// of refusing the ninth match, this takes the smallest matches strictly after `resume` that fit
    /// [`MAX_NAMES`] and [`NAME_BYTES`], and reports that the rest exist. Membership is decided once, in [`Expander::offer`],
    /// so a batched line and an unbatched one cannot disagree about what a pattern designates.
    ///
    /// The policies differ on purpose: an unbatched line refuses a ninth match, and a batched line
    /// exists for it.
    pub fn batch(pattern: &'a [u8], resume: Resume) -> Self {
        Expander {
            pattern,
            set: NameSet::empty(),
            refused: None,
            resume: Some(resume),
            more: false,
        }
    }

    /// Offer one directory entry.
    ///
    /// The dot rule is `glob`'s default (`Dot::Special`), so `*` does not match `.config`, and that
    /// default is not a compatibility habit here: `rm *` hands a child authority over everything it
    /// matched, so the default reading is the one that **grants less**. A user who wants the
    /// dotfiles asks for them and takes the larger grant deliberately.
    pub fn offer(&mut self, name: &[u8], is_directory: bool) {
        if !glob::matches(self.pattern, name) || self.set.contains(name) {
            return;
        }
        if !crate::file_name_fits(name) {
            // It matched and cannot travel in a grant, so refuse here, before either collector: a
            // batch may not drop it either, which would be the silent prefix this exists to refuse.
            self.refused.get_or_insert(Refusal::MatchNotNameable);
            return;
        }
        let Some(resume) = self.resume else {
            if !self.set.push(name, is_directory) {
                self.refused.get_or_insert(Refusal::TooManyNames);
            }
            return;
        };
        // A batch takes what is strictly after its watermark, so a name an earlier batch already
        // designated is not offered to this one however the directory has been rearranged since.
        if let Resume::After(w) = resume
            && name <= w.as_bytes()
        {
            return;
        }
        // Once a match was left out, everything this batch has left out sorts after its last name,
        // and so must every later match it leaves out, or the next batch, which resumes after that
        // last name, would skip one. A match below the last name may still displace it.
        if self.more
            && self.set.last().is_some_and(|last| name > last)
        {
            return;
        }
        if !self.set.keep_smallest(name, is_directory) {
            self.more = true;
        }
    }

    /// The set, or the refusal to print.
    ///
    /// **A pattern that matched nothing is a refusal**, zsh's default rather than bash's. Nothing
    /// refuses `*` in a name, so passing the pattern through would build a grant for a name nobody
    /// has, which a later file called `*.rs` would make live; and an empty expansion is an empty
    /// grant. See notes/glob-grant.md ("A pattern that matched nothing").
    pub fn finish(self) -> Result<NameSet, Refusal> {
        debug_assert!(
            self.resume.is_none(),
            "a batched expander finishes as a Batch"
        );
        if let Some(r) = self.refused {
            return Err(r);
        }
        if self.set.is_empty() {
            return Err(Refusal::NoMatch);
        }
        Ok(self.set)
    }

    /// The batch, for an expander built by [`Expander::batch`].
    ///
    /// **An empty batch is not [`Refusal::NoMatch`] when it is a continuation.** The first batch of
    /// a sweep answers for the whole pattern, so a pattern matching nothing is refused there exactly
    /// as an unbatched one is; a later batch coming back empty means the sweep is over, which is a
    /// normal ending rather than a line that designated nothing. Getting this backwards would print
    /// "no name here matches that pattern" at the *end* of a successful sweep.
    pub fn finish_batch(self) -> Result<Batch, Refusal> {
        debug_assert!(
            self.resume.is_some(),
            "an unbatched expander finishes as a set"
        );
        if let Some(r) = self.refused {
            return Err(r);
        }
        if self.set.is_empty() && self.resume == Some(Resume::Start) {
            return Err(Refusal::NoMatch);
        }
        Ok(Batch {
            names: self.set,
            more: self.more,
        })
    }
}

impl Batch {
    /// Where the batch after this one resumes, or `None` when this was the last.
    ///
    /// Both halves matter: a batch with nothing after it ends the sweep, and so does an empty batch,
    /// because there is no watermark to carry forward.
    pub fn next(&self) -> Option<Resume> {
        if !self.more {
            return None;
        }
        let last = self.names.last()?;
        Name::new(last).map(Resume::After)
    }
}

/// **Whether a token's last component is a pattern**, refusing one anywhere earlier.
///
/// `logs/*.txt` is a pattern over the names in `logs`; `*/report.txt` is not something this system
/// can answer, and the reason is notes/glob.md's rather than a limitation of the matcher:
/// descending into a directory means **holding a capability for it**, so a pattern that selects
/// directories to walk is asking an authority question, and hiding one inside a string matcher is
/// the exact mistake this OS exists to not make.
///
/// A token ending in `..` designates a directory rather than a name, so there is no pattern in it.
pub fn magic_component(token: &[u8]) -> Result<bool, Refusal> {
    let parsed = nav::path(token).map_err(crate::nav_refusal)?;
    let steps = parsed.steps();
    let Some((last, lead)) = steps.split_last() else {
        return Ok(false);
    };
    for step in lead {
        if let Step::Down(name) = step
            && glob::has_magic(name)
        {
            return Err(Refusal::PatternInPath);
        }
    }
    Ok(match last {
        Step::Down(name) => glob::has_magic(name),
        Step::Up => false,
    })
}

/// **Which of a line's words name something here, for a program that hears words** (milestone
/// 205, DECISIONS §170 (how a foreign program is told what to do) clauses 2 to 5). The shell
/// builds it before it plans, as it builds an [`Expansion`], because only the shell can ask the
/// directory; the planner turns it into one [`crate::DirectoryGrant`] at the current directory.
///
/// **Names**: each word whose first component names an entry here, and for a program that may
/// create ([`crate::WordGrant::Create`]) each word that names nothing yet. **Here**: a word that is
/// `.` (calef's N1 ruling, 2026-09-27T06:27Z: "to search here you type `rg pattern .`"). Neither:
/// the line designated nothing and grants nothing.
///
/// Name: provisional (2026-09-27).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Designation {
    names: NameSet,
    here: bool,
}

impl Designation {
    /// Nothing designated.
    pub const fn none() -> Self {
        Designation {
            names: NameSet::empty(),
            here: false,
        }
    }

    /// Add a name found here, or one the program may create. A full set is
    /// [`Refusal::TooManyNames`], and a name that cannot travel in a grant
    /// [`Refusal::MatchNotNameable`], rather than a smaller grant than the line said.
    pub fn name(&mut self, name: &[u8], is_directory: bool) -> Result<(), Refusal> {
        if self.names.contains(name) {
            return Ok(());
        }
        if Name::new(name).is_none() {
            return Err(Refusal::MatchNotNameable);
        }
        if self.names.push(name, is_directory) {
            Ok(())
        } else {
            Err(Refusal::TooManyNames)
        }
    }

    /// A word named the current directory itself.
    pub fn here(&mut self) {
        self.here = true;
    }

    /// The names designated, if the directory itself was not.
    pub fn names(&self) -> &NameSet {
        &self.names
    }

    /// Whether the current directory itself was designated, which subsumes every name.
    pub fn is_here(&self) -> bool {
        self.here
    }

    /// Whether the line designated nothing.
    pub fn is_empty(&self) -> bool {
        !self.here && self.names.is_empty()
    }
}

/// **What the shell expanded, handed to the planner.**
///
/// The shell expands *before* it plans, which is also what Unix does, so there is no divergence to
/// earn here. The structural consequence is that [`crate::plan`] must see the **set** rather than
/// the pattern, since the endowment is the set; this is that argument, and the positional index is
/// carried so the placement stays explicit rather than being inferred from which slot happens to
/// want a name.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Expansion {
    at: Option<usize>,
    names: NameSet,
    /// `Some(here)` when this is a [`Designation`] rather than an expansion, whose names are
    /// `names`. One `NameSet` shared by both rather than a second one beside it, because an
    /// `Expansion` is copied through the shell's deepest frames by value, and a second set there
    /// overflowed its stack at `package install` (CI, 2026-09-27).
    words: Option<bool>,
}

impl Expansion {
    /// Nothing on the line was a pattern, so every token stands for itself.
    pub const fn none() -> Self {
        Expansion {
            at: None,
            names: NameSet::empty(),
            words: None,
        }
    }

    /// **What a line's words designate, for a program that hears words** (milestone 205 (how a
    /// foreign program is told what to do)). No
    /// positional is expanded on such a line, so this and [`Expansion::at`] never meet.
    pub const fn designated(words: Designation) -> Self {
        Expansion {
            at: None,
            names: words.names,
            words: Some(words.here),
        }
    }

    /// The designation, if the shell resolved the line's words.
    pub fn designation(&self) -> Option<Designation> {
        self.words.map(|here| Designation {
            names: self.names,
            here,
        })
    }

    /// The positional at `index` was a pattern, and these are the names it matched.
    pub fn at(index: usize, names: NameSet) -> Self {
        Expansion {
            at: Some(index),
            names,
            words: None,
        }
    }

    /// The names for the positional at `index`, if that is the one the shell expanded.
    pub fn for_positional(&self, index: usize) -> Option<NameSet> {
        (self.at == Some(index)).then_some(self.names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A name one byte longer than a grant carries.
    const TOO_LONG: [u8; MAX_NAME + 1] = [b'x'; MAX_NAME + 1];

    /// Drive the expander over a listing, the way both `echo` and the planner do.
    fn expand(pattern: &[u8], listing: &[(&[u8], bool)]) -> Result<NameSet, Refusal> {
        let mut e = Expander::new(pattern);
        for &(name, is_directory) in listing {
            e.offer(name, is_directory);
        }
        e.finish()
    }

    const DIRECTORY_LISTING: [(&[u8], bool); 4] = [
        (b"one.txt", false),
        (b"two.txt", false),
        (b"three.log", false),
        (b"logs", true),
    ];

    /// The base case, and the one every claim below is a variation on: the set is what matched, in
    /// listing order, and the entry type rides along because the caretaker serves the listing.
    #[test]
    fn the_set_is_what_matched_and_nothing_else() {
        let set = expand(b"*.txt", &DIRECTORY_LISTING).unwrap();
        assert_eq!(set.len(), 2);
        assert!(set.contains(b"one.txt") && set.contains(b"two.txt"));
        assert!(
            !set.contains(b"three.log"),
            "a name in the same directory that did not match is not in the grant",
        );
        assert_eq!(set.only(), None, "two names is not one name");

        let dirs = expand(b"log*", &DIRECTORY_LISTING).unwrap();
        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs.only(), Some(&b"logs"[..]));
        assert_eq!(dirs.iter().next(), Some((&b"logs"[..], true)));
    }

    /// **A pattern that matched nothing is a refusal.** zsh's default, not bash's, and the reason is
    /// in [`Expander::finish`]: the expansion is the grant, so an empty expansion is an empty grant.
    #[test]
    fn a_pattern_that_matched_nothing_refuses_rather_than_passing_itself_through() {
        assert_eq!(expand(b"*.rs", &DIRECTORY_LISTING), Err(Refusal::NoMatch));
        assert!(!Refusal::NoMatch.message().contains("denied"));
        // A pattern is a well-formed component, so bash's pass-through would grant a name nobody
        // has yet.
        assert!(
            crate::file_name_fits(b"*.rs"),
            "if this ever becomes false, the refusal above is still right for the other reason",
        );
    }

    /// **The bound is loud.** Seventeen matches is a refusal with nothing granted, never sixteen
    /// names quietly handed over.
    #[test]
    fn more_matches_than_the_bound_is_a_refusal_and_not_a_truncation() {
        let mut names = [[b'a'; 3]; MAX_NAMES + 1];
        for (i, n) in names.iter_mut().enumerate() {
            n[1] = b'a' + (i / 10) as u8;
            n[2] = b'0' + (i % 10) as u8;
        }
        let listing: [(&[u8], bool); MAX_NAMES + 1] =
            core::array::from_fn(|i| (&names[i][..], false));
        assert_eq!(expand(b"a*", &listing), Err(Refusal::TooManyNames));
        // Exactly the bound is fine: this is a ceiling that is met, not one with slack in it.
        assert_eq!(
            expand(b"a*", &listing[..MAX_NAMES]).unwrap().len(),
            MAX_NAMES
        );
    }

    /// A name that matched and cannot travel in a grant is a refusal too, for the same reason: the
    /// grant would otherwise be a prefix of what the command matched.
    #[test]
    fn a_matched_name_too_long_to_grant_refuses_the_whole_expansion() {
        let listing: [(&[u8], bool); 2] = [(b"one.txt", false), (&TOO_LONG, false)];
        assert_eq!(expand(b"*", &listing), Err(Refusal::MatchNotNameable));
    }

    /// The dot rule, which is a policy about **how much a default grants** rather than a matching
    /// fact: `rm *` must not reach a dotfile nobody typed.
    #[test]
    fn a_bare_star_does_not_reach_a_dotfile() {
        let listing: [(&[u8], bool); 2] = [(b".config", false), (b"visible", false)];
        let set = expand(b"*", &listing).unwrap();
        assert_eq!(set.only(), Some(&b"visible"[..]));
        assert_eq!(
            expand(b".*", &listing).unwrap().only(),
            Some(&b".config"[..]),
            "asking for them by name is how you take the larger grant",
        );
    }

    /// A pattern lives in the **last** component or nowhere, because selecting directories to walk
    /// is an authority question and not a matching one.
    #[test]
    fn a_pattern_may_only_be_the_last_component() {
        assert_eq!(magic_component(b"*.txt"), Ok(true));
        assert_eq!(magic_component(b"logs/*.txt"), Ok(true));
        assert_eq!(magic_component(b"report.txt"), Ok(false));
        assert_eq!(magic_component(b"logs/report.txt"), Ok(false));
        assert_eq!(
            magic_component(b"*/report.txt"),
            Err(Refusal::PatternInPath)
        );
        assert_eq!(magic_component(b"a/*/b"), Err(Refusal::PatternInPath));
        // The refusals a path already has still come first: the token is parsed before anything
        // asks whether it has magic in it.
        assert_eq!(
            magic_component(b"/*/report.txt"),
            Err(Refusal::PatternInPath)
        );
        // An absolute pattern is an ordinary pattern, expanded against the directory its leading
        // path names, which for a leading `/` is the holder's own root.
        assert_eq!(magic_component(b"/etc/*"), Ok(true));
    }

    /// An expansion is placed at the positional it came from, and nowhere else.
    #[test]
    fn an_expansion_belongs_to_one_positional() {
        let set = NameSet::one(b"x", false).unwrap();
        let e = Expansion::at(1, set);
        assert_eq!(e.for_positional(0), None);
        assert_eq!(e.for_positional(1).unwrap().only(), Some(&b"x"[..]));
        assert_eq!(Expansion::none().for_positional(0), None);
    }

    /// The hand-written `Debug` impls exist so a failing assertion prints names instead of zero
    /// padding, so they are pinned exactly, like a message: a `Name` prints as the name, a
    /// `NameSet` as the list. Captured into a buffer because this crate has no allocator, in
    /// tests or out.
    #[test]
    fn names_and_sets_debug_as_what_they_hold() {
        use core::fmt::Write;
        struct Buf {
            bytes: [u8; 64],
            n: usize,
        }
        impl Write for Buf {
            fn write_str(&mut self, s: &str) -> core::fmt::Result {
                let b = s.as_bytes();
                self.bytes[self.n..self.n + b.len()].copy_from_slice(b);
                self.n += b.len();
                Ok(())
            }
        }
        let assert_debug = |v: &dyn core::fmt::Debug, want: &str| {
            let mut buf = Buf {
                bytes: [0; 64],
                n: 0,
            };
            write!(buf, "{v:?}").unwrap();
            assert_eq!(core::str::from_utf8(&buf.bytes[..buf.n]), Ok(want));
        };
        assert_debug(&Name::new(b"a.txt").unwrap(), "\"a.txt\"");
        // The set renders its names as byte lists, not through `Name`'s impl: `iter` yields
        // `&[u8]`, and a slice debugs as numbers. Pinned as it is; if the set is ever taught to
        // print `["a.txt", "logs"]`, this is the test that says so on purpose.
        let set = expand(b"*", &[(b"a.txt", false), (b"logs", true)]).unwrap();
        assert_debug(&set, "[[97, 46, 116, 120, 116], [108, 111, 103, 115]]");
    }

    // ---- batching at the bound (milestone 109) ----

    /// **Twenty names in one directory, listed in an order that is deliberately not sorted.** A
    /// batch that took the first eight the directory yielded would answer differently from one that
    /// took the eight smallest, which is what makes the ordering claims below claims.
    const TWENTY: [&[u8]; 20] = [
        b"b13.txt", b"b06.txt", b"b15.txt", b"b16.txt", b"b17.txt", b"b18.txt", b"b19.txt",
        b"b00.txt", b"b01.txt", b"b02.txt", b"b03.txt", b"b04.txt", b"b05.txt", b"b14.txt",
        b"b07.txt", b"b08.txt", b"b09.txt", b"b10.txt", b"b11.txt", b"b12.txt",
    ];

    /// Which of [`TWENTY`] this is, as a bit, for the seen-mask the sweeps keep. This crate has no
    /// allocator, in tests or out, so "every name exactly once" is twenty bits rather than a sorted
    /// vector.
    fn which(name: &[u8]) -> u32 {
        let i = TWENTY
            .iter()
            .position(|n| *n == name)
            .expect("a batch named something that is not in the directory");
        1 << i
    }

    /// One batch over a listing, the way the shell's sweep drives it.
    fn batch(pattern: &[u8], resume: Resume, listing: &[(&[u8], bool)]) -> Result<Batch, Refusal> {
        let mut e = Expander::batch(pattern, resume);
        for &(name, is_directory) in listing {
            e.offer(name, is_directory);
        }
        e.finish_batch()
    }

    /// [`TWENTY`] as a listing.
    fn twenty() -> [(&'static [u8], bool); 20] {
        core::array::from_fn(|i| (TWENTY[i], false))
    }

    /// **The headline: a match too large to hand over is batched instead of refused**, and the
    /// batches partition the match exactly. No name is designated twice and none is dropped, which
    /// is the property [`Refusal::TooManyNames`] was protecting by refusing outright.
    #[test]
    fn batches_partition_the_match_with_no_name_taken_twice_or_dropped() {
        let listing = twenty();
        assert_eq!(
            expand(b"b*.txt", &listing),
            Err(Refusal::TooManyNames),
            "unbatched, this is the refusal milestone 109 exists to answer",
        );

        let mut seen = 0u32;
        let mut sizes = [0usize; 8];
        let mut batches = 0usize;
        let mut resume = Resume::Start;
        // Bounded so a resume rule that failed to advance fails this test instead of hanging it.
        for _ in 0..8 {
            let b = batch(b"b*.txt", resume, &listing).expect("a batch of a matching pattern");
            sizes[batches] = b.names.len();
            batches += 1;
            for (name, _) in b.names.iter() {
                let bit = which(name);
                assert_eq!(seen & bit, 0, "a name appeared in two batches");
                seen |= bit;
            }
            match b.next() {
                Some(r) => resume = r,
                None => break,
            }
        }
        assert_eq!(batches, 3, "twenty names is eight, eight and four");
        assert_eq!(sizes[..3], [MAX_NAMES, MAX_NAMES, 4]);
        assert_eq!(seen, (1 << 20) - 1, "the batches are not the match");
    }

    /// A batch is the smallest names **in order**, not the first ones the directory yielded, and
    /// that is what makes its last name a boundary rather than a guess.
    #[test]
    fn a_batch_is_the_smallest_names_after_the_watermark_in_order() {
        let b = batch(b"b*.txt", Resume::Start, &twenty()).unwrap();
        let mut prev: &[u8] = b"";
        for (name, _) in b.names.iter() {
            assert!(
                name > prev,
                "a batch is ordered: {name:?} came after {prev:?}"
            );
            prev = name;
        }
        assert_eq!(b.names.iter().next().map(|(n, _)| n), Some(&b"b00.txt"[..]));
        assert_eq!(b.names.last(), Some(&b"b07.txt"[..]));

        // And the second batch continues the ordering rather than restarting it.
        let second = batch(b"b*.txt", b.next().unwrap(), &twenty()).unwrap();
        assert_eq!(
            second.names.iter().next().map(|(n, _)| n),
            Some(&b"b08.txt"[..]),
        );
        assert_eq!(second.names.last(), Some(&b"b15.txt"[..]));
    }

    /// **The watermark is exclusive**, so the name that ended one batch is not in the next. An
    /// inclusive bound would grant it twice, and for `rm` the second grant would name a file that
    /// the first batch has already removed.
    #[test]
    fn the_watermark_is_exclusive() {
        let listing: [(&[u8], bool); 3] = [(b"a", false), (b"b", false), (b"c", false)];
        let b = batch(b"*", Resume::After(Name::new(b"a").unwrap()), &listing).unwrap();
        assert_eq!(b.names.len(), 2);
        assert!(!b.names.contains(b"a") && b.names.contains(b"b"));
        assert!(
            !b.more,
            "three names is one batch even resumed from the first"
        );
    }

    /// **`more` is a fact about what is left, not about how full the batch is.** Exactly a batch's
    /// worth of matches ends the sweep in one round; one more starts a second.
    #[test]
    fn more_says_whether_a_batch_follows() {
        let listing = twenty();
        assert!(
            !batch(b"b*.txt", Resume::Start, &listing[..MAX_NAMES])
                .unwrap()
                .more
        );
        let over = batch(b"b*.txt", Resume::Start, &listing[..MAX_NAMES + 1]).unwrap();
        assert!(over.more);
        assert_eq!(over.names.len(), MAX_NAMES);
        assert_eq!(
            over.next(),
            Name::new(over.names.last().unwrap()).map(Resume::After),
        );
    }

    /// **The case a directory cursor would get wrong**, and the whole reason [`Resume`] is a name.
    ///
    /// A batched `rm` removes what each batch designated, so the directory the next batch enumerates
    /// is missing every name granted so far and every later entry has shifted. Resuming by name
    /// sweeps it exactly once; resuming by position would skip or repeat.
    #[test]
    fn a_sweep_of_a_shrinking_directory_takes_every_name_once() {
        let mut live: [Option<&[u8]>; 20] = core::array::from_fn(|i| Some(TWENTY[i]));
        let mut seen = 0u32;
        let mut resume = Resume::Start;
        for _ in 0..8 {
            let mut e = Expander::batch(b"b*.txt", resume);
            for name in live.iter().flatten() {
                e.offer(name, false);
            }
            let b = e.finish_batch().unwrap();
            for (name, _) in b.names.iter() {
                let bit = which(name);
                assert_eq!(
                    seen & bit,
                    0,
                    "a shrinking directory handed a name over twice"
                );
                seen |= bit;
                // The command runs: every name it was granted is gone before the next enumeration.
                live[bit.trailing_zeros() as usize] = None;
            }
            match b.next() {
                Some(r) => resume = r,
                None => break,
            }
        }
        assert_eq!(seen, (1 << 20) - 1, "the sweep left names behind");
        assert!(live.iter().all(Option::is_none));
    }

    /// **An empty first batch is still `NoMatch`; an empty later one is the end.** Backwards, this
    /// prints "no name here matches that pattern" at the end of a sweep that worked.
    #[test]
    fn an_empty_batch_means_the_end_only_when_it_is_a_continuation() {
        let listing: [(&[u8], bool); 1] = [(b"a.txt", false)];
        assert_eq!(
            batch(b"*.rs", Resume::Start, &listing),
            Err(Refusal::NoMatch)
        );
        let done = batch(b"*", Resume::After(Name::new(b"a.txt").unwrap()), &listing).unwrap();
        assert!(done.names.is_empty() && !done.more);
        assert_eq!(done.next(), None);
    }

    /// **A set is bounded by its bytes as well as its count** (milestone 946): two of the longest
    /// names fit, and a third name of any length is the same refusal as a ninth.
    #[test]
    fn a_set_refuses_past_its_bytes_as_it_does_past_its_count() {
        let a = [b'a'; MAX_NAME];
        let b = [b'b'; MAX_NAME];
        let mut set = NameSet::empty();
        assert!(set.push(&a, false));
        assert!(set.push(&b, true));
        assert_eq!(set.name_bytes(), 2 * MAX_NAME);
        assert_eq!(NAME_BYTES - 2 * MAX_NAME, 2);
        assert!(!set.push(b"ccc", false), "one byte past NAME_BYTES");
        assert!(set.push(b"cc", false), "and two bytes fit exactly");
        assert_eq!(set.name_bytes(), NAME_BYTES);
        let got: [(&[u8], bool); 3] = [(&a, false), (&b, true), (b"cc", false)];
        assert!(set.iter().eq(got.iter().copied()), "{set:?}");
        let listing: [(&[u8], bool); 3] = [(&a, false), (&b, false), (b"ccc", false)];
        assert_eq!(expand(b"*", &listing), Err(Refusal::TooManyNames));
    }

    /// **A batch is a prefix of the ordering even when its bytes, not its count, end it.** With a
    /// long name left out for room, a short name sorting after it must be left out too, or the next
    /// batch would resume after the short one and the long one would never be designated by any
    /// batch. The listing is in an order that tempts exactly that.
    #[test]
    fn a_batch_cut_by_bytes_is_still_a_prefix_of_the_ordering() {
        let a = [b'a'; MAX_NAME];
        let b = [b'b'; MAX_NAME];
        let c = [b'c'; MAX_NAME];
        let listing: [(&[u8], bool); 4] = [(&c, false), (&a, false), (&b, false), (b"d", false)];
        let first = batch(b"*", Resume::Start, &listing).unwrap();
        let want: [(&[u8], bool); 2] = [(&a, false), (&b, false)];
        assert!(first.names.iter().eq(want.iter().copied()), "{:?}", first.names);
        assert!(first.more);
        let second = batch(b"*", first.next().unwrap(), &listing).unwrap();
        let want: [(&[u8], bool); 2] = [(&c, false), (b"d", false)];
        assert!(second.names.iter().eq(want.iter().copied()), "{:?}", second.names);
        assert!(!second.more);
        // And a long name that displaces held ones for room still leaves out only names after
        // what is kept.
        let listing: [(&[u8], bool); 4] = [(b"b1", false), (&c, false), (&b, false), (b"b2", false)];
        let first = batch(b"*", Resume::Start, &listing).unwrap();
        let mut held = first.names.iter().map(|(n, _)| n);
        assert_eq!(held.next(), Some(&b"b1"[..]));
        assert_eq!(held.next(), Some(&b"b2"[..]));
        assert_eq!(held.next(), Some(&b[..]));
        assert_eq!(held.next(), None, "c belongs to the next batch");
    }

    /// **Batching does not soften the other refusal.** A matched name that cannot travel in a grant
    /// stops the sweep, because a batch missing a name the pattern matched is the silent prefix this
    /// mechanism exists to refuse. Batching splits an authority; it never shrinks one.
    #[test]
    fn a_batch_still_refuses_a_match_it_cannot_name() {
        let listing: [(&[u8], bool); 2] = [(b"one.txt", false), (&TOO_LONG, false)];
        assert_eq!(
            batch(b"*", Resume::Start, &listing),
            Err(Refusal::MatchNotNameable),
        );
    }

    /// Membership is decided in one place, so a batched line and an unbatched one designate the same
    /// names: the dot rule and the type bit hold identically under batching.
    #[test]
    fn a_batch_designates_what_an_expansion_would() {
        let listing: [(&[u8], bool); 3] = [(b".config", false), (b"visible", false), (b"d", true)];
        let b = batch(b"*", Resume::Start, &listing).unwrap();
        assert_eq!(
            b.names.len(),
            2,
            "the dot rule is the expander's, not the collector's",
        );
        assert!(!b.names.contains(b".config"));
        let mut it = b.names.iter();
        assert_eq!(it.next(), Some((&b"d"[..], true)));
        assert_eq!(it.next(), Some((&b"visible"[..], false)));
    }

    /// A listing that offers the same name twice (`READDIR` re-reads the directory per round, so a
    /// name added or removed mid-listing can be seen twice) must not put it in the grant twice.
    #[test]
    fn a_name_offered_twice_is_in_the_set_once() {
        let listing: [(&[u8], bool); 3] = [(b"a.txt", false), (b"a.txt", false), (b"b.txt", false)];
        assert_eq!(expand(b"*.txt", &listing).unwrap().len(), 2);
    }

    /// A batch keeps each name's entry type with the name when it slides a larger one up to make
    /// room, so a directory that sorts into the middle is still a directory.
    #[test]
    fn a_name_sorted_into_the_middle_keeps_its_own_entry_type() {
        let listing: [(&[u8], bool); 3] =
            [(b"b3.txt", true), (b"b1.txt", false), (b"b2.txt", true)];
        let b = batch(b"b*.txt", Resume::Start, &listing).unwrap();
        let mut it = b.names.iter();
        assert_eq!(it.next(), Some((&b"b1.txt"[..], false)));
        assert_eq!(it.next(), Some((&b"b2.txt"[..], true)));
        assert_eq!(it.next(), Some((&b"b3.txt"[..], true)));
    }

    #[test]
    fn an_empty_set_has_no_last_name_and_here_is_remembered() {
        assert_eq!(NameSet::empty().last(), None);
        let mut d = Designation::none();
        assert!(!d.is_here());
        d.here();
        assert!(d.is_here());
    }
}

/// List the names in it ([`super::fs::READDIR`]). Separable because enumeration is the right
/// globbing and tab completion consume, and "you may open the file I named" should not imply
/// "you may find out what else is in there".
pub const ENUMERATE: u64 = 1 << 0;
/// Open a name in it for reading, and read a file handle obtained through it.
pub const READ: u64 = 1 << 1;
/// Open a name in it for writing, and write or truncate a file handle obtained through it.
/// Separate from [`READ`] because milestone 47's motivating case is a directory a program may
/// append to and not read.
pub const WRITE: u64 = 1 << 2;
/// Make a new name in it ([`super::fs::CREATE`]).
pub const CREATE: u64 = 1 << 3;
/// Take a name out of it. This is the right the log-writing case exists to withhold: [`WRITE`]
/// and [`CREATE`] without [`REMOVE`] is "add to this, destroy nothing", which is milestone 47's
/// motivating sentence. Three verbs consult it: [`super::fs::RENAME`], whose source name goes
/// away, [`super::fs::UNLINK`], which is `rm`'s, and [`super::fs::RMDIR`], which takes an empty
/// directory's name out of its parent.
pub const REMOVE: u64 = 1 << 4;
/// Walk into a child directory ([`super::fs::OPENDIR`]).
///
/// **Separate from [`READ`] on purpose**, and this is the rung milestone 47 did not name. If
/// descending came with reading, then granting a directory would silently grant its whole
/// subtree, transitively and to any depth, and how much authority a grant carried would be
/// decided by the shape of the tree rather than by the grant. That is ambient authority
/// reintroduced by recursion, which is the thing this milestone exists to refuse.
pub const DESCEND: u64 = 1 << 5;
/// **Assert an arbitrary modification time** ([`super::fs::SETMTIME_AT`], `touch -t`; DECISIONS
/// §112). Every other mutating right bounds what the caller may do to the bytes or names in a
/// directory; this one bounds what it may claim about when it did. Plain `touch`
/// ([`super::fs::SETMTIME`]) needs only [`WRITE`], because the server observed that value. POSIX's
/// `utime()` draws the same line, and DECISIONS §43 (reading the clock is a page, setting it is a
/// page you may write) draws it between reading the wall clock and setting it. See notes/touch.md.
///
/// Name: provisional (milestone 47's mtime lane, 2026-08-24, DECISIONS §112 (`touch`'s two
/// behaviors need two rights)); an architect's to ratify. Candidates considered and set aside:
/// `BACKDATE` (the capability also permits a *future* value, so a name for lying only backward
/// would be narrower than what it grants), `RETIME` (read as "change the pace of something" as
/// readily as "reassign its recorded time"). `SETTIME` was chosen for reading as exactly what
/// it grants, at the cost of sitting one letter from [`super::fs::SETMTIME`]'s name; the two
/// are always written module-qualified (`dir::SETTIME` against `fs::SETMTIME`), the same way
/// [`CREATE`] already sits one identifier from [`super::fs::CREATE`] without confusion.
pub const SETTIME: u64 = 1 << 6;

/// Every right this contract defines. The mount binds its root with exactly this; nothing below
/// the root can ever be constructed with more.
pub const ALL: u64 = ENUMERATE | READ | WRITE | CREATE | REMOVE | DESCEND | SETTIME;

/// **What a recursive removal needs, at every level** (milestone 47's `rm -r`): [`ENUMERATE`] to
/// see what is there, [`DESCEND`] to walk into it, [`REMOVE`] to take names out. Named here
/// because the `rm` program asks for exactly this on every descent and the wiring that grants it
/// hands over exactly this, so the two cannot drift into a capability nobody meant.
///
/// **It carries no [`READ`] and no [`WRITE`]**, which is not an oversight but the interesting
/// part: a program handed this can destroy the subtree and cannot read one byte of it. On Unix
/// the same `rm` could read every file it is about to unlink, because the authority came from
/// the process rather than from the command.
pub const REMOVE_TREE: u64 = ENUMERATE | DESCEND | REMOVE;

/// `EROFS`: a mutating right the capability does not carry. The same number and the same
/// argument as [`super::grant::EROFS`].
pub const EROFS: i32 = 30;
/// `EPERM`: [`ENUMERATE`] withheld. The only refusal here that must be loud rather than
/// concealing, because concealment would mean lying about the directory.
pub const EPERM: i32 = 1;
/// `EISDIR`: [`super::fs::OPEN`] of a name that is a directory. [`super::fs::OPENDIR`] is the
/// verb, and answering with a useless file handle instead is how a caller ends up reading a
/// directory's raw bytes and believing them.
pub const EISDIR: i32 = 21;
/// `ENOTDIR`: [`super::fs::OPENDIR`] of a name that is a file, or a directory verb aimed at a
/// file handle. Same number and same argument as [`super::grant::ENOTDIR`].
pub const ENOTDIR: i32 = 20;
/// `EINVAL`: a name that is not a single component, and [`super::fs::RENAME`]'s refusal to move
/// a **directory** between two directories (the verb's own doc gives the cycle argument).
pub const EINVAL: i32 = 22;
/// `ENOTEMPTY`: [`super::fs::RENAME`] over a destination directory that still has entries in it,
/// and [`super::fs::RMDIR`] of one. On `RMDIR` it is the safety property rather than an
/// inconvenience: a verb that emptied what it found would put a whole subtree behind one call.
pub const ENOTEMPTY: i32 = 39;

/// **What a refusal from this contract means, in one sentence a shell can print.**
///
/// The errno a verb answers is part of this module's design rather than an implementation
/// detail, because it decides what the holder *learns* (see this module's header). So the
/// sentence that renders it belongs here too, next to the decision, instead of in each program
/// that has to explain itself to a human. Every line is a statement about **the capability or
/// the thing**, never about a policy: there is no policy here to have said yes.
///
/// `EACCES` is deliberately absent: nothing in this contract answers it (DECISIONS §27, §47).
pub const fn explain(errno: i32) -> &'static str {
    match errno {
        // ENOENT. Two very different situations, one sentence, and that is the point: a holder
        // that may not reach a name must not be able to tell "it is not there" from "you may
        // not name it". The sentence is true either way.
        2 => "no such name in this directory",
        EPERM => "this capability does not carry that right",
        EROFS => "through this capability that directory is read-only",
        EISDIR => "that is a directory",
        ENOTDIR => "that is not a directory",
        // EEXIST. Create is create, not create-or-open, so this is a fact about the directory.
        17 => "that name is already there",
        ENOTEMPTY => "that directory is not empty",
        // EBADF, EMFILE: facts about a handle rather than about a capability.
        9 => "no such handle",
        24 => "too many handles open at once",
        EINVAL => "that is not a name this contract can resolve",
        // The extended-attribute vocabulary, rendered here beside the rest for this function's
        // reason.
        super::xattr::ENODATA => "that attribute is not set",
        super::xattr::ERANGE => "that is not a name an attribute can have",
        super::xattr::E2BIG => "that attribute value is too large to store",
        super::xattr::ENOSPC => "that already holds as many attributes as it can",
        super::xattr::ENOTSUP => "this capability does not carry extended attributes",
        _ => "the filesystem refused",
    }
}

/// **A rights set on one directory or file handle.** Opaque on purpose: the only ways to make
/// one are [`Rights::root`], which the mount calls once for the directory the endpoint is bound
/// to, and [`Rights::attenuate`], which cannot widen.
///
/// # Examples
///
/// Attenuation is a bitwise AND, so no chain of calls widens a child past its parent.
///
/// ```
/// use filesystem_contract::dir::{self, Rights};
///
/// // What a shell holds over the user's home directory.
/// let home = Rights::root(dir::ALL);
///
/// // What it hands a log writer: somewhere to write, and no power to delete what is there.
/// let logs = home.attenuate(dir::READ | dir::WRITE | dir::CREATE);
/// assert!(logs.allows(dir::WRITE));
/// assert!(logs.denies_all(dir::REMOVE));
///
/// // The log writer passing its authority on cannot widen it back, however it asks.
/// let narrower = logs.attenuate(dir::ALL);
/// assert_eq!(narrower.bits(), logs.bits());
/// assert!(narrower.denies_all(dir::REMOVE));
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rights(u64);

impl Rights {
    /// The rights the endpoint's bound directory carries. This is the only place a rights set is
    /// made out of thin air, and the only caller is the code that binds a server to a directory.
    pub const fn root(mask: u64) -> Self {
        Rights(mask & ALL)
    }

    /// **The child's rights: what the parent has, intersected with what was asked for.**
    ///
    /// The whole of milestone 47's monotonicity property is this one `&`. It is a total
    /// function with no failure mode, which is the point: a caller cannot ask for a right the
    /// parent lacks and receive it, because there is no branch here to get wrong. The server
    /// separately *refuses* a request whose intersection came up short, so a caller is never
    /// silently given less than it asked for, but that refusal is about telling the truth and
    /// this line is about the property.
    pub const fn attenuate(self, requested: u64) -> Self {
        Rights(self.0 & requested)
    }

    /// Whether this set carries **every** right in `needed`. Written as "all of", not "any of",
    /// because an operation that needs two rights and is allowed by either is a hole.
    pub const fn allows(self, needed: u64) -> bool {
        self.0 & needed == needed
    }

    /// Whether it carries none of `any`. The negative form, for the "neither read nor write"
    /// test [`super::fs::OPEN`] makes.
    pub const fn denies_all(self, any: u64) -> bool {
        self.0 & any == 0
    }

    /// The raw mask, for reporting it (a test's verdict, or what `caps` would print). Not a way
    /// back into a [`Rights`]: there is no constructor that takes this.
    pub const fn bits(self) -> u64 {
        self.0
    }
}

/// Machine-checked, because "a child can never exceed its parent" is the claim this whole module
/// exists to make and a test can only try the masks somebody thought of.
#[cfg(kani)]
mod proofs {
    use super::*;

    /// **Attenuation never widens, for every parent and every request.** `allows` is the only
    /// question the server asks a rights set, so the property is stated in its terms: anything
    /// the child permits, the parent permitted.
    /// Falsification: replayable `crates/filesystem_contract/falsifications/dir.proofs.attenuate_never_widens.patch`
    #[kani::proof]
    fn attenuate_never_widens() {
        let parent = Rights(kani::any());
        let requested: u64 = kani::any();
        let needed: u64 = kani::any();
        let child = parent.attenuate(requested);
        // Raw bits beside the `allows` phrasing: a claim about subsets reads plainer as
        // `x & !y == 0` than as a double negation over a predicate.
        assert_eq!(
            child.bits() & !parent.bits(),
            0,
            "a child carries a bit the parent did not",
        );
        assert!(!child.allows(needed) || parent.allows(needed));
    }

    /// And it never widens **at any depth**, which is the property a tree walk depends on: two
    /// descents are still bounded by the root. `attenuate` is idempotent-shaped rather than
    /// merely monotone, and a proof is cheaper than trusting that AND is associative in code
    /// somebody may later rewrite.
    /// Falsification: replayable `crates/filesystem_contract/falsifications/dir.proofs.a_grandchild_is_bounded_by_the_root.patch`
    #[kani::proof]
    fn a_grandchild_is_bounded_by_the_root() {
        let root = Rights(kani::any());
        let a: u64 = kani::any();
        let b: u64 = kani::any();
        let needed: u64 = kani::any();
        let grandchild = root.attenuate(a).attenuate(b);
        assert_eq!(
            grandchild.bits() & !root.bits(),
            0,
            "a grandchild carries a bit the root did not",
        );
        assert!(!grandchild.allows(needed) || root.allows(needed));
    }

    /// A root is bounded by [`ALL`], so a caller that invents a mask with unknown bits set
    /// cannot smuggle one in and have some later version of this module give it a meaning.
    /// Falsification: replayable `crates/filesystem_contract/falsifications/dir.proofs.a_root_carries_nothing_undefined.patch`
    #[kani::proof]
    fn a_root_carries_nothing_undefined() {
        let mask: u64 = kani::any();
        assert!(Rights::root(mask).bits() & !ALL == 0);
    }
}

#[cfg(test)]
mod tests;

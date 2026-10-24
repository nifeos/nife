//! **Navigation in a system with no global namespace** (milestone 47's commands).
//!
//! The pure half of the `cd`, `pwd` and `ls` builtins, which spawn nothing and confer no new
//! authority: the shell rebinds a directory capability it already holds. A name resolves once, at
//! the prompt, into a value ([`crate::FileGrant`]), so a child has no cwd to inherit. `/` is the
//! root of *your* namespace ([`Path::is_from_root`]), `..` stops at it ([`Cwd::ascend`]), and `pwd`
//! renders relative to it ([`Cwd::render`]), so every shell has its own root. [`TwoRoots`] and
//! [`Bindings`] compose a second root and named positions. See notes/shell-navigation.md and
//! notes/grant-expression/grant-plan-crate.md ("Navigation").

/// The deepest a shell tracks below its own root.
///
/// A bound rather than a policy: the shell keeps one directory capability per level so `..` can pop
/// one, and it has no allocator, so the stack is an array. Eight is far past any tree a demonstrator
/// walks by hand, and going deeper is [`Refused::TooDeep`] rather than a silently truncated path.
pub const MAX_DEPTH: usize = 8;

/// The longest single component a path may have, which is [`crate::MAX_FILE_NAME`]: the last
/// component of a path can be a file, and a file may be named that long. A component the shell
/// descends through or stands in is held to [`MAX_DIRECTORY_NAME`] as well ([`Cwd::descend`]).
pub const MAX_NAME: usize = crate::MAX_FILE_NAME;

/// **The longest directory name a [`Cwd`] or a bound name holds**, which is [`crate::WORDS_NAME`]:
/// a directory grant names its directory in the caretaker's two `START` words, and a shell that
/// could `cd` into a directory it cannot grant would be able to reach a place it cannot talk about.
/// It is also what keeps a [`Cwd`] small, and the shell carries one by value in every
/// [`crate::Holdings`] and several in an [`crate::Endowment`].
///
/// Name: provisional (milestone 946 (a granted file name can be longer than sixteen bytes)).
pub const MAX_DIRECTORY_NAME: usize = crate::WORDS_NAME;

/// The bytes [`Cwd::render`] can need: a slash per level, plus each name, plus the leading slash a
/// root renders as on its own.
pub const RENDER_MAX: usize = 1 + MAX_DEPTH * (1 + MAX_DIRECTORY_NAME);

/// Why a token cannot be navigated. Each is a fact about the *name*, never about a permission,
/// which is the distinction the whole model rests on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refused {
    /// A component that is not a name: empty, longer than [`MAX_NAME`], or carrying a byte a name
    /// cannot carry.
    NotAName,
    /// More components than [`MAX_DEPTH`], or a descent that would take the shell past it.
    TooDeep,
    /// A directory to stand in, descend through or bind whose name is over
    /// [`MAX_DIRECTORY_NAME`]. A name that long is a file's, not a place this shell can be.
    ///
    /// Name: provisional (milestone 946).
    DirectoryNameTooLong,
    /// `..` at the root. The clamp, reported rather than silently ignored: at your root there is
    /// nothing above to name, and a shell that said nothing would leave a user believing they had
    /// moved.
    ///
    /// **An absolute path meets the same wall**, and that is the point of rooting one in your own
    /// namespace rather than in a global one: `/..` is refused here exactly as `..` at the root is,
    /// because both ask for the level above the only root there is.
    AtYourRoot,
}

impl Refused {
    /// The line the shell prints. In the capability model's voice, like [`crate::Refusal::message`]:
    /// each says what *is*, never what was denied.
    pub fn message(self) -> &'static str {
        match self {
            Refused::NotAName => "that is not a name: one component, at most 255 bytes",
            Refused::TooDeep => "too deep: this shell tracks at most 8 levels below its root",
            Refused::DirectoryNameTooLong => {
                "that directory's name is longer than this shell can stand in: at most 16 bytes"
            }
            Refused::AtYourRoot => "you are at your root; there is nothing above it to name",
        }
    }
}

/// One step of a parsed path. `.` produces no step at all, and `..` produces [`Step::Up`], which the
/// shell executes by popping a capability rather than by sending a name.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step<'a> {
    /// Descend into this component.
    Down(&'a [u8]),
    /// Go back to the parent, if there is one to go back to.
    Up,
}

/// A parsed path: a bounded sequence of [`Step`]s and where they start from, validated before
/// anything is sent.
///
/// Validation is up front and total, so `cd a/b/c` fails at the prompt when `c` is unnameable rather
/// than half way through, having already moved. The shell then walks the steps one at a time,
/// because **the FS contract takes a single component per request** and this is where the roadmap's
/// "the resolver lives in the client's runtime" is actually true: the server never sees a path, and
/// §27's rule that open-by-path exists only inside a server, relative to one bound directory, holds
/// exactly as it did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Path<'a> {
    steps: [Step<'a>; MAX_DEPTH],
    n: usize,
    from_root: bool,
}

impl<'a> Path<'a> {
    /// The steps, in order.
    pub fn steps(&self) -> &[Step<'a>] {
        &self.steps[..self.n]
    }

    /// **Whether the token began with `/`**, meaning it is resolved from the holder's own root
    /// rather than from where the holder is standing.
    ///
    /// This is the whole of what an absolute path is here, and it is a fact about the *token*
    /// rather than about any capability: the caller supplies the root, so a holder can only ever
    /// root a path in the one it has. There is no second namespace for this bit to select.
    pub fn is_from_root(&self) -> bool {
        self.from_root
    }

    /// The last component, and the steps that lead to the directory holding it.
    ///
    /// This is **resolve-at-grant-time** in one function: `wc logs/report.txt` is a grant of one
    /// file, and the shell has to know which directory to narrow before it can build the grant. A
    /// path whose last step is `..` designates a directory, not a file, so it has no answer here.
    pub fn split_last_component(&self) -> Option<(&[Step<'a>], &'a [u8])> {
        match self.steps().split_last() {
            Some((Step::Down(name), lead)) => Some((lead, name)),
            _ => None,
        }
    }
}

/// Parse a token into a [`Path`], with no IO and no capability consulted.
///
/// Empty components are skipped, so `a//b` and `deeper/` mean what they do everywhere else; that is
/// Unix's behaviour and there is no divergence to earn. A leading `/` sets [`Path::is_from_root`],
/// which is the biggest fact about the token and the only one the steps themselves cannot carry:
/// `/a` and `a` parse to the same single step and mean different places.
pub fn path(token: &[u8]) -> Result<Path<'_>, Refused> {
    let from_root = token.first() == Some(&b'/');
    if token.is_empty() {
        return Err(Refused::NotAName);
    }
    let mut steps = [Step::Up; MAX_DEPTH];
    let mut n = 0;
    for part in token.split(|&b| b == b'/') {
        if part.is_empty() || part == b"." {
            continue;
        }
        if n == MAX_DEPTH {
            return Err(Refused::TooDeep);
        }
        steps[n] = if part == b".." {
            Step::Up
        } else {
            if !component_fits(part) {
                return Err(Refused::NotAName);
            }
            Step::Down(part)
        };
        n += 1;
    }
    if n == 0 && !from_root {
        // `.` on its own: it names the place you already are, which no verb here needs and no verb
        // here can act on. Refusing it beats acting on a name the user did not type.
        //
        // **`/` on its own is not that case**, which is why the two are told apart here. It names
        // your root, which is a place you can go and a place you can list, and it is the one thing
        // `pwd` prints that a user would otherwise be unable to type back.
        return Err(Refused::NotAName);
    }
    Ok(Path {
        steps,
        n,
        from_root,
    })
}

/// Whether one component can be a name at all. The same rule the FS server enforces at its own
/// boundary (`check_component`), stated here so the shell refuses at the prompt rather than sending
/// a request it knows will be `EINVAL`.
pub fn component_fits(part: &[u8]) -> bool {
    !part.is_empty()
        && part.len() <= MAX_NAME
        && part != b"."
        && part != b".."
        && !part.contains(&b'/')
        && !part.contains(&b'\\')
        && !part.contains(&b':')
        && !part.contains(&0)
}

/// Whether one component can be a directory this shell stands in: [`component_fits`], and at most
/// [`MAX_DIRECTORY_NAME`] bytes.
///
/// Name: provisional (milestone 946).
pub fn directory_fits(part: &[u8]) -> bool {
    component_fits(part) && part.len() <= MAX_DIRECTORY_NAME
}

/// **Where a shell is, relative to its own root**: the components it descended through, and nothing
/// above them.
///
/// It is a value, not a handle, which is what makes [`crate::FileGrant`] able to carry one: a grant
/// planned at the prompt records the directory it was resolved against, and a later `cd` cannot
/// change what that grant means.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Cwd {
    names: [[u8; MAX_DIRECTORY_NAME]; MAX_DEPTH],
    lens: [u8; MAX_DEPTH],
    depth: usize,
}

/// Written as the path it means. The derived form is 137 bytes of mostly zeroes, which turns any
/// assertion failure that carries one (and [`crate::Endowment`] carries one) into something nobody
/// will read.
impl core::fmt::Debug for Cwd {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.depth == 0 {
            return f.write_str("/");
        }
        for level in 0..self.depth {
            f.write_str("/")?;
            match core::str::from_utf8(self.component(level)) {
                Ok(s) => f.write_str(s)?,
                Err(_) => write!(f, "{:?}", self.component(level))?,
            }
        }
        Ok(())
    }
}

impl Default for Cwd {
    fn default() -> Self {
        Cwd::root()
    }
}

impl Cwd {
    /// At the root of your namespace, which is where every shell starts and the only place a shell
    /// with no directory capability can be.
    pub const fn root() -> Self {
        Cwd {
            names: [[0; MAX_DIRECTORY_NAME]; MAX_DEPTH],
            lens: [0; MAX_DEPTH],
            depth: 0,
        }
    }

    /// How many levels below the root, 0 at the root.
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Whether this is the root of its namespace.
    pub fn is_root(&self) -> bool {
        self.depth == 0
    }

    /// Record a descent. `false` if the name does not fit, is longer than a directory's may be
    /// ([`MAX_DIRECTORY_NAME`]), or the shell is already as deep as it tracks; the caller must not
    /// have sent the request in that case.
    pub fn descend(&mut self, name: &[u8]) -> bool {
        if self.depth == MAX_DEPTH || !directory_fits(name) {
            return false;
        }
        self.names[self.depth][..name.len()].copy_from_slice(name);
        self.lens[self.depth] = name.len() as u8;
        self.depth += 1;
        true
    }

    /// Pop one level. **`false` at the root, which is the clamp**: there is nothing above it, so
    /// there is nothing to pop and no request to send.
    ///
    /// **The popped component is zeroed, not just forgotten**, so one place has one representation
    /// and the derived `PartialEq` means "the same position". Before milestone 154 (a process that
    /// holds two directory capabilities) it only decremented the depth, and `/logs` reached by
    /// `cd logs/x; cd ..` compared unequal to `/logs` reached directly, because the stale `x` was
    /// still in the array behind the depth.
    pub fn ascend(&mut self) -> bool {
        if self.depth == 0 {
            return false;
        }
        self.depth -= 1;
        self.names[self.depth] = [0; MAX_DIRECTORY_NAME];
        self.lens[self.depth] = 0;
        true
    }

    /// The component at `level` (0 is the first below the root).
    pub fn component(&self, level: usize) -> &[u8] {
        &self.names[level][..self.lens[level] as usize]
    }

    /// The last component, or `None` at the root. What `ls` prints as a heading and what a shell
    /// needs to name the directory it is standing in.
    pub fn last(&self) -> Option<&[u8]> {
        self.depth.checked_sub(1).map(|l| self.component(l))
    }

    /// Render as a path, **relative to this shell's root**: `/` at the root, `/a/b` below it. The
    /// leading slash is honest rather than borrowed: `/` is the root of *your* namespace, which is
    /// Plan 9's answer and the only one that does not imply a namespace above you.
    ///
    /// Returns the number of bytes written. `out` should be [`RENDER_MAX`] bytes; a shorter buffer
    /// truncates rather than panicking, because a shell rendering a prompt must not fault.
    pub fn render(&self, out: &mut [u8]) -> usize {
        let mut n = 0;
        let mut put = |b: u8, n: &mut usize| {
            if *n < out.len() {
                out[*n] = b;
                *n += 1;
            }
        };
        if self.depth == 0 {
            put(b'/', &mut n);
            return n;
        }
        for level in 0..self.depth {
            put(b'/', &mut n);
            for &b in self.component(level) {
                put(b, &mut n);
            }
        }
        n
    }

    /// Apply a sequence of steps, as [`path`] parsed them, **without sending anything**: this is the
    /// planning half, used to resolve a name at grant time. The shell's `cd` walks the same steps
    /// against real capabilities and keeps this in step with them.
    ///
    /// **All of it or none of it**: a step that cannot be taken leaves the position exactly where it
    /// was. `cd a/b` failing at `b` and leaving you in `a` is the shape of bug that makes the next
    /// command act somewhere the user did not think they were, and Unix's `chdir` does not do it
    /// either. The shell's `cd` unwinds the capabilities it opened for the same reason.
    pub fn apply(&mut self, steps: &[Step<'_>]) -> Result<(), Refused> {
        let mut next = *self;
        for step in steps {
            match step {
                Step::Up => {
                    if !next.ascend() {
                        return Err(Refused::AtYourRoot);
                    }
                }
                Step::Down(name) => {
                    if !component_fits(name) {
                        return Err(Refused::NotAName);
                    }
                    if !directory_fits(name) {
                        return Err(Refused::DirectoryNameTooLong);
                    }
                    if !next.descend(name) {
                        return Err(Refused::TooDeep);
                    }
                }
            }
        }
        *self = next;
        Ok(())
    }

    /// **Where a token leaves you**, which is [`apply`](Cwd::apply) with the one thing a token
    /// carries that a step sequence does not: whether it started at your root.
    ///
    /// Every caller that resolves a *token* uses this rather than `apply`, which would silently
    /// resolve a `/` path from wherever the holder stands. It returns a new position rather than
    /// moving, because a grant is planned against where a token lands while the shell stays put.
    pub fn resolve(&self, p: &Path<'_>) -> Result<Cwd, Refused> {
        let mut next = if p.is_from_root() { Cwd::root() } else { *self };
        next.apply(p.steps())?;
        Ok(next)
    }
}

/// **Two directory capabilities, composed under two labels** (milestone 154 (a process that holds
/// two directory capabilities)). Provisional name and shape, named in the milestone's own report
/// rather than ratified.
///
/// A label selects one grant's root by exact match on an absolute path's first component, and
/// nothing later re-selects. Past the label a path resolves as a single [`Cwd`] does, so `..` stops
/// at that grant's own root and `/a/../b` is refused. It is not `bind`'s ordered union. See
/// notes/grant-expression/grant-plan-crate.md ("Navigation").
pub struct TwoRoots<'a> {
    label_a: &'a [u8],
    label_b: &'a [u8],
}

/// Which of a [`TwoRoots`]' two grants an absolute path resolved into.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Which {
    /// The first grant, [`TwoRoots::new`]'s `label_a`.
    A,
    /// The second grant, [`TwoRoots::new`]'s `label_b`.
    B,
}

impl<'a> TwoRoots<'a> {
    /// Compose two labeled roots. The labels need not be distinct at this layer (a caller that
    /// built two identical labels gets `A` back for both, which is a wiring bug for the caller to
    /// have caught before this point, not a runtime refusal a request has to pay for).
    pub const fn new(label_a: &'a [u8], label_b: &'a [u8]) -> Self {
        TwoRoots { label_a, label_b }
    }

    /// Resolve one absolute token against this namespace: which grant it names, and where inside
    /// that grant's own root it lands, or [`Refused`] with the same reasons a single [`Cwd`]
    /// refuses.
    ///
    /// A relative token is [`Refused::NotAName`]: two roots give no reason to prefer either. So is a
    /// token whose first component names neither label, which covers `/` alone and a leading `..`.
    pub fn resolve(&self, token: &[u8]) -> Result<(Which, Cwd), Refused> {
        let p = path(token)?;
        if !p.is_from_root() {
            return Err(Refused::NotAName);
        }
        self.resolve_absolute(&p)
    }

    /// [`TwoRoots::resolve`]'s absolute-path half, split out so [`TwoRoots::resolve_from`] can
    /// reach it without reparsing the token a second time.
    fn resolve_absolute(&self, p: &Path<'_>) -> Result<(Which, Cwd), Refused> {
        self.try_resolve_absolute(p)
            .unwrap_or(Err(Refused::NotAName))
    }

    /// [`TwoRoots::resolve_absolute`], with the "no label matched" case told apart from a real
    /// refusal: `None` when the token's first component is not `Down` at all (a bare `/` or a
    /// leading `..`) or names neither label, `Some` once a label committed. `Holdings::anchor`
    /// (`crate::lib`) makes the same split for a two-grant shell, and falls through to the bind
    /// table only when no label matched, never when a label matched and applying the rest failed.
    fn try_resolve_absolute(&self, p: &Path<'_>) -> Option<Result<(Which, Cwd), Refused>> {
        let (label, rest) = match p.steps().split_first() {
            Some((Step::Down(name), rest)) => (*name, rest),
            _ => return None,
        };
        let which = if label == self.label_a {
            Which::A
        } else if label == self.label_b {
            Which::B
        } else {
            return None;
        };
        let mut cwd = Cwd::root();
        Some(cwd.apply(rest).map(|_| (which, cwd)))
    }

    /// **A two-grant shell's real, single, moving position** (DECISIONS §126,
    /// design/decisions/0126-two-directory-cwd.md), built on [`TwoRoots::resolve`] and
    /// [`Cwd::resolve`] rather than reimplementing either.
    ///
    /// A relative `token` resolves against `pos` in tree `which`, as for one grant; an absolute one
    /// picks the tree by label, which is how a holder moves between trees (`cd /b/somewhere`). `..`
    /// at either tree's own root is [`Refused::AtYourRoot`], [`Cwd::apply`]'s own clamp.
    pub fn resolve_from(
        &self,
        which: Which,
        pos: Cwd,
        token: &[u8],
    ) -> Result<(Which, Cwd), Refused> {
        let p = path(token)?;
        if p.is_from_root() {
            self.resolve_absolute(&p)
        } else {
            Ok((which, pos.resolve(&p)?))
        }
    }

    /// [`TwoRoots::resolve_from`], applied: move `(which, pos)` by `token`. All-or-nothing, the
    /// same rule [`Cwd::apply`] already has: a refused move leaves both untouched.
    pub fn apply_from(
        &self,
        which: &mut Which,
        pos: &mut Cwd,
        token: &[u8],
    ) -> Result<(), Refused> {
        let (w, p) = self.resolve_from(*which, *pos, token)?;
        *which = w;
        *pos = p;
        Ok(())
    }
}

/// How many names one [`Bindings`] can hold.
pub const MAX_BINDS: usize = 4;

/// One name a shell has bound to a position it already reached some other way.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BindEntry {
    name: [u8; MAX_DIRECTORY_NAME],
    name_len: u8,
    pos: Cwd,
}

impl BindEntry {
    /// The name an absolute path's first component must match to reach this entry.
    pub fn name(&self) -> &[u8] {
        &self.name[..self.name_len as usize]
    }

    /// Where this entry points, **before** anything after the bound name in a token is applied.
    pub fn pos(&self) -> Cwd {
        self.pos
    }
}

/// Why [`Bindings::add`] refused. Each variant is a fact about the *table or the name*, the same
/// discipline [`Refused`] states for a path: never a permission, always a shape or a slot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BindRefused {
    /// The token is not a single nameable component: empty, too long, or carrying `/`.
    NotAName,
    /// This name is already bound. `bind` does not silently replace an entry, because a rebind a
    /// user did not intend is exactly the kind of quiet authority change §42 refuses elsewhere;
    /// unbinding first is one more word and never an ambiguity.
    AlreadyBound,
    /// [`MAX_BINDS`] entries are already in use.
    TooMany,
    /// This name is the first component of the path to a mounted tree ([`crate::MountedTree`]).
    /// A bound name wins over the literal first component of an absolute path, so binding it
    /// would hide the mount point. Provisional name, kept from when a second tree had a label.
    ReservedLabel,
}

impl BindRefused {
    /// The line the shell prints, in [`Refused::message`]'s voice: what *is*, never what was
    /// denied.
    pub fn message(self) -> &'static str {
        match self {
            BindRefused::NotAName => "that is not a name: one component, at most 16 bytes",
            BindRefused::AlreadyBound => "that name is already bound; unbind it first",
            BindRefused::TooMany => "too many bound names: this shell tracks at most 4",
            BindRefused::ReservedLabel => "that name leads to where a tree is mounted",
        }
    }
}

/// A shell's bound names, `bind`'s namespace: [`MAX_BINDS`] slots, no allocator, exactly [`Cwd`]'s
/// own shape. A [`BindEntry`] is a value, a [`Cwd`] under a name, with no capability slot and no
/// lifetime, and it resolves through [`Cwd::apply`], so a bound name reaches nothing a direct walk
/// could not. One entry per name: a second bind of a name is refused, never shadowing. See
/// notes/grant-expression/grant-plan-crate.md ("Navigation").
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Bindings {
    entries: [Option<BindEntry>; MAX_BINDS],
}

impl Default for Bindings {
    fn default() -> Self {
        Bindings::none()
    }
}

impl Bindings {
    /// No names bound, `const` so a `Holdings` built at compile time (a host test's fixed
    /// fixtures) can name one without reaching for a non-`const` `Default`.
    pub const fn none() -> Self {
        Bindings {
            entries: [None; MAX_BINDS],
        }
    }

    /// Bind `name` to `pos`, a path in the one tree the shell presents. Refuses a name that cannot be a component, one already
    /// bound, or a full table; never overwrites.
    pub fn add(&mut self, name: &[u8], pos: Cwd) -> Result<(), BindRefused> {
        if !directory_fits(name) {
            return Err(BindRefused::NotAName);
        }
        if self.lookup(name).is_some() {
            return Err(BindRefused::AlreadyBound);
        }
        let slot = self
            .entries
            .iter_mut()
            .find(|e| e.is_none())
            .ok_or(BindRefused::TooMany)?;
        let mut packed = [0u8; MAX_DIRECTORY_NAME];
        packed[..name.len()].copy_from_slice(name);
        *slot = Some(BindEntry {
            name: packed,
            name_len: name.len() as u8,
            pos,
        });
        Ok(())
    }

    /// The entry bound to `name`, if any.
    pub fn lookup(&self, name: &[u8]) -> Option<&BindEntry> {
        self.entries.iter().flatten().find(|e| e.name() == name)
    }

    /// Every bound entry, in the order they were added. What `caps` needs to print a namespace
    /// section with more than the two grant rows in it.
    pub fn iter(&self) -> impl Iterator<Item = &BindEntry> {
        self.entries.iter().flatten()
    }

    /// Split an absolute path's first component against this table: `None` if it does not name a
    /// bound entry, so the caller can fall through to whatever "no bind matched" means for it.
    /// `Some` gives the entry's own stored position **unresolved further** and the steps after the
    /// label, the same split `TwoRoots`'s own absolute-path resolver makes for a grant label; a real
    /// wire walk needs both separately (open the bind's own chain first, then continue), where
    /// [`Bindings::resolve_absolute`] needs only their sum.
    pub fn split_absolute<'p>(&self, p: &'p Path<'p>) -> Option<(Cwd, &'p [Step<'p>])> {
        let (label, rest) = match p.steps().split_first() {
            Some((Step::Down(name), rest)) => (*name, rest),
            _ => return None,
        };
        let entry = self.lookup(label)?;
        Some((entry.pos(), rest))
    }

    /// [`Bindings::split_absolute`], fully resolved: the bound position with the rest of the token
    /// applied, or the [`Refused`] applying it produced (the same [`Refused::TooDeep`] /
    /// [`Refused::AtYourRoot`] a direct [`Cwd::apply`] would give, since that is exactly what runs
    /// underneath). `None` when the label matches no bound entry.
    pub fn resolve_absolute(&self, p: &Path<'_>) -> Option<Result<Cwd, Refused>> {
        let (base, rest) = self.split_absolute(p)?;
        let mut pos = base;
        Some(pos.apply(rest).map(|_| pos))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two routes to one position compare equal, which the derived `PartialEq` only promises if
    /// [`Cwd::ascend`] leaves no stale component behind (milestone 154 found that it did).
    #[test]
    fn two_routes_to_one_position_are_equal() {
        let mut direct = Cwd::root();
        assert!(direct.descend(b"logs"));
        let mut around = Cwd::root();
        assert!(around.descend(b"logs"));
        assert!(around.descend(b"longer-name"));
        assert!(around.ascend());
        assert_eq!(around, direct);
        assert!(around.ascend());
        assert!(around.descend(b"x"));
        let mut x = Cwd::root();
        assert!(x.descend(b"x"));
        assert_eq!(around, x, "a shorter name over a longer one leaves no tail");
    }

    /// Assert what `pwd` would print. A helper rather than a returned `String` because this crate is
    /// `no_std` and has no allocator, in tests or out.
    fn assert_pwd(cwd: &Cwd, want: &[u8]) {
        let mut buf = [0u8; RENDER_MAX];
        let n = cwd.render(&mut buf);
        assert_eq!(
            core::str::from_utf8(&buf[..n]),
            core::str::from_utf8(want),
            "pwd",
        );
    }

    /// Walk a token from a starting position, the way `cd` does.
    fn cd(cwd: &mut Cwd, token: &str) -> Result<(), Refused> {
        let p = path(token.as_bytes())?;
        *cwd = cwd.resolve(&p)?;
        Ok(())
    }

    /// Capture `{:?}` without an allocator, for the same reason [`assert_pwd`] takes a buffer:
    /// this crate has no `String`, in tests or out.
    fn assert_debug(v: &dyn core::fmt::Debug, want: &str) {
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
        let mut buf = Buf {
            bytes: [0; 64],
            n: 0,
        };
        write!(buf, "{v:?}").unwrap();
        assert_eq!(core::str::from_utf8(&buf.bytes[..buf.n]), Ok(want));
    }

    /// The wording, exactly. Each message is the fixed half of a line the prompt prints, so a
    /// drift here is user-visible; a containment check would also pass an empty string.
    #[test]
    fn each_refusal_is_its_own_exact_sentence() {
        assert_eq!(
            Refused::NotAName.message(),
            "that is not a name: one component, at most 255 bytes",
        );
        assert_eq!(
            Refused::DirectoryNameTooLong.message(),
            "that directory's name is longer than this shell can stand in: at most 16 bytes",
        );
        assert_eq!(
            Refused::TooDeep.message(),
            "too deep: this shell tracks at most 8 levels below its root",
        );
        assert_eq!(
            Refused::AtYourRoot.message(),
            "you are at your root; there is nothing above it to name",
        );
    }

    /// `{:?}` on a position prints the path it means, which is the reason the impl is written by
    /// hand; pinned exactly, like a message, because every failing assertion that carries an
    /// endowment renders one. `last` and `is_root` ride along: they are the same two facts (the
    /// root, and the deepest component) read without a formatter.
    #[test]
    fn a_position_debugs_as_the_path_it_means() {
        let mut cwd = Cwd::root();
        assert_debug(&cwd, "/");
        assert_eq!(cwd.last(), None, "the root has no last component");
        cd(&mut cwd, "a/b").unwrap();
        assert_debug(&cwd, "/a/b");
        assert!(!cwd.is_root());
        assert_eq!(cwd.last(), Some(&b"b"[..]));
    }

    /// `RENDER_MAX` is the worst case exactly: the deepest position with the widest names fills
    /// all but one of its bytes, and the spare byte is the lone slash a root renders as, which
    /// never coexists with a level. Slack would hide a truncation bug behind a buffer that always
    /// happened to fit; a byte short would truncate a real prompt.
    #[test]
    fn render_max_is_the_worst_case_plus_the_roots_lone_slash() {
        let mut cwd = Cwd::root();
        for _ in 0..MAX_DEPTH {
            assert!(cwd.descend(b"sixteen-bytes!!!"));
        }
        let mut buf = [0u8; RENDER_MAX];
        let n = cwd.render(&mut buf);
        assert_eq!(n, RENDER_MAX - 1);
        // And nothing was truncated to make that true: a wider buffer renders the same count.
        let mut wide = [0u8; 2 * RENDER_MAX];
        assert_eq!(cwd.render(&mut wide), n);
    }

    /// **`pwd` is relative to your root**, and the root renders as `/` because it is the root of the
    /// only namespace you have.
    #[test]
    fn pwd_is_relative_to_your_own_root() {
        let mut cwd = Cwd::root();
        assert_pwd(&cwd, b"/");
        assert!(cwd.is_root());

        cd(&mut cwd, "logs").unwrap();
        assert_pwd(&cwd, b"/logs");
        cd(&mut cwd, "2026/july").unwrap();
        assert_pwd(&cwd, b"/logs/2026/july");
        assert_eq!(cwd.depth(), 3);
    }

    /// **`..` stops at your root**, which is the divergence and the safety property in one line. It
    /// is not a check on a path: at the root there is no capability to pop, so there is nothing to
    /// get wrong, and no request is sent for the FS server to have to refuse.
    #[test]
    fn dot_dot_clamps_at_your_root_and_never_climbs_out() {
        let mut cwd = Cwd::root();
        assert_eq!(cd(&mut cwd, ".."), Err(Refused::AtYourRoot));
        assert_pwd(&cwd, b"/"); // "a refused `..` must not have moved anything"

        cd(&mut cwd, "a/b").unwrap();
        cd(&mut cwd, "..").unwrap();
        assert_pwd(&cwd, b"/a");
        cd(&mut cwd, "..").unwrap();
        assert_pwd(&cwd, b"/");

        // No number of `..` in one token gets above it either, and the whole token is refused, so
        // the shell has not moved part of the way out.
        let mut cwd = Cwd::root();
        cd(&mut cwd, "a").unwrap();
        assert_eq!(cd(&mut cwd, "../../.."), Err(Refused::AtYourRoot));
        assert_pwd(&cwd, b"/a"); // "a refused path must move nothing at all"
    }

    /// `..` inside a path composes the way it does everywhere, as long as it stays inside.
    #[test]
    fn dot_dot_composes_inside_a_path() {
        let mut cwd = Cwd::root();
        cd(&mut cwd, "a/b/../c").unwrap();
        assert_pwd(&cwd, b"/a/c");
        cd(&mut cwd, "./d/.").unwrap();
        assert_pwd(&cwd, b"/a/c/d");
    }

    /// **`/` is the root of your own namespace**, which is Plan 9's answer: the syntax survives,
    /// and what it roots in is the one directory capability the holder was granted.
    #[test]
    fn an_absolute_path_is_rooted_in_your_own_namespace() {
        let p = path(b"/etc/passwd").unwrap();
        assert!(p.is_from_root());
        assert_eq!(p.steps(), &[Step::Down(b"etc"), Step::Down(b"passwd")]);

        // `/` alone is your root: a place, not a name, so it has no final component to act on.
        let root = path(b"/").unwrap();
        assert!(root.is_from_root());
        assert!(root.steps().is_empty());
        assert_eq!(root.split_last_component(), None);

        // A relative token that merely contains a slash is unchanged, and is *not* from the root.
        assert!(!path(b"a/b").unwrap().is_from_root());
    }

    /// **An absolute path means the same place wherever you stand**, which is the whole of what
    /// the leading `/` buys and the one thing a step sequence cannot say on its own.
    #[test]
    fn an_absolute_path_does_not_depend_on_where_you_are() {
        let mut deep = Cwd::root();
        cd(&mut deep, "a/b/c").unwrap();
        let mut shallow = Cwd::root();

        cd(&mut deep, "/logs/2026").unwrap();
        cd(&mut shallow, "/logs/2026").unwrap();
        assert_pwd(&deep, b"/logs/2026");
        assert_pwd(&shallow, b"/logs/2026");

        // `pwd`'s output is a token you can type back: the round trip.
        let mut buf = [0u8; RENDER_MAX];
        let n = deep.render(&mut buf);
        let mut elsewhere = Cwd::root();
        cd(&mut elsewhere, "somewhere/else").unwrap();
        let p = path(&buf[..n]).unwrap();
        assert_eq!(elsewhere.resolve(&p).unwrap(), deep);
    }

    /// **An absolute path stops at your root exactly as `..` does**, which is why rooting the
    /// syntax in your own namespace grants nothing: there is no level above the root to name, and
    /// `/..` asks for the same nonexistent place that `..` at the root asks for.
    #[test]
    fn an_absolute_path_cannot_climb_above_your_root() {
        let mut cwd = Cwd::root();
        cd(&mut cwd, "a/b").unwrap();
        assert_eq!(cd(&mut cwd, "/.."), Err(Refused::AtYourRoot));
        assert_eq!(cd(&mut cwd, "/../../elsewhere"), Err(Refused::AtYourRoot));
        assert_eq!(cd(&mut cwd, "/a/../../b"), Err(Refused::AtYourRoot));
        // A refused absolute path moves nothing, the same all-or-nothing rule a relative one has.
        assert_pwd(&cwd, b"/a/b");
    }

    /// **What an absolute path reaches, a walk could already have reached**: `/a/b` from anywhere
    /// is `cd` to the root and then two descents, so the syntax adds no reach. That is the
    /// property that keeps a namespace from becoming ambient authority, and it is stated here as
    /// an equality between two ways of arriving rather than as a claim in prose.
    #[test]
    fn an_absolute_path_reaches_exactly_what_a_walk_from_the_root_reaches() {
        for token in ["/a/b", "/logs/2026/july", "/", "/a/b/../c"] {
            let mut absolute = Cwd::root();
            cd(&mut absolute, "somewhere/deep").unwrap();
            cd(&mut absolute, token).unwrap();

            let mut walked = Cwd::root();
            for step in path(token.as_bytes()).unwrap().steps() {
                match step {
                    Step::Down(name) => assert!(walked.descend(name)),
                    Step::Up => assert!(walked.ascend()),
                }
            }
            assert_eq!(absolute, walked, "{token} did not land where a walk lands");
        }
    }

    /// The component rules, which are the FS server's own, checked at the prompt so a request that
    /// cannot succeed is never sent.
    #[test]
    fn a_component_that_cannot_be_a_name_is_refused_before_anything_is_sent() {
        assert_eq!(path(b""), Err(Refused::NotAName));
        assert_eq!(path(b"."), Err(Refused::NotAName));
        let mut long = [b'/'; MAX_NAME + 3];
        long[1..MAX_NAME + 2].fill(b'n');
        assert_eq!(
            path(&long[..MAX_NAME + 2]),
            Err(Refused::NotAName),
            "/ and one byte too many"
        );
        let mut under = [b'n'; MAX_NAME + 3];
        under[..2].copy_from_slice(b"a/");
        assert_eq!(
            path(&under),
            Err(Refused::NotAName),
            "a/ and one byte too many"
        );
        // A component as long as a file's name may be is a path, and only standing in it is not.
        assert!(path(b"a/least_authority_demo").is_ok());
        let mut cwd = Cwd::root();
        assert_eq!(
            cd(&mut cwd, "seventeen-bytes!!"),
            Err(Refused::DirectoryNameTooLong)
        );
        assert_eq!(
            cd(&mut cwd, "seventeen-bytes!!/a"),
            Err(Refused::DirectoryNameTooLong)
        );
        assert!(
            cwd.is_root(),
            "a refused descent leaves the shell where it was"
        );
        assert_eq!(cd(&mut cwd, "sixteen-bytes!!!"), Ok(()));
        assert_eq!(path(b"a/b\\c"), Err(Refused::NotAName));
        assert_eq!(path(b"a:b"), Err(Refused::NotAName));
        // Nine components, one more than the shell tracks. Refused rather than truncated: a
        // truncated path names a different directory.
        assert_eq!(path(b"a/a/a/a/a/a/a/a/a"), Err(Refused::TooDeep));
    }

    /// A descent past the depth the shell tracks is refused, and refused **before** it moves.
    #[test]
    fn a_path_deeper_than_the_shell_tracks_is_refused_rather_than_truncated() {
        let mut cwd = Cwd::root();
        for _ in 0..MAX_DEPTH {
            cd(&mut cwd, "d").unwrap();
        }
        assert_eq!(cd(&mut cwd, "d"), Err(Refused::TooDeep));
        assert_eq!(cwd.depth(), MAX_DEPTH);
    }

    /// The grant-time split: the directory to narrow, and the one name in it.
    #[test]
    fn a_token_splits_into_the_directory_to_resolve_and_the_final_name() {
        let p = path(b"logs/2026/report.txt").unwrap();
        let (lead, name) = p.split_last_component().unwrap();
        assert_eq!(name, b"report.txt");
        assert_eq!(lead, &[Step::Down(b"logs"), Step::Down(b"2026")]);

        // A bare name is the common case: no lead, and the name is resolved where you stand.
        let p = path(b"report.txt").unwrap();
        let (lead, name) = p.split_last_component().unwrap();
        assert!(lead.is_empty());
        assert_eq!(name, b"report.txt");

        // A path ending in `..` designates a directory, so it cannot be the file half of a grant.
        assert!(path(b"a/..").unwrap().split_last_component().is_none());
    }

    /// A render into a short buffer truncates instead of faulting: a shell drawing a prompt must
    /// never be the thing that takes the session down.
    #[test]
    fn rendering_into_a_short_buffer_truncates_rather_than_faulting() {
        let mut cwd = Cwd::root();
        cd(&mut cwd, "abc/def").unwrap();
        let mut small = [0u8; 5];
        let n = cwd.render(&mut small);
        assert_eq!(&small[..n], b"/abc/");
        let mut none = [0u8; 0];
        assert_eq!(cwd.render(&mut none), 0);
    }

    /// **`/a/x` and `/b/y` both resolve, to the grant their own label names.** This is milestone
    /// 154's deliverable in its own words, checked as pure logic before any capability exists.
    #[test]
    fn each_label_reaches_its_own_grant_and_only_its_own_grant() {
        let ns = TwoRoots::new(b"a", b"b");

        let (which, cwd) = ns.resolve(b"/a/x").unwrap();
        assert_eq!(which, Which::A);
        assert_eq!(cwd.depth(), 1);
        assert_eq!(cwd.component(0), b"x");

        let (which, cwd) = ns.resolve(b"/b/y").unwrap();
        assert_eq!(which, Which::B);
        assert_eq!(cwd.depth(), 1);
        assert_eq!(cwd.component(0), b"y");
    }

    /// **`/a/../b` is refused**, the roadmap block's own negative control: "proving neither
    /// subtree can name the other's parent." It is refused for the same reason `..` at a single
    /// grant's root is refused, `Refused::AtYourRoot`, because selecting `a` leaves nothing above
    /// `a`'s own root to pop, and `b` is never reached to be a question.
    #[test]
    fn dot_dot_cannot_cross_from_one_grant_into_the_other() {
        let ns = TwoRoots::new(b"a", b"b");
        assert_eq!(ns.resolve(b"/a/../b"), Err(Refused::AtYourRoot));
        // Symmetric, and however many `..`s a token carries, the whole token is refused rather
        // than moving partway, by `Cwd::apply`'s own all-or-nothing rule.
        assert_eq!(ns.resolve(b"/b/../../a"), Err(Refused::AtYourRoot));
    }

    /// A relative token has no root to pick a grant from, and there is no default grant to fall
    /// back on: a two-root holder is not "standing" in either one.
    #[test]
    fn a_relative_token_has_no_grant_to_resolve_against() {
        let ns = TwoRoots::new(b"a", b"b");
        assert_eq!(ns.resolve(b"x"), Err(Refused::NotAName));
    }

    /// A token whose first component names neither label is refused the same way, and so is the
    /// bare root: nothing in a two-grant namespace answers "what is at the top of both".
    #[test]
    fn an_unknown_label_and_the_bare_root_are_both_refused() {
        let ns = TwoRoots::new(b"a", b"b");
        assert_eq!(ns.resolve(b"/c/x"), Err(Refused::NotAName));
        assert_eq!(ns.resolve(b"/"), Err(Refused::NotAName));
        assert_eq!(ns.resolve(b"/.."), Err(Refused::NotAName));
    }

    /// `..` composes inside a grant exactly as it does for a single [`Cwd`], because it is the
    /// same [`Cwd::apply`] running underneath: this is not a second implementation to keep in
    /// step with the first.
    #[test]
    fn dot_dot_composes_inside_one_grant_the_same_as_a_single_root_does() {
        let ns = TwoRoots::new(b"a", b"b");
        let (which, cwd) = ns.resolve(b"/a/x/../y").unwrap();
        assert_eq!(which, Which::A);
        assert_eq!(cwd.depth(), 1);
        assert_eq!(cwd.component(0), b"y");
    }

    // ---- DECISIONS §126: the real, single, moving `(which, pos)` cwd ----

    /// **A bare relative name resolves against `pos` inside whichever tree `which` currently
    /// names**, identical to a one-grant [`Cwd::resolve`], parameterized by which tree.
    #[test]
    fn a_relative_token_resolves_inside_the_current_tree() {
        let ns = TwoRoots::new(b"a", b"b");
        let mut pos = Cwd::root();
        pos.descend(b"here");
        let (which, next) = ns.resolve_from(Which::B, pos, b"deeper").unwrap();
        assert_eq!(which, Which::B, "a relative token never switches trees");
        assert_pwd(&next, b"/here/deeper");
    }

    /// **An absolute path both resolves and moves between trees**: `cd /b/somewhere` from
    /// anywhere works with no new verb, exactly as §126 describes.
    #[test]
    fn an_absolute_token_moves_to_the_tree_it_names() {
        let ns = TwoRoots::new(b"a", b"b");
        let pos = Cwd::root();
        let (which, next) = ns.resolve_from(Which::A, pos, b"/b/elsewhere").unwrap();
        assert_eq!(which, Which::B);
        assert_pwd(&next, b"/elsewhere");
    }

    /// **`..` at either tree's own root refuses**, the same [`Refused::AtYourRoot`] a one-grant
    /// [`Cwd::apply`] already gives, applied per-tree: standing at `b`'s own root, `..` does not
    /// silently land in `a` or anywhere else.
    #[test]
    fn dot_dot_refuses_at_either_trees_own_root_rather_than_crossing() {
        let ns = TwoRoots::new(b"a", b"b");
        assert_eq!(
            ns.resolve_from(Which::B, Cwd::root(), b".."),
            Err(Refused::AtYourRoot),
        );
        // One level down and back up composes normally, same as a single grant.
        let mut which = Which::A;
        let mut pos = Cwd::root();
        ns.apply_from(&mut which, &mut pos, b"/a/sub").unwrap();
        ns.apply_from(&mut which, &mut pos, b"..").unwrap();
        assert_eq!(which, Which::A);
        assert_pwd(&pos, b"/");
    }

    /// **The starting position is the first-listed grant's own root** (`which = A`), matching the
    /// "slot 0 is always the first grant" precedent milestone 154 already established: a relative
    /// token typed from the start resolves inside `a`, with nothing special about the start
    /// beyond being where a fresh two-grant holder stands.
    #[test]
    fn the_starting_position_is_grant_as_own_root() {
        let ns = TwoRoots::new(b"a", b"b");
        let start = (Which::A, Cwd::root());
        let (which, pos) = ns.resolve_from(start.0, start.1, b"x").unwrap();
        assert_eq!(which, Which::A);
        assert_eq!(pos.component(0), b"x");
    }

    /// [`TwoRoots::apply_from`] is all-or-nothing, [`Cwd::apply`]'s own rule composed rather than
    /// reimplemented: a refused move leaves both `which` and `pos` exactly where they were.
    #[test]
    fn apply_from_moves_nothing_on_a_refusal() {
        let ns = TwoRoots::new(b"a", b"b");
        let mut which = Which::A;
        let mut pos = Cwd::root();
        pos.descend(b"sub");
        assert_eq!(
            ns.apply_from(&mut which, &mut pos, b".."),
            Ok(()),
            "one legal ascent",
        );
        assert_eq!(which, Which::A);
        assert_pwd(&pos, b"/");
        assert_eq!(
            ns.apply_from(&mut which, &mut pos, b".."),
            Err(Refused::AtYourRoot),
        );
        assert_eq!(which, Which::A, "a refusal moved nothing");
        assert_pwd(&pos, b"/");
    }

    // ---- `bind`: a value under a name, milestone 47's own sketch ----

    /// **A bound entry resolves as the position it was given**, no more and no less: the whole of
    /// "a bind entry is a value, not a capability."
    #[test]
    fn a_bound_name_resolves_to_the_position_it_was_given() {
        let mut binds = Bindings::default();
        let mut target = Cwd::root();
        target.descend(b"logs");
        target.descend(b"2026");
        binds.add(b"recent", target).unwrap();

        let p = path(b"/recent").unwrap();
        let cwd = binds.resolve_absolute(&p).unwrap().unwrap();
        assert_eq!(cwd, target);
    }

    /// **The rest of the token composes past the bound name**, exactly as it would past a grant
    /// label: `/recent/report.txt` is the bind's own position plus one more descent.
    #[test]
    fn a_token_composes_past_a_bound_name() {
        let mut binds = Bindings::default();
        let mut target = Cwd::root();
        target.descend(b"logs");
        binds.add(b"recent", target).unwrap();

        let p = path(b"/recent/report.txt").unwrap();
        let cwd = binds.resolve_absolute(&p).unwrap().unwrap();
        assert_pwd(&cwd, b"/logs/report.txt");
    }

    /// **`..` past a bound name does not stop at the bind**: it ascends into the tree the bind
    /// points at, exactly as a direct walk to that position would, because [`Bindings`] reuses
    /// [`Cwd::apply`] rather than re-clamping at the alias. A bind can misdirect (name a different
    /// place than its own words suggest) but never grant more than the resolver could already
    /// reach by typing the real path.
    #[test]
    fn dot_dot_past_a_bound_name_climbs_the_real_tree_not_a_fabricated_root() {
        let mut binds = Bindings::default();
        let mut target = Cwd::root();
        target.descend(b"a");
        target.descend(b"b");
        binds.add(b"recent", target).unwrap();

        // One `..` lands where a direct walk to `/a` would: the bind's own parent, not refused.
        let p = path(b"/recent/..").unwrap();
        let cwd = binds.resolve_absolute(&p).unwrap().unwrap();
        assert_pwd(&cwd, b"/a");

        // A second reaches the real root, and a third refuses there: the same wall a direct `/a/b`
        // would meet, not a wall invented at the alias.
        let p = path(b"/recent/../..").unwrap();
        let cwd = binds.resolve_absolute(&p).unwrap().unwrap();
        assert_pwd(&cwd, b"/");
        let p = path(b"/recent/../../..").unwrap();
        assert_eq!(
            binds.resolve_absolute(&p).unwrap(),
            Err(Refused::AtYourRoot)
        );
    }

    /// A name that matches no bound entry is `None`, not a refusal: the caller (a one-grant
    /// [`crate::Holdings`] or a [`TwoRoots`]) still gets to decide what "nothing bound here" means
    /// for it, which is exactly the split [`TwoRoots::try_resolve_absolute`] makes for a grant
    /// label.
    #[test]
    fn an_unbound_name_is_none_not_a_refusal() {
        let binds = Bindings::default();
        let p = path(b"/nope").unwrap();
        assert_eq!(binds.resolve_absolute(&p), None);
    }

    /// A name already bound refuses a second bind, and the first is untouched: no silent
    /// shadowing, because a rebind the user did not ask for is exactly the quiet authority change
    /// this project refuses elsewhere.
    #[test]
    fn a_name_already_bound_refuses_a_second_bind() {
        let mut binds = Bindings::default();
        let mut first = Cwd::root();
        first.descend(b"a");
        binds.add(b"x", first).unwrap();
        let mut second = Cwd::root();
        second.descend(b"b");
        assert_eq!(binds.add(b"x", second), Err(BindRefused::AlreadyBound),);
        // Still resolves to the first, untouched.
        let p = path(b"/x").unwrap();
        assert_eq!(binds.resolve_absolute(&p).unwrap().unwrap(), first);
    }

    /// The table holds at most [`MAX_BINDS`], loud rather than silently dropping one.
    #[test]
    fn the_table_refuses_a_bind_past_its_capacity() {
        let mut binds = Bindings::default();
        for i in 0..MAX_BINDS {
            let name = [b'a' + i as u8];
            binds.add(&name, Cwd::root()).unwrap();
        }
        assert_eq!(
            binds.add(b"one-too-many", Cwd::root()),
            Err(BindRefused::TooMany),
        );
    }

    /// A name that cannot be a component at all is refused before it ever reaches the table, the
    /// same rule [`component_fits`] already enforces for every other name in this module.
    #[test]
    fn an_unnameable_bind_target_is_refused() {
        let mut binds = Bindings::default();
        assert_eq!(binds.add(b"a/b", Cwd::root()), Err(BindRefused::NotAName),);
        assert_eq!(binds.add(b"", Cwd::root()), Err(BindRefused::NotAName));
    }

    /// [`BindRefused::message`], pinned exactly, [`each_refusal_is_its_own_exact_sentence`]'s
    /// reason: each is a fixed half of a line the prompt prints.
    #[test]
    fn each_bind_refusal_is_its_own_exact_sentence() {
        assert_eq!(
            BindRefused::NotAName.message(),
            "that is not a name: one component, at most 16 bytes",
        );
        assert_eq!(
            BindRefused::AlreadyBound.message(),
            "that name is already bound; unbind it first",
        );
        assert_eq!(
            BindRefused::TooMany.message(),
            "too many bound names: this shell tracks at most 4",
        );
        assert_eq!(
            BindRefused::ReservedLabel.message(),
            "that name leads to where a tree is mounted",
        );
    }
}

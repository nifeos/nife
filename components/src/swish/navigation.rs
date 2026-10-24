//! **Where the shell stands, and the builtins that move it** (milestone 47 (navigation and
//! naming)): [`Nav`], its position inside the directory capability it holds, the builtins `cd`,
//! `ls`, `mkdir`, `bind` and `touch`, and pattern expansion, the one place a pattern becomes a
//! set. See notes/shell-navigation.md.
//!
//! Name: provisional 2026-10-11 (UTC), milestone 844's lane. calef deferred every module name
//! in §273 to a name-ratification pass.

use super::*;

/// **The shell's position inside the one directory capability it holds**, and the capabilities it
/// walked through to get there.
///
/// A working directory, in capability terms, is a directory capability used as the default base for
/// resolving names. This is that, made concrete: `holds.cwd` ([`Nav::holds`]) is the position as a
/// value (what `pwd` prints, and what a grant records at plan time), and [`Nav::handles`] is the
/// stack of directory capabilities that back it, one per level. **`..` is a pop of that stack**, which is why it cannot
/// climb out: at the root there is nothing to pop, and no request is sent for the FS server to have
/// to refuse. Chroot's shape, arrived at from the other direction.
pub(super) struct Nav {
    /// The slot holding the directory capability, or `None` for a shell that was granted none.
    pub(super) dir: Option<u64>,
    /// **The rights the root capability carries**, as this shell was told at spawn.
    ///
    /// It is told rather than asking, and that is a gap in the contract rather than a shortcut:
    /// `filesystem_contract` has no verb that reports what a handle carries. It matters because `OPENDIR`
    /// refuses (`EPERM`) when the intersection is smaller than the request, so a shell that asked
    /// for `dir::ALL` from a narrower capability could not `cd` at all. See notes/shell-navigation.md.
    pub(super) rights: u64,
    /// **What this shell holds, as the planner sees it**: where it stands (`holds.cwd`, a path in
    /// the one tree it presents), the names it has bound (`holds.binds`, values rather than
    /// capabilities, so no cspace slot is spent), and where a second tree is mounted
    /// (`holds.second`, `Some` only for the two-tree witness [`two_trees`] and for the boot shell
    /// that finds an NVMe disk tree at `spawnproto::MOUNTED_TREE_SLOT` and mounts it at `/disk`,
    /// milestone 834 (ioping on nife and Linux)). Kept whole because the resolver reads it on
    /// every walk: an 800-byte `Holdings` built per walking frame overflowed the globbing witness's
    /// stack, the frame budget `glob_grant_tests` records.
    pub(super) holds: grant_plan::Holdings,
    /// `handles[i]` is the directory capability for level `i + 1`; level 0 is [`fs::ROOT`], the
    /// capability the rendezvous itself designates.
    handles: [u64; nav::MAX_DEPTH],
    /// The sweep in progress, when the line is under `xargs` (milestone 109). Zeroed outside one.
    pub(super) batching: Batching,
    /// **The second tree, when this shell holds two**: milestone 154 (a process that holds two
    /// directory capabilities), mounted where `holds.second` says. [`Nav::dir`] and
    /// [`Nav::rights`] stay the first tree's whichever one the shell stands in, because the first
    /// is also where the system's own files are (the manual, the activation table); which tree
    /// [`Nav::handles`] are in follows from `holds.cwd` ([`Nav::which`]).
    pub(super) second: Option<Tree>,
    /// **The background jobs this shell holds** (milestone 48 (job control), phase one): what
    /// `jobs` lists and `wait`, `kill` and `fg` act on. Here rather than in a `static` because it
    /// is session state the prompt threads through every line, as [`Batching`] is.
    pub(super) jobs: Jobs,
}

/// **One directory tree this shell holds**: the slot of the endpoint that serves it and the rights
/// that endpoint carries (milestone 154).
///
/// A handle means nothing without the endpoint it came from: two caretakers number their handles
/// independently, so handle 3 in one tree and handle 3 in the other are different directories.
/// Every request that carries a handle a walk produced is therefore sent on that walk's [`Tree`],
/// never on "the" directory slot.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Tree {
    pub(super) slot: u64,
    pub(super) rights: u64,
}

/// **The state of a batched sweep**, which lives on [`Nav`] because the expander is the one place a
/// pattern becomes a set and a batch is a *kind of expansion* rather than a second mechanism.
///
/// Putting it here is what makes every command path inherit batching for free: `echo`, an
/// invocation, a `caps` preview and a pipeline stage all reach a pattern through [`Nav::expand`],
/// so none of them needs to know that a sweep is running.
#[derive(Clone, Copy, Default)]
pub(super) struct Batching {
    /// Where this batch resumes, or `None` outside a sweep.
    ///
    /// **Cleared once a pattern has been expanded**, so only the *first* pattern on a line is
    /// batched. That is not a new limitation: only the first pattern on a line is expanded at all
    /// (notes/glob-grant.md), because no manifest declares a second name slot.
    pub(super) resume: Option<Resume>,
    /// Which batch this is, counting from one, for the header printed before it runs.
    pub(super) index: u64,
    /// Where the batch after this one resumes, or `None` when this was the last.
    pub(super) next: Option<Resume>,
    /// How many names this batch designated, which is the authority that moved.
    pub(super) names: u64,
}

/// A resolved path lead: the directory handle it designates, plus the temporary capabilities opened
/// to reach it, which the caller either adopts (`cd`) or closes.
pub(super) struct Walk {
    /// The tree the walk is in, which every request on [`Walk::handle`] and [`Walk::tmp`] goes to.
    pub(super) tree: Tree,
    /// The directory the lead designates.
    pub(super) handle: u64,
    /// The capabilities opened on the way down, in order.
    tmp: [u64; nav::MAX_DEPTH],
    n: usize,
}

impl Nav {
    /// A shell holding nothing that names a filesystem. Everything below then answers
    /// [`Say::NoDirectory`], which is a statement about this shell's capability table and not a placeholder.
    pub(super) fn empty() -> Self {
        Nav {
            dir: None,
            rights: 0,
            holds: grant_plan::Holdings::default(),
            handles: [0; nav::MAX_DEPTH],
            batching: Batching::default(),
            second: None,
            jobs: Jobs::new(),
        }
    }

    /// A shell rooted at the directory capability in `slot`, carrying `rights`. The slot is a
    /// parameter because the two wirings put the directory in different places: a witness holds
    /// nothing else and gets slot 0, and a shell with a terminal already owes 0..3 to it.
    pub(super) fn rooted_at(slot: u64, rights: u64) -> Self {
        Nav {
            dir: Some(slot),
            rights,
            holds: grant_plan::Holdings {
                directory: true,
                ..grant_plan::Holdings::default()
            },
            handles: [0; nav::MAX_DEPTH],
            batching: Batching::default(),
            second: None,
            jobs: Jobs::new(),
        }
    }

    /// Which tree this shell stands in: the second when its path runs through the mount point,
    /// the first otherwise ([`grant_plan::Holdings::locate`]).
    fn which(&self) -> nav::Which {
        self.holds.locate(&self.holds.cwd).0
    }

    /// The tree `which` names. The second tree of a shell that holds one is the first, which no
    /// caller reaches: `which` only comes back [`nav::Which::B`] out of a two-grant [`Holdings`].
    ///
    /// [`Holdings`]: grant_plan::Holdings
    pub(super) fn tree(&self, which: nav::Which) -> Tree {
        match (which, self.second) {
            (nav::Which::B, Some(t)) => t,
            _ => Tree {
                slot: self.dir.unwrap_or(DIR),
                rights: self.rights,
            },
        }
    }

    /// The tree this shell stands in, which is where [`Nav::here`] and [`Nav::handles`] live.
    pub(super) fn cur(&self) -> Tree {
        self.tree(self.which())
    }

    /// The first tree, which is where this system keeps its own files (the manual, the activation
    /// table, a package to install) whichever tree the shell stands in.
    pub(super) fn first(&self) -> Tree {
        self.tree(nav::Which::A)
    }

    /// The handle for the level we are standing on. [`Nav::handles`] holds the levels of the
    /// position *inside* the tree the shell stands in, which under a mount point is shorter than
    /// the path `pwd` prints.
    pub(super) fn here(&self) -> u64 {
        self.at(self.holds.locate(&self.holds.cwd).1.depth())
    }

    /// The handle for `level` levels below the root.
    pub(super) fn at(&self, level: usize) -> u64 {
        match level.checked_sub(1) {
            None => fs::ROOT,
            Some(i) => self.handles[i],
        }
    }

    /// [`Nav::name_call`] on a named tree. Both trees share one page with the FS server, which is
    /// sound for `narrow_dir`'s reason: this shell is one thread with one `CALL` in flight.
    pub(super) fn name_call_in(
        &self,
        t: Tree,
        verb: u64,
        handle: u64,
        name: &[u8],
        w1: u64,
    ) -> i64 {
        put_page(name);
        call(t.slot, fs::req(verb, handle, name.len() as u64), w1).0 as i64
    }

    /// Close a handle we opened in the tree we stand in. A failure here is not reportable and not
    /// recoverable: the reply is dropped deliberately rather than turned into a refusal for
    /// something the user did not ask.
    pub(super) fn close(&self, handle: u64) {
        self.close_in(self.cur(), handle);
    }

    /// [`Nav::close`] on a named tree.
    pub(super) fn close_in(&self, t: Tree, handle: u64) {
        call(t.slot, fs::req(fs::CLOSE, handle, 0), 0);
    }

    /// **Resolve a path without moving and open it**, from the root when the token began with `/`
    /// and from where the shell stands otherwise; [`Nav::walk_steps`] has the whole of it.
    fn walk(&mut self, p: &nav::Path<'_>) -> Result<Walk, Say> {
        self.walk_steps(p.is_from_root(), p.steps())
    }

    /// [`Nav::walk`] stopping one component short, which every verb that acts on a *name* needs.
    ///
    /// The path is resolved first, then opened: [`grant_plan::Holdings::anchor`] says where the
    /// steps start, [`Cwd::apply`] applies them (refusing `..` past the root with nothing sent, and
    /// a path deeper than [`nav::MAX_DEPTH`], which bounds [`Walk::tmp`]), and
    /// [`grant_plan::Holdings::locate`] says which tree serves the result. The walk opens it one
    /// `OPENDIR` at a time from that tree's root, since the FS contract takes one component per
    /// request. So `..` from a mount point is its parent in the first tree, and a bound name can
    /// misdirect a path but not manufacture a boundary (milestone 154; notes/shell/the-program.md).
    pub(super) fn walk_steps(&mut self, from_root: bool, steps: &[Step<'_>]) -> Result<Walk, Say> {
        let mut place = Cwd::root();
        let rest = self.holds.anchor(from_root, steps, &mut place);
        place.apply(rest).map_err(Say::Refused)?;
        let (which, pos) = self.holds.locate(&place);
        let mut w = Walk {
            tree: self.tree(which),
            handle: fs::ROOT,
            tmp: [0; nav::MAX_DEPTH],
            n: 0,
        };
        // One level at a time from that tree's own root. One step per call rather than a staged
        // array of steps, which is the frame this shell's few pages of stack cannot spare.
        for level in 0..pos.depth() {
            self.descend(&mut w, pos.component(level))?;
        }
        Ok(w)
    }

    /// **One level further down a walk**: one `OPENDIR` on the walk's own tree. A level that
    /// cannot be opened closes everything the walk opened before it.
    fn descend(&self, w: &mut Walk, name: &[u8]) -> Result<(), Say> {
        let r = self.name_call_in(w.tree, fs::OPENDIR, w.handle, name, w.tree.rights);
        if r < 0 {
            self.unwind(w);
            return Err(Say::Failed(-r as i32));
        }
        w.tmp[w.n] = r as u64;
        w.n += 1;
        w.handle = r as u64;
        Ok(())
    }

    /// Give back every capability a walk opened. Called on failure, and by every verb that resolved
    /// a path only to act on it: a handle nobody closes pins a node in the server for the rest of
    /// the boot, exactly as a leaked fd does.
    pub(super) fn unwind(&self, w: &Walk) {
        for &handle in &w.tmp[..w.n] {
            self.close_in(w.tree, handle);
        }
    }

    /// Parse and validate a path operand: the steps, and the path in the one tree they land on.
    /// [`grant_plan::Holdings::resolve_path`], so a bound name and the root are told apart exactly
    /// as [`Nav::walk_steps`] and the planner tell them apart. A shell with one tree and nothing
    /// bound resolves byte for byte as it did before either existed.
    pub(super) fn plan_path<'a>(&self, token: &'a [u8]) -> Result<(nav::Path<'a>, Cwd), Say> {
        let p = nav::path(token).map_err(Say::Refused)?;
        let target = self.holds.resolve_path(&p).map_err(Say::Refused)?;
        Ok((p, target))
    }

    /// **`cd`**: rebind where names resolve. An empty operand is your root, because there is no
    /// `HOME` here and the root is the one distinguished place you have: it is what you were
    /// granted. `cd /` is the same place said out loud, which is what `pwd` has always printed.
    ///
    /// The move is all or nothing. A `cd a/b` that fails at `b` leaves the shell in the directory it
    /// started in, because the next command would otherwise act somewhere the user does not think
    /// they are.
    pub(super) fn cd(&mut self, token: &[u8]) -> Say {
        if self.dir.is_none() {
            return Say::NoDirectory;
        }
        if token.is_empty() {
            return self.go_home();
        }
        let (p, target) = match self.plan_path(token) {
            Ok(v) => v,
            Err(s) => return s,
        };
        let w = match self.walk(&p) {
            Ok(w) => w,
            Err(s) => return s,
        };
        // Adopt the walk: every walk starts at its tree's root, so the whole old stack is given
        // back, on the endpoint of the tree being left (still the current one until `cwd` moves),
        // and the handles the walk opened become the new stack.
        let depth = self.holds.locate(&self.holds.cwd).1.depth();
        for level in 0..depth {
            self.close(self.handles[level]);
        }
        self.handles[..w.n].copy_from_slice(&w.tmp[..w.n]);
        self.holds.cwd = target;
        Say::Nothing
    }

    /// Back to the root: close everything we descended through and forget it.
    fn go_home(&mut self) -> Say {
        let depth = self.holds.locate(&self.holds.cwd).1.depth();
        for level in 0..depth {
            self.close(self.handles[level]);
        }
        self.holds.cwd = Cwd::root();
        Say::Nothing
    }

    /// **`ls`**: enumerate a directory, the cwd by default. One `READDIR` per page of entries, with
    /// the cursor advanced by what was decoded, so a directory larger than the local buffer is read
    /// in rounds rather than truncated.
    ///
    /// `each` is called with every entry: the interactive prompt prints them, and the navigating
    /// witness checks them. A listing is a rendering of authority, so what is in it is a claim worth
    /// checking rather than just printing.
    pub(super) fn ls(&mut self, token: &[u8], each: &mut dyn FnMut(&[u8], bool)) -> Say {
        if self.dir.is_none() {
            return Say::NoDirectory;
        }
        let (listed, tree, handle, walked) = if token.is_empty() {
            (self.holds.cwd, self.cur(), self.here(), None)
        } else {
            let (p, target) = match self.plan_path(token) {
                Ok(v) => v,
                Err(s) => return s,
            };
            match self.walk(&p) {
                Ok(w) => (target, w.tree, w.handle, Some(w)),
                Err(s) => return s,
            }
        };

        // **A mount point is listed in its parent**, as a Unix mount point is, whether or not the
        // first tree has a directory of that name: the second tree is what the name reaches, and a
        // listing that hid it would be a namespace the user could `cd` into but never see.
        let mount = self.holds.mounted.map(|sd| sd.mount());
        let mount_here = mount.filter(|m| {
            let mut parent = *m;
            parent.ascend() && parent == listed
        });
        let mut seen = false;
        let said = self.each_entry(tree, handle, &mut |name, is_dir| {
            seen |= mount_here.as_ref().and_then(|m| m.last()) == Some(name);
            each(name, is_dir);
        });
        if let Some(name) = mount_here.as_ref().and_then(|m| m.last())
            && said == Say::Nothing
            && !seen
        {
            each(name, true);
        }
        if let Some(w) = walked {
            self.unwind(&w);
        }
        said
    }

    /// **`mkdir`**: make a directory and, in the same verb, obtain a capability to it. It needs
    /// `CREATE` **and** `DESCEND`, because a directory you could not have walked into would be a way
    /// to mint a capability out of a right that was withheld.
    ///
    /// The capability it mints is given straight back. `mkdir` makes a directory; `cd` is how you go
    /// there, and a shell that silently moved you would be doing two things under one word.
    pub(super) fn mkdir(&mut self, token: &[u8]) -> Say {
        self.act(token, |nav, t, handle, name| {
            let r = nav.name_call_in(t, fs::MKDIR, handle, name, t.rights);
            if r < 0 {
                Say::Failed(-r as i32)
            } else {
                nav.close_in(t, r as u64);
                Say::Nothing
            }
        })
    }

    /// **`bind <target> <name>`**: name a position this shell already reached some other way, so
    /// `/<name>/...` reaches it too (milestone 47; [`nav::Bindings`] is the mechanism milestone 154's
    /// second grant unblocked). `target` resolves through the bind-aware [`Nav::plan_path`], so a
    /// bind under a bind composes. A builtin like [`Nav::mkdir`]: it mints no capability, spawns
    /// nothing, and files a value under a name. A bound position is a path in the one tree, so it
    /// can point through a mount point, and [`grant_plan::Holdings::bind`] refuses a name that
    /// would hide the path to one (milestone 154).
    pub(super) fn bind(&mut self, target: &[u8], name: &[u8]) -> Say {
        if self.dir.is_none() {
            return Say::NoDirectory;
        }
        let (_, pos) = match self.plan_path(target) {
            Ok(v) => v,
            Err(s) => return s,
        };
        match self.holds.bind(name, pos) {
            Ok(()) => Say::Nothing,
            Err(r) => Say::CannotBind(r),
        }
    }

    /// **`touch`**: create an empty file if the name is not there (a no-op if it already is), then
    /// bump its modification time. Bare `touch` sets it to now (`fs::SETMTIME`, needs only the
    /// `WRITE` this shell already holds); `-t` asserts a caller-chosen instant (`fs::SETMTIME_AT`,
    /// needs `WRITE` **and** `SETTIME`, DECISIONS §112). The create half is `mkdir`'s shape, one
    /// right narrower: `fs::CREATE` mints a name and this builtin gives the handle straight back,
    /// because touching a file is not opening it.
    ///
    /// **Milestone 47 built the create half first and the mtime half here**, in that order, because
    /// the mtime half needed a decided rights question (DECISIONS §112) and the create half did
    /// not. See notes/touch.md.
    pub(super) fn touch(&mut self, args: TouchOperands) -> Say {
        // `-t`'s text is parsed before anything is named, so a malformed instant is refused before
        // the create half runs: a caller should not see a file appear and then learn its `-t` text
        // was unusable. The create half and the mtime half are still two separate wire requests
        // (there is no combined verb), but this at least orders the local, no-wire-cost check first.
        let at_seconds: Option<u64> = match args.at {
            None => None,
            Some(text) => match calendar::DateTime::parse_rfc3339_bytes(text) {
                Ok(dt) => match u64::try_from(dt.to_unix()) {
                    Ok(secs) => Some(secs),
                    Err(_) => return Say::NotAnInstant,
                },
                Err(_) => return Say::NotAnInstant,
            },
        };

        self.act(args.name, |nav, t, handle, name| {
            let r = nav.name_call_in(t, fs::CREATE, handle, name, 0);
            if r >= 0 {
                nav.close_in(t, r as u64);
            } else {
                let errno = -r as i32;
                // `CREATE` is create, not create-or-open (DECISIONS §27): an existing name answers
                // `EEXIST`, which is exactly the outcome this half of `touch` wants for a name that
                // is already there. Not in `filesystem_contract::dir`'s named list, for
                // `components/src/rm.rs::ENOENT`'s reason: this contract answers it from one rung only,
                // so there is nothing else in this program that would collide with a local name for
                // it.
                const EEXIST: i32 = 17;
                if errno != EEXIST {
                    return Say::Failed(errno);
                }
            }

            let r = match at_seconds {
                None => nav.name_call_in(t, fs::SETMTIME, handle, name, 0),
                Some(secs) => nav.name_call_in(t, fs::SETMTIME_AT, handle, name, secs),
            };
            if r < 0 {
                return Say::Failed(-r as i32);
            }
            Say::Nothing
        })
    }

    /// The shape `mkdir` has, and the witness's removals share: resolve everything but the last
    /// component, act on that component in the directory it named, and give back whatever the
    /// resolution opened.
    pub(super) fn act(
        &mut self,
        token: &[u8],
        f: impl Fn(&mut Nav, Tree, u64, &[u8]) -> Say,
    ) -> Say {
        if self.dir.is_none() {
            return Say::NoDirectory;
        }
        if token.is_empty() {
            return Say::NeedsAName;
        }
        let (p, _) = match self.plan_path(token) {
            Ok(v) => v,
            Err(s) => return s,
        };
        let Some((lead, name)) = p.split_last_component() else {
            // The token ends in `..`, so it designates a directory rather than a name in one.
            return Say::Refused(Refused::NotAName);
        };
        let w = match self.walk_steps(p.is_from_root(), lead) {
            Ok(w) => w,
            Err(s) => return s,
        };
        let said = f(self, w.tree, w.handle, name);
        self.unwind(&w);
        said
    }
}

// ---- expansion: the one place a pattern becomes a set (milestone 47's globbing lane) ----

impl Nav {
    /// **Expand one pattern token against the directory its leading path names**: the only function
    /// that turns a pattern into names, for `echo` (which prints the set) and the grant planner
    /// (which grants it) alike, so the expansion you see is the grant. It needs `ENUMERATE` and
    /// nothing more (DECISIONS §47 (a directory capability carries six rights, and a child can
    /// never exceed its parent)), and a shell that may not enumerate gets the server's `EPERM`
    /// rather than a quietly empty set, which would be the silent degradation §42 (a filesystem
    /// declares what it offers and must be truthful) forbids.
    pub(super) fn expand(&mut self, token: &[u8]) -> Result<NameSet, Say> {
        if self.dir.is_none() {
            return Err(Say::NoDirectory);
        }
        let (p, _) = self.plan_path(token)?;
        let Some((lead, pattern)) = p.split_last_component() else {
            return Err(Say::Refused(Refused::NotAName));
        };
        let w = self.walk_steps(p.is_from_root(), lead)?;

        // **The batched and unbatched expanders decide membership identically**, which is why the
        // sweep is a policy on this one function rather than a second path: `xargs rm *.txt` and
        // `rm *.txt` cannot disagree about what the pattern designates, only about how much of it
        // one invocation is handed.
        let resume = self.batching.resume;
        let mut e = match resume {
            Some(r) => Expander::batch(pattern, r),
            None => Expander::new(pattern),
        };
        let said = self.each_entry(w.tree, w.handle, &mut |name, is_dir| e.offer(name, is_dir));
        self.unwind(&w);
        if said != Say::Nothing {
            return Err(said);
        }
        let Some(_) = resume else {
            return e.finish().map_err(Say::Cannot);
        };
        let batch = e.finish_batch().map_err(Say::Cannot)?;
        // Only the first pattern of a line is batched, and only the first is expanded at all.
        self.batching.resume = None;
        self.batching.next = batch.next();
        self.batching.names = batch.names.len() as u64;
        // **Printed before anything runs, once per batch**, which is the per-batch form of the
        // property the globbing lane exists for: what is shown is what *this* invocation is handed.
        // The union is never shown because the union is exactly what nobody can hold.
        swish::write_batch(self.batching.index, &batch.names, &mut print);
        Ok(batch.names)
    }

    /// Read a directory in rounds, calling `each` with every entry. The body `ls` had, lifted out so
    /// the listing loop is written once: a listing is a rendering of authority whether it is being
    /// printed or matched against.
    fn each_entry(&mut self, t: Tree, handle: u64, each: &mut dyn FnMut(&[u8], bool)) -> Say {
        let mut cursor = 0u64;
        let mut buf = [0u8; LISTING];
        // Bounded so a server whose cursor does not advance costs a short listing rather than a
        // prompt that never comes back.
        for _ in 0..ROUNDS {
            let n = call(t.slot, fs::req(fs::READDIR, handle, 0), cursor).0 as i64;
            if n < 0 {
                return Say::Failed(-n as i32);
            }
            if n == 0 {
                break;
            }
            let n = (n as usize).min(buf.len());
            get_page(n, &mut buf);
            let mut seen = 0u64;
            for (name, is_dir) in dirent::iter(&buf[..n]) {
                each(name, is_dir);
                seen += 1;
            }
            if seen == 0 {
                break;
            }
            cursor += seen;
        }
        Say::Nothing
    }
}

/// The local buffer one `READDIR` round is decoded from. The shared page is sixteen times larger, so
/// a listing is read in rounds; this program has one stack page and cannot hold the page itself.
const LISTING: usize = 256;
/// The most `READDIR` rounds one `ls` will make. A ceiling, not a limit on directories: it is here
/// so a cursor that fails to advance costs a short listing instead of a prompt that never returns.
pub(super) const ROUNDS: usize = 16;

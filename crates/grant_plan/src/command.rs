//! **What a parsed line is, and what a grant holds**: `Command`, `Endowment`, the file and
//! directory grants, `rm`'s option bits, and the shell's `Holdings`. `RunSpec` is in `parsing`,
//! beside the one function that builds it (§272, fork 3).
//!
//! Moved out of the crate's root unchanged by milestone 842 (the grant-plan crate root is split
//! along its seams), as §272 (the grant-plan crate root splits into private modules) lays out; the
//! root re-exports every public item at its old path.
//!
//! Name: provisional (milestone 842, 2026-10-11), as are its sibling modules' names.

use crate::expand::{Name, NameSet};
use crate::{Prog, RunSpec, line, nav};

/// A parsed command line. The shell dispatches on this; only [`Command::Run`] carries a grant
/// expression that must be planned against a manifest.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Command<'a> {
    /// An empty line. Reprompt.
    Empty,
    /// `help`.
    Help,
    /// `echo <text>`: print the rest of the line verbatim.
    Echo(&'a [u8]),
    /// `caps`: print this shell's own endowment. `caps <command>` (a tail) previews what that
    /// command would grant; the tail is carried for the shell to re-parse. The tail is the command
    /// you would have typed, unprefixed, which is the whole point of the spelling: what you inspect
    /// and what you run cannot drift apart.
    Caps(&'a [u8]),
    /// `time <command>`: run that command exactly as typed and say how long it took.
    ///
    /// **The second prefix word, and it carries a whole command line for [`Caps`](Command::Caps)'s
    /// reason**: `time date | wc` must time the pipeline, so the tail keeps its operators and the
    /// shell re-dispatches it down the path it would have taken untimed. What you time is what you
    /// run, the same way `caps` inspects what you would run.
    ///
    /// **It grants nothing.** The tail's endowment is the tail's; the *timing* is done with the
    /// shell's own clock, so a child holding no clock at all can still be timed and no authority
    /// moves because the word is on the line. That is what makes it a prefix word rather than a
    /// program: a `time` program would have to be handed the thing it timed.
    Time(&'a [u8]),
    /// `xargs <command>`: run that command once per **batch** of what its pattern matched
    /// (milestone 109). The name is provisional.
    ///
    /// **The third prefix word, and it is a prefix word for the same reason the other two are**: its
    /// operand is a whole command line, and what it changes is how the shell *runs* the line rather
    /// than anything any program does. It grants nothing itself, which is the test [`Command::Time`] states:
    /// a batching *program* would have to be handed the union of every batch, and there is no
    /// carrier for that union. That is the premise of the whole milestone, so a program in the
    /// middle could only hold the **directory**, which is milestone 47's rejected
    /// "directory plus a name list" answer wearing a different hat.
    ///
    /// Unix can make `xargs` a program precisely because its `xargs` holds *nothing*: names in a
    /// pipe are text, and `rm`'s authority comes from its uid. Here a name in a pipe is still text,
    /// so a `find | xargs rm` analogue would spawn an `rm` holding no authority at all. The pipe
    /// cannot carry the thing that has to be batched.
    Xargs(&'a [u8]),
    /// `cd [path]`: rebind the shell's position inside the directory capability it already holds.
    /// **A builtin, in the same category as `caps`**: it spawns nothing, grants nothing, and confers
    /// no new authority, because a working directory *is* a capability the shell holds used as the
    /// default base for resolving names (see [`nav`]).
    ///
    /// An empty operand means **your root**. There is no `HOME` here, and the root of your namespace
    /// is the one distinguished place you have: it is exactly what you were granted.
    Cd(&'a [u8]),
    /// `pwd`: print where you are, **relative to your root** ([`nav::Cwd::render`]). Naming anything
    /// above it would imply a namespace that does not exist.
    Pwd,
    /// `ls [path]`: enumerate a directory you can reach, the cwd by default. A builtin rather than a
    /// program, which retires the worry that a listing program would be over-granted: it would have
    /// to hold the power to read everything it lists.
    Ls(&'a [u8]),
    /// `mkdir <path>`: make a directory and, in the same verb, obtain a capability to it
    /// (`filesystem_contract::fs::MKDIR` is descend-with-creation). Needs `CREATE` **and** `DESCEND`.
    Mkdir(&'a [u8]),
    /// `touch <path>` or `touch -t <RFC-3339-instant> <path>`: create an empty file if the name is
    /// not there (a no-op if it already is), then bump its modification time. Bare `touch` sets it
    /// to now; `-t` asserts a caller-chosen instant (DECISIONS §112: the ability to *lie about
    /// history*). **A builtin, in [`Mkdir`](Command::Mkdir)'s category rather than [`Prog::Rm`]'s**:
    /// even the `-t` half takes no more than this shell's own directory capability
    /// (`filesystem_contract::dir::WRITE` for the bare form, `dir::WRITE | dir::SETTIME` for `-t`),
    /// which `mkdir` and the create half already established as the model, so there is nothing to
    /// attenuate and nothing gained by confining it to a program.
    ///
    /// **This parser does not interpret the `-t` text.** [`TouchArgs::at`] carries the token
    /// exactly as typed; `date`'s own `FMT_RFC3339` output is a valid input to it, so
    /// `touch -t "$(date)" name` (RFC 3339 mode) round-trips through this shell without either
    /// side inventing a format. Converting it to a Unix-seconds value is `calendar`'s job and
    /// happens where the grant is made (`components/src/swish.rs`'s `touch`), the same layering `date`
    /// itself uses: this crate classifies tokens, it does not do calendar arithmetic.
    ///
    /// See notes/touch.md for what is still not built (Unix's compact `[[CC]YY]MMDDhhmm[.ss]`
    /// `-t` format is not accepted, only RFC 3339) and the wire verbs this dispatches to.
    Touch(TouchArgs<'a>),
    /// `bind <target> <name>`: name a position this shell already reached some other way, so
    /// `/<name>/...` reaches it too (milestone 47, "`bind` is not blocked on a mount table. It
    /// is blocked on a second grant"; milestone 154 supplied the second grant, [`nav::Bindings`]
    /// is the mechanism it unblocked). **A builtin, [`Mkdir`](Command::Mkdir)'s category**: it
    /// mints no capability and spawns nothing, it only records a value this shell already
    /// resolved, under a name.
    ///
    /// `target` is resolved exactly as any other operand is, at bind time, against wherever this
    /// shell stands; `name` is the single component the bound entry is filed under. Neither is
    /// tokenized further, the same "one opaque trailing blob" rule [`Command::Cd`] and
    /// [`Command::Mkdir`] already take their one path operand as.
    Bind(&'a [u8], &'a [u8]),
    /// `apropos <term>`: **name the installed pages that mention a word** (milestone 40 phase 2).
    /// The operand is one word, folded the way the host builder folded the text it indexed.
    ///
    /// **A builtin, in [`Ls`](Command::Ls)'s category rather than [`Prog::Rm`]'s**, and the
    /// argument is the one that milestone's own note reached: search *is* enumeration, the shell
    /// already holds enumeration over what it can see, and a searching *program* would have to be
    /// handed a capability to the whole documentation store in order to read every shard in it.
    /// That is a wider grant than the answer needs and a new principal holding it, for a command
    /// that moves no authority at all.
    ///
    /// **What it produces is names, never capabilities.** A result is a store location a person can
    /// then type at `mdr`, and *that* line is where the grant happens, resolved against the
    /// directory the shell holds exactly as any other designation is. So search cannot widen what
    /// its caller could already reach, which is the property `mdr notes/ipc-naming.md` rests on.
    Apropos(&'a [u8]),
    /// `vouch <path>`: **the owner vouches for a file's bytes** (DECISIONS §221 (the boot prompt is
    /// the owner's console)), so they run vouched, recorded under the path's last component. An
    /// activation request (`spawnproto::Activation::Vouch`) on the spawn endpoint, which `jig`'s
    /// installer endpoint refuses (milestone 809 (the package client becomes a program)), so a
    /// vouch stays the owner's console's, and the tail is classified by [`vouch_verb`](crate::vouch_verb). Name:
    /// provisional (2026-09-26).
    Vouch(&'a [u8]),
    /// `user suspend <name>` and `user resume <name>`: **the owner suspends or resumes an identity**
    /// (milestone 152 (durable delegation), calef's §108 (disabling credentials kills the durable session) ruling of 2026-09-26, names ratified), by
    /// editing the suspended list at the root of this shell's directory. The tail is classified by
    /// [`user_verb`](crate::user_verb).
    User(&'a [u8]),
    /// A program invocation: `<prog> [--mem N] [token ...]`. Named `Run` for the act of running a
    /// program, not for a verb on the line; milestone 47 deleted the verb. A first word that is not
    /// a builtin lands here even when no such program exists, and [`plan`](crate::plan) answers
    /// [`Refusal::NoSuchProgram`](crate::Refusal::NoSuchProgram), which is the "there is nothing there to name" shape of
    /// no-ambient-authority applied to programs.
    Run(RunSpec<'a>),
}

/// [`Command::Touch`]'s operand: the name, and (with `-t`) the instant asserted for it.
///
/// `at` carries the `-t` token's bytes exactly as typed, unparsed: this crate classifies which
/// token is which, it does not know what an RFC 3339 instant is. `None` is bare `touch`, which
/// bumps `name`'s mtime to now with no assertion at all.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TouchArgs<'a> {
    /// The `-t` operand's text, unparsed. `None` for bare `touch`.
    pub at: Option<&'a [u8]>,
    /// The name to create-if-absent and then touch.
    pub name: &'a [u8],
}

/// The most short-option letters one invocation may carry (`rm -rfv` is three). A ceiling rather
/// than a limit anybody meets, and past it the line is [`Refusal::Unexpected`](crate::Refusal::Unexpected) rather than quietly
/// short of an option the user typed.
pub const MAX_FLAGS: usize = 8;

/// What a valid `run` resolves to: exactly the authority to hand the child, and nothing else.
/// Reading this is reading the whole endowment, which is §14's "one literal tells you a process's
/// authority" made concrete.
///
/// **It borrows nothing from the command line**, and that stopped being an accident with milestone
/// 47's globbing lane: a name a pattern produced comes out of a directory listing rather than out of
/// the line, so the grant has to own it. What falls out is the rule the rest of a grant already
/// followed by hand ([`FileGrant::dir`] is a value, not a pointer at the shell): once an endowment
/// is planned, nothing that happens afterwards can change what it means.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Endowment {
    pub prog: Prog,
    /// The integer argument to start it with (0 when the program takes none).
    pub arg: u64,
    /// Pages of untyped to split from the shell's own budget and grant (0 = none).
    pub mem_pages: u64,
    /// The one file to narrow a directory capability down to, and the direction, or `None`.
    /// Delivered as an endpoint served by a file caretaker (`components/src/fs_file_caretaker.rs`), so
    /// what the child ends up holding designates this name and nothing else.
    pub file: Option<FileGrant>,
    /// The one directory to narrow down to, and the **names** in it the program is to act on, or
    /// `None`. Delivered as an endpoint served by a caretaker, so what the child ends up holding
    /// reaches that directory and nothing above or beside it: `components/src/fs_subtree_caretaker.rs`
    /// for a set of one, `components/src/fs_nameset_caretaker.rs` for the set a pattern matched.
    pub dir: Option<DirGrant>,
    /// **The short options that were on the line**, as a bitmask: bit `i` is set when the manifest's
    /// `flags[i]` was typed. Numbered by position in the manifest rather than by letter, so nothing
    /// here has to know what any option means; the program and the manifest agree on the order, and
    /// a host test pins them (`grant_plan::rmopt`).
    pub flags: u64,
    /// **What goes in the child's output slot** (milestone 50): the shell's own result endpoint,
    /// the pipe to the next stage, or a file sink. Reading this line is reading everything `>` and
    /// `|` did, and it is one field because they are one mechanism.
    pub sink: line::Sink,
    /// **What goes in the child's input slot**: nothing, the pipe from the previous stage, or a
    /// file streamed out over the same contract. [`sink`](Endowment::sink)'s mirror.
    pub source: line::Source,
    /// **Where this program's declared second stream goes** (milestone 50's `2>`, DECISIONS §67).
    /// [`line::Diagnostics::None`] for the programs that declare none, which is all of them but
    /// `date`; the shell mints and delegates a second endpoint only for the rest.
    pub diagnostics: line::Diagnostics,
    /// Grant the shared result endpoint.
    pub reports: bool,
    /// Wire the two-tier `^C` path for this job (mirrors the manifest). When true the shell mints
    /// the per-job interrupt channel and runs the escalation policy while the job is foreground.
    pub interruptible: bool,
    /// **Whether this stage may write before it has read to the end** (mirrors
    /// [`InputDeclaration::Required::writes_while_reading`](crate::InputDeclaration::Required::writes_while_reading); `false` for a program that reads nothing,
    /// which cannot be fed and so cannot be the thing that blocks).
    ///
    /// It is carried on the endowment rather than looked up from `prog.manifest()` because
    /// [`check_chain`](crate::check_chain) asks about a **whole planned line** and because [`plan_against_with`](crate::plan_against_with) can be
    /// given an explicit manifest. That is what keeps the check live, tested logic rather than a
    /// branch nothing reaches: no shipped program declares `true` yet, and the host tests plan one
    /// that does, exactly as `FileGrantDeclaration::Required` is exercised.
    pub writes_while_reading: bool,
}

/// One resolved per-file grant: the directory it was resolved against, the name the command
/// designated, and the direction the program's manifest declared. Those three are the whole
/// authority; `filesystem_contract::grant` packs the name and the direction into the file caretaker's start
/// arguments, and the directory is which capability the caretaker is handed to narrow.
///
/// # The cwd stops at the process boundary, and this value is where that is true
///
/// `wc report.txt` resolves the name against the shell's position **at the moment the grant is
/// made**, and this is that moment: [`plan_against`](crate::plan_against) walks any leading path once, here, and records
/// where it landed. The child receives a capability to that one file. It has no cwd, inherits
/// nothing, and cannot re-resolve anything, which is the point: a child that could re-resolve a name
/// later would have ambient authority smuggled in through a string.
///
/// That [`dir`](FileGrant::dir) is a **value** rather than a reference to the shell's position is
/// what makes it structural instead of a promise. A `cd` after this grant is planned cannot change
/// what the grant means, because there is nothing here that points at the shell any more.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FileGrant {
    /// **Which of the shell's trees `dir` is in**, since milestone 154 (a process that holds two
    /// directory capabilities): always [`nav::Which::A`] for a
    /// shell holding one directory, and whichever tree the operand named for a shell holding two.
    /// A required field rather than a default, so nothing that builds a grant can forget that a
    /// position means nothing without the tree it is a position in.
    pub which: nav::Which,
    /// The directory the name was resolved in, relative to the root of [`FileGrant::which`]'s
    /// tree, fixed at plan time.
    pub dir: nav::Cwd,
    /// The final component. Always a single component: a path was resolved into `dir`, not passed
    /// on, because the FS contract takes one component per request and no server here walks a path.
    ///
    /// Owned rather than borrowed from the line, because a pattern that matched exactly one file is
    /// a legitimate way to designate one and that name came from a directory listing.
    pub name: Name,
    pub writable: bool,
}

/// One resolved **directory** grant (milestone 47's `rm -r`, generalized by its globbing lane): the
/// directory the child is handed a capability to, the **set of names** in it the program was told to
/// act on, and whether that capability must reach below the directory or only name what is directly
/// in it.
///
/// It is [`FileGrant`]'s shape one rung up, and the same rule holds for the same reason: `dir` is
/// **where the operand's leading path landed, fixed at plan time**, as a value rather than a pointer
/// at the shell, so a later `cd` cannot change what an already-planned grant means. The child holds
/// a capability to that directory, has no cwd, and cannot re-resolve anything.
///
/// # Why the name is a set
///
/// `rm old.txt` designates one name and `rm *.txt` designates five hundred, and the roadmap's four
/// candidate answers to what the second grants have one survivor: **a directory capability
/// attenuated to a name set**. Five hundred file capabilities exhausts capability slots; the
/// directory plus a name list over-grants catastrophically (the program could touch anything in that
/// directory, which is the thing this model refuses); making `rm` a builtin dodges the question and
/// costs `rm` as a program.
///
/// So [`names`](DirGrant::names) is a set, a literal operand is the set of one, and the
/// generalization is smaller than it looks: `fs_file_caretaker` already serves a namespace of
/// exactly one name, and `components/src/fs_nameset_caretaker.rs` serves the same protocol over a wider
/// one. **Nothing new in the kernel.**
///
/// **These three together are the whole authority**, which is what makes `caps rm -r logs` worth
/// typing: the names at risk are printed before anything happens, and a bug in the program's
/// recursion can only reach what this designates.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DirGrant {
    /// **Which of the shell's trees `dir` is in**, [`FileGrant::which`]'s reason (milestone 154).
    pub which: nav::Which,
    /// The directory the capability designates, relative to the root of [`DirGrant::which`]'s
    /// tree, fixed at plan time.
    pub dir: nav::Cwd,
    /// The names in it the program is to act on: exactly one for a literal operand, the matched set
    /// for a pattern. Each is a single component, for [`FileGrant`]'s reason: a path was resolved
    /// into `dir` rather than passed on.
    ///
    /// **Empty means the whole directory**, and only a program that hears words gets that: its
    /// line said `.` ([`expand::Designation::is_here`](crate::expand::Designation::is_here)). Every other grant carries at least one name.
    pub names: NameSet,
    /// The capability must reach **below** this directory (walk into it and list it), because the
    /// program was given the option its manifest declares for that. False means it may take names
    /// out of this one directory and cannot even see what is under it.
    pub subtree: bool,
}

/// **`rm`'s options**, by the bit they occupy in [`Endowment::flags`], which is their position in
/// [`Prog::Rm`]'s manifest. Named here so the shell, the program and the tests share one definition
/// rather than three copies of an ordering; `rm_options_are_numbered_by_their_position` pins them.
pub mod rmopt {
    /// `-r`: recurse. It is also `rm`'s `subtree_flag`, so typing it visibly widens the grant.
    pub const RECURSIVE: u64 = 1 << 0;
    /// `-f`: ignore a name that is not there, and do not let its absence change the exit status.
    /// It is **not** "ignore errors": a removal that fails for any other reason still reports.
    pub const FORCE: u64 = 1 << 1;
    /// `-v`: say what was removed. It exists because the default says nothing.
    pub const VERBOSE: u64 = 1 << 2;
}

/// **What the shell itself holds**, which is what decides whether a designation can be backed at all.
///
/// This is why it is a parameter rather than a constant. "You hold no such capability" must be a
/// statement about the shell's actual capability table, not a hardcoded era: the same command line is a
/// refusal in a shell that was granted no directory and a real grant in one that was, and neither
/// the parser nor the manifest can tell them apart. Phase 1 hardcoded the refusal, which was true
/// then and would have quietly become a lie.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Holdings {
    /// The shell holds a directory capability it can narrow into a per-file grant.
    pub dir: bool,
    /// **A second directory capability, mounted at a position in the first** (milestone 154, a
    /// process that holds two directory capabilities). `None` for a one-grant shell, which
    /// resolves and prints exactly as it always has. Provisional name and shape.
    pub second: Option<SecondDir>,
    /// **Where the shell stands, in the one tree it presents**: a path from its root, through a
    /// mount point when it stands in the second tree ([`Holdings::locate`] says which tree that
    /// is and where inside it). A working directory is a directory capability used as the default
    /// base for resolving names, so it belongs here beside the fact that there is one. Defaults to
    /// the root, which is also the only position a shell holding no directory can be in.
    pub cwd: nav::Cwd,
    /// **Names this shell has bound to a position it already reached some other way**
    /// (`bind`, milestone 47 (navigation and naming); [`nav::Bindings`]'s own doc has the full
    /// design). A bound position is a path in the one tree, like [`Holdings::cwd`], so a bind can
    /// point through a mount point with nothing special to say about it.
    pub binds: nav::Bindings,
}

/// **A second tree, and where it is mounted in the first** (milestone 154). Provisional name and
/// shape.
///
/// calef ruled the presentation on 2026-09-26: *"One tree with other trees mounted at names in
/// it."* The user sees one root; the second tree appears at a mount point, like a Unix mount or a
/// Plan 9 `bind`, and which capability serves a name is bookkeeping the shell keeps to itself. So
/// the whole of what this records is the mount point. Where the shell stands is a path in the one
/// tree ([`Holdings::cwd`]); which tree that is follows from whether the path runs through here.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SecondDir {
    mount: nav::Cwd,
}

impl SecondDir {
    /// A second tree mounted at `mount`. `None` at the root: the root is the first tree's, and a
    /// mount there would hide all of it.
    pub fn mounted_at(mount: nav::Cwd) -> Option<Self> {
        (!mount.is_root()).then_some(SecondDir { mount })
    }

    /// Where the second tree is mounted, as a path in the one tree.
    pub fn mount(&self) -> nav::Cwd {
        self.mount
    }
}

impl Holdings {
    /// **Where `token` lands, without moving**: a path in the one tree this shell presents.
    /// [`Holdings::locate`] turns it into the tree that serves it and the position inside that
    /// tree.
    ///
    /// A relative token resolves from where the shell stands and an absolute one from the root, or
    /// from a bound position when its first component is a bound name. `..` is plain path
    /// arithmetic, so `..` from a mount point is its parent in the first tree, as a Unix mount is,
    /// and `..` at the root refuses as it always has.
    pub fn resolve(&self, token: &[u8]) -> Result<nav::Cwd, nav::Refused> {
        let p = nav::path(token)?;
        self.resolve_path(&p)
    }

    /// [`Holdings::resolve`] for a token already parsed.
    pub fn resolve_path(&self, p: &nav::Path<'_>) -> Result<nav::Cwd, nav::Refused> {
        self.resolve_steps(p.is_from_root(), p.steps())
    }

    /// [`Holdings::resolve`] for a bare step sequence and where it starts from, which is what a
    /// path's *lead* is (every step but the last).
    pub fn resolve_steps(
        &self,
        from_root: bool,
        steps: &[nav::Step<'_>],
    ) -> Result<nav::Cwd, nav::Refused> {
        let mut pos = nav::Cwd::root();
        let rest = self.anchor(from_root, steps, &mut pos);
        pos.apply(rest)?;
        Ok(pos)
    }

    /// **Where a step sequence starts**, written into `pos`, and the steps left to apply from
    /// there: where the shell stands for a relative sequence, a bound position with the bound
    /// name consumed for one whose first component is bound, and the root otherwise.
    ///
    /// **The position is an out-parameter rather than part of the result**, and that is a measured
    /// choice rather than a style: every caller is on a shell's walking or planning path, the shell
    /// runs a debug build on a few pages of stack, and returning a `Cwd` by value put nearly 2 KiB
    /// in this frame, which overflowed the globbing witness (`glob_grant_tests`).
    pub fn anchor<'s, 'p>(
        &self,
        from_root: bool,
        steps: &'s [nav::Step<'p>],
        pos: &mut nav::Cwd,
    ) -> &'s [nav::Step<'p>] {
        if !from_root {
            *pos = self.cwd;
            return steps;
        }
        *pos = nav::Cwd::root();
        if let Some((nav::Step::Down(first), rest)) = steps.split_first()
            && let Some(entry) = self.binds.lookup(first)
        {
            *pos = entry.pos();
            return rest;
        }
        steps
    }

    /// **Which tree serves a path in the one tree, and where inside it**: the second tree, from
    /// its own root, for a path that runs through its mount point; the first tree, unchanged, for
    /// everything else. This is the bookkeeping calef's ruling keeps out of sight: a path never
    /// says which tree it is in, and this is the one place that decides.
    pub fn locate(&self, place: &nav::Cwd) -> (nav::Which, nav::Cwd) {
        if let Some(sd) = &self.second {
            let m = &sd.mount;
            if place.depth() >= m.depth()
                && (0..m.depth()).all(|l| place.component(l) == m.component(l))
            {
                let mut inside = nav::Cwd::root();
                for l in m.depth()..place.depth() {
                    inside.descend(place.component(l));
                }
                return (nav::Which::B, inside);
            }
        }
        (nav::Which::A, *place)
    }

    /// [`Holdings::locate`] reversed: the path in the one tree for a position inside `which`. What
    /// `caps` prints for a planned grant, which records the tree and the position rather than the
    /// path it came from.
    pub fn place(&self, which: nav::Which, pos: &nav::Cwd) -> nav::Cwd {
        match (which, &self.second) {
            (nav::Which::B, Some(sd)) => {
                let mut place = sd.mount;
                for l in 0..pos.depth() {
                    place.descend(pos.component(l));
                }
                place
            }
            _ => *pos,
        }
    }

    /// **`bind`'s mutator**: name a position this shell already reached some other way
    /// ([`nav::Bindings::add`]'s own doc has the full design), refusing, in addition to everything
    /// the bind table itself refuses, a name that would hide the path to the mount point: a bound
    /// name wins over the literal first component of an absolute path.
    pub fn bind(&mut self, name: &[u8], pos: nav::Cwd) -> Result<(), nav::BindRefused> {
        if let Some(sd) = &self.second
            && name == sd.mount.component(0)
        {
            return Err(nav::BindRefused::ReservedLabel);
        }
        self.binds.add(name, pos)
    }
}

#[cfg(test)]
mod tests;

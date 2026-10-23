//! **What a parsed line is, and what a grant holds**: `Command`, `Endowment`, the file and
//! directory grants, `rm`'s option bits, and the shell's `Holdings`. `ParsedInvocation` is in `parsing`,
//! beside the one function that builds it (§272 (the grant-plan crate root splits into private
//! modules), fork 3). The crate root re-exports every public item.
//!
//! Name: provisional (milestone 842, 2026-10-11), as are its sibling modules' names.

use crate::expand::{Name, NameSet};
use crate::{ParsedInvocation, Program, line, nav};

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
    /// A prefix word, carrying a whole command line for [`Caps`](Command::Caps)'s reason: `time
    /// date | wc` times the pipeline, so the tail keeps its operators and the shell re-dispatches
    /// it as it would untimed. It grants nothing: the shell times the tail with its own clock, so a
    /// child holding no clock can be timed. See notes/time-command.md.
    Time(&'a [u8]),
    /// `xargs <command>`: run that command once per **batch** of what its pattern matched
    /// (milestone 109 (`xargs`: batching a grant too large to hand over)). The name is provisional.
    ///
    /// A prefix word that grants nothing, as [`Command::Time`] is. A batching program would have to
    /// hold the whole directory, since nothing carries the union of the batches, and a name in a
    /// pipe is text that grants a program nothing. See notes/glob-grant.md.
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
    /// not there (a no-op if it already is), then set its modification time to now, or with `-t`
    /// to a caller-chosen instant (DECISIONS §112: the ability to *lie about history*).
    ///
    /// A builtin in [`Mkdir`](Command::Mkdir)'s category: both forms take only the shell's own
    /// directory capability (`dir::WRITE`, and `dir::SETTIME` for `-t`). [`TouchOperands::at`]
    /// carries the `-t` text unparsed; `calendar` reads it where the grant is made (`Nav::touch` in
    /// `components/src/swish/navigation.rs`). See notes/touch.md.
    Touch(TouchOperands<'a>),
    /// `bind <target> <name>`: name a position this shell already reached some other way, so
    /// `/<name>/...` reaches it too ([`nav::Bindings`]). A builtin in [`Mkdir`](Command::Mkdir)'s
    /// category: it mints no capability and spawns nothing.
    ///
    /// `target` is resolved at bind time against wherever this shell stands, as any operand is;
    /// `name` is the single component the entry is filed under. Neither is tokenized further, as
    /// [`Command::Cd`]'s and [`Command::Mkdir`]'s one path operand is not.
    Bind(&'a [u8], &'a [u8]),
    /// `apropos <term>`: **name the installed pages that mention a word** (milestone 40 (documentation
    /// as a system service) phase 2). The operand is one word, folded the way the host builder
    /// folded the text it indexed.
    ///
    /// A builtin in [`Ls`](Command::Ls)'s category: search is enumeration the shell already holds,
    /// and a searching program would need the whole documentation store. It yields names, never
    /// capabilities, so a grant happens only when a person types a result at a program. See
    /// notes/documentation/apropos-builtin.md.
    Apropos(&'a [u8]),
    /// `vouch <path>`: **the owner vouches for a file's bytes** (DECISIONS §221 (the boot prompt is
    /// the owner's console)), so they run vouched, recorded under the path's last component. An
    /// activation request (`spawnproto::Activation::Vouch`) on the spawn endpoint, which `jig`'s
    /// installer endpoint refuses (milestone 809 (the package client becomes a program)), so a
    /// vouch stays the owner's console's, and the tail is classified by [`vouch_verb`](crate::vouch_verb).
    /// A builtin because it names no package: the bytes are what is vouched. Name: provisional
    /// (2026-09-26).
    Vouch(&'a [u8]),
    /// `user suspend <name>` and `user resume <name>`: **the owner suspends or resumes an identity**
    /// (milestone 152 (durable delegation), calef's §108 (disabling credentials kills the durable session) ruling of 2026-09-26, names ratified), by
    /// editing the suspended list at the root of this shell's directory. The tail is classified by
    /// [`user_verb`](crate::user_verb). A builtin because that file is the owner's authority only
    /// when this shell is the owner's console; a confined shell's root is its own subtree, where a
    /// `suspended` file is one nothing reads.
    User(&'a [u8]),
    /// `jobs`: **list the background jobs this shell holds** (milestone 48 (job control), phase
    /// one). A builtin in `caps`'s category: a job is the capabilities the shell holds for it, so
    /// listing jobs is the shell reading its own holdings. Name: provisional.
    Jobs,
    /// `wait [%N]`: **block until a background job finishes**, every job when no operand names
    /// one. The tail is classified by [`job_operand`](crate::job_operand). Name: provisional.
    Wait(&'a [u8]),
    /// `kill [-9] %N`: **ask a background job to stop, or with `-9` tear it down.** The two tiers
    /// are DECISIONS §24 (interrupting the foreground process)'s: the cooperative flag on the job's shared frame, and `DESTROY` of the
    /// region the shell built it from. The tail is classified by [`kill_operands`](crate::kill_operands).
    /// Name: provisional.
    Kill(&'a [u8]),
    /// `fg [%N]`: **watch a background job in the foreground**, the most recent when no operand
    /// names one, under the same `^C` escalation a job started in the foreground gets. Nothing is
    /// revoked or granted: no job holds terminal input, so foreground means the shell is watching.
    /// Name: provisional.
    Fg(&'a [u8]),
    /// `bg [%N]`: **resume a job `^Z` stopped, without watching it**, the newest stopped job when
    /// no operand names one. The resume is `ThreadControlBlock::RESUME` on the TCB the shell holds
    /// for a supervised job (DECISIONS §309 (a supervised job's `ThreadControlBlock` goes to the
    /// shell that asked for it)).
    ///
    /// Name: provisional, minted 2026-10-18 (UTC) by lane/48-ctrl-z.
    Bg(&'a [u8]),
    /// A program invocation: `<prog> [--mem N] [token ...]`. Named `Run` for the act of running a
    /// program, not for a verb on the line; milestone 47 deleted the verb. A first word that is not
    /// a builtin lands here even when no such program exists, and [`plan`](crate::plan) answers
    /// [`Refusal::NoSuchProgram`](crate::Refusal::NoSuchProgram), which is the "there is nothing there to name" shape of
    /// no-ambient-authority applied to programs.
    Run(ParsedInvocation<'a>),
}

/// [`Command::Touch`]'s operand: the name, and (with `-t`) the instant asserted for it.
///
/// `at` carries the `-t` token's bytes exactly as typed, unparsed: this crate classifies which
/// token is which, it does not know what an RFC 3339 instant is. `None` is bare `touch`, which
/// bumps `name`'s mtime to now with no assertion at all.
///
/// Name: ratified 2026-10-18 (calef: "Approve the family").
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TouchOperands<'a> {
    /// The `-t` operand's text, unparsed. `None` for bare `touch`.
    pub at: Option<&'a [u8]>,
    /// The name to create-if-absent and then touch.
    pub name: &'a [u8],
}

/// The most short-option letters one invocation may carry (`rm -rfv` is three). A ceiling rather
/// than a limit anybody meets, and past it the line is [`Refusal::Unexpected`](crate::Refusal::Unexpected) rather than quietly
/// short of an option the user typed.
///
/// Name: ratified 2026-10-18 (calef: "Yes").
pub const MAX_SHORT_OPTIONS: usize = 8;

/// What a valid `run` resolves to: exactly the authority to hand the child, and nothing else.
/// Reading this is reading the whole endowment, which is §14's "one literal tells you a process's
/// authority" made concrete.
///
/// **It borrows nothing from the command line**: a name a pattern produced comes out of a directory
/// listing rather than out of the line, so the grant owns it. With [`FileGrant::directory`] a
/// value, not a pointer at the shell, nothing after planning can change what an endowment means.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Endowment {
    pub program: Program,
    /// The integer argument to start it with (0 when the program takes none).
    pub program_argument: u64,
    /// Pages of untyped to split from the shell's own budget and grant (0 = none).
    pub memory_grant_pages: u64,
    /// The one file to narrow a directory capability down to, and the direction, or `None`.
    /// Delivered as an endpoint served by a file caretaker (`components/src/fs_file_caretaker.rs`), so
    /// what the child ends up holding designates this name and nothing else.
    pub file: Option<FileGrant>,
    /// The one directory to narrow down to, and the **names** in it the program is to act on, or
    /// `None`. Delivered as an endpoint served by a caretaker, so what the child ends up holding
    /// reaches that directory and nothing above or beside it: `components/src/fs_subtree_caretaker.rs`
    /// for a set of one, `components/src/fs_nameset_caretaker.rs` for the set a pattern matched.
    pub directory: Option<DirectoryGrant>,
    /// **The short options that were on the line**, as a bitmask: bit `i` is set when the manifest's
    /// `short_options[i]` was typed. Numbered by position in the manifest rather than by letter, so nothing
    /// here has to know what any option means; the program and the manifest agree on the order, and
    /// a host test pins them (`grant_plan::rmopt`).
    ///
    /// Name: ratified 2026-10-18 (calef: "Yes").
    pub short_options: u64,
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
/// [`directory`](FileGrant::directory) is where [`plan_against`](crate::plan_against) resolved
/// the operand's leading path, held as a value, so a `cd` after planning cannot change what the
/// grant means and the child has no cwd to re-resolve against. See notes/shell-navigation.md ("The
/// cwd stops at the process boundary").
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
    pub directory: nav::Cwd,
    /// The final component. Always a single component: a path was resolved into `dir`, not passed
    /// on, because the FS contract takes one component per request and no server here walks a path.
    ///
    /// Owned rather than borrowed from the line, because a pattern that matched exactly one file is
    /// a legitimate way to designate one and that name came from a directory listing.
    pub name: Name,
    pub writable: bool,
}

/// One resolved **directory** grant: the directory the child is handed a capability to, the
/// **set of names** in it the program was told to act on, and whether that capability must reach
/// below the directory or only name what is directly in it. These three are the whole authority,
/// which is what `caps rm -r logs` prints before anything happens.
///
/// It is [`FileGrant`]'s shape one rung up: the directory is fixed at plan time as a value. The
/// names are a set because `rm *.txt` designates many, and a directory capability attenuated to
/// a name set (`components/src/fs_nameset_caretaker.rs`) is the one answer that neither exhausts
/// slots nor over-grants. A literal operand is the set of one. See notes/glob-grant.md.
///
/// Name: ratified 2026-10-18 (calef: "Approve the family").
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DirectoryGrant {
    /// **Which of the shell's trees `dir` is in**, [`FileGrant::which`]'s reason (milestone 154).
    pub which: nav::Which,
    /// The directory the capability designates, relative to the root of [`DirectoryGrant::which`]'s
    /// tree, fixed at plan time.
    pub directory: nav::Cwd,
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

/// **`rm`'s options**, by the bit they occupy in [`Endowment::short_options`], which is their position in
/// [`Program::Rm`]'s manifest. Named here so the shell, the program and the tests share one definition
/// rather than three copies of an ordering; `rm_options_are_numbered_by_their_position` pins them.
pub mod rmopt {
    /// `-r`: recurse. It is also `rm`'s `subtree_option`, so typing it visibly widens the grant.
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
/// the parser nor the manifest can tell them apart.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Holdings {
    /// The shell holds a directory capability it can narrow into a per-file grant.
    pub directory: bool,
    /// **A second directory capability, mounted at a position in the first** (milestone 154, a
    /// process that holds two directory capabilities). `None` for a one-grant shell, which
    /// resolves and prints exactly as it always has.
    ///
    /// Name: ratified 2026-10-18 (calef: "Yes"). The shape is provisional.
    pub mounted: Option<MountedTree>,
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

/// **A second tree, and where it is mounted in the first** (milestone 154).
///
/// calef ruled the presentation on 2026-09-26: *"One tree with other trees mounted at names in
/// it."* The user sees one root; the second tree appears at a mount point, like a Unix mount or a
/// Plan 9 `bind`, and which capability serves a name is bookkeeping the shell keeps to itself. So
/// the whole of what this records is the mount point. Where the shell stands is a path in the one
/// tree ([`Holdings::cwd`]); which tree that is follows from whether the path runs through here.
///
/// Name: ratified 2026-10-18 (calef: "Yes"). The shape is provisional.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MountedTree {
    mount: nav::Cwd,
}

impl MountedTree {
    /// A second tree mounted at `mount`. `None` at the root: the root is the first tree's, and a
    /// mount there would hide all of it.
    pub fn mounted_at(mount: nav::Cwd) -> Option<Self> {
        (!mount.is_root()).then_some(MountedTree { mount })
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
        if let Some(sd) = &self.mounted {
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
        match (which, &self.mounted) {
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
        if let Some(sd) = &self.mounted
            && name == sd.mount.component(0)
        {
            return Err(nav::BindRefused::ReservedLabel);
        }
        self.binds.add(name, pos)
    }
}

#[cfg(test)]
mod tests;

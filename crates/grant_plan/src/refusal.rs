//! **Why an invocation was refused**: `Refusal`, the fixed sentence each variant prints, and
//! `CapabilityKind`.
//!
//! Moved out of the crate's root unchanged by milestone 842 (the grant-plan crate root is split
//! along its seams), as §272 (the grant-plan crate root splits into private modules) lays out; the
//! root re-exports every public item at its old path.
//!
//! Name: provisional (milestone 842, 2026-10-11), as are its sibling modules' names.

/// Why an invocation was refused, decided at the prompt before any spawn. Each variant maps to one
/// legible line; [`Refusal::message`] is the fixed half, host-tested so the wording cannot drift.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refusal {
    /// The named program does not exist. Also where a mistyped builtin lands, since with the `run`
    /// verb gone the first word is either a builtin or a program name.
    NoSuchProgram,
    /// The command designated a resource this shell holds no capability for: a file named at a
    /// program that takes one, in a shell that was never granted a directory to narrow. This is the
    /// refusal the milestone is about: not "permission denied" but "there is nothing you hold that
    /// could grant this."
    NoSuchCapability(CapabilityKind),
    /// The program takes no file, but a token was left over that nothing else could be. The
    /// milestone's inversion cuts both ways: a name the program has no use for is authority the user
    /// did not mean to move, so it is refused rather than granted-and-ignored. **This is the refusal
    /// the `file:` prefix was wrongly credited with**: `least_authority_demo 5 extra` stops here because `least_authority_demo`'s
    /// manifest says `FileSpec::Forbidden`, which is what refused it before the prefix went away.
    FileForbidden,
    /// The program is endowed a file and the command named none. Naming it on the line is the only
    /// way it can get one, so this is caught at the prompt rather than as an empty slot at runtime.
    FileRequired,
    /// The program is endowed a **directory** and the command named nothing in one. `rm` with no
    /// operand is this: there is no default, because a default would be a grant nobody typed.
    PathRequired,
    /// A short option this program does not declare. Refused at the prompt rather than handed over
    /// for the program to ignore, and it is the same rule as an unplaceable token: **the line means
    /// exactly what it says or it does not run.** It matters more than it looks here, because an
    /// option can widen a grant (`rm -r`), so a shell that silently dropped one would be a shell
    /// whose printed preview and actual transfer could disagree.
    NoSuchOption,
    /// The named file is not something this shell can express: empty, a component longer than the
    /// two argument words a grant's name rides in (`filesystem_contract::grant::MAX_NAME`), deeper than the
    /// shell tracks, or a path that designates a directory rather than a file.
    FileNotNameable,
    /// The program takes no memory grant, but `--mem` was given.
    ///
    /// Name: ratified 2026-10-18 (calef: "Approve the family").
    MemoryGrantForbidden,
    /// The program requires a memory grant, but none was given.
    ///
    /// Name: ratified 2026-10-18 (calef: "Approve the family").
    MemoryGrantRequired,
    /// `--mem N` was outside the program's declared range.
    ///
    /// Name: ratified 2026-10-18 (calef: "Approve the family").
    MemoryGrantOutOfRange { min: u64, max: u64 },
    /// The program requires an integer argument, but none was given.
    ///
    /// Name: ratified 2026-10-18 (calef: "Approve the family").
    ProgramArgumentRequired,
    /// The program takes no argument, but one was given.
    ///
    /// Name: ratified 2026-10-18 (calef: "Approve the family").
    ProgramArgumentForbidden,
    /// **A pattern matched no name here** (milestone 47's globbing lane). zsh's default rather than
    /// bash's pass-the-pattern-through, and the model forces it: the expansion *is* the grant, so an
    /// empty expansion is an empty grant and running the command would be running it with an
    /// authority nobody named. See [`expand::Expander::finish`](crate::expand::Expander::finish).
    NoMatch,
    /// **A pattern matched more names than one grant can carry.** Unix's `ARG_MAX` with a more
    /// honest cause: not a buffer that ran out but a capability the child could neither hold nor be
    /// shown. Loud at the prompt with nothing spawned, because a glob that quietly granted a prefix
    /// of what it matched is the worst outcome this mechanism has.
    TooManyNames,
    /// A name a pattern matched cannot travel in a grant (longer than
    /// [`expand::MAX_NAME`](crate::expand::MAX_NAME)). Refused whole rather than dropped from the set, for
    /// [`Refusal::TooManyNames`]'s reason.
    MatchNotNameable,
    /// A pattern somewhere other than a token's **last** component (`*/report.txt`). Selecting
    /// directories to walk is an authority question, not a matching one, so it is not something a
    /// string matcher gets to answer (notes/glob.md).
    PatternInPath,
    /// A pattern reached the grant planner with no expansion behind it. It is a fact about the
    /// shell rather than about the line: expanding needs `ENUMERATE` on the directory, and a shell
    /// that holds none cannot say what the pattern designates. Never a silent grant of a name with a
    /// `*` in it, which is a name no directory here has.
    Unexpanded,
    /// A pattern matched more than one name at a program that is granted exactly one **file**. The
    /// count is the designation, so this is a fact about the line rather than a policy.
    AmbiguousFile,
    // ---- milestone 50's operators. Every one of these is decided before anything is spawned,
    // which matters more here than elsewhere: a pipeline that half-starts leaves a process blocked
    // on a channel nobody will ever write to.
    /// **`>` or `<` with nothing after it.** A redirection is a designation, and there is nothing
    /// designated.
    NoRedirectTarget,
    /// **One stage, two destinations** (`date > a > b`), or two inputs. Neither "the last wins" nor
    /// "the first wins" is a rule anybody should have to remember.
    RedirectRepeated,
    /// **A redirection somewhere a pipeline already answers the question.** `a > f | b` says where
    /// `a`'s bytes go and then pipes them somewhere else. Refused rather than resolved by
    /// precedence.
    RedirectMidPipeline,
    /// **A word after a redirection's name** (`< f wc`). The spelling this shell takes is `wc < f`.
    /// See notes/pipes.md: refusing the other order is what keeps a stage's text a slice of the
    /// line, and `grant_plan` has no allocator.
    WordAfterRedirect,
    /// **A pipeline stage with no command in it** (`| wc`, `a |`).
    ///
    /// **This listed `a || b` until milestone 67 and no longer can**: `||` is a connector now, and
    /// the shell splits on it before [`line::split`](crate::line::split) ever sees the line, so that spelling is two
    /// commands rather than a pipeline with a hole in it. A `||` still reaches this function in a
    /// direct call, which is why the sequence splitter has to run first rather than merely usually
    /// running first.
    EmptyStage,
    /// **More stages than one line may carry** ([`line::MAX_STAGES`](crate::line::MAX_STAGES)). A ceiling on processes and
    /// endpoints, not on a buffer.
    TooManyStages,
    /// **A pattern where a redirection's name goes.** A redirection designates one destination and
    /// a pattern designates a set, so even a pattern that matched exactly one name would make what
    /// the line writes to depend on what is in the directory.
    PatternInRedirect,
    /// **This program's output is not a byte stream**, so there is nothing for `>` or `|` to
    /// substitute. See [`OutputSpec`](crate::OutputSpec): `least_authority_demo 9` answers with a number in a register, and a file
    /// sink handed that word would write nothing legible into the file.
    NotAByteStream,
    /// **This program declares no second output stream**, and the line put a `2>` on it (DECISIONS
    /// §67). The truthful refusal the decision names: `2>` binds to something the manifest offers,
    /// and a program whose diagnostics ride its one output in-band has nothing for it to bind to.
    /// It is not "you may not do that"; it is "there is no second stream here to name", which is
    /// [`NoSuchProgram`](Refusal::NoSuchProgram)'s shape applied to a stream.
    NoDiagnosticStream,
    /// **This program reads no input**, and the line gave it one. Authority that moved for no
    /// reason, refused for the same reason an unplaceable token is.
    InputForbidden,
    /// **This program reads an input stream and the line gave it none.** `wc` on its own would
    /// block on a receive forever; the manifest says so, so the prompt can. **The refusal Unix
    /// cannot produce**, because there fd 0 always exists.
    InputRequired,
    /// **This shell would be both the writer and the reader of the line, and it can only wait on
    /// one thing.** Every stage declares that it writes while it reads
    /// ([`InputSpec::Required::writes_while_reading`](crate::InputSpec::Required::writes_while_reading)), so nothing in the chain absorbs the stream,
    /// and the shell's feed would block against a stage that is blocked writing back.
    ///
    /// It is a fact about **this shell** rather than about the program, which is why the fix is to
    /// put a reader after it (`| wc`) rather than to change anything the program does. See
    /// [`check_chain`](crate::check_chain) and notes/pipes.md: with no select, no poll and no timed receive, one wait
    /// point per process is the whole of the reason.
    NoReaderButThisShell,
    /// **A builtin in a pipeline stage that the shell cannot supply.** `cd | wc` is the shape: a
    /// builtin rebinds what the shell holds and produces no stream. The producing builtins (`echo`,
    /// `ls`, `pwd`) are not this, because the shell writes their bytes into the pipe itself.
    NotAPipelineStage,
    /// A token no slot in the manifest can hold, and that no other refusal reads truer for: a
    /// second integer at a program that takes one, a flag that is not `--mem`, a name past the one
    /// file a program is granted. Refused rather than ignored, so authority the user thought they
    /// were granting never silently evaporates.
    Unexpected,
    // ---- milestone 67's quoting and sequencing. Both are facts about the *line*, decided before
    // any manifest is consulted, which is why they sit at the end rather than beside the grant
    // refusals above.
    /// **A quote that never closes** (`wc 'my notes.txt`). The line is refused whole rather than
    /// run to the last complete word, because a line missing its end designates something nobody
    /// typed. See [`word`](crate::word).
    UnclosedQuote,
    /// **A quote that wraps part of a word** (`a"b"`, `'it''s'`). Every token here is a slice of
    /// the line you typed, so two quoted pieces cannot be joined into one word; the alternative is
    /// a byte buffer, and `grant_plan` has no allocator. Refused rather than misread, because both
    /// readings (join them, or take the quotes literally) would be silently wrong. See [`word`](crate::word).
    PartlyQuoted,
    /// **A connector with nothing on one side of it** (`&& date`, `date &&`). The mirror of
    /// [`EmptyStage`](Refusal::EmptyStage) one level out: `&&` asks about a command that ran, and
    /// there is no command.
    EmptySegment,
    /// **More commands on one line than the shell will sequence.** A ceiling like
    /// [`TooManyStages`](Refusal::TooManyStages), and a line that wants more is better written as
    /// several lines than silently truncated.
    TooManySegments,
    // ---- milestone 205's argv (DECISIONS §170).
    /// **A pattern on the line of a program that hears words** (`rg foo *.rs`). A pattern elsewhere
    /// designates the names it matched, and that is a grant; here the words carry no authority, so
    /// expanding one would hand over names without handing over anything they name. Refused rather
    /// than passed through literally, because every other shell a person has used would have
    /// expanded it, and a program searching a file called `*.rs` is a silent misreading. Quoting
    /// the word passes it as it is. Name: provisional.
    PatternInArguments,
    /// **The words do not fit on one argument page** (`program_argument_contract::CAPACITY`, 4,080 bytes
    /// with a four-byte length per word). Refused whole rather than truncated, for
    /// [`Refusal::TooManyNames`]'s reason. Name: provisional.
    ArgumentsTooLong,
}

/// The kind of capability a command designated but the shell cannot back. Phase 1 has one; the
/// enum exists so milestone 32 (files) and later device/endpoint grants slot in without changing
/// the refusal's shape.
///
/// Name: recorded (design/naming/capability-spelling-worklist.md). `CapKind` until 2026-10-11 (UTC),
/// renamed by #1913 under calef's ruling of 2026-10-06 that capability is not abbreviated as cap.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CapabilityKind {
    /// A file or directory, designated by naming it. Arrives with milestone 32's FS server.
    File,
}

impl Refusal {
    /// The fixed, program-independent half of the refusal line. The shell prefixes the program
    /// name or the offending token where a variant needs it. The strings are the deliverable's
    /// voice: a refusal that reads like the capability model, not like errno.
    pub fn message(self) -> &'static str {
        match self {
            // The hint is not decoration. With `run` gone, a mistyped builtin and a mistyped
            // program name arrive at the same refusal, so this line has to be able to point at
            // both halves of what the prompt understands.
            Refusal::NoSuchProgram => "no such program (try 'help' for the builtins)",
            Refusal::NoSuchCapability(CapabilityKind::File) => {
                "you hold no such capability: this shell was granted no directory to narrow"
            }
            // "drop the name", in the same voice as "drop the --mem": the token would have been a
            // file, and this program is declared to take none.
            Refusal::FileForbidden => "takes no file; drop the name",
            Refusal::FileRequired => "is granted one file; name it on the line",
            Refusal::PathRequired => "is granted the directory holding what you name; name it",
            Refusal::NoSuchOption => "no such option (this shell will not pass one along unread)",
            Refusal::FileNotNameable => {
                "that is not a name this shell can grant: at most 16 bytes a component, 8 deep"
            }
            // Each of these says what the *pattern* designates, because that is the grant. None of
            // them can be phrased as a permission, and none should be: nothing was denied, the
            // command either named something grantable or it did not.
            Refusal::NoMatch => "no name here matches that pattern, so there is nothing to grant",
            Refusal::TooManyNames => {
                "that pattern matched more names than one grant can carry (at most 8)"
            }
            Refusal::MatchNotNameable => {
                "a name it matched cannot travel in a grant: at most 16 bytes"
            }
            Refusal::PatternInPath => {
                "a pattern names files, not directories to walk: only the last component may be one"
            }
            Refusal::Unexpanded => {
                "there is nothing here to expand that pattern against: this shell holds no directory"
            }
            Refusal::AmbiguousFile => {
                "is granted one file, and that pattern matched more than one name"
            }
            Refusal::MemoryGrantForbidden => "takes no memory grant; drop the --mem",
            Refusal::MemoryGrantRequired => "needs a memory grant; add --mem <pages>",
            Refusal::MemoryGrantOutOfRange { .. } => "memory grant is out of the range it declares",
            Refusal::ProgramArgumentRequired => "needs an integer argument",
            Refusal::ProgramArgumentForbidden => "takes no argument",
            Refusal::Unexpected => {
                "unexpected argument (this shell will not grant what it cannot place)"
            }
            // The operators' refusals. Each says what the line designates rather than what was
            // denied, in the same voice as the grant refusals above: nothing here consults a
            // permission, and a pipeline that could not be built is a line that did not name one.
            Refusal::NoRedirectTarget => "a redirection names a file; this one names nothing",
            Refusal::RedirectRepeated => {
                "one output and one input to a stage: this line names two of one of them"
            }
            Refusal::RedirectMidPipeline => {
                "the pipeline already says where that goes: only the first stage takes input and \
                 only the last is redirected"
            }
            Refusal::WordAfterRedirect => {
                "the redirection goes last: write 'wc < report.txt', not '< report.txt wc'"
            }
            Refusal::EmptyStage => "a pipeline stage with no command in it",
            Refusal::TooManyStages => "that is more stages than one line carries (at most 4)",
            Refusal::PatternInRedirect => {
                "a redirection names one file, and a pattern designates a set"
            }
            Refusal::NotAByteStream => {
                "does not write a byte stream, so there is nothing for > or | to redirect"
            }
            Refusal::NoDiagnosticStream => {
                "declares no second output, so there is nothing for 2> to name (its diagnostics \
                 ride its output)"
            }
            Refusal::InputForbidden => "reads no input; there is no slot for those bytes to go in",
            Refusal::InputRequired => {
                "reads an input stream: name a file, redirect with '<', or pipe into it"
            }
            Refusal::NotAPipelineStage => {
                "is a builtin that produces no stream, so it cannot be a stage of a pipeline"
            }
            // Phrased about the line rather than about the program, because it is true of neither
            // one alone: the program is fine and the shell is fine, and what cannot be done is this
            // shell feeding it and reading it at the same time.
            Refusal::NoReaderButThisShell => {
                "writes while it reads, and this shell can only wait on one thing at a time: give \
                 it a reader that is not this shell, as in '| wc'"
            }
            // Milestone 67's. Each names what the line is missing rather than what it did wrong,
            // and the quoting pair say *why* the shape is the shape: a token here is a slice of
            // what you typed, so there is nothing to join pieces into.
            Refusal::UnclosedQuote => "a quote here is never closed, so the line ends mid-name",
            Refusal::PartlyQuoted => {
                "quote a whole word: this shell hands a name over as the bytes you typed, so it \
                 cannot join pieces of one"
            }
            Refusal::EmptySegment => "a connector with no command on one side of it",
            Refusal::TooManySegments => "that is more commands than one line sequences (at most 8)",
            Refusal::PatternInArguments => {
                "its words are not expanded, so a pattern would reach it as it is: quote it to mean that"
            }
            Refusal::ArgumentsTooLong => {
                "those words do not fit on its argument page (4,080 bytes, four more for each word)"
            }
        }
    }
}

#[cfg(test)]
mod tests;

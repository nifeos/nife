//! **The operators: `>`, `<` and `|`.** The shape of a command line, one level above
//! [`crate::parse`]: the line becomes a [`Line`] of stages plus the file names at either end, and
//! each stage is then parsed as a whole line is. `>` and `|` put a different capability in a
//! program's output slot, and `<` in its input slot, so this module yields only [`Sink`] and
//! [`Source`] values. It refuses a word after a redirection ([`Refusal::WordAfterRedirect`]), a
//! redirection on the wrong end of a pipeline ([`Refusal::RedirectMidPipeline`]) and a pattern as a
//! target ([`Refusal::PatternInRedirect`]). `2>` names where a declared second stream goes (§67 (a
//! program's second stream is a declaration)); planning refuses it for a program declaring none
//! ([`Refusal::NoDiagnosticStream`]). See notes/pipes.md, and
//! notes/grant-expression/grant-plan-crate.md for the rest.

//! # BUGS
//!
//! - A line has one diagnostic stream, so `2>` on a pipeline where only some stages declare one is
//!   refused.
//! - `<<` is refused as [`Refusal::NoRedirectTarget`], about a missing name, not a missing feature.
//! - A quoted operator stays a byte, so `> "*.txt"` writes a file named `*.txt`.

use crate::{Refusal, word};

/// The most stages one pipeline may carry. Four is past what anybody types interactively and it is
/// a real ceiling rather than a buffer size: each stage costs a process and an endpoint, and a line
/// that wants more is better written as two lines than silently truncated.
pub const MAX_STAGES: usize = 4;

/// **Where a stage's bytes go**, which is the whole of what `>` and `|` decide.
///
/// The three variants are three different capabilities in one slot. The program holds an endpoint
/// with `WRITE` in every case and has no message it could send to ask which.
///
/// The file variant is large because a file grant's name may be 255 bytes, and a `Box` is not an
/// option: this crate is `no_std` with no `alloc`, so it stays host-testable. The size is paid where
/// it lands, which notes/glob-grant/xargs.md ("The bytes a set carries") measures.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Sink {
    /// The shell's own result endpoint: what a program is given when nothing on the line redirects
    /// it. Not "the terminal": the shell reads the bytes and decides what to do with them, which is
    /// what makes the default destination a capability like any other.
    #[default]
    Report,
    /// The next stage of a pipeline. An endpoint the shell minted, held `WRITE` here and `READ` by
    /// the stage downstream, and nothing else exists: **the rendezvous is the pipe**.
    Pipe,
    /// A file, named on the line, and what `>` versus `>>` decided about the bytes already in it.
    /// What the program holds is an endpoint either way, so it cannot seek, truncate, re-read or
    /// stat, and it cannot tell the two spellings apart.
    File(crate::FileGrant, Mode),
}

/// **What a `>` does to the bytes already in the file, which is the whole of `>` versus `>>`.**
///
/// It is a fact about how the **shell** opens the file and not about the stage that writes it
/// (DECISIONS §55: the file behind a `>` is the shell itself). A sink has no seek and appends by
/// construction, so both spellings hand the program the identical capability and differ only in
/// where the shell's first write lands. That is why this rides beside [`Sink::File`] rather than
/// inside [`crate::FileGrant`]: the grant is the authority, and this is not part of it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Mode {
    /// `>`: empty the file before the command runs. Emptying it *before* is what makes
    /// `ls > out.txt` report one more name than the `ls` before it did, which is Unix's order and
    /// is worth being able to see.
    #[default]
    Truncate,
    /// `>>`: keep what is there and start writing at the end of it.
    Append,
}

/// **Where a program's second output goes**, which is the whole of `2>` (DECISIONS §67).
///
/// It is [`Sink`]'s shape with one variant fewer, and the missing one says what the mechanism is:
/// there is no `Diagnostics::Pipe`, because a diagnostic that flowed down a `|` into a `wc` would be
/// counted as output, which is exactly the conflation the second stream exists to end.
///
/// [`None`](Diagnostics::None) is not "the program was silenced". It is **the program declares no
/// second stream**, which is every program but `date` today: its diagnostics ride its one output
/// in-band, as they always did.
///
/// The file variant is large because a file grant's name may be 255 bytes, and a `Box` is not an
/// option: this crate is `no_std` with no `alloc`, so it stays host-testable. The size is paid where
/// it lands, which notes/glob-grant/xargs.md ("The bytes a set carries") measures.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Diagnostics {
    /// This program's manifest declares no second output. Nothing is minted and nothing is
    /// delegated, and a `2>` aimed here is [`crate::Refusal::NoDiagnosticStream`].
    #[default]
    None,
    /// The declared default: the shell holds the other end and prints what arrives. This is what
    /// makes `date > when.txt` on a clockless machine print its complaint **and write nothing into
    /// the file**, which is the loss §67 exists to fix.
    Printed,
    /// `2>` named a file. The shell backs it exactly as it backs a `>` (DECISIONS §55), so the
    /// program still holds one endpoint per stream and can seek, truncate, re-read and stat
    /// neither.
    File(crate::FileGrant, Mode),
}

/// **Where a stage's bytes come from**, which is what `<` and the right-hand side of `|` decide.
/// [`Sink`]'s mirror, and deliberately the same contract read from the other end.
///
/// The file variant is large because a file grant's name may be 255 bytes, and a `Box` is not an
/// option: this crate is `no_std` with no `alloc`, so it stays host-testable. The size is paid where
/// it lands, which notes/glob-grant/xargs.md ("The bytes a set carries") measures.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Source {
    /// No input slot at all. A program declaring [`InputDeclaration::Forbidden`](crate::InputDeclaration) always
    /// has this, and it is the reason such a program can never block waiting for bytes nobody is
    /// going to send.
    #[default]
    None,
    /// The previous stage of a pipeline.
    Pipe,
    /// A file, streamed out over the sink contract by an adapter process.
    File(crate::FileGrant),
}

/// A whole command line with the operators taken out of it: the stages, and the names on the ends.
///
/// The stages are **slices of the line**, so this borrows and allocates nothing. Each is parsed
/// with [`crate::parse`], which is the same function that read a whole line before this module
/// existed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Line<'a> {
    stages: [&'a [u8]; MAX_STAGES],
    nstages: usize,
    /// The name after `<`, if the line has one. It belongs to the first stage.
    pub input: Option<&'a [u8]>,
    /// The name after `>` or `>>`, if the line has one. It belongs to the last stage.
    pub output: Option<&'a [u8]>,
    /// The name after `2>` or `2>>`, if the line has one. Like [`output`](Line::output) it is
    /// written at the tail, and unlike it what it names belongs to the **line**: the shell mints one
    /// diagnostic endpoint and every declaring stage writes to it.
    pub diagnostics: Option<&'a [u8]>,
    /// Which of the two spellings named it. Private because it means nothing without
    /// [`output`](Line::output), and a caller that could set the two independently could describe a
    /// line nobody could type; read it through [`mode`](Line::mode).
    append: bool,
    /// [`append`](Line::append) for `2>>`. Private for the same reason.
    diag_append: bool,
}

impl<'a> Line<'a> {
    /// The stages, in order, left to right.
    pub fn stages(&self) -> &[&'a [u8]] {
        &self.stages[..self.nstages]
    }

    /// How many stages the line has. Always at least one, which is why this is not spelled `len`:
    /// there is no empty line at this level, an empty line is one empty stage and
    /// [`crate::parse`] reads it as [`Command::Empty`](crate::Command::Empty).
    pub fn stage_count(&self) -> usize {
        self.nstages
    }

    /// Whether the line is a single stage with nothing on either end, which is every line that
    /// could be typed before this module existed. The shell's old path is kept for exactly these,
    /// so nothing that worked before pays for the operators.
    pub fn is_plain(&self) -> bool {
        self.nstages == 1
            && self.input.is_none()
            && self.output.is_none()
            && self.diagnostics.is_none()
    }

    /// Whether the line's `>` empties the file first. [`Mode::Truncate`] on a line with no `>` at
    /// all, which nothing reads: there is no file for it to describe.
    pub fn mode(&self) -> Mode {
        if self.append {
            Mode::Append
        } else {
            Mode::Truncate
        }
    }

    /// [`mode`](Line::mode) for the file `2>` named. `2>` and `2>>` differ in exactly what `>` and
    /// `>>` differ in, and for the same reason: the shell opens the file, so append is an initial
    /// offset rather than anything the program could ask for.
    pub fn diagnostics_mode(&self) -> Mode {
        if self.diag_append {
            Mode::Append
        } else {
            Mode::Truncate
        }
    }

    /// What stage `i`'s output slot holds: the pipe when something is downstream, the file when the
    /// line named one, and the shell's result endpoint otherwise.
    ///
    /// `file` is the caller's already-designated [`crate::FileGrant`], because resolving a name
    /// needs the shell's position and this module deliberately knows nothing about directories.
    pub fn sink_for(&self, i: usize, file: Option<crate::FileGrant>) -> Sink {
        if i + 1 < self.nstages {
            Sink::Pipe
        } else {
            match file {
                Some(g) => Sink::File(g, self.mode()),
                None => Sink::Report,
            }
        }
    }

    /// What stage `i`'s input slot holds. The mirror of [`sink_for`](Line::sink_for).
    pub fn source_for(&self, i: usize, file: Option<crate::FileGrant>) -> Source {
        if i > 0 {
            Source::Pipe
        } else {
            match file {
                Some(g) => Source::File(g),
                None => Source::None,
            }
        }
    }
}

/// Whether this byte is one of the three single-character operators.
fn is_op(b: u8) -> bool {
    b == b'|' || b == b'<' || b == b'>'
}

/// **Whether a `2>` starts here**, which is the one operator this grammar cannot recognise from a
/// single byte.
///
/// `word_start` is where the current word began, and the digit only counts as an operator at a word
/// boundary: `date 2> err` is a redirection, `least_authority_demo 2 > out` is the integer `2` followed by a `>`,
/// and `wc2>f` writes to a file from a program called `wc2`. Every shell draws the line in the same
/// place, and drawing it anywhere else would make a program's own argument disappear.
fn is_diag_op(line: &[u8], i: usize, word_start: usize) -> bool {
    line[i] == b'2'
        && line.get(i + 1) == Some(&b'>')
        && (i == word_start || line[i - 1].is_ascii_whitespace())
}

/// **Split a command line into stages and redirection targets.**
///
/// Pure and allocation-free: every slice it returns points into `line`. The module doc names the
/// refusals and the note that says why.
///
/// # Examples
///
/// ```
/// # use grant_plan::line::{split, Line, Mode};
/// let l = split(b"date | wc").unwrap();
/// assert_eq!(l.stages(), [&b"date"[..], b"wc"]);
/// assert!(l.input.is_none() && l.output.is_none());
///
/// let l = split(b"date > report.txt").unwrap();
/// assert_eq!(l.stages(), [&b"date"[..]]);
/// assert_eq!(l.output, Some(&b"report.txt"[..]));
/// assert_eq!(l.mode(), Mode::Truncate);
///
/// // `>>` names the same destination and only changes what happens to what is already in it.
/// assert_eq!(split(b"date >> report.txt").unwrap().mode(), Mode::Append);
///
/// // Whitespace around an operator is optional, as it is in every shell.
/// assert_eq!(split(b"date>report.txt").unwrap().output, Some(&b"report.txt"[..]));
/// ```
pub fn split(line: &[u8]) -> Result<Line<'_>, Refusal> {
    let mut stages = [&b""[..]; MAX_STAGES];
    let mut r = Redirects::new();
    let mut n = 0usize;

    let mut i = 0usize;
    loop {
        if n == MAX_STAGES {
            return Err(Refusal::TooManyStages);
        }
        let start = i;
        i = scan_command_words(line, start)?;
        stages[n] = crate::trim(&line[start..i]);

        // Then the redirections, which must all come after the command words.
        i = read_redirections(line, i, n, &mut r)?;

        n += 1;
        if i >= line.len() {
            break;
        }
        // The only operator that can be here is `|`, because the loop above consumed every `<` and
        // `>` it could reach.
        debug_assert_eq!(line[i], b'|');
        i += 1;
    }

    check_placement(&stages, n, &r)?;
    check_redirect_patterns(n, &r)?;

    Ok(Line {
        stages,
        nstages: n,
        input: r.ins[0],
        output: r.outs[n - 1],
        diagnostics: r.diags[n - 1],
        append: r.apps[n - 1],
        diag_append: r.diag_apps[n - 1],
    })
}

/// Each stage's redirection names, as [`split`] reads them: one slot per stream per stage.
struct Redirects<'a> {
    ins: [Option<&'a [u8]>; MAX_STAGES],
    outs: [Option<&'a [u8]>; MAX_STAGES],
    diags: [Option<&'a [u8]>; MAX_STAGES],
    // Whether each redirection's name arrived quoted. A quoted name is not a pattern, whatever
    // bytes are in it, so `date > "*.txt"` writes to a file called `*.txt` rather than being
    // refused for designating a set (milestone 67, notes/swish-language.md).
    in_q: [bool; MAX_STAGES],
    out_q: [bool; MAX_STAGES],
    diag_q: [bool; MAX_STAGES],
    apps: [bool; MAX_STAGES],
    diag_apps: [bool; MAX_STAGES],
}

impl Redirects<'_> {
    /// No redirection on any stage.
    fn new() -> Self {
        Redirects {
            ins: [None; MAX_STAGES],
            outs: [None; MAX_STAGES],
            diags: [None; MAX_STAGES],
            in_q: [false; MAX_STAGES],
            out_q: [false; MAX_STAGES],
            diag_q: [false; MAX_STAGES],
            apps: [false; MAX_STAGES],
            diag_apps: [false; MAX_STAGES],
        }
    }
}

/// One stage's command words, from `start`: answers the index of the first bare operator, or the
/// line's end.
fn scan_command_words(line: &[u8], start: usize) -> Result<usize, Refusal> {
    // The command words: everything up to the first **bare** operator or the end of the line.
    // Quoted is the whole of what milestone 67 added here: `echo "a > b"` has no redirection on
    // it, because the `>` arrives while the cursor is inside a quote.
    let mut i = start;
    let mut c = word::Cursor::new();
    while i < line.len() {
        if !c.is_open() && (is_op(line[i]) || is_diag_op(line, i, start)) {
            break;
        }
        c.step(line[i]);
        i += 1;
    }
    if c.is_open() {
        return Err(Refusal::UnclosedQuote);
    }
    Ok(i)
}

/// Stage `n`'s redirections, read from `i` into `r`. Answers where the next stage's `|` (or the
/// line's end) is.
fn read_redirections<'a>(
    line: &'a [u8],
    mut i: usize,
    n: usize,
    r: &mut Redirects<'a>,
) -> Result<usize, Refusal> {
    while i < line.len() && (line[i] == b'<' || line[i] == b'>' || is_diag_op(line, i, i)) {
        // Which stream this operator names, and how many characters spell it. `>>` is one
        // operator written with two characters and `2>>` one written with three; the extra
        // characters are consumed here rather than left to be read as operators with empty
        // names. A second `<` is not consumed: there is no here-document, so `<<` falls through
        // and is refused for the name it does not have.
        #[derive(PartialEq)]
        enum Which {
            In,
            Out,
            Diag,
        }
        let (which, append, width) = match line[i] {
            b'<' => (Which::In, false, 1),
            b'>' => {
                let a = line.get(i + 1) == Some(&b'>');
                (Which::Out, a, if a { 2 } else { 1 })
            }
            _ => {
                let a = line.get(i + 2) == Some(&b'>');
                (Which::Diag, a, if a { 3 } else { 2 })
            }
        };
        i += width;
        while i < line.len() && line[i].is_ascii_whitespace() {
            i += 1;
        }
        // **The name, which may be quoted** (milestone 67). The scan stops at a *bare*
        // whitespace or operator byte, so `> "my out.txt"` is one name and the space in it is
        // part of what is designated rather than the end of it. That is the correctness gap this
        // shell had: a file with a space in its name could not be written to at all.
        let t = i;
        i = word::span(line, i, &|b| b.is_ascii_whitespace() || is_op(b));
        if i == t {
            return Err(Refusal::NoRedirectTarget);
        }
        let name = word::read(&line[t..i])?;
        let (slot, quoted) = match which {
            Which::In => (&mut r.ins, &mut r.in_q),
            Which::Out => {
                r.apps[n] = append;
                (&mut r.outs, &mut r.out_q)
            }
            Which::Diag => {
                r.diag_apps[n] = append;
                (&mut r.diags, &mut r.diag_q)
            }
        };
        if slot[n].is_some() {
            return Err(Refusal::RedirectRepeated);
        }
        slot[n] = Some(name.text);
        quoted[n] = name.quoted;
        // A word after the name is the `< f wc` spelling. Refused rather than reordered: see
        // the module note.
        let mut j = i;
        while j < line.len() && line[j].is_ascii_whitespace() {
            j += 1;
        }
        if j < line.len() && !is_op(line[j]) && !is_diag_op(line, j, j) {
            return Err(Refusal::WordAfterRedirect);
        }
        i = j;
    }
    Ok(i)
}

/// Every stage has a command, and each redirection is on the stage it belongs to.
fn check_placement(
    stages: &[&[u8]; MAX_STAGES],
    n: usize,
    r: &Redirects<'_>,
) -> Result<(), Refusal> {
    // A pipeline stage with no command in it. A single empty stage is the empty line, which is a
    // prompt press and not a mistake.
    if n > 1 && stages[..n].iter().any(|s| s.is_empty()) {
        return Err(Refusal::EmptyStage);
    }
    // Input belongs to the head and output to the tail. Anywhere else the line is answering the
    // same question twice.
    for k in 0..n {
        if r.ins[k].is_some() && k != 0 {
            return Err(Refusal::RedirectMidPipeline);
        }
        if r.outs[k].is_some() && k + 1 != n {
            return Err(Refusal::RedirectMidPipeline);
        }
        // `2>` goes where `>` goes. What it names belongs to the line rather than to the stage it
        // is written on (the shell mints one diagnostic endpoint for the whole line), so putting it
        // anywhere but the tail would suggest a per-stage choice this shell does not make.
        if r.diags[k].is_some() && k + 1 != n {
            return Err(Refusal::RedirectMidPipeline);
        }
    }
    Ok(())
}

/// No unquoted redirection name on the line is a pattern.
fn check_redirect_patterns(n: usize, r: &Redirects<'_>) -> Result<(), Refusal> {
    // A redirection designates one destination, so a pattern in one designates the wrong number of
    // things. Refused here, where the token is read, rather than expanded and then counted.
    //
    // **A quoted name is exempt, and that is not a loophole**: quoting is what says "this is one
    // name, spelled with these bytes", so `> "*.txt"` designates exactly one destination and is the
    // only way to write to a file somebody managed to call `*.txt`. The check is about how many
    // things the token designates, and a quoted token designates one.
    for (t, quoted) in [
        (r.ins[0], r.in_q[0]),
        (r.outs[n - 1], r.out_q[n - 1]),
        (r.diags[n - 1], r.diag_q[n - 1]),
    ] {
        if let Some(t) = t
            && !quoted
            && glob::has_magic(t)
        {
            return Err(Refusal::PatternInRedirect);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The line every command was before this module existed: one stage, no operators, and the text
    /// handed on untouched.
    #[test]
    fn a_line_with_no_operator_is_one_stage() {
        for line in [
            &b"least_authority_demo 9"[..],
            b"echo hello  world",
            b"",
            b"   ",
        ] {
            let l = split(line).unwrap();
            assert_eq!(
                l.stage_count(),
                1,
                "{}",
                core::str::from_utf8(line).unwrap()
            );
            assert!(l.is_plain());
            assert_eq!(l.stages()[0], crate::trim(line));
        }
    }

    #[test]
    fn a_pipe_splits_and_trims() {
        let l = split(b"date | wc").unwrap();
        assert_eq!(l.stages(), [&b"date"[..], b"wc"]);
        assert!(!l.is_plain());
        // No spaces at all, which is legal in every shell and must mean the same thing here.
        assert_eq!(split(b"date|wc").unwrap().stages(), [&b"date"[..], b"wc"]);
        // Three stages, because a pipeline is not a special case of two.
        assert_eq!(
            split(b"echo a | wc | wc").unwrap().stages(),
            [&b"echo a"[..], b"wc", b"wc"],
        );
    }

    #[test]
    fn redirections_come_off_the_ends() {
        let l = split(b"date > report.txt").unwrap();
        assert_eq!(l.stages(), [&b"date"[..]]);
        assert_eq!(l.output, Some(&b"report.txt"[..]));
        assert_eq!(l.input, None);

        let l = split(b"wc < report.txt").unwrap();
        assert_eq!(l.stages(), [&b"wc"[..]]);
        assert_eq!(l.input, Some(&b"report.txt"[..]));

        // Both ends of one stage, and both ends of a pipeline.
        let l = split(b"wc <in >out").unwrap();
        assert_eq!((l.input, l.output), (Some(&b"in"[..]), Some(&b"out"[..])));
        let l = split(b"wc < in | wc > out").unwrap();
        assert_eq!(l.stages(), [&b"wc"[..], b"wc"]);
        assert_eq!((l.input, l.output), (Some(&b"in"[..]), Some(&b"out"[..])));
    }

    /// The operator does not need whitespace, and the name it takes stops at the next operator.
    #[test]
    fn an_operator_needs_no_whitespace() {
        let l = split(b"date>report.txt").unwrap();
        assert_eq!(l.stages(), [&b"date"[..]]);
        assert_eq!(l.output, Some(&b"report.txt"[..]));
        let l = split(b"wc<in>out").unwrap();
        assert_eq!((l.input, l.output), (Some(&b"in"[..]), Some(&b"out"[..])));
    }

    /// **`>>` names the same file and only changes what happens to what is in it.**
    ///
    /// Everything else about the line is identical, which is the claim: append is not a second
    /// destination, a second capability or a second protocol. It is one bit about how the shell
    /// opens the file it was already going to open.
    #[test]
    fn append_is_the_same_redirection_with_one_bit_changed() {
        let t = split(b"date > report.txt").unwrap();
        let a = split(b"date >> report.txt").unwrap();
        assert_eq!(t.stages(), a.stages());
        assert_eq!(t.output, a.output);
        assert_eq!(t.mode(), Mode::Truncate);
        assert_eq!(a.mode(), Mode::Append);
        // The two characters are one operator, so no whitespace is needed on either side of it and
        // the name still stops where the name stops.
        let a = split(b"date>>report.txt").unwrap();
        assert_eq!(a.output, Some(&b"report.txt"[..]));
        assert_eq!(a.mode(), Mode::Append);
        // And it reaches the sink, which is the only place anything downstream reads it.
        assert!(matches!(a.sink_for(0, None), Sink::Report));
        // A line with no `>` on it has nothing to append to, and says Truncate because that is the
        // default rather than because it means anything.
        assert_eq!(split(b"date | wc").unwrap().mode(), Mode::Truncate);
    }

    /// `>>` inherits every rule `>` has, because it is the same operator: the tail of the pipeline
    /// only, one per stage, one name, and a name that is not a pattern.
    #[test]
    fn append_inherits_every_rule_truncate_has() {
        assert_eq!(split(b"date >> f | wc"), Err(Refusal::RedirectMidPipeline));
        assert_eq!(split(b"date >> a >> b"), Err(Refusal::RedirectRepeated));
        assert_eq!(split(b"date > a >> b"), Err(Refusal::RedirectRepeated));
        assert_eq!(split(b"date >> *.txt"), Err(Refusal::PatternInRedirect));
        assert_eq!(split(b"date >> f extra"), Err(Refusal::WordAfterRedirect));
        assert_eq!(split(b"date >>"), Err(Refusal::NoRedirectTarget));
        assert_eq!(split(b"date >>>f"), Err(Refusal::NoRedirectTarget));
        // The legal shape at the tail of a pipeline, with an input on the head.
        let l = split(b"wc < a | wc >> b").unwrap();
        assert_eq!((l.input, l.output), (Some(&b"a"[..]), Some(&b"b"[..])));
        assert_eq!(l.mode(), Mode::Append);
    }

    /// **`2>` is read as an operator and the `2` never reaches the command** (DECISIONS §67).
    ///
    /// The stage text is the assertion. A `2` left in it would arrive at the manifest check as a
    /// positional token, and `date 2> err.txt` would be refused for an argument `date` does not
    /// take rather than run with its diagnostics redirected.
    #[test]
    fn a_diagnostic_redirection_comes_off_the_tail_like_an_output_one() {
        let l = split(b"date 2> err.txt").unwrap();
        assert_eq!(l.stages(), [&b"date"[..]]);
        assert_eq!(l.diagnostics, Some(&b"err.txt"[..]));
        assert_eq!(l.output, None);
        assert_eq!(l.diagnostics_mode(), Mode::Truncate);
        assert!(!l.is_plain(), "a line with a 2> on it is not the old path");

        // Both operators on one stage, in either order, naming two different files.
        let l = split(b"date > out.txt 2> err.txt").unwrap();
        assert_eq!(
            (l.output, l.diagnostics),
            (Some(&b"out.txt"[..]), Some(&b"err.txt"[..]))
        );
        let l = split(b"date 2> err.txt > out.txt").unwrap();
        assert_eq!(
            (l.output, l.diagnostics),
            (Some(&b"out.txt"[..]), Some(&b"err.txt"[..]))
        );
        // And with no whitespace anywhere, which is legal for every other operator here.
        let l = split(b"date>out.txt 2>err.txt").unwrap();
        assert_eq!(
            (l.output, l.diagnostics),
            (Some(&b"out.txt"[..]), Some(&b"err.txt"[..]))
        );
    }

    /// **The digit is only an operator at a word boundary**, which is what keeps a program's own
    /// integer argument from being eaten by a spelling.
    #[test]
    fn a_two_inside_a_word_is_part_of_the_word() {
        // `least_authority_demo 2 > out` is the integer 2 and an ordinary output redirection. If the digit were
        // read as an operator wherever it appeared, this line would silently lose its argument.
        let l = split(b"least_authority_demo 2 > out.txt").unwrap();
        assert_eq!(l.stages(), [&b"least_authority_demo 2"[..]]);
        assert_eq!((l.output, l.diagnostics), (Some(&b"out.txt"[..]), None));
        // And a digit glued to the end of a word stays in the word.
        let l = split(b"wc2>out.txt").unwrap();
        assert_eq!(l.stages(), [&b"wc2"[..]]);
        assert_eq!((l.output, l.diagnostics), (Some(&b"out.txt"[..]), None));
    }

    /// `2>>` is to `2>` what `>>` is to `>`, and it inherits every rule the others have, because it
    /// is the same operator with one bit different.
    #[test]
    fn the_diagnostic_redirection_appends_and_inherits_every_rule() {
        let a = split(b"date 2>> err.txt").unwrap();
        assert_eq!(a.diagnostics, Some(&b"err.txt"[..]));
        assert_eq!(a.diagnostics_mode(), Mode::Append);
        // The two spellings name the same file and differ in nothing else.
        let t = split(b"date 2> err.txt").unwrap();
        assert_eq!(t.diagnostics, a.diagnostics);
        assert_eq!(t.stages(), a.stages());

        assert_eq!(split(b"date 2> a 2> b"), Err(Refusal::RedirectRepeated));
        assert_eq!(split(b"date 2> a 2>> b"), Err(Refusal::RedirectRepeated));
        assert_eq!(split(b"date 2> f | wc"), Err(Refusal::RedirectMidPipeline));
        assert_eq!(split(b"date 2> *.txt"), Err(Refusal::PatternInRedirect));
        assert_eq!(split(b"date 2> f extra"), Err(Refusal::WordAfterRedirect));
        assert_eq!(split(b"date 2>"), Err(Refusal::NoRedirectTarget));
        assert_eq!(split(b"date 2>>"), Err(Refusal::NoRedirectTarget));
        assert_eq!(split(b"date 2>>>f"), Err(Refusal::NoRedirectTarget));
        // The legal shape at the tail of a pipeline.
        let l = split(b"echo a | wc > out.txt 2>> err.txt").unwrap();
        assert_eq!(l.stages(), [&b"echo a"[..], b"wc"]);
        assert_eq!(
            (l.output, l.diagnostics),
            (Some(&b"out.txt"[..]), Some(&b"err.txt"[..]))
        );
        assert_eq!(
            (l.mode(), l.diagnostics_mode()),
            (Mode::Truncate, Mode::Append)
        );
    }

    /// **There is no here-document**, and `<<` is refused rather than read as `<` twice. The second
    /// `<` is an operator, so the first one has no name after it; the refusal is about the missing
    /// name, which is the module note's one wording gap.
    #[test]
    fn there_is_no_here_document() {
        assert_eq!(split(b"wc << EOF"), Err(Refusal::NoRedirectTarget));
    }

    #[test]
    fn a_redirection_needs_a_name() {
        for line in [&b"date >"[..], b"date > ", b"wc <", b"date > | wc"] {
            assert_eq!(
                split(line),
                Err(Refusal::NoRedirectTarget),
                "{}",
                core::str::from_utf8(line).unwrap(),
            );
        }
    }

    /// Two answers to one question. Neither "the last wins" nor "the first wins" is a rule anybody
    /// should have to remember, so the line is refused.
    #[test]
    fn a_stage_has_one_input_and_one_output() {
        assert_eq!(split(b"date > a > b"), Err(Refusal::RedirectRepeated));
        assert_eq!(split(b"wc < a < b"), Err(Refusal::RedirectRepeated));
        // But one of each is fine, in either order.
        assert!(split(b"wc < a > b").is_ok());
        assert!(split(b"wc > b < a").is_ok());
    }

    /// The pipeline already says where the middle stages' bytes go. A redirection there would be a
    /// second, contradictory answer.
    #[test]
    fn only_the_ends_of_a_pipeline_may_be_redirected() {
        assert_eq!(split(b"date > f | wc"), Err(Refusal::RedirectMidPipeline));
        assert_eq!(split(b"date | wc < f"), Err(Refusal::RedirectMidPipeline));
        assert_eq!(
            split(b"echo a | wc > f | wc"),
            Err(Refusal::RedirectMidPipeline),
        );
        // And the legal shape: input on the head, output on the tail.
        let l = split(b"wc < a | wc > b").unwrap();
        assert_eq!((l.input, l.output), (Some(&b"a"[..]), Some(&b"b"[..])));
    }

    #[test]
    fn a_word_may_not_follow_a_redirection() {
        assert_eq!(split(b"< f wc"), Err(Refusal::WordAfterRedirect));
        assert_eq!(split(b"date > f extra"), Err(Refusal::WordAfterRedirect));
        // A following *operator* is not a word, so this stays legal.
        assert!(split(b"wc < f | wc").is_ok());
    }

    /// **`date || wc` was in this list until milestone 67 and is not any more.** `||` is a
    /// connector, and the shell splits on it before this function is reached, so at a prompt that
    /// line is two commands. What it means *here* is unchanged, and that is exactly why the split
    /// has to be outside: this function has no way to tell the two readings apart.
    #[test]
    fn a_pipeline_stage_must_have_a_command_in_it() {
        for line in [&b"| wc"[..], b"date |", b"|"] {
            assert_eq!(
                split(line),
                Err(Refusal::EmptyStage),
                "{}",
                core::str::from_utf8(line).unwrap(),
            );
        }
        // The empty line is one empty stage and not a pipeline, so it is not this refusal.
        assert!(split(b"").unwrap().is_plain());
    }

    #[test]
    fn a_pipeline_has_a_ceiling() {
        assert!(split(b"echo a | wc | wc | wc").is_ok());
        assert_eq!(
            split(b"echo a | wc | wc | wc | wc"),
            Err(Refusal::TooManyStages)
        );
    }

    /// A redirection designates one destination; a pattern designates a set. Refused where the
    /// token is read rather than expanded and then counted, because the count is not the point:
    /// even a pattern that matched exactly one name is a set that happens to have one member, and
    /// letting it through would make what a line writes to depend on what is in the directory.
    #[test]
    fn a_pattern_may_not_name_a_redirection() {
        assert_eq!(split(b"date > *.txt"), Err(Refusal::PatternInRedirect));
        assert_eq!(split(b"wc < log?.txt"), Err(Refusal::PatternInRedirect));
        // A name with no magic in it is untouched.
        assert!(split(b"date > report.txt").is_ok());
    }

    /// **An operator inside quotes is an ordinary byte** (milestone 67), which is what makes
    /// `echo "a > b"` print an angle bracket instead of writing a file called `b`.
    #[test]
    fn a_quoted_operator_does_not_split_the_line() {
        let l = split(b"echo \"a > b\"").unwrap();
        assert_eq!(l.stages(), [&b"echo \"a > b\""[..]]);
        assert_eq!(l.output, None);
        assert!(l.is_plain(), "a quoted `>` must not leave the old path");
        // Every operator, not just `>`: one cursor answers for all of them.
        for line in [
            &b"echo 'a | b'"[..],
            b"echo 'a < b'",
            b"echo 'a 2> b'",
            b"echo 'a >> b'",
        ] {
            let l = split(line).unwrap();
            assert_eq!(
                l.stage_count(),
                1,
                "{}",
                core::str::from_utf8(line).unwrap()
            );
            assert_eq!((l.input, l.output, l.diagnostics), (None, None, None));
        }
    }

    /// **The correctness gap this milestone opened**: a file whose name has a space in it could not
    /// be written to, so it could not be named, so it could not be granted.
    #[test]
    fn a_redirection_can_name_a_file_with_a_space_in_it() {
        let l = split(b"date > \"my out.txt\"").unwrap();
        assert_eq!(l.stages(), [&b"date"[..]]);
        // The quotes are **off** by the time anything downstream sees the name, because what is
        // designated is the name and not the spelling.
        assert_eq!(l.output, Some(&b"my out.txt"[..]));
        assert_eq!(l.mode(), Mode::Truncate);

        // Both quote forms, both ends, and `>>` too, because they are one operator each.
        let l = split(b"wc < 'my in.txt' >> \"my out.txt\"").unwrap();
        assert_eq!(
            (l.input, l.output, l.mode()),
            (
                Some(&b"my in.txt"[..]),
                Some(&b"my out.txt"[..]),
                Mode::Append
            ),
        );
        // And the word-after-a-redirection rule still holds: the quoted name ends where its quote
        // ends, so `extra` is a word after it rather than part of it.
        assert_eq!(
            split(b"date > 'my out.txt' extra"),
            Err(Refusal::WordAfterRedirect),
        );
    }

    /// **A quoted name designates one thing, so it is not a pattern**, which is the only exemption
    /// quoting buys anywhere in this crate. It is not a loophole: the check counts destinations, and
    /// a quoted token is one.
    #[test]
    fn quoting_a_redirection_name_is_how_a_file_called_star_is_written_to() {
        assert_eq!(
            split(b"date > \"*.txt\"").unwrap().output,
            Some(&b"*.txt"[..])
        );
        assert_eq!(split(b"wc < '?.log'").unwrap().input, Some(&b"?.log"[..]));
        // Bare, it is still refused, so the exemption is quoting and not a hole in the check.
        assert_eq!(split(b"date > *.txt"), Err(Refusal::PatternInRedirect));
    }

    /// A quote that never closes takes the whole line with it, wherever it opened. Running the
    /// complete prefix of a line that is missing its end would run something nobody typed.
    #[test]
    fn an_unclosed_quote_refuses_the_line() {
        for line in [
            &b"echo 'hello"[..],
            b"date > \"my out.txt",
            b"echo \"a | wc",
        ] {
            assert_eq!(
                split(line),
                Err(Refusal::UnclosedQuote),
                "{}",
                core::str::from_utf8(line).unwrap(),
            );
        }
    }

    /// [`Line::sink_for`] and [`Line::source_for`] are the whole of "which capability goes in which
    /// slot", so the table they implement is worth pinning: the head reads, the tail writes, and the
    /// joints are pipes.
    #[test]
    fn the_ends_of_a_pipeline_are_the_only_ones_that_touch_a_file() {
        let l = split(b"echo a | wc | wc").unwrap();
        assert_eq!(l.sink_for(0, None), Sink::Pipe);
        assert_eq!(l.sink_for(1, None), Sink::Pipe);
        assert_eq!(l.sink_for(2, None), Sink::Report);
        assert_eq!(l.source_for(0, None), Source::None);
        assert_eq!(l.source_for(1, None), Source::Pipe);
        assert_eq!(l.source_for(2, None), Source::Pipe);
    }
}

//! **Reading a line**: `tokenize`, `argv`, `parse`, the `user` and `vouch` verbs, `parse_run`,
//! and `ParsedInvocation`. `parse_run` is the only thing that builds a `ParsedInvocation`, so its fields stay private
//! here (§272, fork 3).
//!
//! The crate root re-exports every public item (§272 (the grant-plan crate root splits into
//! private modules)).
//!
//! Name: provisional (milestone 842, 2026-10-11), as are its sibling modules' names.

use crate::{
    Command, MAX_SHORT_OPTIONS, Refusal, TouchOperands, parse_u64, split_first_word, trim, word,
};

/// Split a command line into whitespace-separated tokens, writing them into `out` and returning
/// the filled prefix. A tiny `no_std` tokenizer: no allocation, bounded by `out.len()`. Tokens
/// past `out.len()` are dropped (a command line with more than a handful of tokens is a mistake,
/// not a workload).
///
/// **Whitespace inside quotes is not a separator** (milestone 67). The quotes stay on the token,
/// because this function's job is where a token ends and [`word::read`]'s is what it means; a
/// caller that wants the name rather than the spelling asks for it. That split is what lets
/// [`parse_run`] refuse `a"b"` in one place instead of every scanner having to.
pub fn tokenize<'a, 'b>(line: &'a [u8], out: &'b mut [&'a [u8]]) -> &'b [&'a [u8]] {
    let mut n = 0;
    let mut i = 0;
    let mut c = word::Cursor::new();
    while i < line.len() && n < out.len() {
        // A whitespace byte can never open or close a quote, so the cursor's state cannot change
        // while it is being skipped and this loop does not have to step it.
        while i < line.len() && !c.is_open() && line[i].is_ascii_whitespace() {
            i += 1;
        }
        let start = i;
        while i < line.len() && !(!c.is_open() && line[i].is_ascii_whitespace()) {
            c.step(line[i]);
            i += 1;
        }
        if i > start {
            out[n] = &line[start..i];
            n += 1;
        }
    }
    &out[..n]
}

/// **Assemble the argv of a program that hears words** (milestone 205 (how a foreign program is
/// told what to do), DECISIONS §170; see [`ProgramArgumentDeclaration::Words`](crate::ProgramArgumentDeclaration::Words)), onto `page`, and return how many
/// words it carries.
///
/// `line` is one command: the program's name and its words, no operator. Every word is read as
/// [`parse_run`] reads one, quotes off, and none is classified. The name is `argv[0]` as typed,
/// because `ripgrep` skips the first element and `clap` treats it as the binary's name. Refused,
/// with the page left not an argv, for [`check_words`]'s reasons, which the planner already asked
/// of the same line.
pub fn argv(
    line: &[u8],
    page: &mut [u8; program_argument_contract::PAGE_BYTES],
) -> Result<u32, Refusal> {
    let mut b = program_argument_contract::PageBuilder::new(page);
    each_word(line, &mut |w| {
        b.push(w).map_err(|_| Refusal::ArgumentsTooLong)
    })?;
    Ok(b.finish())
}

/// **Whether [`argv`] would take `line`**, without a page to write on. The planner asks it, so a
/// line is refused at the prompt, and in `caps`, before anything is spawned. Three reasons: quotes
/// that do not make sense ([`word::read`]'s), an unquoted pattern
/// ([`Refusal::PatternInArguments`]), and more than one page ([`Refusal::ArgumentsTooLong`]). There
/// is no word-count ceiling beyond the page, unlike [`parse_run`]'s sixteen tokens, because nothing
/// here is placed into a slot.
pub fn check_words(line: &[u8]) -> Result<(), Refusal> {
    let mut used = 0usize;
    each_word(line, &mut |w| {
        used += 4 + w.len();
        if used > program_argument_contract::CAPACITY {
            Err(Refusal::ArgumentsTooLong)
        } else {
            Ok(())
        }
    })
}

/// Every word of `line`, quotes off, with an unquoted pattern refused. The one walk [`argv`] and
/// [`check_words`] share, so the check and the page cannot disagree about what a word is, and the
/// shell's designation walks it too (milestone 205), so it designates from the same words the
/// program hears. Name: provisional.
pub fn each_word(
    line: &[u8],
    f: &mut dyn FnMut(&[u8]) -> Result<(), Refusal>,
) -> Result<(), Refusal> {
    let mut rest = line;
    loop {
        let mut one = [&b""[..]; 1];
        let Some(&token) = tokenize(rest, &mut one).first() else {
            return Ok(());
        };
        // `token` is a slice of `rest`, so what follows it is everything after its last byte.
        let end = token.as_ptr() as usize - rest.as_ptr() as usize + token.len();
        rest = &rest[end..];
        let w = word::read(token)?;
        if !w.quoted && glob::has_magic(w.text) {
            return Err(Refusal::PatternInArguments);
        }
        f(w.text)?;
    }
}
/// **Every word [`parse`] answers as a builtin**, for the shell's Tab completion (milestone 47
/// (navigation and naming), DECISIONS §227 (how Tab reaches the shell)). A builtin added to
/// `parse` and not here still works; it only is not offered on Tab. The test beside `parse` holds
/// the other direction: everything listed here is a builtin.
///
/// Name: provisional (milestone 47, 2026-09-26).
pub const BUILTINS: &[&[u8]] = &[
    b"apropos", b"bg", b"bind", b"caps", b"cd", b"echo", b"fg", b"help", b"jobs", b"kill", b"ls",
    b"mkdir", b"pwd", b"time", b"touch", b"wait", b"xargs",
];

/// Parse a whole command line into a [`Command`]. Pure and allocation-free.
///
/// The first token selects the command, and one that is not a builtin is a **program name**.
/// `echo` keeps the rest of the line verbatim; the prefix words (`caps`, `time`, `xargs`) keep it
/// unsplit, since their operand is a command line; everything else works on tokens. Builtins win
/// over programs, so [`Program::from_name`](crate::Program::from_name) must never learn a name
/// that is also a builtin.
pub fn parse(line: &[u8]) -> Command<'_> {
    let trimmed = trim(line);
    if trimmed.is_empty() {
        return Command::Empty;
    }
    let (first, rest) = split_first_word(trimmed);
    // A quoted builtin name still names the builtin. A first word whose quotes do not make sense
    // falls through to `parse_run`, the one place a quoting refusal is raised.
    let first = match word::read(first) {
        Ok(w) => w.text,
        Err(_) => return Command::Run(parse_run(trimmed)),
    };
    match first {
        b"help" => Command::Help,
        b"echo" => Command::Echo(rest),
        b"caps" => Command::Caps(rest),
        b"time" => Command::Time(rest),
        b"xargs" => Command::Xargs(rest),
        // A path operand is one designation however many slashes it has, so `trim` is all the
        // classification it needs.
        b"cd" => Command::Cd(trim(rest)),
        b"pwd" => Command::Pwd,
        b"ls" => Command::Ls(trim(rest)),
        b"mkdir" => Command::Mkdir(trim(rest)),
        // One path, or with `-t` two words: the instant's text, unparsed, and then the name.
        b"touch" => {
            let rest = trim(rest);
            match rest.strip_prefix(b"-t") {
                Some(after_flag)
                    if after_flag.is_empty() || after_flag[0].is_ascii_whitespace() =>
                {
                    let (at, name) = split_first_word(trim(after_flag));
                    Command::Touch(TouchOperands {
                        at: Some(at),
                        name: trim(name),
                    })
                }
                _ => Command::Touch(TouchOperands {
                    at: None,
                    name: rest,
                }),
            }
        }
        // One word for the target, and the trimmed rest for the name, untokenized.
        b"bind" => {
            let rest = trim(rest);
            let (target, name) = split_first_word(rest);
            Command::Bind(target, trim(name))
        }
        b"apropos" => Command::Apropos(trim(rest)),
        b"vouch" => Command::Vouch(trim(rest)),
        b"user" => Command::User(trim(rest)),
        b"jobs" => Command::Jobs,
        b"wait" => Command::Wait(trim(rest)),
        b"kill" => Command::Kill(trim(rest)),
        b"fg" => Command::Fg(trim(rest)),
        b"bg" => Command::Bg(trim(rest)),
        // `rm` is deliberately not a builtin: a builtin runs with the shell's whole endowment, and
        // a destructive loop takes an attenuated grant instead. Whether a program exists is
        // `plan`'s question, not the parser's.
        _ => Command::Run(parse_run(trimmed)),
    }
}

/// **Which job an operand names** (milestone 48): `%N` or a bare `N`, counted from one. `Ok(None)`
/// is an empty operand (every job to `wait`, the newest to `fg`); `Err(())` names no job.
///
/// Name: provisional, minted 2026-10-18 (UTC) by milestone/48-job-control.
///
/// ```
/// use grant_plan::job_operand;
/// assert_eq!(job_operand(b"%2"), Ok(Some(2)));
/// assert_eq!(job_operand(b"2"), Ok(Some(2)));
/// assert_eq!(job_operand(b""), Ok(None));
/// assert_eq!(job_operand(b"%0"), Err(()));
/// assert_eq!(job_operand(b"%1 %2"), Err(()));
/// ```
#[allow(clippy::result_unit_err)]
pub fn job_operand(tail: &[u8]) -> Result<Option<usize>, ()> {
    let t = trim(tail);
    if t.is_empty() {
        return Ok(None);
    }
    let digits = t.strip_prefix(b"%").unwrap_or(t);
    // Two digits is the whole range a job number can have; anything longer names no job, and
    // refusing it here keeps the sum below from overflowing.
    if digits.is_empty() || digits.len() > 2 || !digits.iter().all(u8::is_ascii_digit) {
        return Err(());
    }
    let n = digits
        .iter()
        .fold(0usize, |n, d| n * 10 + usize::from(d - b'0'));
    if n == 0 { Err(()) } else { Ok(Some(n)) }
}

/// **`kill`'s operands** (milestone 48 (job control)): whether `-9` asked for the forcible tier, and
/// the job, which is required rather than guessed.
///
/// Name: provisional, minted 2026-10-18 (UTC) by milestone/48-job-control.
///
/// ```
/// use grant_plan::kill_operands;
/// assert_eq!(kill_operands(b"%1"), Ok((false, 1)));
/// assert_eq!(kill_operands(b"-9 %3"), Ok((true, 3)));
/// assert_eq!(kill_operands(b""), Err(()));
/// assert_eq!(kill_operands(b"-15 %1"), Err(()));
/// ```
#[allow(clippy::result_unit_err)]
pub fn kill_operands(tail: &[u8]) -> Result<(bool, usize), ()> {
    let t = trim(tail);
    let (force, job) = match t.strip_prefix(b"-9") {
        Some(rest) if rest.first().is_some_and(u8::is_ascii_whitespace) => (true, rest),
        _ => (false, t),
    };
    match job_operand(job) {
        Ok(Some(n)) => Ok((force, n)),
        _ => Err(()),
    }
}

/// **What `user` was asked to do** (milestone 152, calef's §108 ruling of 2026-09-26).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UserVerb<'a> {
    /// `user suspend <name>`: list the identity as suspended.
    Suspend(&'a [u8]),
    /// `user resume <name>`: take the identity off the list.
    Resume(&'a [u8]),
    /// Anything else. The shell answers with the two forms and changes nothing.
    Usage,
}

/// Classify [`Command::User`]'s tail: a verb and one name of at most sixteen bytes, the most an
/// identity can be and still have a subtree (`filesystem_contract::grant::MAX_NAME`).
pub fn user_verb(tail: &[u8]) -> UserVerb<'_> {
    let (verb, rest) = split_first_word(trim(tail));
    let name = trim(rest);
    let one_name = !name.is_empty()
        && name.len() <= 16
        && !name.starts_with(b"#")
        && !name.iter().any(u8::is_ascii_whitespace);
    match verb {
        b"suspend" if one_name => UserVerb::Suspend(name),
        b"resume" if one_name => UserVerb::Resume(name),
        _ => UserVerb::Usage,
    }
}

/// Classify [`Command::Vouch`]'s tail: one operand, a path (it has a `/`), whose last component
/// ([`vouched_name`]) the installed-programs table can record. `None` for anything else, which the shell
/// answers with what it takes.
pub fn vouch_verb(tail: &[u8]) -> Option<&[u8]> {
    let path = trim(tail);
    let one_word = !path.is_empty() && !path.iter().any(u8::is_ascii_whitespace);
    match vouched_name(path) {
        Some(_) if one_word && path.contains(&b'/') => Some(path),
        _ => None,
    }
}

/// **The name a vouched file is recorded under**: its path's last component, which is what a
/// person sees in `caps`. `None` when it is empty, longer than the
/// sixteen bytes a packed name carries (`filesystem_contract::grant::MAX_NAME`), `.` or `..`, or
/// holds a `#`, which the installed-programs table's lines cannot carry.
pub fn vouched_name(path: &[u8]) -> Option<&[u8]> {
    let name = path.rsplit(|&b| b == b'/').next().unwrap_or(path);
    let recordable = !name.is_empty()
        && name.len() <= 16
        && name != b"."
        && name != b".."
        && !name.contains(&b'#');
    recordable.then_some(name)
}

/// Parse a whole program invocation (the line, starting at the program name) into a [`ParsedInvocation`].
///
/// It recognizes `--mem N` in any position and takes the first other token as the program name;
/// every token after that is kept **in order and unclassified**, because deciding which one is the
/// argument and which is the file needs the manifest (see [`plan_against`](crate::plan_against)). The one thing it does
/// classify is a flag: a token starting with `-` that is not `--mem` is refused rather than allowed
/// to fall into the file position, which is the single way a typo could otherwise become a
/// capability transfer.
pub fn parse_run(line: &[u8]) -> ParsedInvocation<'_> {
    let mut toks: [&[u8]; 16] = [b""; 16];
    let toks = tokenize(line, &mut toks);

    let mut memory_grant = None;
    let mut program: &[u8] = b"";
    let mut positional_tokens = [&b""[..]; MAX_POSITIONALS];
    let mut quoted_positionals = [false; MAX_POSITIONALS];
    let mut positional_count = 0;
    let mut short_options = [0u8; MAX_SHORT_OPTIONS];
    let mut short_option_count = 0;
    let mut unexpected = None;
    let mut misquoted = None;
    let mut have_prog = false;

    let mut i = 0;
    while i < toks.len() {
        // The quotes come off before anything is classified: `"--mem"` is a name and `--mem` an
        // option, and checking for flags first would make a quoted name into a flag.
        let (t, quoted) = match word::read(toks[i]) {
            Ok(w) => (w.text, w.quoted),
            Err(r) => {
                if misquoted.is_none() {
                    misquoted = Some(r);
                }
                i += 1;
                continue;
            }
        };
        if !quoted && t == b"--mem" {
            // The next token is the page count. A missing or non-numeric value leaves memory_grant None,
            // which the plan turns into MemoryGrantRequired for a program that needs it.
            if i + 1 < toks.len() {
                // Unquoted first, so `--mem "16"` is the same grant as `--mem 16`. Quoting decides
                // what a word is and this one has to be a number either way.
                memory_grant = word::read(toks[i + 1]).ok().and_then(|w| parse_u64(w.text));
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        // A quoted token is never an option: `rm "-r"` names a file called `-r`, not the flag that
        // widens a directory grant.
        if !quoted && t.first() == Some(&b'-') {
            // A cluster of short options (`-r`, `-rf`), kept for the manifest to check. Anything
            // else shaped like a flag is refused, so an unknown flag never designates a file.
            let letters = &t[1..];
            let short = !letters.is_empty() && letters.iter().all(u8::is_ascii_alphabetic);
            if short && short_option_count + letters.len() <= MAX_SHORT_OPTIONS {
                for &c in letters {
                    short_options[short_option_count] = c;
                    short_option_count += 1;
                }
            } else if unexpected.is_none() {
                unexpected = Some(t);
            }
        } else if !have_prog {
            program = t;
            have_prog = true;
        } else if positional_count < MAX_POSITIONALS {
            positional_tokens[positional_count] = t;
            quoted_positionals[positional_count] = quoted;
            positional_count += 1;
        } else if unexpected.is_none() {
            unexpected = Some(t);
        }
        i += 1;
    }

    ParsedInvocation {
        program,
        line,
        memory_grant,
        positional_tokens,
        quoted_positionals,
        positional_count,
        short_options,
        short_option_count,
        unexpected,
        misquoted,
    }
}

/// The most positional tokens a [`ParsedInvocation`] keeps. Two would cover every manifest shape today (an
/// argument and a file); the headroom exists so a third token is *refused* rather than dropped
/// before anything looks at it.
pub const MAX_POSITIONALS: usize = 4;

/// The parsed form of a program invocation, before the manifest check. [`plan`](crate::plan) turns it into an
/// [`Endowment`](crate::Endowment) or a [`Refusal`].
///
/// **The positional tokens are deliberately unclassified here.** The parser knows the shape of a
/// token, not what it means: which one is the integer argument and which is the file is a question
/// only the program's [`Manifest`](crate::Manifest) can answer, and milestone 47 removed the `file:` prefix precisely
/// because the manifest was already answering it. Keeping them in order and letting [`plan_against`](crate::plan_against)
/// place them is what makes that true in the code rather than only in the prose.
///
/// Name: ratified 2026-10-18 (calef: "Approve the family").
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ParsedInvocation<'a> {
    /// The program name as typed (may not resolve).
    pub program: &'a [u8],
    /// **The whole invocation as it was typed**, which is what a program that hears words is given
    /// ([`ProgramArgumentDeclaration::Words`], milestone 205). Everything else here is a classification of it, and a
    /// classification is exactly what §170 (how a foreign program is told what to do) said such a
    /// program must not get.
    line: &'a [u8],
    /// `--mem N`, if given. Recognized before or after the program name, because with the `run`
    /// verb gone the line reads `memory_grant_depleter --mem 16` the way every other command line does.
    pub memory_grant: Option<u64>,
    /// The positional tokens after the program name, in order, filled prefix in `positional_tokens[..positional_count]`.
    positional_tokens: [&'a [u8]; MAX_POSITIONALS],
    positional_count: usize,
    /// The short-option letters typed, in order, filled prefix in `short_options[..short_option_count]`. Collected without
    /// knowing the program, because **an option letter is not authority**: what it may do is widen a
    /// grant, and that is decided in [`plan_against`] against the manifest, at the prompt, before
    /// anything is spawned. A letter no manifest declares is [`Refusal::NoSuchOption`] there.
    short_options: [u8; MAX_SHORT_OPTIONS],
    short_option_count: usize,
    /// Which of [`positionals`](ParsedInvocation::positionals) were quoted, in the same order. A quoted word
    /// is **never a pattern**, whatever bytes are in it, which is the one authority-visible thing
    /// quoting does: `rm "*.txt"` designates one name and `rm *.txt` designates a set.
    quoted_positionals: [bool; MAX_POSITIONALS],
    /// A token nothing can place: a flag-shaped token that is not `--mem`, or one past
    /// [`MAX_POSITIONALS`]. Kept so the shell refuses it rather than silently ignoring authority the
    /// user thought they were granting. A **flag** that fell through to the file position would be
    /// the one way a typo could turn into a capability transfer, so it never reaches that position.
    pub unexpected: Option<&'a [u8]>,
    /// **The line's quotes do not make sense** ([`Refusal::UnclosedQuote`] or
    /// [`Refusal::PartlyQuoted`]), carried rather than returned because [`parse_run`] has no error
    /// channel and because this is a fact about the *line*, not about any program.
    ///
    /// [`plan_against_with`](crate::plan_against_with) answers it **before** it resolves the program, which is the one place
    /// the ordering matters: a line whose words cannot be read has no reliable first word either, so
    /// "no such program" would be a true sentence about a name nobody typed.
    pub misquoted: Option<Refusal>,
}

impl<'a> ParsedInvocation<'a> {
    /// The positional tokens, in the order typed. [`plan_against`](crate::plan_against) places them into the slots the
    /// manifest declares; nothing else should interpret them.
    pub fn positionals(&self) -> &[&'a [u8]] {
        &self.positional_tokens[..self.positional_count]
    }

    /// The whole invocation as typed, for [`argv`]. See the field.
    pub fn line(&self) -> &'a [u8] {
        self.line
    }

    /// The short-option letters typed, in order. Meaningless until a manifest says which exist.
    pub fn options(&self) -> &[u8] {
        &self.short_options[..self.short_option_count]
    }

    /// **Whether positional `i` was quoted**, which is the whole of what quoting decides about
    /// authority: a quoted word designates itself and is never expanded, so `rm "*.txt"` hands over
    /// one name where `rm *.txt` hands over a set.
    ///
    /// `false` for an index past the end, because a word that is not there was not quoted.
    pub fn is_quoted(&self, i: usize) -> bool {
        i < self.positional_count && self.quoted_positionals[i]
    }
}

#[cfg(test)]
mod tests;

//! **Reading a line**: `tokenize`, `argv`, `parse`, the `user` and `vouch` verbs, `parse_run`,
//! and `RunSpec`. `parse_run` is the only thing that builds a `RunSpec`, so its fields stay private
//! here (§272, fork 3).
//!
//! Moved out of the crate's root unchanged by milestone 842 (the grant-plan crate root is split
//! along its seams), as §272 (the grant-plan crate root splits into private modules) lays out; the
//! root re-exports every public item at its old path.
//!
//! Name: provisional (milestone 842, 2026-10-11), as are its sibling modules' names.

use crate::{
    Command, MAX_SHORT_OPTIONS, Refusal, TouchArgs, parse_u64, split_first_word, trim, word,
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
/// told what to do), DECISIONS §170; see [`ArgSpec::Words`](crate::ArgSpec::Words)), onto `page`, and return how many
/// words it carries.
///
/// `line` is one command: the program's name and its words, with no operator on it. Every word is
/// read exactly as [`parse_run`] reads one, quotes off, and none is classified: `-i`,
/// `--color=never` and `8` all arrive as the bytes that were typed. The program's name is `argv[0]`,
/// as typed, because `ripgrep` skips the first element and `clap` treats it as the binary's name.
///
/// Refused, with the page left not an argv, for [`check_words`]'s reasons. The planner has already
/// asked that question of the same line, so a refusal here means the line changed in between.
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
    b"apropos", b"bind", b"caps", b"cd", b"echo", b"help", b"ls", b"mkdir", b"pwd", b"time",
    b"touch", b"xargs",
];

/// Parse a whole command line into a [`Command`]. Pure and allocation-free.
///
/// The grammar is small: the first token selects the command, and a first token that is not a
/// builtin is a **program name**, because milestone 47 deleted the `run` verb. `echo` keeps the
/// rest of the line verbatim (so `echo   two  spaces` prints its spaces); everything else works on
/// tokens.
///
/// Builtins win over programs, which is why [`Prog::from_name`](crate::Prog::from_name) must never learn a name that is
/// also a builtin. A handful of reserved words is the whole cost of a shell where every command is
/// typed the same way.
///
/// **Two of them are prefix words**, `caps` and `time`, and they are the ones that keep the rest of
/// the line intact instead of tokenizing it: what they operate on is a command line, not an operand.
pub fn parse(line: &[u8]) -> Command<'_> {
    let trimmed = trim(line);
    if trimmed.is_empty() {
        return Command::Empty;
    }
    let (first, rest) = split_first_word(trimmed);
    // **A quoted builtin name still names the builtin**, because quoting decides what a word *is*
    // and this match is about what the word says. A first word whose quotes do not make sense falls
    // through to [`parse_run`], which is the one place a quoting refusal is raised; raising it here
    // as well would be a second copy of the same sentence.
    let first = match word::read(first) {
        Ok(w) => w.text,
        Err(_) => return Command::Run(parse_run(trimmed)),
    };
    match first {
        b"help" => Command::Help,
        b"echo" => Command::Echo(rest),
        b"caps" => Command::Caps(rest),
        // The second prefix word (milestone 86). Like `caps`, its operand is a whole command line
        // rather than a token, so `rest` is kept unsplit and the shell re-dispatches it.
        b"time" => Command::Time(rest),
        // The third (milestone 109), and the same shape for the same reason: batching is about how
        // the shell hands a designation over, so its operand is the line it batches.
        b"xargs" => Command::Xargs(rest),
        // The navigation builtins (milestone 47). They take a path operand rather than tokens,
        // because a path is one designation however many slashes it has, and `trim` is all the
        // classification they need.
        b"cd" => Command::Cd(trim(rest)),
        b"pwd" => Command::Pwd,
        b"ls" => Command::Ls(trim(rest)),
        b"mkdir" => Command::Mkdir(trim(rest)),
        // `touch` takes one word (`cd`/`ls`/`mkdir`'s reason: `trim` is all the classification a
        // bare path needs) unless it starts with `-t`, DECISIONS §112's arbitrary-instant form,
        // which is two words: the RFC 3339 text (unparsed here, see `TouchArgs`'s own doc) and then
        // the name.
        b"touch" => {
            let rest = trim(rest);
            match rest.strip_prefix(b"-t") {
                Some(after_flag)
                    if after_flag.is_empty() || after_flag[0].is_ascii_whitespace() =>
                {
                    let (at, name) = split_first_word(trim(after_flag));
                    Command::Touch(TouchArgs {
                        at: Some(at),
                        name: trim(name),
                    })
                }
                _ => Command::Touch(TouchArgs {
                    at: None,
                    name: rest,
                }),
            }
        }
        // `bind <target> <name>` (milestone 47/154). One word for the target, everything after it
        // (trimmed) for the name: the same "one opaque trailing blob" rule `cd`/`mkdir` already
        // use for their single path operand, generalized to two operands here rather than
        // tokenizing the name further.
        b"bind" => {
            let rest = trim(rest);
            let (target, name) = split_first_word(rest);
            Command::Bind(target, trim(name))
        }
        // **Search the documentation store** (milestone 40 phase 2). A builtin for exactly
        // [`Command::Ls`]'s reason, stated there: a search *is* an enumeration, and a searching
        // program would have to be handed the store's directory in order to read every shard in it.
        // The operand is a word rather than a path, so `trim` is all the classification it needs.
        b"apropos" => Command::Apropos(trim(rest)),
        // **The owner vouches for a file** (DECISIONS §221). Its own word, a builtin, because it
        // names no package: the bytes are the whole of what is vouched, and `jig` is not asked.
        b"vouch" => Command::Vouch(trim(rest)),
        // **The owner suspends or resumes an identity** (milestone 152's §108 ruling). A builtin
        // because what it edits is a file at this shell's own root, which is the owner's authority
        // only when this shell is the owner's console: a confined shell's root is its own subtree,
        // where a `suspended` file is one nothing reads.
        b"user" => Command::User(trim(rest)),
        // **`rm` is deliberately not here.** It was a builtin in the commands lane and is a program
        // now (milestone 47's rmdir lane): a builtin runs with the shell's whole endowment, and a
        // destructive loop should take an explicit attenuated grant instead. Since builtins are
        // matched first, deleting the line is the entire change at this level; `Prog::from_name`
        // picks the word up.
        // Not a builtin, so it is a program invocation, and the whole line (name included) is the
        // grant expression. Whether the program exists is `plan`'s question, not the parser's.
        _ => Command::Run(parse_run(trimmed)),
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

/// Parse a whole program invocation (the line, starting at the program name) into a [`RunSpec`].
///
/// It recognizes `--mem N` in any position and takes the first other token as the program name;
/// every token after that is kept **in order and unclassified**, because deciding which one is the
/// argument and which is the file needs the manifest (see [`plan_against`](crate::plan_against)). The one thing it does
/// classify is a flag: a token starting with `-` that is not `--mem` is refused rather than allowed
/// to fall into the file position, which is the single way a typo could otherwise become a
/// capability transfer.
pub fn parse_run(line: &[u8]) -> RunSpec<'_> {
    let mut toks: [&[u8]; 16] = [b""; 16];
    let toks = tokenize(line, &mut toks);

    let mut mem = None;
    let mut prog: &[u8] = b"";
    let mut pos = [&b""[..]; MAX_POSITIONALS];
    let mut posq = [false; MAX_POSITIONALS];
    let mut npos = 0;
    let mut opts = [0u8; MAX_SHORT_OPTIONS];
    let mut nopts = 0;
    let mut unexpected = None;
    let mut misquoted = None;
    let mut have_prog = false;

    let mut i = 0;
    while i < toks.len() {
        // **The quotes come off before anything is classified** (milestone 67), because what a
        // token *is* is what it says: `"--mem"` is a name and `--mem` is an option, and deciding
        // that after the flag check would make a quoted name into a flag.
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
            // The next token is the page count. A missing or non-numeric value leaves mem None,
            // which the plan turns into MemRequired for a program that needs it.
            if i + 1 < toks.len() {
                // Unquoted first, so `--mem "16"` is the same grant as `--mem 16`. Quoting decides
                // what a word is and this one has to be a number either way.
                mem = word::read(toks[i + 1]).ok().and_then(|w| parse_u64(w.text));
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        // A quoted token is never an option, which is the same rule one line up: `rm "-r"` names a
        // file called `-r`, and reading it as the flag that widens a directory grant into a subtree
        // walk would be the loudest possible version of a typo becoming a capability transfer.
        if !quoted && t.first() == Some(&b'-') {
            // A cluster of short options (`-r`, `-rf`), kept for the manifest to check. Anything
            // else shaped like a flag is refused: it never becomes a program name and never reaches
            // the file position, because an unknown flag silently designating a file is exactly the
            // typo-into-a-grant the old `file:` prefix was there to prevent, and this is the part
            // of that job worth keeping.
            let letters = &t[1..];
            let short = !letters.is_empty() && letters.iter().all(u8::is_ascii_alphabetic);
            if short && nopts + letters.len() <= MAX_SHORT_OPTIONS {
                for &c in letters {
                    opts[nopts] = c;
                    nopts += 1;
                }
            } else if unexpected.is_none() {
                unexpected = Some(t);
            }
        } else if !have_prog {
            prog = t;
            have_prog = true;
        } else if npos < MAX_POSITIONALS {
            pos[npos] = t;
            posq[npos] = quoted;
            npos += 1;
        } else if unexpected.is_none() {
            unexpected = Some(t);
        }
        i += 1;
    }

    RunSpec {
        prog,
        line,
        mem,
        pos,
        posq,
        npos,
        opts,
        nopts,
        unexpected,
        misquoted,
    }
}

/// The most positional tokens a [`RunSpec`] keeps. Two would cover every manifest shape today (an
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
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RunSpec<'a> {
    /// The program name as typed (may not resolve).
    pub prog: &'a [u8],
    /// **The whole invocation as it was typed**, which is what a program that hears words is given
    /// ([`ArgSpec::Words`], milestone 205). Everything else here is a classification of it, and a
    /// classification is exactly what §170 (how a foreign program is told what to do) said such a
    /// program must not get.
    line: &'a [u8],
    /// `--mem N`, if given. Recognized before or after the program name, because with the `run`
    /// verb gone the line reads `memory_grant_depleter --mem 16` the way every other command line does.
    pub mem: Option<u64>,
    /// The positional tokens after the program name, in order, filled prefix in `pos[..npos]`.
    pos: [&'a [u8]; MAX_POSITIONALS],
    npos: usize,
    /// The short-option letters typed, in order, filled prefix in `opts[..nopts]`. Collected without
    /// knowing the program, because **an option letter is not authority**: what it may do is widen a
    /// grant, and that is decided in [`plan_against`] against the manifest, at the prompt, before
    /// anything is spawned. A letter no manifest declares is [`Refusal::NoSuchOption`] there.
    opts: [u8; MAX_SHORT_OPTIONS],
    nopts: usize,
    /// Which of [`positionals`](RunSpec::positionals) were quoted, in the same order. A quoted word
    /// is **never a pattern**, whatever bytes are in it, which is the one authority-visible thing
    /// quoting does: `rm "*.txt"` designates one name and `rm *.txt` designates a set.
    posq: [bool; MAX_POSITIONALS],
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

impl<'a> RunSpec<'a> {
    /// The positional tokens, in the order typed. [`plan_against`](crate::plan_against) places them into the slots the
    /// manifest declares; nothing else should interpret them.
    pub fn positionals(&self) -> &[&'a [u8]] {
        &self.pos[..self.npos]
    }

    /// The whole invocation as typed, for [`argv`]. See the field.
    pub fn line(&self) -> &'a [u8] {
        self.line
    }

    /// The short-option letters typed, in order. Meaningless until a manifest says which exist.
    pub fn options(&self) -> &[u8] {
        &self.opts[..self.nopts]
    }

    /// **Whether positional `i` was quoted**, which is the whole of what quoting decides about
    /// authority: a quoted word designates itself and is never expanded, so `rm "*.txt"` hands over
    /// one name where `rm *.txt` hands over a set.
    ///
    /// `false` for an index past the end, because a word that is not there was not quoted.
    pub fn is_quoted(&self, i: usize) -> bool {
        i < self.npos && self.posq[i]
    }
}

#[cfg(test)]
mod tests;

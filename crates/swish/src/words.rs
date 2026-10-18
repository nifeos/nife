//! **Pattern versus text, and what a word designates**: [`is_pattern`], [`expansion`],
//! [`designation`], [`echo`] (the half of globbing that costs no authority) and quoting
//! ([`Piece`], [`pieces`], milestone 67 (`swish` the language)).
//!
//! Moved out of the crate root by milestone 884 (the shell logic crate is split along its seams).
//!
//! Name: provisional (milestone 884, 2026-10-12 UTC), for calef's naming pass.

use super::*;

/// **Whether this token is a designation to resolve rather than a word to print.**
///
/// `glob::has_magic` first, deliberately: an [`echo`] word is arbitrary text (`a:b` is not a path
/// and must not be refused as one), so nothing asks whether a token is a *nameable* path until it is
/// established that it is a pattern at all. Once it is, a pattern anywhere but the last component is
/// refused, because selecting directories to walk is an authority question (notes/glob.md).
pub fn is_pattern(token: &[u8]) -> Result<bool, Refusal> {
    if !glob::has_magic(token) {
        return Ok(false);
    }
    grant_plan::expand::magic_component(token)
}

/// **Expand the invocation's pattern operand, before anything is planned.**
///
/// The shell expands and then plans, which is what Unix does, so there is no divergence to earn.
/// What is different is the consequence: the planner must see the **set** rather than the pattern,
/// because the endowment is the set. Only the first pattern is expanded, and that is not a
/// limitation being hidden: no manifest declares two name slots, so a second operand of any kind is
/// already an unplaceable token and a refusal.
///
/// `expand` is the shell's directory read, which is the only part of this that needs a capability.
pub fn expansion(
    spec: &RunSpec,
    expand: &mut dyn FnMut(&[u8]) -> Result<NameSet, Say>,
) -> Result<Expansion, Say> {
    // **A program that hears words has nothing expanded; its words are designated instead**
    // (milestone 205 (how a foreign program is told what to do), DECISIONS §170 (how a foreign
    // program is told what to do)). A pattern would designate names and grant nothing they name, so
    // `grant_plan::check_words` refuses an unquoted one; a word that names something here is
    // granted it, by [`designation`].
    if let Some(ArgSpec::Words(grant)) = Program::from_name(spec.prog).map(|p| p.manifest().arg) {
        return designation(spec.line(), grant, expand);
    }
    for (i, token) in spec.positionals().iter().enumerate() {
        // **A quoted word designates itself** (milestone 67). This is the whole of what quoting
        // does to authority, and it is a narrowing: `rm "*.txt"` hands over one name where
        // `rm *.txt` hands over the set. Asked before `is_pattern`, because the question "is this a
        // pattern" is only worth asking about a word nobody quoted.
        if spec.is_quoted(i) {
            continue;
        }
        match is_pattern(token) {
            Ok(false) => continue,
            Ok(true) => return Ok(Expansion::at(i, expand(token)?)),
            Err(r) => return Err(Say::Cannot(r)),
        }
    }
    Ok(Expansion::none())
}

/// **Which of a line's words name something here** (milestone 205 (how a foreign program is told
/// what to do), §170 clauses 2 to 5), for a program that hears words: the shell's half of the
/// designation, which the planner turns into one directory grant at the current directory.
///
/// Every word after the program's name is asked, in `grant_plan::each_word`'s reading, so the
/// words designated are the words the program hears. For each one:
///
/// - a word shaped like an option (`-i`, `--color=never`) designates nothing;
/// - an absolute path or one that starts with `..` designates nothing, because the program's root
///   is this directory and `std` refuses both before they reach the wire;
/// - `.` (or `./`) designates this directory itself, which is calef's N1 ruling
///   (2026-09-27T06:27Z): a line that names no file grants nothing, and `rg pattern .` is how a
///   person says "here";
/// - otherwise its first component is looked up here with `resolve`, the shell's directory read. A
///   name that is here is designated. One that is not is inert bytes (a pattern, a count), unless
///   the program may create ([`grant_plan::WordGrant::Create`]), which is §170's clause 3.
///
/// So `rg needle src/main.rs` designates `src`, and the program can read all of `src`, not only
/// `main.rs`: the caretaker filters at this directory and nowhere below (see this crate's BUGS).
/// And a pattern that happens to be a file's name is designated too, read-only.
pub fn designation(
    line: &[u8],
    grant: grant_plan::WordGrant,
    resolve: &mut dyn FnMut(&[u8]) -> Result<NameSet, Say>,
) -> Result<Expansion, Say> {
    let mut d = grant_plan::expand::Designation::none();
    let mut first = true;
    let mut said = Ok(());
    let walked = grant_plan::each_word(line, &mut |w| {
        if core::mem::take(&mut first) || w.first() == Some(&b'-') || said.is_err() {
            return Ok(());
        }
        let mut rest = w;
        while let Some(r) = rest.strip_prefix(b"./") {
            rest = r;
        }
        if rest.is_empty() || rest == b"." {
            d.here();
            return Ok(());
        }
        let name = rest.split(|&b| b == b'/').next().unwrap_or(rest);
        // A quoted word with a glob byte in it is bytes the program hears, never a pattern this
        // shell expands: asking the directory for it would designate whatever it matched.
        if rest.first() == Some(&b'/') || name == b".." || name.is_empty() || glob::has_magic(name)
        {
            return Ok(());
        }
        match resolve(name) {
            Ok(set) => match set.iter().next() {
                Some((found, is_dir)) => d.name(found, is_dir),
                None => Ok(()),
            },
            Err(Say::Cannot(Refusal::NoMatch)) if grant == grant_plan::WordGrant::Create => {
                d.name(name, false)
            }
            Err(Say::NoDirectory) => {
                said = Err(Say::NoDirectory);
                Ok(())
            }
            Err(Say::Failed(e)) => {
                said = Err(Say::Failed(e));
                Ok(())
            }
            // Not here, or not a name at all: inert bytes.
            Err(_) => Ok(()),
        }
    });
    match (walked, said) {
        // A shell holding no directory designates nothing, and the planner grants nothing.
        (_, Err(Say::NoDirectory)) => Ok(Expansion::designated(
            grant_plan::expand::Designation::none(),
        )),
        (_, Err(s)) => Err(s),
        (Err(r), _) => Err(Say::Cannot(r)),
        (Ok(()), Ok(())) => Ok(Expansion::designated(d)),
    }
}

/// Write a set as [`echo`] shows it and as [`write_preview`] previews it: the names, one space
/// apart, in the order the directory yielded them. One renderer, so what is displayed in the two
/// places cannot differ in a way that hides a difference in the authority.
pub fn write_set(set: &NameSet, out: &mut dyn FnMut(&[u8])) {
    for (i, (name, _)) in set.iter().enumerate() {
        if i > 0 {
            out(b" ");
        }
        out(name);
    }
}

/// **`echo`, which expands**: `echo *.txt` prints literally what `rm *.txt` would transfer, at no
/// authority. Words with no magic are copied byte for byte, spacing included; only a pattern is
/// replaced by what it designates. A **quoted** word prints what is between its quotes and never
/// expands, so `echo "*.txt"` names a name where `echo *.txt` names a set. [`STATUS_WORD`] prints
/// `status`, how `$?` is read; `echo "$?"` prints the two characters ([`grant_plan::word`]).
///
/// # Examples
///
/// ```
/// use grant_plan::expand::NameSet;
/// use swish::{Say, echo};
///
/// // A directory holding two files, standing in for the one the shell would have to ask a server
/// // for. Everything else on this path is the real thing.
/// let mut listing = |_pattern: &[u8]| -> Result<NameSet, Say> {
///     let mut set = NameSet::empty();
///     set.push(b"notes.txt", false);
///     set.push(b"report.txt", false);
///     Ok(set)
/// };
///
/// let mut printed = Vec::new();
/// let said = echo(
///     b"the set: *.txt",
///     swish::Status::Ran,
///     &mut listing,
///     &mut |b| printed.extend_from_slice(b),
/// );
///
/// assert!(matches!(said, Say::Nothing));
/// // The words with no magic in them came through byte for byte; the pattern became what it
/// // designates, which is exactly what `rm *.txt` would have been granted.
/// assert_eq!(printed, b"the set: notes.txt report.txt");
/// ```
pub fn echo(
    text: &[u8],
    status: Status,
    expand: &mut dyn FnMut(&[u8]) -> Result<NameSet, Say>,
    out: &mut dyn FnMut(&[u8]),
) -> Say {
    let status_word = |w: &[u8]| (w == STATUS_WORD).then(|| status.digits());
    let said = pieces(text, &status_word, &mut |piece| {
        match piece {
            Piece::Space(s) | Piece::Quoted(s) | Piece::Substituted(s) => out(s),
            Piece::Word(w) => match is_pattern(w) {
                Ok(false) => out(w),
                Ok(true) => match expand(w) {
                    Ok(set) => write_set(&set, out),
                    // A pattern that matched nothing stops the line rather than printing itself.
                    // That is the same answer `rm` gets, and it has to be: if `echo` printed the
                    // pattern where `rm` refuses, the two would disagree about what the line
                    // designates, which is the one thing this pairing exists to rule out.
                    Err(s) => return Err(s),
                },
                Err(r) => return Err(Say::Cannot(r)),
            },
        }
        Ok(())
    });
    match said {
        Ok(()) => Say::Nothing,
        Err(s) => s,
    }
}

/// **One piece of a line, as the shell reads it for substitution** (milestone 47 (navigation and
/// naming), §141 (application is grant)).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Piece<'a> {
    /// Whitespace between words, verbatim.
    Space(&'a [u8]),
    /// A quoted word's text, never expanded or substituted.
    Quoted(&'a [u8]),
    /// A word that `substitute` replaced. **Always one word**, whatever its bytes are.
    Substituted(&'a [u8]),
    /// A bare word nothing replaced.
    Word(&'a [u8]),
}

/// **Split a line into words, then substitute, never the other way round.** This is the rule that
/// keeps word splitting out of this shell (§141 (application is grant), "kill word splitting"): the
/// line is split on the whitespace that was *typed*, and a word `substitute` replaces becomes one
/// [`Piece::Substituted`] whatever it contains. A value with a space in it is never split into two
/// words, because nothing here looks at a substituted value's bytes again.
///
/// Before the shell had an allocator this held because `$?` is one of three `'static` digits.
/// Now that it has one, it holds because substitution has exactly this one seam, and
/// `a_substituted_value_is_never_split` fails if it ever splits.
///
/// `each` may stop the line by returning an `Err`, which is returned. A word whose quotes do not
/// make sense is refused as `Say::Cannot`.
pub fn pieces<'a>(
    text: &'a [u8],
    substitute: &dyn Fn(&[u8]) -> Option<&'a [u8]>,
    each: &mut dyn FnMut(Piece<'a>) -> Result<(), Say>,
) -> Result<(), Say> {
    let mut i = 0;
    while i < text.len() {
        let space = i;
        while i < text.len() && text[i].is_ascii_whitespace() {
            i += 1;
        }
        if i > space {
            each(Piece::Space(&text[space..i]))?;
        }
        let word = i;
        // A word ends at the first bare whitespace, so `echo "two  spaces"` is one word and keeps
        // the spacing inside it.
        i = grant_plan::word::span(text, i, &|b| b.is_ascii_whitespace());
        if i == word {
            continue;
        }
        let token = grant_plan::word::read(&text[word..i]).map_err(Say::Cannot)?;
        if token.quoted {
            each(Piece::Quoted(token.text))?;
        } else if let Some(value) = substitute(token.text) {
            each(Piece::Substituted(value))?;
        } else {
            each(Piece::Word(token.text))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;

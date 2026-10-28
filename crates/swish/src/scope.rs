//! **The scoped block: `scope { commands }`** (milestone 52 (subshells without `fork`), DECISIONS §160 (what a subshell copies)), the one form that
//! scopes this shell's own state and groups commands. This module reads the block and decides, before
//! anything in it runs, whether it may run: the tail has to be one well-formed block, blocks nest
//! only [`MAXIMUM_DEPTH`] deep, and nothing inside may change what leaving the block cannot put
//! back. The restoring itself is the shell's, in `components/src/swish/scope.rs`, since the state
//! is the shell's. See notes/swish-language/scoped-block.md.

//! # BUGS
//!
//! - Only a background job is a change the block refuses. This shell has no variables or options
//!   yet, so there is nothing else of its own to put back, and a change to the world (a file made,
//!   a program run) stays done, as it does in every shell.
//! - A brace is syntax only as a word of its own (`scope { ls }`); `echo {` prints a brace.
//!
//! Name: provisional (milestone 52, 2026-10-27 UTC). `scope` is the spelling §160 leaves for
//! calef's naming pass.

use grant_plan::word::Cursor;
use grant_plan::{Command, Refusal, trim};

use crate::sequence;

/// How many scoped blocks may nest. Each level is a frame on a stack of a few pages (the stack
/// that has run out four times already, notes/pipes/the-boot.md), so the ceiling is small and a
/// deeper line is refused. Must match [`Refusal::ScopeTooDeep`]'s message.
///
/// Name: provisional, minted 2026-10-27 (UTC) by milestone/52-subshells.
pub const MAXIMUM_DEPTH: usize = 3;

/// Whether `line[i]` is a brace standing as a word of its own.
fn is_word_brace(line: &[u8], i: usize) -> bool {
    let edge = |b: u8| b.is_ascii_whitespace() || matches!(b, b';' | b'&' | b'|');
    matches!(line[i], b'{' | b'}')
        && (i == 0 || edge(line[i - 1]))
        && (i + 1 == line.len() || edge(line[i + 1]))
}

/// Whether the `{` at `line[i]` opens a block: it is a word of its own, and the word before it is
/// `scope`.
pub(crate) fn opens_block(line: &[u8], i: usize) -> bool {
    if line[i] != b'{' || !is_word_brace(line, i) {
        return false;
    }
    let before = &line[..i];
    let end = before
        .iter()
        .rposition(|b| !b.is_ascii_whitespace())
        .map_or(0, |p| p + 1);
    let word_start = before[..end]
        .iter()
        .rposition(|&b| b.is_ascii_whitespace() || matches!(b, b';' | b'&' | b'|'))
        .map_or(0, |p| p + 1);
    &before[word_start..end] == b"scope"
}

/// Whether the byte at `line[i]` is a brace that the block scanner counts, given how deep it
/// already is. Returns `+1` for an opening brace, `-1` for a closing one, `0` for neither.
pub(crate) fn brace_step(line: &[u8], i: usize, depth: usize) -> i8 {
    if depth == 0 {
        return i8::from(opens_block(line, i));
    }
    match line[i] {
        b'{' if is_word_brace(line, i) => 1,
        b'}' if is_word_brace(line, i) => -1,
        _ => 0,
    }
}

/// **The commands inside a block's `{ ... }`**, trimmed. `tail` is everything after the word
/// `scope`. Refused as [`Refusal::ScopeMalformed`] when it is not exactly one block: no opening
/// brace, a brace that never closes, an empty block, or text after the closing brace.
///
/// ```
/// use swish::scope::body;
/// assert_eq!(body(b" { cd logs; ls } "), Ok(&b"cd logs; ls"[..]));
/// assert!(body(b"{ ls } | wc").is_err());
/// assert!(body(b"{ ls").is_err());
/// assert!(body(b"ls").is_err());
/// ```
///
/// Name: provisional, minted 2026-10-27 (UTC) by milestone/52-subshells.
pub fn body(tail: &[u8]) -> Result<&[u8], Refusal> {
    let t = trim(tail);
    if t.first() != Some(&b'{') || !is_word_brace(t, 0) {
        return Err(Refusal::ScopeMalformed);
    }
    let mut c = Cursor::new();
    let mut depth = 0usize;
    for i in 0..t.len() {
        if !c.is_open() && i > 0 {
            match t[i] {
                b'{' if is_word_brace(t, i) => depth += 1,
                b'}' if is_word_brace(t, i) => {
                    if depth == 0 {
                        let inner = trim(&t[1..i]);
                        if i + 1 != t.len() || inner.is_empty() {
                            return Err(Refusal::ScopeMalformed);
                        }
                        return Ok(inner);
                    }
                    depth -= 1;
                }
                _ => {}
            }
        }
        c.step(t[i]);
    }
    Err(Refusal::ScopeMalformed)
}

/// **Whether `tail` (after the word `scope`) is a block this shell will run**, decided in full
/// before anything runs. Returns the block's commands. Refuses a malformed block, a nest deeper
/// than [`MAXIMUM_DEPTH`], and a block that would change a background job, which leaving the block
/// cannot put back ([`Refusal::ScopeCannotRestore`]): the shell holds a job as capabilities,
/// and starting, stopping, waiting for or resuming one hands off or consumes them.
///
/// ```
/// use grant_plan::Refusal;
/// use swish::scope::check;
/// assert!(check(b"{ cd logs && ls }").is_ok());
/// assert_eq!(check(b"{ kill %1 }"), Err(Refusal::ScopeCannotRestore));
/// assert_eq!(check(b"{ interrupt_heeder & }"), Err(Refusal::ScopeCannotRestore));
/// assert_eq!(check(b"{ scope { scope { scope { scope { ls } } } } }"), Err(Refusal::ScopeTooDeep));
/// ```
///
/// Name: provisional, minted 2026-10-27 (UTC) by milestone/52-subshells.
pub fn check(tail: &[u8]) -> Result<&[u8], Refusal> {
    let inner = body(tail)?;
    walk(inner, 1)?;
    Ok(inner)
}

/// Look through `commands` for a nested block or a job change.
fn walk(commands: &[u8], level: usize) -> Result<(), Refusal> {
    if sequence::background(commands).is_some() {
        return Err(Refusal::ScopeCannotRestore);
    }
    // A line that cannot be cut is refused by the shell when the block runs it.
    let Ok(seq) = sequence::split(commands) else {
        return Ok(());
    };
    for &(_, segment) in seq.segments() {
        let mut command = grant_plan::parse(segment);
        // The prefix words wrap a whole command line, which may itself be a block or a job verb.
        while let Command::Time(t) | Command::Xargs(t) = command {
            command = grant_plan::parse(t);
        }
        match command {
            Command::Kill(_) | Command::Wait(_) | Command::Fg(_) | Command::Bg(_) => {
                return Err(Refusal::ScopeCannotRestore);
            }
            Command::Scope(t) => {
                if level >= MAXIMUM_DEPTH {
                    return Err(Refusal::ScopeTooDeep);
                }
                walk(body(t)?, level + 1)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// **Whether a command, or a pipeline stage in it, opens with `(`.** `( ... )` has no meaning
/// here (§160, §312 (swish has one way to do each job)), and a line holding one is refused whole before any of it runs, since cutting
/// it at its connectors would run half of a group somebody meant to run together.
///
/// Name: provisional, minted 2026-10-27 (UTC) by milestone/52-subshells.
pub fn has_parenthesized_command(segment: &[u8]) -> bool {
    let mut c = Cursor::new();
    let mut stage_start = true;
    for (i, &b) in segment.iter().enumerate() {
        if !c.is_open() {
            if stage_start && !b.is_ascii_whitespace() {
                if b == b'(' {
                    return true;
                }
                stage_start = false;
            }
            if b == b'|' && segment.get(i + 1) != Some(&b'|') && (i == 0 || segment[i - 1] != b'|')
            {
                stage_start = true;
            }
        }
        c.step(b);
    }
    false
}

#[cfg(test)]
mod tests;

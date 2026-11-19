//! **glob: does this pattern match this name?** An `fnmatch(3)`-style matcher for one file name. It
//! is a total function on two byte strings: no IO, no allocation, no `unsafe`, a cost bound a caller
//! can compute before matching, and Kani proofs of the corners. See notes/glob.md.
//!
//! Three things a caller does not guess:
//!
//! - **Bytes, not characters.** The match is `fnmatch(3)` in the C locale, so `?` is one byte.
//! - **One name, not a path.** `/` is an ordinary byte. `matches(b"*.txt", b"dir/notes.txt")` is
//!   `true`: a `*` crosses separators, and a caller that matches a path splits it first.
//! - **A leading `.` needs a literal `.`** by default ([`LeadingDot::Special`]). `.` and `..` are ordinary
//!   names to the matcher, so `.*` matches both; the layer that enumerates a directory decides
//!   whether to offer them.
//!
//! A malformed pattern means itself, and there is no `**`, qualifier or POSIX class
//! (notes/glob-scope.md). The work of [`match_steps`] is bounded by [`cost_bound`]. [`has_magic`]
//! and [`literal`] are not counted: each decodes a `[` by scanning to its closing `]` or the end,
//! so a pattern of `n` unterminated brackets costs about `n * n / 2` byte reads in either, which is
//! small at the lengths of a shell word.
//!
//! # Syntax
//! | syntax | meaning |
//! |---|---|
//! | `*` | any run of bytes, including none |
//! | `?` | exactly one byte |
//! | `[abc]` | one byte from the set |
//! | `[a-z]` | one byte from the inclusive range |
//! | `[!abc]`, `[^abc]` | one byte *not* in the set |
//! | `\x` | the byte `x`, with no special meaning |
//! | anything else | itself |
//!
//! Name: ratified 2026-11-14 (UTC) for `byte_glob`: the standard term glob, with the bytes it matches
//! said in the name.

#![no_std]
#![forbid(unsafe_code)]

/// Whether a leading `.` in the name is special.
///
/// This is `fnmatch(3)`'s `FNM_PERIOD` as a two-valued type rather than a flag bit, because there
/// are exactly two answers and the compiler should be able to say which one a call site chose.
///
/// Name: provisional, ruled provisionally by the maintainer on 2026-11-14 (UTC) (block 1092
/// decision 2); the architect may revisit it before the first tag. It was `Dot`; the new spelling
/// says which dot the policy is about, as `FNM_PERIOD` and `require_literal_leading_dot` do.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LeadingDot {
    /// A name starting with `.` matches only a pattern starting with a **literal** `.` (which `\.`
    /// counts as, and `[.]` does not). What [`matches()`] uses, and what a shell user expects: `*`
    /// does not list `.config`.
    Special,
    /// A leading `.` is an ordinary byte. `*` matches `.config`. For callers matching against
    /// something that is not a user-facing filename listing.
    Ordinary,
}

/// **Does `pattern` match `name`?** A leading `.` in the name is special; see [`LeadingDot`].
///
/// `name` is one name, not a path: `/` is an ordinary byte, so `*` matches across it.
///
/// Total: every pair of byte strings has an answer, and no pattern makes this panic or hang.
///
/// # Examples
///
/// ```
/// use byte_glob::{matches, matches_with, has_magic, literal, LeadingDot};
///
/// assert!(matches(b"*.txt", b"notes.txt"));
/// assert!(!matches(b"*.txt", b"notes.md"));
///
/// // One byte, not one character, and not a leading dot.
/// assert!(matches(b"?ar", b"bar"));
/// assert!(!matches(b"*", b".hidden"));
///
/// // One name, not a path: `/` is an ordinary byte.
/// assert!(matches(b"*.txt", b"dir/notes.txt"));
/// assert!(matches_with(b"*", b".hidden", LeadingDot::Ordinary));
///
/// // Classes, ranges, negation.
/// assert!(matches(b"[abc]at", b"cat"));
/// assert!(matches(b"log[0-9].txt", b"log7.txt"));
/// assert!(matches(b"[!a]at", b"cat"));
/// assert!(!matches(b"[!c]at", b"cat"));
///
/// // A malformed pattern is a literal, not an error.
/// assert!(matches(b"[foo", b"[foo"));
///
/// // The shell asks two questions of a word: is it a pattern at all, and if not, what name is it?
/// assert!(has_magic(b"*.txt"));
/// assert!(!has_magic(br"a\*b"));
/// let mut buf = [0u8; 16];
/// assert_eq!(literal(br"a\*b", &mut buf), Some(&b"a*b"[..]));
/// assert_eq!(literal(b"*.txt", &mut buf), None); // it has magic; expand it instead
/// ```
pub fn matches(pattern: &[u8], name: &[u8]) -> bool {
    match_steps(pattern, name, LeadingDot::Special).0
}

/// [`matches()`], with the leading-dot rule under the caller's control.
///
/// # Examples
///
/// ```
/// use byte_glob::{matches, matches_with, LeadingDot};
///
/// assert!(!matches(b"*", b".config"));                       // `*` does not list dotfiles
/// assert!(matches(b".*", b".config"));                       // an explicit dot does
/// assert!(matches_with(b"*", b".config", LeadingDot::Ordinary));    // ... unless you ask otherwise
/// ```
pub fn matches_with(pattern: &[u8], name: &[u8], dot: LeadingDot) -> bool {
    match_steps(pattern, name, dot).0
}

/// **Could this pattern match anything other than one fixed name?**
///
/// True when the pattern contains an unescaped `*` or `?`, or a bracket expression that is actually
/// closed. False means the pattern is a literal, and [`literal`] will give you the name it is.
///
/// Precise rather than eager, so an unterminated `[` is a literal byte: a false "yes" costs the
/// shell a directory enumeration for a word that names one file. See notes/glob.md.
pub fn has_magic(pattern: &[u8]) -> bool {
    let mut j = 0;
    while let Some((elem, _)) = decode(pattern, j, 0) {
        match elem {
            Element::Byte { next, .. } => j = next,
            _ => return true,
        }
    }
    false
}

/// **The one name a magic-free pattern names**, with its escapes stripped, written into `out`.
///
/// `Some(name)` when the pattern has no magic and `out` was long enough; `None` when it has magic
/// (call the expander instead) or would not fit. `a\*b` is the file called `a*b`; this exists so
/// no caller reimplements escape-stripping.
pub fn literal<'a>(pattern: &[u8], out: &'a mut [u8]) -> Option<&'a [u8]> {
    let mut j = 0;
    let mut n = 0;
    while let Some((elem, _)) = decode(pattern, j, 0) {
        match elem {
            Element::Byte { byte, next } => {
                *out.get_mut(n)? = byte;
                n += 1;
                j = next;
            }
            _ => return None,
        }
    }
    Some(&out[..n])
}

/// [`matches_with`], plus the number of steps it took: the cost of the match, made observable.
///
/// A "step" is one byte of pattern examined, including the bytes a bracket expression is scanned
/// over and the ones re-examined after a backtrack. It is not a wall-clock measure. The one
/// promise about the count is that it is at most [`cost_bound`] of the two lengths; its exact value
/// is not promised across versions. It exists so the claim "this cannot blow up" is an
/// inequality something checks rather than a paragraph, by way of [`cost_bound`].
pub fn match_steps(pattern: &[u8], name: &[u8], dot: LeadingDot) -> (bool, usize) {
    let mut steps = 0usize;

    // The leading-dot rule, applied once and up front. It is a property of the two strings' first
    // bytes, so putting it in the loop would only mean asking it repeatedly.
    if dot == LeadingDot::Special
        && name.first() == Some(&b'.')
        && !opens_with_a_literal_dot(pattern)
    {
        return (false, steps);
    }

    let n = name.len();
    let mut i = 0usize; // next byte of `name` to account for
    let mut j = 0usize; // next element of `pattern` to apply

    // The single backtrack point: where to resume in the pattern after the most recent `*`, and how
    // much of the name that `*` is currently absorbing. `None` until a `*` is seen, and a later `*`
    // simply replaces it. This is the whole anti-blowup argument (notes/glob.md, "The algorithm").
    let mut star_next: Option<usize> = None;
    let mut star_absorbed = 0usize;

    loop {
        if i >= n {
            // The name is spent. Nothing can be gained by backtracking, since that only makes the
            // `*` absorb more of a name that has no more bytes, so the answer is fixed: whatever is
            // left of the pattern has to be able to match emptiness, and only `*` can.
            while pattern.get(j) == Some(&b'*') {
                j += 1;
                steps += 1;
            }
            return (j == pattern.len(), steps);
        }

        let advanced = match decode(pattern, j, name[i]) {
            // Pattern exhausted with name left over. Not a match here; the backtrack below decides
            // whether an earlier `*` can swallow the remainder.
            None => false,
            Some((elem, scanned)) => {
                steps += scanned;
                match elem {
                    Element::Star => {
                        star_next = Some(j + 1);
                        star_absorbed = i;
                        j += 1;
                        true
                    }
                    Element::Any { next } => {
                        i += 1;
                        j = next;
                        true
                    }
                    Element::Byte { byte, next } => {
                        if byte == name[i] {
                            i += 1;
                            j = next;
                            true
                        } else {
                            false
                        }
                    }
                    Element::Class { hit, next } => {
                        if hit {
                            i += 1;
                            j = next;
                            true
                        } else {
                            false
                        }
                    }
                }
            }
        };

        if advanced {
            continue;
        }

        match star_next {
            // Retry with the last `*` absorbing one more byte. `star_absorbed` only ever grows, and
            // it is bounded by the name's length, which is what makes the total work polynomial.
            Some(resume) => {
                star_absorbed += 1;
                i = star_absorbed;
                j = resume;
                steps += 1;
            }
            None => return (false, steps),
        }
    }
}

/// **The most steps a match of these two lengths can cost**, computable before matching starts.
///
/// At most `name_bytes + 1` backtracks, between two of them `i + j` strictly increases up to
/// `name_bytes + pattern_bytes`, and each iteration is at most one pass over the pattern to decode and
/// one to test a bracket expression. Saturating, so absurd lengths give `usize::MAX` rather than a
/// wrapped bound. Loose on purpose: the point is polynomial in both lengths with no term in the
/// number of `*`s. See notes/glob.md ("The algorithm, and why it cannot be made to hang").
pub const fn cost_bound(pattern_bytes: usize, name_bytes: usize) -> usize {
    let iterations = name_bytes
        .saturating_add(2)
        .saturating_mul(name_bytes.saturating_add(pattern_bytes).saturating_add(1));
    iterations
        .saturating_mul(pattern_bytes.saturating_mul(2).saturating_add(2))
        .saturating_add(pattern_bytes)
        .saturating_add(2)
}

// ---- the pattern's elements ------------------------------------------------------------------

/// One element of a pattern, **resolved against the name byte it is being applied to**.
///
/// [`Element::Class`] carries a `hit` rather than its body's bounds because a bracket expression is
/// scanned once, deciding membership as it goes: two loops nested in the match loop were past what
/// Kani could unroll (notes/glob/proofs.md, §46 (thin primitives or whole subsystems) rule 1).
///
/// Callers with no name byte in hand ([`has_magic`], [`literal`], [`opens_with_a_literal_dot`]) pass
/// any byte and read only the variant.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Element {
    /// `*`
    Star,
    /// `?`, and where the pattern continues after it.
    Any { next: usize },
    /// A literal byte, which may have arrived as `\x`, as an unterminated `[`, or as itself.
    Byte { byte: u8, next: usize },
    /// A closed `[...]`, and whether the byte it was asked about is in it (negation applied).
    Class { hit: bool, next: usize },
}

/// Decode the element at `j` as applied to the name byte `c`, and say how many pattern bytes that
/// cost.
///
/// `None` at the end of the pattern. Every other input decodes to something: this is where "there
/// is no malformed pattern" is implemented, by turning an unclosed `[` and a trailing `\` into the
/// bytes they are.
fn decode(pattern: &[u8], j: usize, c: u8) -> Option<(Element, usize)> {
    let byte = *pattern.get(j)?;
    let elem = match byte {
        b'*' => (Element::Star, 1),
        b'?' => (Element::Any { next: j + 1 }, 1),
        b'\\' => match pattern.get(j + 1) {
            Some(&escaped) => (
                Element::Byte {
                    byte: escaped,
                    next: j + 2,
                },
                2,
            ),
            // Nothing to escape. bash and fnmatch(3) both read this as the backslash itself.
            None => (
                Element::Byte {
                    byte: b'\\',
                    next: j + 1,
                },
                1,
            ),
        },
        b'[' => {
            let (closed, scanned) = class_at(pattern, j, c);
            match closed {
                Some((hit, next)) => (Element::Class { hit, next }, scanned),
                // No closing bracket anywhere, so it was never a bracket expression. The scan that
                // established that is charged for; it is the one place a step costs more than O(1).
                None => (
                    Element::Byte {
                        byte: b'[',
                        next: j + 1,
                    },
                    scanned,
                ),
            }
        }
        _ => (Element::Byte { byte, next: j + 1 }, 1),
    };
    Some(elem)
}

/// Walk the bracket expression opened at `open`, deciding in one pass both where it ends and whether
/// `c` is a member of it.
///
/// Returns `(Some((hit, next)), scanned)` for a closed expression, with negation already applied, or
/// `(None, scanned)` when there is no closing `]`, in which case the caller treats the `[` as a
/// literal byte. `scanned` is charged either way. A `!` or `^` right after `[` negates; a `]` right
/// after that is a member; a `-` is a range only before a byte that is not the terminator, so
/// `[a-]` and `[-a]` both mean "a hyphen or an a".
fn class_at(pattern: &[u8], open: usize, c: u8) -> (Option<(bool, usize)>, usize) {
    let mut k = open + 1;
    let negated = matches!(pattern.get(k), Some(b'!') | Some(b'^'));
    let mut scanned = 1usize;
    if negated {
        k += 1;
        scanned += 1;
    }

    let mut hit = false;
    let mut at_start = true;

    loop {
        let Some(&byte) = pattern.get(k) else {
            return (None, scanned);
        };
        scanned += 1;
        if byte == b']' && !at_start {
            return (Some((hit ^ negated, k + 1)), scanned);
        }
        at_start = false;

        // An escape inside a class, which is how `[\]]` names a `]`. Strict POSIX bracket
        // expressions have no escapes; glibc's fnmatch and bash both honour one, and matching them
        // is worth more than matching the standard nobody implements.
        let (lo, after_lo) = if byte == b'\\' && k + 1 < pattern.len() {
            scanned += 1;
            (pattern[k + 1], k + 2)
        } else {
            (byte, k + 1)
        };

        let ranged = pattern.get(after_lo) == Some(&b'-')
            && matches!(pattern.get(after_lo + 1), Some(&b) if b != b']');
        if ranged {
            let start = after_lo + 1;
            let (hi, after_hi) = if pattern[start] == b'\\' && start + 1 < pattern.len() {
                (pattern[start + 1], start + 2)
            } else {
                (pattern[start], start + 1)
            };
            scanned += after_hi - after_lo;
            // A reversed range (`[z-a]`) is empty. POSIX calls it undefined; glibc makes it match
            // nothing, which is the answer that cannot surprise anyone into a larger grant.
            if lo <= c && c <= hi {
                hit = true;
            }
            k = after_hi;
        } else {
            if lo == c {
                hit = true;
            }
            k = after_lo;
        }
    }
}

/// Does the pattern begin with a literal `.`?
///
/// `.` and `\.` do; `[.]`, `*` and `?` do not. That is glibc's `FNM_PERIOD` rule, and the reason a
/// bracket expression does not count is that the rule exists to stop a *wildcard* reaching a
/// dotfile, and a class is a wildcard however few members it has.
fn opens_with_a_literal_dot(pattern: &[u8]) -> bool {
    matches!(
        decode(pattern, 0, 0),
        Some((Element::Byte { byte: b'.', .. }, _))
    )
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod verification;

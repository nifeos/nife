//! **glob: does this pattern match this name?** A total function on two byte strings: no IO, no
//! syscalls, no allocation, no capability. One matcher serves both the shell's expansion and its
//! grant, so `echo *.txt` prints exactly what `rm *.txt` would hand over. Written rather than
//! depended on because it is on the verification path (§46 (thin primitives or whole subsystems;
//! we write everything in between)). See notes/glob.md.
//!
//! Bytes, as `fnmatch(3)` in the C locale; a malformed pattern means itself; a leading `.` needs a
//! literal `.` ([`Dot::Special`]); no `**`, qualifiers or POSIX classes (notes/glob-scope.md). The
//! work is bounded by [`cost_bound`], which a caller can check before matching.
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
//! Name: ratified 2026-08-01 (calef, the naming tenet): a standard term that is already right.

#![no_std]

/// Whether a leading `.` in the name is special.
///
/// This is `fnmatch(3)`'s `FNM_PERIOD` as a two-valued type rather than a flag bit, because there
/// are exactly two answers and the compiler should be able to say which one a call site chose.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dot {
    /// A name starting with `.` matches only a pattern starting with a **literal** `.` (which `\.`
    /// counts as, and `[.]` does not). What [`matches()`] uses, and what a shell user expects: `*`
    /// does not list `.config`.
    Special,
    /// A leading `.` is an ordinary byte. `*` matches `.config`. For callers matching against
    /// something that is not a user-facing filename listing.
    Ordinary,
}

/// **Does `pattern` match `name`?** A leading `.` in the name is special; see [`Dot`].
///
/// Total: every pair of byte strings has an answer, and no pattern makes this panic or hang.
///
/// # Examples
///
/// ```
/// use glob::{matches, matches_with, has_magic, literal, Dot};
///
/// assert!(matches(b"*.txt", b"notes.txt"));
/// assert!(!matches(b"*.txt", b"notes.md"));
///
/// // One byte, not one character, and not a leading dot.
/// assert!(matches(b"?ar", b"bar"));
/// assert!(!matches(b"*", b".hidden"));
/// assert!(matches_with(b"*", b".hidden", Dot::Ordinary));
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
    match_steps(pattern, name, Dot::Special).0
}

/// [`matches()`], with the leading-dot rule under the caller's control.
///
/// # Examples
///
/// ```
/// use glob::{matches, matches_with, Dot};
///
/// assert!(!matches(b"*", b".config"));                       // `*` does not list dotfiles
/// assert!(matches(b".*", b".config"));                       // an explicit dot does
/// assert!(matches_with(b"*", b".config", Dot::Ordinary));    // ... unless you ask otherwise
/// ```
pub fn matches_with(pattern: &[u8], name: &[u8], dot: Dot) -> bool {
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
            Elem::Byte { next, .. } => j = next,
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
            Elem::Byte { byte, next } => {
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
/// over and the ones re-examined after a backtrack. It is not a wall-clock measure and it is not
/// stable across changes to this crate; it exists so the claim "this cannot blow up" is an
/// inequality something checks rather than a paragraph, by way of [`cost_bound`].
pub fn match_steps(pattern: &[u8], name: &[u8], dot: Dot) -> (bool, usize) {
    let mut steps = 0usize;

    // The leading-dot rule, applied once and up front. It is a property of the two strings' first
    // bytes, so putting it in the loop would only mean asking it repeatedly.
    if dot == Dot::Special && name.first() == Some(&b'.') && !opens_with_a_literal_dot(pattern) {
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
                    Elem::Star => {
                        star_next = Some(j + 1);
                        star_absorbed = i;
                        j += 1;
                        true
                    }
                    Elem::Any { next } => {
                        i += 1;
                        j = next;
                        true
                    }
                    Elem::Byte { byte, next } => {
                        if byte == name[i] {
                            i += 1;
                            j = next;
                            true
                        } else {
                            false
                        }
                    }
                    Elem::Class { hit, next } => {
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
/// At most `name_len + 1` backtracks, between two of them `i + j` strictly increases up to
/// `name_len + pattern_len`, and each iteration is at most one pass over the pattern to decode and
/// one to test a bracket expression. Saturating, so absurd lengths give `usize::MAX` rather than a
/// wrapped bound. Loose on purpose: the point is polynomial in both lengths with no term in the
/// number of `*`s. See notes/glob.md ("The algorithm, and why it cannot be made to hang").
pub const fn cost_bound(pattern_len: usize, name_len: usize) -> usize {
    let iterations = name_len
        .saturating_add(2)
        .saturating_mul(name_len.saturating_add(pattern_len).saturating_add(1));
    iterations
        .saturating_mul(pattern_len.saturating_mul(2).saturating_add(2))
        .saturating_add(pattern_len)
        .saturating_add(2)
}

// ---- the pattern's elements ------------------------------------------------------------------

/// One element of a pattern, **resolved against the name byte it is being applied to**.
///
/// [`Elem::Class`] carries a `hit` rather than its body's bounds because a bracket expression is
/// scanned once, deciding membership as it goes: two loops nested in the match loop were past what
/// Kani could unroll (notes/glob/proofs.md, §46 (thin primitives or whole subsystems) rule 1).
///
/// Callers with no name byte in hand ([`has_magic`], [`literal`], [`opens_with_a_literal_dot`]) pass
/// any byte and read only the variant.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Elem {
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
fn decode(pattern: &[u8], j: usize, c: u8) -> Option<(Elem, usize)> {
    let byte = *pattern.get(j)?;
    let elem = match byte {
        b'*' => (Elem::Star, 1),
        b'?' => (Elem::Any { next: j + 1 }, 1),
        b'\\' => match pattern.get(j + 1) {
            Some(&escaped) => (
                Elem::Byte {
                    byte: escaped,
                    next: j + 2,
                },
                2,
            ),
            // Nothing to escape. bash and fnmatch(3) both read this as the backslash itself.
            None => (
                Elem::Byte {
                    byte: b'\\',
                    next: j + 1,
                },
                1,
            ),
        },
        b'[' => {
            let (closed, scanned) = class_at(pattern, j, c);
            match closed {
                Some((hit, next)) => (Elem::Class { hit, next }, scanned),
                // No closing bracket anywhere, so it was never a bracket expression. The scan that
                // established that is charged for; it is the one place a step costs more than O(1).
                None => (
                    Elem::Byte {
                        byte: b'[',
                        next: j + 1,
                    },
                    scanned,
                ),
            }
        }
        _ => (Elem::Byte { byte, next: j + 1 }, 1),
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
        Some((Elem::Byte { byte: b'.', .. }, _))
    )
}

// ---- host tests ------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_literal_pattern_matches_itself_and_nothing_else() {
        assert!(matches(b"notes.txt", b"notes.txt"));
        assert!(!matches(b"notes.txt", b"notes.txtx"));
        assert!(!matches(b"notes.txt", b"notes.tx"));
        assert!(!matches(b"notes.txt", b""));
    }

    #[test]
    fn star_matches_any_run_including_none() {
        assert!(matches(b"*", b""));
        assert!(matches(b"*", b"a"));
        assert!(matches(b"*", b"anything at all"));
        assert!(matches(b"a*", b"a"));
        assert!(matches(b"*a", b"a"));
        assert!(matches(b"a*b", b"ab"));
        assert!(matches(b"a*b", b"axxxb"));
        assert!(!matches(b"a*b", b"axxx"));
        assert!(matches(b"**", b"anything")); // two stars, one meaning: see the module comment
    }

    #[test]
    fn question_matches_exactly_one_byte() {
        assert!(matches(b"?", b"a"));
        assert!(!matches(b"?", b""));
        assert!(!matches(b"?", b"ab"));
        assert!(matches(b"???", b"abc"));
        // One byte, not one character: `é` is two bytes in UTF-8 and takes two `?`.
        assert!(!matches(b"?", "\u{e9}".as_bytes()));
        assert!(matches(b"??", "\u{e9}".as_bytes()));
    }

    #[test]
    fn classes_ranges_and_negation() {
        assert!(matches(b"[abc]", b"b"));
        assert!(!matches(b"[abc]", b"d"));
        assert!(matches(b"[a-z]", b"q"));
        assert!(!matches(b"[a-z]", b"Q"));
        assert!(matches(b"[!a]", b"b"));
        assert!(!matches(b"[!a]", b"a"));
        assert!(matches(b"[^a]", b"b"));
        assert!(!matches(b"[^a]", b"a"));
        assert!(matches(b"log[0-9][0-9].txt", b"log42.txt"));
        assert!(!matches(b"log[0-9][0-9].txt", b"log4x.txt"));
        // A negated class still has to match a byte; it is not `*`.
        assert!(!matches(b"[!a]", b""));
        assert!(!matches(b"[!a]", b"bb"));
    }

    /// The corners real glob implementations get wrong, each a decision notes/glob.md ("Corners")
    /// records, pinned so a rewrite has to argue with a test rather than with prose.
    #[test]
    fn the_corners_everyone_gets_wrong() {
        // An unterminated `[` is a literal `[`, not an error and not a wildcard.
        assert!(matches(b"[foo", b"[foo"));
        assert!(!matches(b"[foo", b"f"));
        assert!(matches(b"[", b"["));
        assert!(matches(b"[!", b"[!"));
        assert!(matches(b"[]", b"[]")); // `]` first is a member, so this class never closes

        // `]` as the first member, which is the only way to have one without an escape.
        assert!(matches(b"[]]", b"]"));
        assert!(matches(b"[!]]", b"a"));
        assert!(!matches(b"[!]]", b"]"));
        assert!(matches(br"[\]]", b"]"));

        // `-` first or last is a member, not a range.
        assert!(matches(b"[a-]", b"a"));
        assert!(matches(b"[a-]", b"-"));
        assert!(!matches(b"[a-]", b"b"));
        assert!(matches(b"[-a]", b"-"));
        assert!(matches(b"[-a]", b"a"));

        // A reversed range is empty.
        assert!(!matches(b"[z-a]", b"m"));
        assert!(!matches(b"[z-a]", b"z"));

        // The empty pattern matches the empty name and nothing else.
        assert!(matches(b"", b""));
        assert!(!matches(b"", b"a"));
        assert!(!matches(b"a", b""));

        // A trailing backslash is a backslash.
        assert!(matches(br"a\", br"a\"));
        assert!(matches(br"\", br"\"));
        assert!(matches(br"\*", b"*"));
        assert!(!matches(br"\*", b"anything"));
        assert!(matches(br"\\", br"\"));

        // POSIX character classes are not implemented, and this pins what the syntax does instead
        // rather than leaving a reader to discover it: a class of `[:alpha`, then a literal `]`.
        assert!(matches(b"[[:alpha:]]", b"a]"));
        assert!(matches(b"[[:alpha:]]", b":]"));
        assert!(!matches(b"[[:alpha:]]", b"z]"));
        assert!(!matches(b"[[:alpha:]]", b"a"));
    }

    #[test]
    fn a_leading_dot_is_special_by_default() {
        assert!(!matches(b"*", b".config"));
        assert!(!matches(b"?config", b".config"));
        assert!(!matches(b"[.]config", b".config"));
        assert!(matches(b".config", b".config"));
        assert!(matches(b".*", b".config"));
        assert!(matches(br"\.config", b".config"));
        assert!(!matches(b"*", b"."));
        assert!(!matches(b"*", b".."));
        assert!(matches(b".*", b".."));

        // A dot anywhere else is an ordinary byte.
        assert!(matches(b"*", b"a.config"));
        assert!(matches(b"a*", b"a.config"));

        // And the rule is switchable, because it is a policy.
        assert!(matches_with(b"*", b".config", Dot::Ordinary));
        assert!(matches_with(b"?", b".", Dot::Ordinary));
        assert!(matches_with(b"[.]config", b".config", Dot::Ordinary));
    }

    #[test]
    fn magic_and_the_literal_behind_it() {
        assert!(has_magic(b"*.txt"));
        assert!(has_magic(b"a?b"));
        assert!(has_magic(b"[ab]"));
        assert!(!has_magic(b"notes.txt"));
        assert!(!has_magic(br"a\*b"));
        assert!(!has_magic(b"[foo")); // never closes, so it is a literal `[foo`
        assert!(!has_magic(b""));

        let mut buf = [0u8; 32];
        assert_eq!(literal(b"notes.txt", &mut buf), Some(&b"notes.txt"[..]));
        assert_eq!(literal(br"a\*b", &mut buf), Some(&b"a*b"[..]));
        assert_eq!(literal(b"[foo", &mut buf), Some(&b"[foo"[..]));
        assert_eq!(literal(b"", &mut buf), Some(&b""[..]));
        assert_eq!(literal(b"*.txt", &mut buf), None);

        // Too small an output buffer is `None`, not a panic and not a truncation: a truncated
        // filename is a different file, and this is the path that decides what gets granted.
        let mut tiny = [0u8; 3];
        assert_eq!(literal(b"abcd", &mut tiny), None);
        assert_eq!(literal(b"abc", &mut tiny), Some(&b"abc"[..]));
    }

    /// A magic-free pattern matches its own literal and nothing else, which is what lets the shell
    /// skip enumeration for a word with no magic in it. Checked over every escape corner rather
    /// than asserted once.
    #[test]
    fn no_magic_means_the_literal_is_the_only_match() {
        for pattern in [
            &b"notes.txt"[..],
            br"a\*b",
            br"a\?b",
            br"\[ab\]",
            b"[foo",
            br"a\",
            b"",
            b".config",
        ] {
            let mut buf = [0u8; 32];
            let len = literal(pattern, &mut buf).expect("no magic").len();
            assert!(
                matches_with(pattern, &buf[..len], Dot::Ordinary),
                "{pattern:?} should match its own literal"
            );
            // And nothing else: a byte appended or removed breaks it.
            buf[len] = b'x';
            assert!(!matches_with(pattern, &buf[..len + 1], Dot::Ordinary));
            if len > 0 {
                assert!(!matches_with(pattern, &buf[..len - 1], Dot::Ordinary));
            }
        }
    }

    // ---- the exhaustive cross-check ----------------------------------------------------------

    /// The naive matcher everybody writes first: try every way a `*` could split the name, by
    /// recursion. Exponential, and correct by inspection, which is exactly the trade this is for.
    ///
    /// It shares [`decode`] with the real matcher on purpose. What the
    /// cross-check below compares is therefore the **search strategy** and nothing else: if the two
    /// disagree, the greedy single-backtrack loop is wrong, not the syntax.
    fn naive(pattern: &[u8], name: &[u8], j: usize, i: usize) -> bool {
        // Past the end of the name the byte does not matter: every arm that could read it returns
        // false first.
        let c = name.get(i).copied().unwrap_or(0);
        let Some((elem, _)) = decode(pattern, j, c) else {
            return i == name.len();
        };
        match elem {
            Elem::Star => {
                let mut absorbed = i;
                loop {
                    if naive(pattern, name, j + 1, absorbed) {
                        return true;
                    }
                    if absorbed == name.len() {
                        return false;
                    }
                    absorbed += 1;
                }
            }
            _ if i >= name.len() => false,
            Elem::Any { next } => naive(pattern, name, next, i + 1),
            Elem::Byte { byte, next } => byte == name[i] && naive(pattern, name, next, i + 1),
            Elem::Class { hit, next } => hit && naive(pattern, name, next, i + 1),
        }
    }

    /// **The greedy matcher agrees with exhaustive search, on every pattern and name in a domain
    /// small enough to enumerate completely.**
    ///
    /// Every pattern of length 0..=5 over one representative of each syntactic role (`a`, `b`, `*`,
    /// `?`, `[`, `]`, `!`, `-`, `\`) against every name of length 0..=3 over `a`, `b`, `-`: 66,430
    /// patterns times 40 names, 2,657,200 comparisons. Five is the shortest length that reaches
    /// `[a-b]` and `[!ab]`. Enumeration settles the equivalence a solver struggles to unwind; see
    /// notes/glob/proofs.md ("What enumeration proves instead").
    #[test]
    fn greedy_agrees_with_exhaustive_search_over_every_short_pattern() {
        const PAT: &[u8] = b"ab*?[]!-\\";
        const NAME: &[u8] = b"ab-";

        let mut pattern = [0u8; 5];
        let mut name = [0u8; 3];
        let mut checked = 0u64;
        let mut hits = 0u64;

        // Under Miri, every 61st pattern: 61 is prime and not a power of the 9-byte alphabet, so the
        // sample lands on every length and role, ~44,000 comparisons. The complete enumeration is
        // native-only; "Miri-clean" means the sample (notes/undefined-behavior.md).
        let stride: u64 = if cfg!(miri) { 61 } else { 1 };
        for plen in 0..=5usize {
            let combos = (PAT.len() as u64).pow(plen as u32);
            for pc in (0..combos).step_by(stride as usize) {
                let mut v = pc;
                for slot in pattern.iter_mut().take(plen) {
                    *slot = PAT[(v % PAT.len() as u64) as usize];
                    v /= PAT.len() as u64;
                }
                let p = &pattern[..plen];
                for nlen in 0..=3usize {
                    let ncombos = (NAME.len() as u64).pow(nlen as u32);
                    for nc in 0..ncombos {
                        let mut v = nc;
                        for slot in name.iter_mut().take(nlen) {
                            *slot = NAME[(v % NAME.len() as u64) as usize];
                            v /= NAME.len() as u64;
                        }
                        let n = &name[..nlen];
                        let fast = matches_with(p, n, Dot::Ordinary);
                        let slow = naive(p, n, 0, 0);
                        assert_eq!(fast, slow, "pattern {p:?} name {n:?}");
                        checked += 1;
                        hits += u64::from(fast);
                    }
                }
            }
        }
        // The completeness pin: the enumeration (or, under Miri, the stride's exact sample of it)
        // actually ran. The Miri number is the same arithmetic at stride 61:
        // (1 + 1 + 2 + 12 + 108 + 969) patterns x 40 names.
        assert_eq!(checked, if cfg!(miri) { 43_720 } else { 2_657_200 });
        // Not vacuous: two matchers that both said "no" to everything would agree, so the 15,956
        // native matches are pinned. The Miri sample checks only agreement, since its hit count is
        // an accident of the stride.
        if cfg!(miri) {
            assert!(hits > 0 && hits < checked, "{hits} matches");
        } else {
            assert!(hits > 10_000 && hits < checked - 10_000, "{hits} matches");
        }
    }

    /// **The worst case over the proof's own domain, measured rather than guessed**: where the
    /// harnesses' `#[kani::unwind]` numbers come from.
    ///
    /// An unwind bound must exceed the most iterations any loop runs, and one set too high grows the
    /// formula for nothing. Every outer iteration adds at least one step, so the worst step count is
    /// a safe bound on iterations. The nine-byte alphabet is sound because the step count depends on
    /// a byte's syntactic role, not its value. Skipped under Miri: a strided sample can miss the
    /// pinned argmax (`at_three == 10`) and fail against correct code. See notes/glob/proofs.md.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "asserts the exact worst case over 1.8M runs; a sample would miss the argmax and \
                  fail against correct code"
    )]
    fn the_worst_case_over_the_proof_domain_is_what_the_unwind_bounds_are_set_from() {
        const PAT: &[u8] = b"ab*?[]!-\\";
        const NAME: &[u8] = b"ab-";

        let mut worst = [0usize; 5]; // indexed by max(pattern len, name len)
        let mut pattern = [0u8; 4];
        let mut name = [0u8; 4];

        for plen in 0..=4usize {
            for pc in 0..(PAT.len() as u64).pow(plen as u32) {
                let mut v = pc;
                for slot in pattern.iter_mut().take(plen) {
                    *slot = PAT[(v % PAT.len() as u64) as usize];
                    v /= PAT.len() as u64;
                }
                for nlen in 0..=4usize {
                    for nc in 0..(NAME.len() as u64).pow(nlen as u32) {
                        let mut v = nc;
                        for slot in name.iter_mut().take(nlen) {
                            *slot = NAME[(v % NAME.len() as u64) as usize];
                            v /= NAME.len() as u64;
                        }
                        let bucket = plen.max(nlen);
                        for dot in [Dot::Special, Dot::Ordinary] {
                            let (_, steps) = match_steps(&pattern[..plen], &name[..nlen], dot);
                            worst[bucket] = worst[bucket].max(steps);
                        }
                    }
                }
            }
        }

        // Cumulative, since a harness bounded at four bytes admits every shorter input too.
        let at_three = worst[0].max(worst[1]).max(worst[2]).max(worst[3]);
        let at_four = at_three.max(worst[4]);
        assert_eq!(at_three, 10, "the three-byte worst case moved");
        assert_eq!(at_four, 17, "the four-byte worst case moved");
    }

    // ---- the blowup that is not there --------------------------------------------------------

    /// **The classic backtracking bomb, at a scale that would settle the question.**
    ///
    /// `a*a*a*...*b` against a long run of `a` with no `b` makes a recursive matcher try every split
    /// of the run between the stars. With ten stars and 100,000 bytes this one walks the name a few
    /// times, and its measured step count is under the [`cost_bound`] computed before the match.
    #[test]
    fn the_pathological_pattern_stays_inside_its_bound() {
        // 100,000 bytes native, where a quadratic matcher would show; 2,000 under Miri, which cannot
        // afford ~4M matcher steps. Both assertions run at both sizes (notes/undefined-behavior.md).
        const NAME_LEN: usize = if cfg!(miri) { 2_000 } else { 100_000 };
        let name = [b'a'; NAME_LEN];
        let pattern = b"a*a*a*a*a*a*a*a*a*a*b";
        let (hit, steps) = match_steps(pattern, &name[..], Dot::Ordinary);
        assert!(!hit);
        assert!(
            steps <= cost_bound(pattern.len(), NAME_LEN),
            "{steps} steps exceeded the bound {}",
            cost_bound(pattern.len(), NAME_LEN)
        );

        // The measured cost is linear-ish in practice, far under the quadratic bound. Asserted
        // loosely, because the point is the order of magnitude: a bomb would be off by many.
        assert!(steps < 40 * NAME_LEN, "{steps} steps is not linear-ish");
    }

    /// The other shape of the same attack: many `?` after a star, and a name that nearly matches.
    /// This one genuinely is quadratic, and the test pins that it is quadratic rather than worse.
    #[test]
    fn a_star_followed_by_a_long_tail_stays_polynomial() {
        // Shrunk under Miri like the test above: this one is genuinely quadratic, so its native
        // 2,000 x 512 is ~1M matcher steps, minutes under an interpreter that add no new paths.
        const NAME_LEN: usize = if cfg!(miri) { 200 } else { 2_000 };
        const PATTERN_LEN: usize = if cfg!(miri) { 64 } else { 512 };
        let name = [b'a'; NAME_LEN];
        let mut pattern = [b'?'; PATTERN_LEN];
        pattern[0] = b'*';
        pattern[PATTERN_LEN - 1] = b'b';

        let (hit, steps) = match_steps(&pattern[..], &name[..], Dot::Ordinary);
        assert!(!hit);
        assert!(steps <= cost_bound(pattern.len(), NAME_LEN));
    }

    /// Every unterminated bracket in a long pattern rescans to the end, which is the one place the
    /// per-step cost is not constant. Bounded, and the bound knows about it.
    #[test]
    fn a_pattern_full_of_unterminated_brackets_stays_inside_its_bound() {
        let pattern = [b'['; 400];
        let name = [b'['; 400];
        let (hit, steps) = match_steps(&pattern[..], &name[..], Dot::Ordinary);
        assert!(hit);
        assert!(steps <= cost_bound(pattern.len(), name.len()));
    }

    /// The cost bound saturates instead of wrapping. A wrapped bound would be a small number, and
    /// every `steps <= bound` assertion in this file would then be checking nothing.
    #[test]
    fn the_cost_bound_saturates() {
        assert_eq!(cost_bound(usize::MAX, usize::MAX), usize::MAX);
        assert_eq!(cost_bound(0, 0), 6); // the empty pattern against the empty name
    }

    /// The step count is a real measure of work, not a constant. Without this the bound assertions
    /// above would pass on a counter that never moved.
    #[test]
    fn the_step_count_grows_with_the_work() {
        let short = match_steps(b"*b", b"aaaa", Dot::Ordinary).1;
        let long = match_steps(b"*b", b"aaaaaaaaaaaaaaaa", Dot::Ordinary).1;
        assert!(long > short);
        assert!(short > 0);
    }

    // ---- what mutation testing found (milestone 85) -------------------------------------------
    //
    // These run under `cargo test`, outside `#[cfg(kani)]`. See notes/mutation-testing.md.

    /// **Each syntactic extra costs exactly what it scans**: a negation marker one step, an escape
    /// one step, a range's tail its two bytes, a trailing star one step. Kani proves only that the
    /// count stays under [`cost_bound`], which a counter stuck at zero satisfies; these say what it
    /// counts, as differences between near-identical matches.
    ///
    /// The range is asserted twice. At the head of a one-member class `scanned` is 2 and the tail is
    /// 2, so `+`, `*`, `-` and `/` mutations of `scanned += after_hi - after_lo` agree; one member in
    /// front of the range separates them.
    #[test]
    fn each_class_feature_costs_its_own_scan() {
        let steps = |p: &[u8], n: &[u8]| match_steps(p, n, Dot::Special).1;
        assert_eq!(
            steps(b"[!b]", b"a"),
            steps(b"[b]", b"b") + 1,
            "negation marker"
        );
        assert_eq!(steps(b"[\\b]", b"b"), steps(b"[b]", b"b") + 1, "escape");
        assert_eq!(
            steps(b"[a-c]", b"b"),
            steps(b"[ac]", b"a") + 1,
            "range tail"
        );
        assert_eq!(
            steps(b"[xa-c]", b"b"),
            steps(b"[xac]", b"c") + 1,
            "range tail, past the coincidence at the head of the class"
        );
        assert_eq!(
            steps(b"ab**", b"ab"),
            steps(b"ab*", b"ab") + 1,
            "trailing star"
        );
    }

    /// An escaped `]` as a range's high end: `[A-\]]` is the range 0x41..=0x5D. The parse has to
    /// take the byte AFTER the backslash as the endpoint and resume AFTER the escape, and both
    /// indices were mutable without any test noticing.
    #[test]
    fn an_escaped_bracket_can_end_a_range() {
        assert!(matches(b"[A-\\]]", b"["), "0x5B is inside A..=0x5D");
        assert!(matches(b"[A-\\]]", b"]"), "the endpoint itself is inside");
        assert!(!matches(b"[A-\\]]", b"^"), "0x5E is one past the endpoint");
        // A plain range followed by a literal, so the resume index shows in what comes next.
        assert!(matches(b"[a-c]d", b"bd"));
        assert!(!matches(b"[a-c]d", b"bx"));
        // A class that ends in a bare escape is unterminated, never a read past the end.
        assert!(!matches(b"[a-\\", b"b"));
    }

    /// **A range covers its interior and nothing else**, the half of the parse no step count sees.
    ///
    /// A resume index that lands inside the range costs the same total steps, but widens the class,
    /// and a wider class is a larger grant.
    #[test]
    fn a_range_admits_neither_its_hyphen_nor_a_reversed_endpoint() {
        // Resuming at the `-` would re-read it as an ordinary member. It is punctuation here, and
        // the `[a-]` and `[-a]` spellings above are where a hyphen *is* a member.
        assert!(!matches(b"[a-c]", b"-"));
        assert!(matches(b"[a-c]", b"b"));

        // Resuming at the high endpoint would re-read it as an ordinary member, which is invisible
        // for a range that contains it. A reversed range contains neither end, so it is the
        // spelling that tells the two apart.
        assert!(!matches(b"[z-a]", b"a"));
        assert!(!matches(b"[z-a]", b"z"));
    }
}

// ---- machine-checked proofs ------------------------------------------------------------------

/// The proofs, and the bounds they run under.
///
/// The harnesses quantify over every byte value of patterns and names up to three bytes (2^48
/// pairs, every arm of the decoder) but not over length. The length-independent claims are the
/// host tests': the exhaustive cross-check and the 100,000-byte blowup tests. See notes/glob.md
/// ("What is proved") and notes/glob/proofs.md.
#[cfg(kani)]
mod verification {
    use super::*;

    /// **Three, measured rather than chosen**: four bytes each passed 3 GB and ten minutes. Three
    /// reaches `[a]`, `\*a`, a trailing backslash, an unterminated bracket and two stars; `[!a]` and
    /// `[a-b]` are reached by `a_negated_class_is_the_exact_complement_of_the_class`.
    const P: usize = 3;
    const N: usize = 3;

    /// Two symbolic lengths, within the bound above, so the empty pattern, the empty name and a
    /// pattern that ends mid-class are all reached. Symbolic length is the costliest thing here.
    fn any_lengths() -> (usize, usize) {
        let plen: usize = kani::any();
        let nlen: usize = kani::any();
        kani::assume(plen <= P);
        kani::assume(nlen <= N);
        (plen, nlen)
    }

    /// **Matching is total.**
    ///
    /// For every pattern and name of up to three arbitrary bytes, under both [`Dot`] settings: no
    /// panic, overflow or out-of-bounds index, and the loop terminates within the unwinding bound,
    /// which is the property that matters for an untrusted pattern.
    /// Falsification: replayable `crates/glob/falsifications/verification.matching_is_total.patch`
    #[kani::proof]
    #[kani::unwind(11)]
    fn matching_is_total() {
        let pattern: [u8; P] = kani::any();
        let name: [u8; N] = kani::any();
        let (plen, nlen) = any_lengths();
        let dot = if kani::any() {
            Dot::Special
        } else {
            Dot::Ordinary
        };
        let _ = match_steps(&pattern[..plen], &name[..nlen], dot);
    }

    /// **The cost of a match never exceeds the bound computed from the two lengths alone**, over
    /// every pattern and name of up to three arbitrary bytes.
    ///
    /// This proves the accounting, not the absence of blowup: `cost_bound(3, 3)` is 285, which no
    /// matcher exceeds on three bytes, but an off-by-one in the charging shows here. Separate from
    /// totality because `cost_bound`'s multiplies on top of the match loop exhausted the solver.
    /// See notes/glob/proofs.md.
    /// Falsification: replayable `crates/glob/falsifications/verification.the_cost_never_exceeds_the_bound.patch`
    #[kani::proof]
    #[kani::unwind(11)]
    fn the_cost_never_exceeds_the_bound() {
        const SHORT: usize = 3;
        let pattern: [u8; SHORT] = kani::any();
        let name: [u8; SHORT] = kani::any();
        let plen: usize = kani::any();
        let nlen: usize = kani::any();
        kani::assume(plen <= SHORT);
        kani::assume(nlen <= SHORT);

        let (_, steps) = match_steps(&pattern[..plen], &name[..nlen], Dot::Ordinary);
        assert!(steps <= cost_bound(plen, nlen));
    }

    /// **A pattern with no magic in it matches exactly one name: itself, unescaped**, over every
    /// pattern of up to three arbitrary bytes, escape corners and unterminated brackets included.
    /// The shell's grant planner rests on it: if [`has_magic`] says no, it asks for one name.
    /// Falsification: replayable `crates/glob/falsifications/verification.no_magic_means_the_pattern_is_its_own_only_match.patch`
    #[kani::proof]
    #[kani::unwind(11)]
    fn no_magic_means_the_pattern_is_its_own_only_match() {
        let pattern: [u8; P] = kani::any();
        let name: [u8; N] = kani::any();
        let (plen, nlen) = any_lengths();
        let p = &pattern[..plen];
        let n = &name[..nlen];

        // An assumption rather than an early `return`, to state the restriction; it is no faster
        // (notes/glob/proofs.md, "The cost").
        kani::assume(!has_magic(p));
        let mut buf = [0u8; P];
        let lit = literal(p, &mut buf).expect("a magic-free pattern always has a literal");
        assert_eq!(matches_with(p, n, Dot::Ordinary), n == lit);
    }

    /// **`*` matches every name, and `?` matches exactly the one-byte names.**
    ///
    /// The two base cases, over every name of up to three arbitrary bytes. Stated with
    /// [`Dot::Ordinary`] so they are claims about the matcher; the leading-dot policy is the next
    /// harness, separately, because it is a separate thing.
    /// Falsification: replayable `crates/glob/falsifications/verification.star_matches_anything_and_question_matches_one_byte.patch`
    #[kani::proof]
    #[kani::unwind(12)]
    fn star_matches_anything_and_question_matches_one_byte() {
        let name: [u8; N] = kani::any();
        let nlen: usize = kani::any();
        kani::assume(nlen <= N);
        let n = &name[..nlen];

        assert!(matches_with(b"*", n, Dot::Ordinary));
        assert!(matches_with(b"**", n, Dot::Ordinary));
        assert_eq!(matches_with(b"?", n, Dot::Ordinary), nlen == 1);
        assert_eq!(matches_with(b"??", n, Dot::Ordinary), nlen == 2);
        // `*` around anything still matches everything that thing matches.
        assert_eq!(matches_with(b"*?*", n, Dot::Ordinary), nlen >= 1);
    }

    /// **The leading-dot rule is exactly "a wildcard cannot reach a dotfile".**
    ///
    /// Under [`Dot::Special`] a name beginning with `.` matches only a pattern beginning with a
    /// literal `.`, and any other name matches identically under both settings. The name is bounded
    /// at two bytes, enough for a predicate on its first byte; the pattern keeps three so `.*`, `\.x`
    /// and `[.]` are reachable. See notes/glob/proofs.md.
    /// Falsification: replayable `crates/glob/falsifications/verification.the_dot_rule_only_touches_names_that_start_with_a_dot.patch`
    #[kani::proof]
    #[kani::unwind(9)]
    fn the_dot_rule_only_touches_names_that_start_with_a_dot() {
        const DOT_N: usize = 2;
        let pattern: [u8; P] = kani::any();
        let name: [u8; DOT_N] = kani::any();
        let plen: usize = kani::any();
        let nlen: usize = kani::any();
        kani::assume(plen <= P);
        kani::assume(nlen <= DOT_N);
        let p = &pattern[..plen];
        let n = &name[..nlen];

        let special = matches_with(p, n, Dot::Special);
        let ordinary = matches_with(p, n, Dot::Ordinary);

        if n.first() == Some(&b'.') {
            // Special is never more permissive, and is permissive only for a pattern that says `.`.
            assert!(!special || ordinary);
            assert!(!special || opens_with_a_literal_dot(p));
        } else {
            assert_eq!(special, ordinary);
        }
    }

    /// **`[xy]` and `[!xy]` partition the byte space**, over every two-byte class body and every
    /// byte, the range spelling `[a-b]` included; this also reaches the four- and five-byte forms.
    ///
    /// The body excludes `!` and `^` at its head (`[!!y]` is the complement of `[!y]`, as Kani's
    /// counterexample showed), and `]` and `\`, which make the two spellings different patterns.
    /// Falsification: replayable `crates/glob/falsifications/verification.a_negated_class_is_the_exact_complement_of_the_class.patch`
    #[kani::proof]
    #[kani::unwind(10)]
    fn a_negated_class_is_the_exact_complement_of_the_class() {
        let body: [u8; 2] = kani::any();
        let c: u8 = kani::any();

        kani::assume(body[0] != b']' && body[1] != b']');
        kani::assume(body[0] != b'\\' && body[1] != b'\\');
        kani::assume(body[0] != b'!' && body[0] != b'^');

        let plain = [b'[', body[0], body[1], b']'];
        let negated = [b'[', b'!', body[0], body[1], b']'];
        let name = [c];

        assert_eq!(
            matches_with(&plain, &name, Dot::Ordinary),
            !matches_with(&negated, &name, Dot::Ordinary)
        );
        // Not vacuous: the range spelling is inside the quantifier, so this is a claim about `[a-b]`
        // as well as about `[ab]`.
        kani::cover!(body[1] == b'-');
        kani::cover!(matches_with(&plain, &name, Dot::Ordinary));
    }
}

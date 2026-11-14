//! Host tests: the matcher against a naive reference, exhaustively on short inputs, and the
//! blowup shapes at length.
//!
//! Name: provisional, minted 2026-11-14 (UTC) by lane/outside-review-1089-1100.

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
fn naive_matches(pattern: &[u8], name: &[u8], j: usize, i: usize) -> bool {
    // Past the end of the name the byte does not matter: every arm that could read it returns
    // false first.
    let c = name.get(i).copied().unwrap_or(0);
    let Some((elem, _)) = decode(pattern, j, c) else {
        return i == name.len();
    };
    match elem {
        Element::Star => {
            let mut absorbed = i;
            loop {
                if naive_matches(pattern, name, j + 1, absorbed) {
                    return true;
                }
                if absorbed == name.len() {
                    return false;
                }
                absorbed += 1;
            }
        }
        _ if i >= name.len() => false,
        Element::Any { next } => naive_matches(pattern, name, next, i + 1),
        Element::Byte { byte, next } => {
            byte == name[i] && naive_matches(pattern, name, next, i + 1)
        }
        Element::Class { hit, next } => hit && naive_matches(pattern, name, next, i + 1),
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

    // Under Miri, every 61st pattern: 61 is prime and not a power of the 9-byte alphabet, so
    // the sample lands on every length and role, ~44,000 comparisons. The complete enumeration
    // is native-only; "Miri-clean" means the sample (notes/undefined-behavior.md).
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
                    let slow = naive_matches(p, n, 0, 0);
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
/// An unwind bound must exceed the most iterations any loop runs, and one set too high grows
/// the formula for nothing. Every outer iteration adds at least one step, so the worst step
/// count is a safe bound on iterations. The nine-byte alphabet is sound because the step count
/// depends on a byte's syntactic role, not its value. Skipped under Miri: a strided sample can
/// miss the pinned argmax (`at_three == 10`) and fail against correct code. See
/// notes/glob/proofs.md.
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
/// `a*a*a*...*b` against a long run of `a` with no `b` makes a recursive matcher try every
/// split of the run between the stars. With ten stars and 100,000 bytes this one walks the name
/// a few times, and its measured step count is under the [`cost_bound`] computed before the
/// match.
#[test]
fn the_pathological_pattern_stays_inside_its_bound() {
    // 100,000 bytes native, where a quadratic matcher would show; 2,000 under Miri, which
    // cannot afford ~4M matcher steps. Both assertions run at both sizes
    // (notes/undefined-behavior.md).
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
/// The range is asserted twice. At the head of a one-member class `scanned` is 2 and the tail
/// is 2, so `+`, `*`, `-` and `/` mutations of `scanned += after_hi - after_lo` agree; one
/// member in front of the range separates them.
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

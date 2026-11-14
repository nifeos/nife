//! The proofs, and the bounds they run under.
//!
//! The harnesses quantify over every byte value of patterns and names up to three bytes (2^48
//! pairs, every arm of the decoder) but not over length. The length-independent claims are the
//! host tests': the exhaustive cross-check and the 100,000-byte blowup tests. See notes/glob.md
//! ("What is proved") and notes/glob/proofs.md.
//!
//! Name: provisional, minted 2026-11-14 (UTC) by lane/outside-review-1089-1100.

use super::*;

/// **Three, measured rather than chosen**: four bytes each passed 3 GB and ten minutes. Three
/// reaches `[a]`, `\*a`, a trailing backslash, an unterminated bracket and two stars; `[!a]`
/// and `[a-b]` are reached by `a_negated_class_is_the_exact_complement_of_the_class`.
const P: usize = 3;
const N: usize = 3;

/// Either leading-dot rule, chosen symbolically.
fn any_dot() -> Dot {
    if kani::any() {
        Dot::Special
    } else {
        Dot::Ordinary
    }
}

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
    let _ = match_steps(&pattern[..plen], &name[..nlen], any_dot());
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
    let pattern: [u8; P] = kani::any();
    let name: [u8; N] = kani::any();
    let (plen, nlen) = any_lengths();

    let (_, steps) = match_steps(&pattern[..plen], &name[..nlen], any_dot());
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
    assert_eq!(matches_with(p, n, any_dot()), n == lit);
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
/// at two bytes, enough for a predicate on its first byte; the pattern keeps three so `.*`,
/// `\.x` and `[.]` are reachable. See notes/glob/proofs.md.
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

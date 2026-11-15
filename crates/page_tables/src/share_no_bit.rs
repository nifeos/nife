//! The compile-time disjointness check the descriptor words' mutation exclusions rest on.
//!
//! Name: provisional, minted 2026-10-31 (UTC) by lane/954-paging-noise.

/// Fail the build unless the words share no bit, pairwise: `assert_share_no_bit!(a, b, c)` asserts
/// `a & b == 0`, `a & c == 0` and `b & c == 0`. Where the words are the operands of a `|`, that
/// `|` is its own `^`, so the mutation `replace | with ^` cannot be killed by any test; the
/// compiler holds the argument instead, and `.cargo/mutants.toml` names the constant that uses
/// this macro in the exclusion's reason (milestone 954 (a survivor accepted once is not accepted
/// forever), part 4). Each pair is its own `assert!`, so the check carries no `|`, no `&&` and no
/// loop, and so no mutant of its own.
///
/// BUGS: a `macro_rules!` body is not mutated, so nothing runs a mutant of this macro; a mistake
/// here would make every assertion vacuous, and the crate's tests cannot see it. The check is to
/// break one word on purpose (give two the same bit) and watch the build fail, which this lane did
/// for each constant that uses it.
macro_rules! assert_share_no_bit {
    ($only:expr $(,)?) => {};
    ($first:expr, $($rest:expr),+ $(,)?) => {
        $( assert!($first & $rest == 0); )+
        assert_share_no_bit!($($rest),+);
    };
}

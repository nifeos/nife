//! **The routing of a typed line**: [`Route`] and [`route`], including the one ordering with a
//! bug behind it, the prefix words answered before the line is split on its operators.
//!
//! Name: provisional (milestone 884, 2026-10-12 UTC), for calef's naming pass.

use super::*;

/// **What kind of thing the user typed**, decided before anything is invoked.
///
/// The order the variants are decided in is load-bearing (see [`route`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Route<'a> {
    /// `caps`, with everything after it. The tail is a **whole command line**, operators included.
    Caps(&'a [u8]),
    /// `time`, with everything after it (milestone 86). The tail is a whole command line for
    /// [`Caps`](Route::Caps)'s reason, and the shell re-dispatches it, so what is timed is the line
    /// taking the path it would have taken untimed.
    Time(&'a [u8]),
    /// `xargs`, with everything after it (milestone 109). The tail is a whole command line for
    /// [`Caps`](Route::Caps)'s reason, and the shell re-dispatches it **once per batch** of what its
    /// pattern matched. The name is provisional.
    Xargs(&'a [u8]),
    /// One command, with no operators on it.
    One(&'a [u8]),
    /// A line with `>`, `<` or `|` on it, already split into stages.
    Pipeline(Line<'a>),
    /// The line cannot be split at all, so there is nothing to run.
    Cannot(Refusal),
}

/// **Read one line and decide what it is.**
///
/// The prefix words (`caps`, `time`, `xargs`) are asked about before the line is split, because
/// their operand is a whole command line: splitting first would hand `caps date | wc` the word
/// `date` and lose the rest. A line with no operators pays nothing for them.
///
/// ```
/// use swish::{Route, route};
///
/// // The tail keeps the pipe. `caps` will preview two stages, not one.
/// assert!(matches!(route(b"caps date | wc"), Route::Caps(b"date | wc")));
/// // And `time` keeps it for the same reason: it times the pipeline, not its first word.
/// assert!(matches!(route(b"time date | wc"), Route::Time(b"date | wc")));
/// // The third prefix word (milestone 109), which batches a whole line for the same reason.
/// assert!(matches!(route(b"xargs rm *.txt"), Route::Xargs(b"rm *.txt")));
/// assert!(matches!(route(b"least_authority_demo 7"), Route::One(b"least_authority_demo 7")));
/// match route(b"date | wc") {
///     Route::Pipeline(l) => assert_eq!(l.stages().len(), 2),
///     other => panic!("expected a pipeline, got {other:?}"),
/// }
/// ```
pub fn route(cmd: &[u8]) -> Route<'_> {
    match grant_plan::parse(cmd) {
        Command::Caps(tail) => return Route::Caps(tail),
        Command::Time(tail) => return Route::Time(tail),
        Command::Xargs(tail) => return Route::Xargs(tail),
        _ => {}
    }
    match line::split(cmd) {
        Ok(l) if l.is_plain() => Route::One(l.stages()[0]),
        Ok(l) => Route::Pipeline(l),
        Err(r) => Route::Cannot(r),
    }
}

#[cfg(test)]
mod tests;

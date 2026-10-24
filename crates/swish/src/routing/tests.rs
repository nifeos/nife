//! Tests of routing: the ordering with a bug behind it.

use super::*;

#[test]
fn caps_is_answered_before_the_line_is_split() {
    // The whole reason `route` asks about `caps` first. Splitting first would hand the preview
    // the word `date` and lose the pipe, and the preview would then be a true statement about a
    // line the user did not type.
    match route(b"caps date | wc") {
        Route::Caps(tail) => assert_eq!(tail, b"date | wc"),
        other => panic!("expected caps, got {other:?}"),
    }
}

#[test]
fn a_line_with_no_operators_is_one_command() {
    match route(b"  least_authority_demo 7  ") {
        Route::One(stage) => assert_eq!(grant_plan::trim(stage), b"least_authority_demo 7"),
        other => panic!("expected one command, got {other:?}"),
    }
}

#[test]
fn operators_make_a_pipeline_of_the_stages_typed() {
    match route(b"date | wc") {
        Route::Pipeline(l) => {
            assert_eq!(l.stages().len(), 2);
            assert_eq!(grant_plan::trim(l.stages()[0]), b"date");
            assert_eq!(grant_plan::trim(l.stages()[1]), b"wc");
        }
        other => panic!("expected a pipeline, got {other:?}"),
    }
}

#[test]
fn a_redirection_with_nothing_after_it_runs_nothing() {
    // A refusal at routing time is a line that never reaches a planner, so nothing is spawned
    // and nothing is opened.
    match route(b"date >") {
        Route::Cannot(r) => assert_eq!(r, Refusal::NoRedirectTarget),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// `time` and `xargs` take a whole line as their operand for `caps`'s reason, so they are
/// answered before the split too. The doctest on `route` says the same, but `script/coverage`
/// counts no doctest.
#[test]
fn time_and_xargs_keep_the_operators_of_the_line_they_take() {
    match route(b"time date | wc") {
        Route::Time(tail) => assert_eq!(tail, b"date | wc"),
        other => panic!("expected time, got {other:?}"),
    }
    match route(b"xargs rm *.txt") {
        Route::Xargs(tail) => assert_eq!(tail, b"rm *.txt"),
        other => panic!("expected xargs, got {other:?}"),
    }
}

use super::record;

/// **The default record is 0, which is the whole backward-compatibility claim.**
///
/// Every caller written before the selector existed passed 0 into the then-unused x1. If this
/// constant ever moves, those callers silently start asking for a different record and read a
/// cpu id as a run state. A wire's compatibility is a claim about a number, so it is asserted
/// against the number rather than described in prose (milestone 85 (mutation testing over the host crates): a mutant that rewrites the
/// constant is what this notices).
#[test]
#[allow(clippy::assertions_on_constants)]
fn the_state_record_is_zero_so_pre_selector_callers_select_it() {
    assert_eq!(record::STATE, 0);
}

/// Known records are known and nothing else is, including the two an unknown selector most
/// plausibly arrives as: one past the end (a reader built against a later kernel) and a wild
/// value (a register that held something else).
///
/// **"One past the end" is written against the last constant rather than as a literal**, so
/// adding a record moves it instead of quietly turning this line into an assertion that a
/// *known* record is unknown.
#[test]
fn only_the_records_this_kernel_answers_are_known() {
    assert!(record::is_known(record::STATE));
    assert!(record::is_known(record::PLACEMENT));
    assert!(record::is_known(record::CPU_TIME));
    assert!(!record::is_known(record::CPU_TIME + 1));
    assert!(!record::is_known(u64::MAX));
}

/// **No record value may collide with [`record::NO_CPU`]**, because a reader that walked a
/// domain with a selector equal to the "no answer" sentinel would have no way to tell a record
/// from its own absence. Cheap to assert, impossible to notice later.
#[test]
fn the_no_answer_sentinel_is_not_also_a_record() {
    assert!(!record::is_known(record::NO_CPU));
}

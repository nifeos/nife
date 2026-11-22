//! Tests of the byte helpers the crate root keeps.

use crate::*;

#[test]
fn a_file_name_is_valid_up_to_the_grant_name_limit() {
    assert!(is_valid_file_name(b"count"));
    assert!(is_valid_file_name(&[b'n'; MAXIMUM_FILE_NAME_BYTES]));
    assert!(!is_valid_file_name(&[b'n'; MAXIMUM_FILE_NAME_BYTES + 1]));
    assert!(!is_valid_file_name(b""));
}

#[test]
fn parse_u64_rejects_junk() {
    assert_eq!(parse_u64(b"123"), Some(123));
    assert_eq!(parse_u64(b""), None);
    assert_eq!(parse_u64(b"1a"), None);
    assert_eq!(parse_u64(b"-1"), None);
}

/// **The by-value sizes of the planning types are pinned**, so a field added to one shows its cost
/// in a failing test and not in a shell that overflowed its stack. The planning functions take and
/// return these by value, and the crate has already paid for two stack overflows that way. Raising
/// a number here is a decision to make on purpose. The numbers are for a 64-bit target.
#[cfg(target_pointer_width = "64")]
#[test]
fn the_planning_types_have_the_sizes_the_stack_budget_assumes() {
    use core::mem::size_of;
    assert_eq!(size_of::<Endowment>(), 4344);
    assert_eq!(size_of::<ShellHoldings>(), 3312);
    assert_eq!(size_of::<expansion::NameSet>(), 544);
    assert_eq!(size_of::<navigation::DirectoryPath>(), 528);
}

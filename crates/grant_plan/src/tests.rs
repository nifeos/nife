//! Tests of the byte helpers the crate root keeps.

use crate::*;

#[test]
fn a_grant_name_limit_matches_the_filesystem_contract() {
    // `grant_plan` deliberately does not depend on `filesystem_contract` (the shell's parser must not need the
    // filesystem contract to check a command line), so the constant is duplicated. That is only
    // safe if a change to one without the other fails here, on the host, in milliseconds.
    assert_eq!(MAXIMUM_FILE_NAME, filesystem_contract::grant::MAXIMUM_NAME);
    assert_eq!(WORDS_NAME, filesystem_contract::grant::WORDS_NAME);
    assert!(file_name_fits(b"least_authority_demo"));
    assert!(file_name_fits(&[b'n'; MAXIMUM_FILE_NAME]));
    assert!(!file_name_fits(&[b'n'; MAXIMUM_FILE_NAME + 1]));
    assert!(!file_name_fits(b""));
}

#[test]
fn parse_u64_rejects_junk() {
    assert_eq!(parse_u64(b"123"), Some(123));
    assert_eq!(parse_u64(b""), None);
    assert_eq!(parse_u64(b"1a"), None);
    assert_eq!(parse_u64(b"-1"), None);
}

/// **The region sizes the shell splits and the progenitor counts are exactly these pages**
/// (milestone 933 (every job is built the way a supervised job is)): a native job, a
/// directory-granted one, a `std` one, and the pool of six native regions and one `std` region. A
/// size that drifted on one side would make the progenitor refuse a region the shell split, or the
/// shell split one the progenitor wasted.
#[test]
fn the_region_a_job_is_held_in_has_exactly_the_pages_both_sides_count() {
    assert_eq!(JOB_REGION_PAGES, 60);
    assert_eq!(DIRECTORY_JOB_REGION_PAGES, 98);
    assert_eq!(STD_REGION_PAGES, 384);
    assert_eq!(JOB_POOL_PAGES, 6 * 60 + 384);
    let native = Program::LeastAuthorityDemo.manifest();
    assert_eq!(job_region_pages(&native, false), 60);
    assert_eq!(job_region_pages(&native, true), 98);
    let std = Program::StdExerciser.manifest();
    assert_eq!(job_region_pages(&std, false), 384);
    assert_eq!(job_region_pages(&std, true), 384);
}

/// **A held job is sent the shell's budget as its share view only when its program declares one**:
/// `free` and `slabtop` do, `date` and `ps` do not. Both sides ask this one function, so a wrong
/// answer in either direction sends a capability nobody reads or reads one nobody sent.
#[test]
fn only_a_program_that_declares_a_share_view_is_sent_one() {
    assert!(spawnproto::held_shares(Program::Free));
    assert!(spawnproto::held_shares(Program::Slabtop));
    assert!(!spawnproto::held_shares(Program::Date));
    assert!(!spawnproto::held_shares(Program::Ps));
    for p in Program::ALL {
        assert_eq!(spawnproto::held_shares(*p), p.manifest().share, "{p:?}");
    }
}

/// **The by-value sizes of the planning types are pinned**, so a field added to one shows its cost
/// in a failing test and not in a shell that overflowed its stack. The planning functions take and
/// return these by value, and the crate has already paid for two stack overflows that way. Raising
/// a number here is a decision to make on purpose. The numbers are for a 64-bit target.
#[cfg(target_pointer_width = "64")]
#[test]
fn the_planning_types_have_the_sizes_the_stack_budget_assumes() {
    use core::mem::size_of;
    assert_eq!(size_of::<Endowment>(), 2344);
    assert_eq!(size_of::<Holdings>(), 1008);
    assert_eq!(size_of::<expand::NameSet>(), 544);
    assert_eq!(size_of::<nav::Cwd>(), 144);
}

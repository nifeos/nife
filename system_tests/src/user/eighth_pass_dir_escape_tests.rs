//! **The attenuation lattice, one right at a time, and each shell running the other's script**
//! (an eighth outsider pass at the confinement claim, claims 19 and 24).
//!
//! Claim 19, per right: two grants each drop one of the read-only grant's `DESCEND | READ |
//! ENUMERATE`, and the verdict must lose exactly that right's bit. Claim 24, mirrored: each shell
//! runs the other root's script, completing its skeleton (`ALWAYS`) while reaching nothing of the
//! tree it probes. Cross-ISA: the caretaker, the protocol and the shell are portable programs
//! (DECISIONS §19 (architectural parity is a tenet)). See
//! notes/confinement-claims/the-eighth-pass-tests.md.

use filesystem_contract::dir;
use filesystem_fixture::{dirscape as esc, navscape as nb, tree};

use super::dir_capability_tests::{assert_verdict, attack_a_subtree};
use super::shell_navigation_tests::{ALWAYS, navigate};
use super::*;

/// **A grant that drops one right must lose exactly that right's capability and no other.**
///
/// The read-only verdict is `OPENED_ITS_OWN | ENUMERATED | DESCENDED | READ_ATTRS`. Without
/// `ENUMERATE` the listing goes; without `READ` both opens go; the descent and everything above
/// the grant stay identical. Missing bits are asserted as strongly as present ones: more reach is
/// the escape, and less makes the refusals prove nothing.
///
/// Falsification: unfalsified. Written by the eighth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn a_grant_that_drops_one_right_loses_exactly_that_right() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(read_only) = attack_a_subtree(dir::DESCEND | dir::READ | dir::ENUMERATE, 1) else {
        crate::testing::skip!("no RedoxFS disk attached");
    };
    assert_verdict(
        read_only,
        esc::OPENED_ITS_OWN | esc::ENUMERATED | esc::DESCENDED | esc::READ_ATTRS,
        "read-only",
    );

    let no_enumerate = attack_a_subtree(dir::DESCEND | dir::READ, 1)
        .expect("the service was wired for the read-only run");
    assert_verdict(
        no_enumerate,
        esc::OPENED_ITS_OWN | esc::DESCENDED | esc::READ_ATTRS,
        "enumerate-less",
    );
    assert_eq!(
        no_enumerate & esc::ENUMERATED,
        0,
        "a grant with no ENUMERATE right still listed a directory",
    );

    let no_read = attack_a_subtree(dir::DESCEND | dir::ENUMERATE, 1)
        .expect("the service was wired for the read-only run");
    assert_verdict(no_read, esc::ENUMERATED | esc::DESCENDED, "read-less");
    assert_eq!(
        no_read & (esc::OPENED_ITS_OWN | esc::READ_ATTRS),
        0,
        "a grant with no READ right still opened a file or read its attributes",
    );
}

/// **Each shell, running the other root's script, reaches nothing of it.**
///
/// The shell rooted at `other` runs the script written for `sub` (every probe aims at `inner`,
/// directly and by absolute path, plus the listing check), and the shell rooted at `sub` runs the
/// `other` script (every probe aims at `secret`). Both must complete the script's root-relative
/// skeleton (`ALWAYS`), which is what makes their refusals mean anything, and both must come back
/// with none of the probed tree's bits: no reach, no absolute reach, no sighting in a listing.
///
/// Falsification: unfalsified. Written by the eighth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn each_shell_running_the_others_script_names_nothing_of_it() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(other_running_the_sub_script) = navigate(tree::OTHER, 5) else {
        crate::testing::skip!("no RedoxFS disk attached");
    };
    let sub_running_the_other_script =
        navigate(tree::SUB, 6).expect("the service was wired for the first shell");

    assert_eq!(
        other_running_the_sub_script & ALWAYS,
        ALWAYS,
        "the shell at `other` could not complete the script's root-relative skeleton, so its \
         refusals prove nothing",
    );
    assert_eq!(
        other_running_the_sub_script
            & (nb::REACHED_INNER | nb::ABSOLUTE_REACHED_INNER | nb::SAW_INNER),
        0,
        "the shell at `other`, running the `sub` script, reached or saw a name that exists only \
         in the other root",
    );

    assert_eq!(
        sub_running_the_other_script & ALWAYS,
        ALWAYS,
        "the shell at `sub` could not complete the script's root-relative skeleton, so its \
         refusals prove nothing",
    );
    assert_eq!(
        sub_running_the_other_script
            & (nb::REACHED_SECRET | nb::ABSOLUTE_REACHED_SECRET | nb::SAW_SECRET),
        0,
        "the shell at `sub`, running the `other` script, reached or saw a name that exists only \
         in the other root",
    );
}

//! Tests of [`Say`]'s sentences, `bind`'s refusal among them, and the heap cap's.

use super::*;
use crate::test_support::shown;

#[test]
fn every_say_but_nothing_is_one_line() {
    assert_eq!(shown(|o| write_say(Say::Nothing, o)), "");
    for s in [
        Say::Refused(Refused::AtYourRoot),
        Say::Refused(Refused::NotAName),
        Say::Failed(-2),
        Say::NoDirectory,
        Say::NeedsAName,
        Say::Cannot(Refusal::NoMatch),
        Say::CannotBind(nav::BindRefused::TooMany),
    ] {
        let line = shown(|o| write_say(s, o));
        assert!(line.ends_with('\n'), "{s:?} does not end its line");
        assert_eq!(line.matches('\n').count(), 1, "{s:?} printed two lines");
        assert!(line.len() > 4, "{s:?} printed nothing worth reading");
    }
}

#[test]
fn a_filesystem_refusal_keeps_the_filesystems_own_words() {
    // The shell does not get to invent a friendlier sentence for an errno. This pins that the
    // rendering goes through `filesystem_contract`, which is where the number was chosen.
    assert_eq!(
        shown(|o| write_say(Say::Failed(-2), o)).trim(),
        filesystem_contract::dir::explain(-2)
    );
}

/// [`Say::CannotBind`] renders [`nav::BindRefused`]'s own message, the same shape
/// [`Say::Cannot`] and [`Say::Refused`] already use for their own refusal types.
#[test]
fn cannot_bind_renders_the_bind_refusals_own_message() {
    let s = shown(|o| write_say(Say::CannotBind(nav::BindRefused::AlreadyBound), o));
    assert_eq!(s, "  that name is already bound; unbind it first\n");
}

/// The heap cap is printed in KiB, by division, in the two places that name it.
#[test]
fn the_heap_cap_is_named_in_kibibytes() {
    let say = shown(|o| write_say(Say::HeapFull, o));
    assert!(say.contains("its cap is 32 KiB"), "{say}");
    let held = shown(|o| write_holdings(128, Holdings::default(), None, None, o));
    assert!(held.contains("the first 32 KiB of them"), "{held}");
}

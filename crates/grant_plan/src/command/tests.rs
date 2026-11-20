//! Tests of the shell's `Holdings`: one tree, a mount, and `bind`.

use crate::test_support::{WITH_DIRECTORY, at, mounted};
use crate::*;

// ---- milestone 154 (a process that holds two directory capabilities): one tree, a mount ----

/// A one-grant [`Holdings`] resolves exactly as it always has.
#[test]
fn a_one_grant_holdings_resolves_exactly_as_before() {
    assert_eq!(WITH_DIRECTORY.resolve(b"logs"), Ok(at(&[b"logs"])));
    assert_eq!(
        WITH_DIRECTORY.resolve(b".."),
        Err(navigation::Refused::AtYourRoot)
    );
    assert_eq!(
        WITH_DIRECTORY.locate(&at(&[b"logs"])),
        (navigation::Which::A, at(&[b"logs"]))
    );
}

/// **One tree, with the second mounted in it** (calef, 2026-09-26): a path through the mount
/// point is the second tree from its own root, every other path is the first, and `..` from
/// the mount point is its parent in the first tree, as a Unix mount is.
#[test]
fn a_mounted_tree_is_a_path_in_the_one_tree() {
    let holds = mounted();
    let usb = holds.resolve(b"/media/usb/photos").unwrap();
    assert_eq!(holds.locate(&usb), (navigation::Which::B, at(&[b"photos"])));
    assert_eq!(
        holds.locate(&at(&[b"media", b"usb"])),
        (navigation::Which::B, navigation::Cwd::root())
    );
    // Beside the mount point and above it: the first tree.
    assert_eq!(
        holds.locate(&at(&[b"media"])),
        (navigation::Which::A, at(&[b"media"]))
    );
    assert_eq!(
        holds.locate(&at(&[b"media", b"usbx"])),
        (navigation::Which::A, at(&[b"media", b"usbx"])),
    );
    // `..` from the mount point, and from inside it, climbs the one tree.
    let mut standing = mounted();
    standing.cwd = at(&[b"media", b"usb"]);
    let up = standing.resolve(b"..").unwrap();
    assert_eq!(
        standing.locate(&up),
        (navigation::Which::A, at(&[b"media"]))
    );
    assert_eq!(
        standing.resolve(b"../../..").map(|_| ()),
        Err(navigation::Refused::AtYourRoot)
    );
    // `place` is `locate` reversed.
    assert_eq!(holds.place(navigation::Which::B, &at(&[b"photos"])), usb);
    assert!(MountedTree::mounted_at(navigation::Cwd::root()).is_none());
}

// ---- `bind` (milestone 47 (navigation and naming): "blocked on a second grant") ----

/// **A one-grant `Holdings` resolves absolute paths exactly as before when nothing is
/// bound**, and a bound name is reachable once one is: `bind` is additive, not a new mode.
#[test]
fn a_one_grant_holdings_binds_a_name_and_resolves_through_it() {
    let mut holds = Holdings {
        directory: true,
        ..Holdings::default()
    };
    assert_eq!(holds.resolve(b"/etc/passwd"), Ok(at(&[b"etc", b"passwd"])));
    let target = at(&[b"logs", b"2026"]);
    holds.bind(b"recent", target).unwrap();
    assert_eq!(holds.resolve(b"/recent"), Ok(target));
    assert_eq!(
        holds.resolve(b"/recent/report.txt"),
        Ok(at(&[b"logs", b"2026", b"report.txt"])),
    );
    assert_eq!(holds.resolve(b"/etc/passwd"), Ok(at(&[b"etc", b"passwd"])));
}

/// **A bind can point through a mount point, and cannot hide the path to one**: a bound name
/// wins over the literal first component of an absolute path, so binding `media` would make
/// `/media/usb` unreachable.
#[test]
fn a_bind_reaches_into_a_mount_and_cannot_hide_one() {
    let mut holds = mounted();
    assert_eq!(
        holds.bind(b"media", navigation::Cwd::root()),
        Err(navigation::BindRefused::ReservedLabel),
    );
    holds
        .bind(b"photos", at(&[b"media", b"usb", b"photos"]))
        .unwrap();
    let p = holds.resolve(b"/photos/2026").unwrap();
    assert_eq!(
        holds.locate(&p),
        (navigation::Which::B, at(&[b"photos", b"2026"]))
    );
}

/// **`bind`'s own refusals surface through `Holdings`**.
#[test]
fn holdings_bind_surfaces_the_bind_tables_own_refusals() {
    let mut holds = Holdings {
        directory: true,
        ..Holdings::default()
    };
    holds.bind(b"x", navigation::Cwd::root()).unwrap();
    assert_eq!(
        holds.bind(b"x", navigation::Cwd::root()),
        Err(navigation::BindRefused::AlreadyBound),
    );
}

// ---- survivor triage of the 2026-10-03 census ---------------------------------------------

#[test]
fn a_second_tree_remembers_where_it_is_mounted() {
    let mut at = navigation::Cwd::root();
    assert!(at.descend(b"second"));
    let second = MountedTree::mounted_at(at).expect("not the root");
    assert_eq!(second.mount(), at);
    assert!(MountedTree::mounted_at(navigation::Cwd::root()).is_none());
}

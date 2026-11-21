//! Tests of the shell's `ShellHoldings`: one tree, a mount, and `bind`.

use crate::test_support::{WITH_DIRECTORY, at, mounted};
use crate::*;

// ---- milestone 154 (a process that holds two directory capabilities): one tree, a mount ----

/// A one-grant [`ShellHoldings`] resolves exactly as it always has.
#[test]
fn a_one_grant_holdings_resolves_exactly_as_before() {
    assert_eq!(WITH_DIRECTORY.resolve(b"logs"), Ok(at(&[b"logs"])));
    assert_eq!(
        WITH_DIRECTORY.resolve(b".."),
        Err(navigation::PathRefusal::AtYourRoot)
    );
    assert_eq!(
        WITH_DIRECTORY.serving_tree(&at(&[b"logs"])),
        (navigation::GrantSide::First, at(&[b"logs"]))
    );
}

/// **One tree, with the second mounted in it** (calef, 2026-09-26): a path through the mount
/// point is the second tree from its own root, every other path is the first, and `..` from
/// the mount point is its parent in the first tree, as a Unix mount is.
#[test]
fn a_mounted_tree_is_a_path_in_the_one_tree() {
    let shell_holdings = mounted();
    let usb = shell_holdings.resolve(b"/media/usb/photos").unwrap();
    assert_eq!(
        shell_holdings.serving_tree(&usb),
        (navigation::GrantSide::Second, at(&[b"photos"]))
    );
    assert_eq!(
        shell_holdings.serving_tree(&at(&[b"media", b"usb"])),
        (
            navigation::GrantSide::Second,
            navigation::DirectoryPath::root()
        )
    );
    // Beside the mount point and above it: the first tree.
    assert_eq!(
        shell_holdings.serving_tree(&at(&[b"media"])),
        (navigation::GrantSide::First, at(&[b"media"]))
    );
    assert_eq!(
        shell_holdings.serving_tree(&at(&[b"media", b"usbx"])),
        (navigation::GrantSide::First, at(&[b"media", b"usbx"])),
    );
    // `..` from the mount point, and from inside it, climbs the one tree.
    let mut standing = mounted();
    standing.working_directory = at(&[b"media", b"usb"]);
    let up = standing.resolve(b"..").unwrap();
    assert_eq!(
        standing.serving_tree(&up),
        (navigation::GrantSide::First, at(&[b"media"]))
    );
    assert_eq!(
        standing.resolve(b"../../..").map(|_| ()),
        Err(navigation::PathRefusal::AtYourRoot)
    );
    // `path_in_one_tree` is `serving_tree` reversed.
    assert_eq!(
        shell_holdings.path_in_one_tree(navigation::GrantSide::Second, &at(&[b"photos"])),
        usb
    );
    assert!(MountedTree::mounted_at(navigation::DirectoryPath::root()).is_none());
}

// ---- `bind` (milestone 47 (navigation and naming): "blocked on a second grant") ----

/// **A one-grant `ShellHoldings` resolves absolute paths exactly as before when nothing is
/// bound**, and a bound name is reachable once one is: `bind` is additive, not a new mode.
#[test]
fn a_one_grant_holdings_binds_a_name_and_resolves_through_it() {
    let mut shell_holdings = ShellHoldings {
        directory: true,
        ..ShellHoldings::default()
    };
    assert_eq!(
        shell_holdings.resolve(b"/etc/passwd"),
        Ok(at(&[b"etc", b"passwd"]))
    );
    let target = at(&[b"logs", b"2026"]);
    shell_holdings.bind(b"recent", target).unwrap();
    assert_eq!(shell_holdings.resolve(b"/recent"), Ok(target));
    assert_eq!(
        shell_holdings.resolve(b"/recent/report.txt"),
        Ok(at(&[b"logs", b"2026", b"report.txt"])),
    );
    assert_eq!(
        shell_holdings.resolve(b"/etc/passwd"),
        Ok(at(&[b"etc", b"passwd"]))
    );
}

/// **A bind can point through a mount point, and cannot hide the path to one**: a bound name
/// wins over the literal first component of an absolute path, so binding `media` would make
/// `/media/usb` unreachable.
#[test]
fn a_bind_reaches_into_a_mount_and_cannot_hide_one() {
    let mut shell_holdings = mounted();
    assert_eq!(
        shell_holdings.bind(b"media", navigation::DirectoryPath::root()),
        Err(navigation::BindRefusal::ReservedLabel),
    );
    shell_holdings
        .bind(b"photos", at(&[b"media", b"usb", b"photos"]))
        .unwrap();
    let p = shell_holdings.resolve(b"/photos/2026").unwrap();
    assert_eq!(
        shell_holdings.serving_tree(&p),
        (navigation::GrantSide::Second, at(&[b"photos", b"2026"]))
    );
}

/// **`bind`'s own refusals surface through `ShellHoldings`**.
#[test]
fn holdings_bind_surfaces_the_bind_tables_own_refusals() {
    let mut shell_holdings = ShellHoldings {
        directory: true,
        ..ShellHoldings::default()
    };
    shell_holdings
        .bind(b"x", navigation::DirectoryPath::root())
        .unwrap();
    assert_eq!(
        shell_holdings.bind(b"x", navigation::DirectoryPath::root()),
        Err(navigation::BindRefusal::AlreadyBound),
    );
}

// ---- survivor triage of the 2026-10-03 census ---------------------------------------------

#[test]
fn a_second_tree_remembers_where_it_is_mounted() {
    let mut at = navigation::DirectoryPath::root();
    assert!(at.descend(b"second"));
    let second = MountedTree::mounted_at(at).expect("not the root");
    assert_eq!(second.mount_point(), at);
    assert!(MountedTree::mounted_at(navigation::DirectoryPath::root()).is_none());
}

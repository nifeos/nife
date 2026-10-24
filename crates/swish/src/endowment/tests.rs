//! Tests of the shell's own endowment, its `bind` rows and the census of held slots.

use grant_plan::MountedTree;
use grant_plan::nav::Cwd;

use super::*;
use crate::test_support::shown;

/// **A one-grant shell with something bound gets a namespace section too**, not only a
/// two-grant one: `bind` is additive to whatever the shell already prints, and a shell with
/// nothing bound prints exactly as before (no `namespace:` line at all), which the second half
/// of this test pins.
#[test]
fn a_bound_name_prints_its_own_namespace_row() {
    let mut target = Cwd::root();
    target.descend(b"logs");
    target.descend(b"2026");
    let mut binds = nav::Bindings::none();
    binds.add(b"recent", target).unwrap();
    let holdings = Holdings {
        directory: true,
        mounted: None,
        cwd: Cwd::root(),
        binds,
    };
    let s = shown(|o| write_holdings(128, holdings, None, None, o));
    assert!(s.contains("namespace: names bound"), "{s}");
    assert!(s.contains("bind recent -> /logs/2026"), "{s}");

    // Nothing bound: no namespace section at all, the same claim
    // `holding_a_directory_changes_exactly_one_line_of_the_endowment` already pins for the
    // directory row itself.
    let empty = shown(|o| {
        write_holdings(
            128,
            Holdings {
                directory: true,
                mounted: None,
                cwd: Cwd::root(),
                binds: nav::Bindings::none(),
            },
            None,
            None,
            o,
        );
    });
    assert!(!empty.contains("namespace:"), "{empty}");
    assert!(!empty.contains("bind "), "{empty}");
}

#[test]
fn holding_a_directory_changes_exactly_one_line_of_the_endowment() {
    let with = shown(|o| {
        write_holdings(
            128,
            Holdings {
                directory: true,
                mounted: None,
                cwd: Cwd::root(),
                binds: nav::Bindings::none(),
            },
            None,
            None,
            o,
        );
    });
    let without = shown(|o| write_holdings(128, Holdings::default(), None, None, o));
    let differing = with
        .lines()
        .zip(without.lines())
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(differing, 1);
    assert!(with.contains("cap 4  endpoint  directory"));
    assert!(without.contains("no directory capability"));
    // The budget row is the one number the shell cannot query, so it must be the caller's.
    assert!(with.contains("128 pages"));
}

fn at(components: &[&[u8]]) -> Cwd {
    let mut c = Cwd::root();
    for name in components {
        assert!(c.descend(name));
    }
    c
}

/// **Milestone 154's own words, made real**: "`caps` gains a namespace section with more than
/// one row". The second capability is a row of its own and says where it is mounted; a
/// one-grant shell prints exactly the old single row.
#[test]
fn a_second_grant_prints_its_row_and_where_it_is_mounted() {
    let holdings = Holdings {
        directory: true,
        mounted: MountedTree::mounted_at(at(&[b"media", b"usb"])),
        cwd: Cwd::root(),
        binds: nav::Bindings::none(),
    };
    let s = shown(|o| write_holdings(128, holdings, None, None, o));
    assert!(s.contains("cap 4  endpoint  directory"), "{s}");
    assert!(
        s.contains("cap 19 endpoint  directory  a second tree, mounted at /media/usb"),
        "{s}"
    );
    assert!(s.contains("namespace: one tree"), "{s}");
    let one_grant = shown(|o| {
        write_holdings(
            128,
            Holdings {
                directory: true,
                ..Holdings::default()
            },
            None,
            None,
            o,
        );
    });
    assert!(!one_grant.contains("namespace:"), "{one_grant}");
}

/// **A mounted tree prints as paths in the one tree** (calef, 2026-09-26): `pwd` inside it is
/// the mount path, a directory grant's preview under it is the mount path, and a bound name
/// pointing into it is the mount path. No label reaches the screen.
#[test]
fn a_mounted_tree_prints_as_paths_in_the_one_tree() {
    let mut holdings = Holdings {
        directory: true,
        mounted: MountedTree::mounted_at(at(&[b"media", b"usb"])),
        cwd: at(&[b"media", b"usb", b"logs"]),
        binds: nav::Bindings::none(),
    };
    assert_eq!(shown(|o| write_pwd(&holdings, o)), "  /media/usb/logs\n");

    let s = shown(|o| {
        write_capabilities(
            b"rm /media/usb/old/x",
            128,
            holdings,
            None,
            None,
            &mut |_| Ok(NameSet::empty()),
            o,
        );
    });
    assert!(
        s.contains("dir      /media/usb/old  (the directory holding x)"),
        "{s}"
    );

    holdings.bind(b"here", holdings.cwd).expect("a free name");
    let s = shown(|o| write_holdings(128, holdings, None, None, o));
    assert!(s.contains("bind here -> /media/usb/logs"), "{s}");
}

/// **A clock in the endowment is one row, and the row says it cannot be handed on** (milestone
/// 86).
///
/// The rights half is the whole point. Before this milestone `caps` said the shell held no clock
/// and could not endow one; now it holds one and *still* cannot endow one, because what it was
/// granted is `READ` without `GRANT`. Those are different sentences about the same guarantee, and
/// a table that printed only the object would have lost it.
#[test]
fn a_clock_is_one_row_and_it_says_the_shell_cannot_pass_it_on() {
    let with = shown(|o| write_holdings(128, Holdings::default(), Some(5), None, o));
    assert!(with.contains("cap 5  frame     clock"), "{with}");
    assert!(with.contains("NOT delegable"), "{with}");

    // The slot is the caller's to state rather than a constant here, because it moves with the
    // wiring: a shell granted no directory has one fewer capability under it.
    let lower = shown(|o| write_holdings(128, Holdings::default(), Some(4), None, o));
    assert!(lower.contains("cap 4  frame     clock"), "{lower}");

    // And a shell granted none says what is missing.
    let without = shown(|o| write_holdings(128, Holdings::default(), None, None, o));
    assert!(without.contains("was\n     granted none"), "{without}");
    assert!(without.contains("nothing to measure with"), "{without}");
}

#[test]
fn the_census_lists_exactly_the_slots_the_kernel_says_are_held() {
    let s = shown(|o| write_census(31, &|n| matches!(n, 0 | 1 | 2 | 22 | 30), o));
    assert_eq!(s, "  slots held: 0 1 2 22 30\n");
    let none = shown(|o| write_census(31, &|_| false, o));
    assert_eq!(none, "  slots held:\n");
}

//! **The server acts on no name its own name check refuses** (milestone 825 (a hostile client races the file server's name window), calef's "Refused
//! names" ruling, 2026-10-15 UTC): the refused-name claim, over [`Server::handle`], one arm at a
//! time.
//!
//! The compiler holds the claim first: every verb that resolves a name takes a [`crate::NameCopy`],
//! and `NameCopy`'s `compile_fail` doctest is the record. These tests are the second mechanism: they
//! drive `handle` with every class of refused name (`..`, `.`, empty, a name with `/`, `\` or `:`,
//! the attribute store's directory, and invalid UTF-8) and require `EINVAL` and, for the mutating
//! verbs, an untouched directory, catching an arm whose check is wrong. A legal name through the
//! same arm is the control, so a refusal that reaches nothing proves nothing.

//! # What neither reaches
//!
//! A client whose second thread rewrites the window between the check and the use (milestone 812
//! (threads in one address space)) is a data race safe single-threaded host code cannot stage, so
//! these tests do not fire it. The type closes it: the resolve reads the server's copy.

use filesystem_contract::fs;

use super::*;

/// [`crate::ServeEdges`] for a dispatch that never reaches the device: the name arms refuse before
/// any sync or write. A `SYNC` reaching here would be a test bug, so it panics.
struct NoEdges;
impl crate::ServeEdges for NoEdges {
    fn sync(&mut self) -> i64 {
        panic!("a name-arm test reached the device");
    }
}

/// Every class of name `check_component` refuses, as window bytes. `/` and the empty name become
/// empty path components, `..` and `.` are the walk escapes, `\` and `:` and the attribute store's
/// directory are this server's own additions, and the last is not UTF-8 at all.
const REFUSED: &[&[u8]] = &[
    b"..",
    b".",
    b"",
    b"/",
    b"a/../b",
    b"a\\b",
    b"a:b",
    filesystem_contract::xattr::STORE_DIR.as_bytes(),
    &[0xff, 0xfe],
];

/// Stage `name` at the start of the window and return the request's length word.
fn stage(window: &mut [u8], name: &[u8]) -> u64 {
    window[..name.len()].copy_from_slice(name);
    name.len() as u64
}

/// The root directory's listing, so a mutating verb can be shown to have changed nothing.
fn root_listing(srv: &mut Server<DiskMemory>) -> Vec<(String, bool)> {
    // Handle 0 is the bound root, a directory carrying every right, so it enumerates.
    let mut names = list(srv, fs::ROOT as u32, BLOCK).expect("list the root");
    names.sort();
    names
}

/// **`OPEN` acts on no refused name, and opens a legal one.** The control proves the arm is live:
/// `motd` is in the tree, so a refusal of `..` is about the name and not about an empty directory.
#[test]
fn open_refuses_every_refused_name_and_opens_a_legal_one() {
    let mut srv = server_with_tree();
    let mut window = vec![0u8; fs::TRANSFER_MAX];
    for &name in REFUSED {
        let len = stage(&mut window, name);
        let (r0, _) = srv.handle(
            0,
            fs::req(fs::OPEN, fs::ROOT, len),
            0,
            &mut window,
            &mut NoEdges,
        );
        assert_eq!(
            r0,
            -(EINVAL as i64),
            "OPEN acted on the refused name {name:?} instead of refusing it",
        );
    }
    let len = stage(&mut window, b"motd");
    let (r0, _) = srv.handle(
        0,
        fs::req(fs::OPEN, fs::ROOT, len),
        0,
        &mut window,
        &mut NoEdges,
    );
    assert!(r0 >= 0, "OPEN refused the legal control name motd: {r0}");
}

/// **`OPENDIR` acts on no refused name, and descends into a legal one.** `sub` is a directory in
/// the tree, so the control descends; the second word is the rights the child is asked to carry.
#[test]
fn opendir_refuses_every_refused_name_and_descends_a_legal_one() {
    let mut srv = server_with_tree();
    let mut window = vec![0u8; fs::TRANSFER_MAX];
    for &name in REFUSED {
        let len = stage(&mut window, name);
        let (r0, _) = srv.handle(
            0,
            fs::req(fs::OPENDIR, fs::ROOT, len),
            filesystem_contract::dir::ALL,
            &mut window,
            &mut NoEdges,
        );
        assert_eq!(
            r0,
            -(EINVAL as i64),
            "OPENDIR acted on the refused name {name:?}",
        );
    }
    let len = stage(&mut window, b"sub");
    let (r0, _) = srv.handle(
        0,
        fs::req(fs::OPENDIR, fs::ROOT, len),
        filesystem_contract::dir::ALL,
        &mut window,
        &mut NoEdges,
    );
    assert!(r0 >= 0, "OPENDIR refused the legal control name sub: {r0}");
}

/// **`CREATE` and `MKDIR` act on no refused name, and leave the directory untouched when they
/// refuse.** The listing before and after is identical: a `CREATE "../escape"` that made an entry
/// named `../escape` in the bound directory is exactly the landmine `names.rs` records, so the
/// side effect is asserted, not only the error.
#[test]
fn create_and_mkdir_refuse_every_refused_name_and_change_nothing() {
    let mut srv = server_with_tree();
    let before = root_listing(&mut srv);
    let mut window = vec![0u8; fs::TRANSFER_MAX];
    for &code in &[fs::CREATE, fs::MKDIR] {
        for &name in REFUSED {
            let len = stage(&mut window, name);
            let (r0, _) = srv.handle(
                0,
                fs::req(code, fs::ROOT, len),
                0,
                &mut window,
                &mut NoEdges,
            );
            assert_eq!(
                r0,
                -(EINVAL as i64),
                "verb {code} acted on the refused name {name:?}",
            );
        }
    }
    assert_eq!(
        before,
        root_listing(&mut srv),
        "a refused CREATE or MKDIR changed the directory",
    );
    // The controls: a legal file and a legal directory both take, through the same two arms.
    let len = stage(&mut window, b"made-file");
    let (r0, _) = srv.handle(
        0,
        fs::req(fs::CREATE, fs::ROOT, len),
        0,
        &mut window,
        &mut NoEdges,
    );
    assert!(r0 >= 0, "CREATE refused a legal name: {r0}");
    let len = stage(&mut window, b"made-dir");
    let (r0, _) = srv.handle(
        0,
        fs::req(fs::MKDIR, fs::ROOT, len),
        0,
        &mut window,
        &mut NoEdges,
    );
    assert!(r0 >= 0, "MKDIR refused a legal name: {r0}");
}

/// **`UNLINK` and `RMDIR` act on no refused name, and remove nothing when they refuse.** The
/// listing is unchanged, so a refused removal did not take a real entry out by a name that
/// happened to collide once `check_component` was skipped.
#[test]
fn unlink_and_rmdir_refuse_every_refused_name_and_remove_nothing() {
    let mut srv = server_with_tree();
    let before = root_listing(&mut srv);
    let mut window = vec![0u8; fs::TRANSFER_MAX];
    for &code in &[fs::UNLINK, fs::RMDIR] {
        for &name in REFUSED {
            let len = stage(&mut window, name);
            let (r0, _) = srv.handle(
                0,
                fs::req(code, fs::ROOT, len),
                0,
                &mut window,
                &mut NoEdges,
            );
            assert_eq!(
                r0,
                -(EINVAL as i64),
                "verb {code} acted on the refused name {name:?}",
            );
        }
    }
    assert_eq!(
        before,
        root_listing(&mut srv),
        "a refused UNLINK or RMDIR changed the directory",
    );
}

/// **The mtime verbs act on no refused name.** All three take a name under a directory handle
/// ([`fs::GETMTIME`], [`fs::SETMTIME`], [`fs::SETMTIME_AT`]); the refused name is `EINVAL` before
/// any node is touched, and `motd`'s mtime reads back to prove the arm is live.
#[test]
fn the_mtime_verbs_refuse_every_refused_name() {
    let mut srv = server_with_tree();
    let mut window = vec![0u8; fs::TRANSFER_MAX];
    for &code in &[fs::GETMTIME, fs::SETMTIME, fs::SETMTIME_AT] {
        for &name in REFUSED {
            let len = stage(&mut window, name);
            let (r0, _) = srv.handle(
                0,
                fs::req(code, fs::ROOT, len),
                0,
                &mut window,
                &mut NoEdges,
            );
            assert_eq!(
                r0,
                -(EINVAL as i64),
                "verb {code} acted on the refused name {name:?}",
            );
        }
    }
    let len = stage(&mut window, b"motd");
    let (r0, _) = srv.handle(
        0,
        fs::req(fs::GETMTIME, fs::ROOT, len),
        0,
        &mut window,
        &mut NoEdges,
    );
    assert!(
        r0 >= 0,
        "GETMTIME refused the legal control name motd: {r0}"
    );
}

/// **`RENAME` acts on no refused name in either operand, and changes nothing when it refuses.**
/// Source and destination both ride in the window; the second word packs the destination handle
/// and length. A refused source with a legal destination, and a legal source with a refused
/// destination, are both `EINVAL`, and a legal-to-legal rename is the control.
#[test]
fn rename_refuses_a_refused_name_in_either_operand_and_changes_nothing() {
    let mut srv = server_with_tree();
    let before = root_listing(&mut srv);
    let mut window = vec![0u8; fs::TRANSFER_MAX];
    let rename = |srv: &mut Server<DiskMemory>, window: &mut [u8], src: &[u8], dst: &[u8]| -> i64 {
        window[..src.len()].copy_from_slice(src);
        window[src.len()..src.len() + dst.len()].copy_from_slice(dst);
        let w1 = fs::rename_dst(fs::ROOT, dst.len() as u64);
        srv.handle(
            0,
            fs::req(fs::RENAME, fs::ROOT, src.len() as u64),
            w1,
            window,
            &mut NoEdges,
        )
        .0
    };
    for &bad in REFUSED {
        // A refused source, a legal destination.
        assert_eq!(
            rename(&mut srv, &mut window, bad, b"dst-ok"),
            -(EINVAL as i64),
            "RENAME acted on the refused source {bad:?}",
        );
        // A legal source, a refused destination.
        assert_eq!(
            rename(&mut srv, &mut window, b"motd", bad),
            -(EINVAL as i64),
            "RENAME acted on the refused destination {bad:?}",
        );
    }
    assert_eq!(
        before,
        root_listing(&mut srv),
        "a refused RENAME changed the directory",
    );
    // The control: motd -> renamed, a legal-to-legal rename through the same arm.
    assert_eq!(
        rename(&mut srv, &mut window, b"motd", b"renamed"),
        0,
        "RENAME refused a legal pair"
    );
}

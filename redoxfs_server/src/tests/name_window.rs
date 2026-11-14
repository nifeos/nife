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
    let mut window = vec![0u8; fs::TRANSFER_MAXIMUM];
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
    let mut window = vec![0u8; fs::TRANSFER_MAXIMUM];
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
    let mut window = vec![0u8; fs::TRANSFER_MAXIMUM];
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
    let mut window = vec![0u8; fs::TRANSFER_MAXIMUM];
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
    let mut window = vec![0u8; fs::TRANSFER_MAXIMUM];
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
    let mut window = vec![0u8; fs::TRANSFER_MAXIMUM];
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

/// **The reserved store name is refused at every depth, by every name-taking verb, and listed in no
/// directory.** The root-level arms above drive `handle` with the name under the root; this one
/// drives each verb from a subdirectory handle, drives the path-taking verbs (`OPEN`, `OPENDIR`)
/// with the name as a middle and as a last step, and plants a node of that name in the
/// subdirectory behind the server's back to show a listing hides it there too.
#[test]
fn the_store_name_is_refused_at_every_depth_and_listed_nowhere() {
    use filesystem_contract::xattr::STORE_DIR;

    let mut srv = server_with_tree();
    let root = fs::ROOT as u32;
    let sub = srv
        .open_dir(root, &named("sub"), filesystem_contract::dir::ALL)
        .unwrap();
    let mut window = vec![0u8; fs::TRANSFER_MAXIMUM];

    // Every verb whose operand is a name, from the subdirectory handle.
    for &code in &[
        fs::OPEN,
        fs::OPENDIR,
        fs::CREATE,
        fs::MKDIR,
        fs::UNLINK,
        fs::RMDIR,
        fs::GETMTIME,
        fs::SETMTIME,
        fs::SETMTIME_AT,
    ] {
        let len = stage(&mut window, STORE_DIR.as_bytes());
        let (r0, _) = srv.handle(
            0,
            fs::req(code, sub as u64, len),
            0,
            &mut window,
            &mut NoEdges,
        );
        assert_eq!(
            r0,
            -(EINVAL as i64),
            "verb {code} took the store name in a subdirectory"
        );
    }

    // The path-taking verbs, with the name as a middle step and as the last step.
    for path in [
        format!("sub/{STORE_DIR}"),
        format!("{STORE_DIR}/x"),
        format!("sub/{STORE_DIR}/x"),
    ] {
        for &code in &[fs::OPEN, fs::OPENDIR] {
            let len = stage(&mut window, path.as_bytes());
            let (r0, _) = srv.handle(
                0,
                fs::req(code, fs::ROOT, len),
                0,
                &mut window,
                &mut NoEdges,
            );
            assert_eq!(r0, -(EINVAL as i64), "verb {code} walked the path {path:?}");
        }
    }

    // RENAME between directories, the store name in either operand.
    let sub_rename = |srv: &mut Server<DiskMemory>, window: &mut [u8], src: &[u8], dst: &[u8]| {
        window[..src.len()].copy_from_slice(src);
        window[src.len()..src.len() + dst.len()].copy_from_slice(dst);
        let w1 = fs::rename_dst(fs::ROOT, dst.len() as u64);
        srv.handle(
            0,
            fs::req(fs::RENAME, sub as u64, src.len() as u64),
            w1,
            window,
            &mut NoEdges,
        )
        .0
    };
    assert_eq!(
        sub_rename(&mut srv, &mut window, b"inner", STORE_DIR.as_bytes()),
        -(EINVAL as i64),
        "RENAME took the store name as a destination from a subdirectory",
    );
    assert_eq!(
        sub_rename(&mut srv, &mut window, STORE_DIR.as_bytes(), b"anywhere"),
        -(EINVAL as i64),
        "RENAME took the store name as a source from a subdirectory",
    );

    // A node with that name in the subdirectory, made under the server, is not listed.
    srv.fs
        .tx(|transaction| {
            let (ptr, _) = srv_lookup_sub(transaction)?;
            transaction.create_node(ptr, STORE_DIR, redoxfs::Node::MODE_FILE | 0o600, 0, 0)?;
            Ok(())
        })
        .unwrap();
    let listed = list(&mut srv, sub, BLOCK).unwrap();
    assert!(
        listed.iter().any(|(n, _)| n == "inner"),
        "the control entry vanished: {listed:?}"
    );
    assert!(
        !listed.iter().any(|(n, _)| n == STORE_DIR),
        "a subdirectory listing named the store: {listed:?}",
    );
}

/// The `sub` directory's node in the root, for a test that plants an entry under it.
fn srv_lookup_sub<D: redoxfs::Disk>(
    transaction: &mut redoxfs::Transaction<D>,
) -> syscall::Result<(redoxfs::TreePtr<redoxfs::Node>, ())> {
    let node = transaction.find_node(redoxfs::TreePtr::root(), "sub")?;
    Ok((node.ptr(), ()))
}

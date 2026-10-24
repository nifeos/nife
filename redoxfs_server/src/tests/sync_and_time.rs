//! **`SYNC`'s rights and `mtime`'s two forms**, against the server over a block-chunked image.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// A sync is a write-side authority, and a read-only capability may not ask for one (milestone
/// 55). This pins the choice argued at `filesystem_contract::fs::SYNC`: a device flush stalls the
/// queue for every client, and a holder with nothing written has nothing to make durable. It also
/// pins the shape: either kind of handle, because the answer is about the storage, which is
/// `STATFS`'s rule and why this goes through `node_at` rather than `file_at`.
#[test]
fn a_sync_needs_write_and_takes_a_handle_of_either_kind() {
    let mut fs = FileSystem::create(BlockDisk(VecIo(vec![0u8; 16 * 1024 * 1024])), None, 0, 0)
        .expect("mkfs through BlockDisk");
    fs.tx(|tx| {
        tx.create_node(TreePtr::root(), "scratch", Node::MODE_FILE | 0o644, 0, 0)?;
        tx.create_node(TreePtr::root(), "sub", Node::MODE_DIR | 0o755, 0, 0)?;
        Ok(())
    })
    .expect("populate");
    let image = fs.disk.0.0;
    let mut srv = Server::open(BlockDisk(VecIo(image))).expect("Server::open");

    // The bound directory carries every right, so it may ask.
    assert!(
        srv.sync_permitted(filesystem_contract::fs::ROOT as u32)
            .is_ok()
    );

    // A file handle opened through it inherits READ | WRITE, so it may too: the verb is not
    // about the file, and refusing a file handle would be inventing a distinction.
    let file = srv.open_file(&named("scratch")).expect("open scratch");
    assert!(srv.sync_permitted(file).is_ok());

    // A directory capability narrowed to reading may not. EROFS rather than EACCES, because
    // through this capability the storage takes no writes and there is no policy that could
    // have said yes.
    let ro = srv
        .open_dir(
            filesystem_contract::fs::ROOT as u32,
            &named("sub"),
            dir::ENUMERATE | dir::READ | dir::DESCEND,
        )
        .expect("open sub read-only");
    assert_eq!(
        srv.sync_permitted(ro).unwrap_err().errno,
        dir::EROFS,
        "a read-only capability must not be able to make the whole device flush"
    );

    // And a handle nobody minted is EBADF, which is the check that makes the right meaningful:
    // a forged handle must not be a way around it.
    assert_eq!(srv.sync_permitted(9999).unwrap_err().errno, EBADF);
}

/// **`touch`'s bare form observes a value the server chose, and it moves forward** (milestone
/// 47, DECISIONS §112). Two `set_mtime_now` calls on the same name must not read back equal:
/// if they did, `touch` could not be used for a staleness check, which is the property every
/// caller of bare `touch` actually depends on.
#[test]
fn setting_mtime_to_now_twice_observes_two_different_times_and_neither_is_the_callers_choice() {
    let mut srv = server_with_tree();
    let root = filesystem_contract::fs::ROOT as u32;

    let before = srv
        .mtime(root, &named("motd"))
        .expect("read the seeded mtime");
    srv.set_mtime_now(root, &named("motd"))
        .expect("touch, bare");
    let first = srv
        .mtime(root, &named("motd"))
        .expect("read back the first touch");
    srv.set_mtime_now(root, &named("motd"))
        .expect("touch again");
    let second = srv
        .mtime(root, &named("motd"))
        .expect("read back the second touch");

    assert!(
        first > before,
        "a bare touch must move the mtime forward from what the file was seeded with"
    );
    assert!(
        second > first,
        "two bare touches in sequence must not observe the same mtime, or a make-style \
         staleness check could not tell them apart"
    );
}

/// **`touch -t`'s round trip: the value read back is exactly the value asserted, not "now"**
/// (milestone 47, DECISIONS §112). This is the test that makes the two-right split real rather
/// than declared: if `set_mtime_at` silently fell back to the server's own clock, every rights
/// check in [`each_rung_of_the_ladder_gates_exactly_its_own_verb`] would still pass and this is
/// the only thing that would catch it.
#[test]
fn setting_an_arbitrary_mtime_round_trips_exactly_and_is_not_now() {
    let mut srv = server_with_tree();
    let root = filesystem_contract::fs::ROOT as u32;

    // The same instant the kernel-booted witness asserts with a real `touch -t` command line
    // (`filesystem_fixture::tree::NAV_TOUCH_AT_UNIX`), reused here rather than a second
    // literal so the two round-trip checks (this one in-process, that one over the real wire)
    // are provably checking the same claim. Nowhere near this server's own advancing clock
    // (which starts at 1 and has moved by single digits by the time this test runs), so a
    // fallback to "now" cannot coincidentally pass.
    const ASSERTED: u64 = filesystem_fixture::tree::NAV_TOUCH_AT_UNIX;
    srv.set_mtime_at(root, &named("motd"), ASSERTED)
        .expect("write an arbitrary mtime with full rights");
    assert_eq!(
        srv.mtime(root, &named("motd")).expect("read it back"),
        ASSERTED,
        "the round trip must return exactly the asserted value"
    );

    // And a second, ordinary touch does NOT preserve it: `set_mtime_now` is unconditional, so
    // an arbitrary value set with -t is exactly as overwritable by a bare touch as any other
    // mtime, which is the honest answer for "how long does a lie about history last".
    srv.set_mtime_now(root, &named("motd"))
        .expect("touch, bare");
    assert_ne!(
        srv.mtime(root, &named("motd")).expect("read it back again"),
        ASSERTED,
        "a bare touch after an arbitrary one must move the mtime again, not preserve the lie"
    );
}

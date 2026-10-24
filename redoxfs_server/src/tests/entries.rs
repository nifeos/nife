use super::*;

// --- RENAME (milestone 47, DECISIONS §42's verb) ---

/// **A rename moves the name and the bytes follow it**, within a directory and across two of
/// them. The cross-directory case is the one POSIX mandates and the one an application's
/// temp-file-then-rename idiom needs, so it is not enough for the same-directory case to work.
#[test]
fn a_rename_moves_a_name_within_a_directory_and_between_two() {
    let mut srv = server_with_tree();
    let root = filesystem_contract::fs::ROOT as u32;
    let sub = srv.open_dir(root, &named("sub"), dir::ALL).unwrap();
    let deeper = srv.open_dir(sub, &named("deeper"), dir::ALL).unwrap();

    // Within one directory: `sub/inner` becomes `sub/renamed`, contents intact.
    srv.rename(sub, &named("inner"), sub, &named("renamed"))
        .expect("rename");
    assert_eq!(
        srv.open_file_at(sub, &named("inner"))
            .err()
            .map(|e| e.errno),
        Some(syscall::error::ENOENT),
        "the old name must be gone, not merely shadowed",
    );
    let h = srv
        .open_file_at(sub, &named("renamed"))
        .expect("the new name");
    let mut buf = [0u8; 64];
    let n = srv.read(h, 0, &mut buf).unwrap();
    assert_eq!(&buf[..n], b"inside the grant");

    // Across two directories, which is the same-filesystem move POSIX mandates be atomic.
    srv.rename(sub, &named("renamed"), deeper, &named("moved"))
        .expect("move");
    assert_eq!(
        srv.open_file_at(sub, &named("renamed"))
            .err()
            .map(|e| e.errno),
        Some(syscall::error::ENOENT),
    );
    let h = srv
        .open_file_at(deeper, &named("moved"))
        .expect("the moved name");
    let n = srv.read(h, 0, &mut buf).unwrap();
    assert_eq!(
        &buf[..n],
        b"inside the grant",
        "the bytes followed the name"
    );
}

/// **The `REMOVE` rung, which had no verb until this one.** A capability that may create and
/// write and not remove cannot move a name out; one that may remove and not create cannot move a
/// name in. Both answer `EROFS`, and the pair is the point: a rename is two authorities and
/// either one missing is enough.
#[test]
fn a_rename_needs_remove_where_it_takes_from_and_create_where_it_puts() {
    let mut srv = server_with_tree();
    let root = filesystem_contract::fs::ROOT as u32;
    // Everything but REMOVE: milestone 47's motivating sentence, "add to this, destroy nothing".
    let append = srv
        .open_dir(root, &named("sub"), dir::ALL & !dir::REMOVE)
        .unwrap();
    assert_eq!(
        srv.rename(append, &named("inner"), append, &named("elsewhere"))
            .err()
            .map(|e| e.errno),
        Some(EROFS),
        "a capability that cannot remove cannot rename away from itself either",
    );
    // And the refusal is about the capability, not about the name: it is the same answer for a
    // name that is not there, so this verb cannot be used to probe for one.
    assert_eq!(
        srv.rename(append, &named("absent"), append, &named("x"))
            .err()
            .map(|e| e.errno),
        Some(EROFS),
    );

    // Everything but CREATE: it may give a name up and has nowhere to put one.
    let mut srv = server_with_tree();
    let taker = srv
        .open_dir(root, &named("sub"), dir::ALL & !dir::CREATE)
        .unwrap();
    assert_eq!(
        srv.rename(taker, &named("inner"), taker, &named("elsewhere"))
            .err()
            .map(|e| e.errno),
        Some(EROFS),
    );

    // With both, the same request through the same code succeeds, so the refusals above are
    // statements about the rights rather than about a verb that never works.
    let mut srv = server_with_tree();
    let full = srv.open_dir(root, &named("sub"), dir::ALL).unwrap();
    srv.rename(full, &named("inner"), full, &named("elsewhere"))
        .expect("REMOVE and CREATE together must be enough");
}

/// **Cross-directory rights are checked per directory, not once**, which is the case a
/// single-capability check would get wrong: taking a name out of a directory you may remove
/// from and putting it into one you may not create in is two different refusals.
#[test]
fn a_cross_directory_rename_consults_both_directories_rights() {
    let mut srv = server_with_tree();
    let root = filesystem_contract::fs::ROOT as u32;
    let src = srv.open_dir(root, &named("sub"), dir::ALL).unwrap();
    // A destination with no CREATE. Reached by descending twice so it is a genuinely different
    // capability rather than the same handle spelled differently.
    let dst = srv
        .open_dir(src, &named("deeper"), dir::ALL & !dir::CREATE)
        .unwrap();
    assert_eq!(
        srv.rename(src, &named("inner"), dst, &named("landed"))
            .err()
            .map(|e| e.errno),
        Some(EROFS),
        "the destination's missing CREATE must refuse, even though the source may remove",
    );
    // The source is untouched by the refusal: a half-done rename is the failure this verb exists
    // to make impossible.
    assert!(srv.open_file_at(src, &named("inner")).is_ok());

    let wide = srv.open_dir(src, &named("deeper"), dir::ALL).unwrap();
    srv.rename(src, &named("inner"), wide, &named("landed"))
        .expect("rename");
    assert!(srv.open_file_at(wide, &named("landed")).is_ok());
}

/// A rename **replaces** an existing destination of the same kind, which is what the temp-file
/// idiom depends on, and refuses one of a different kind with POSIX's own two answers.
#[test]
fn a_rename_replaces_a_file_and_refuses_to_cross_kinds() {
    let mut srv = server_with_tree();
    let root = filesystem_contract::fs::ROOT as u32;
    let sub = srv.open_dir(root, &named("sub"), dir::ALL).unwrap();
    let tmp = srv.create_file_at(sub, &named("tmp")).unwrap();
    srv.write(tmp, 0, b"the new contents").unwrap();

    srv.rename(sub, &named("tmp"), sub, &named("inner"))
        .expect("replace");
    let h = srv.open_file_at(sub, &named("inner")).unwrap();
    let mut buf = [0u8; 64];
    let n = srv.read(h, 0, &mut buf).unwrap();
    assert_eq!(
        &buf[..n],
        b"the new contents",
        "the destination must hold the source's bytes, whole",
    );
    assert_eq!(
        srv.open_file_at(sub, &named("tmp")).err().map(|e| e.errno),
        Some(syscall::error::ENOENT),
    );

    // A file over a directory, and a directory over a file. RedoxFS's rename_node allows both
    // (it passes the destination's own type to remove_node, so it never compares the two);
    // these are our checks, so the verb means one thing regardless of backend.
    assert_eq!(
        srv.rename(sub, &named("inner"), sub, &named("deeper"))
            .err()
            .map(|e| e.errno),
        Some(EISDIR),
    );
    assert_eq!(
        srv.rename(sub, &named("deeper"), sub, &named("inner"))
            .err()
            .map(|e| e.errno),
        Some(ENOTDIR),
    );
    // And a non-empty destination directory survives, which is the engine's ENOTEMPTY.
    let d = srv.make_dir(sub, &named("empty-dir"), dir::ALL).unwrap();
    let _ = d;
    assert_eq!(
        srv.rename(sub, &named("empty-dir"), sub, &named("deeper"))
            .err()
            .map(|e| e.errno),
        Some(syscall::error::ENOTEMPTY),
    );
}

/// **A directory renames in place and does not move between directories**, and the refusal is
/// the contract's rather than an accident: moving a directory into one of its own descendants is
/// how a tree becomes a detached cycle, POSIX guards it with a path-prefix test, and this
/// contract has no paths to take a prefix of. Renaming inside one directory cannot create a
/// cycle because the parent does not change, which is exactly where the line is drawn.
#[test]
fn a_directory_renames_in_place_and_refuses_to_move_between_directories() {
    let mut srv = server_with_tree();
    let root = filesystem_contract::fs::ROOT as u32;
    let sub = srv.open_dir(root, &named("sub"), dir::ALL).unwrap();

    srv.rename(sub, &named("deeper"), sub, &named("renamed-dir"))
        .expect("a directory renames in place");
    let d = srv
        .open_dir(sub, &named("renamed-dir"), dir::ALL)
        .expect("and it is still a directory, with its contents");
    assert!(srv.open_file_at(d, &named("leaf")).is_ok());

    // The cycle this refusal prevents, spelled out: `sub` moved into something inside `sub`.
    assert_eq!(
        srv.rename(root, &named("sub"), d, &named("sub"))
            .err()
            .map(|e| e.errno),
        Some(EINVAL),
        "moving a directory between directories is not offered, and says so",
    );
    // Even a harmless-looking move (out of the grant's reach entirely) takes the same answer,
    // because the rule is about the verb and not about which pair happens to be safe.
    assert_eq!(
        srv.rename(sub, &named("renamed-dir"), root, &named("promoted"))
            .err()
            .map(|e| e.errno),
        Some(EINVAL),
    );
}

/// A name is a single component on both halves, a self-rename is a no-op success, and a rename
/// through a file handle is `ENOTDIR`. Each of these is a way to make the server treat one kind
/// of thing as another.
#[test]
fn a_rename_refuses_what_every_other_name_taking_verb_refuses() {
    let mut srv = server_with_tree();
    let root = filesystem_contract::fs::ROOT as u32;
    let sub = srv.open_dir(root, &named("sub"), dir::ALL).unwrap();

    for bad in ["..", ".", "", "a/b", "/abs", "with:colon"] {
        assert_eq!(
            srv.rename(sub, &named(bad), sub, &named("x"))
                .err()
                .map(|e| e.errno),
            Some(EINVAL),
            "a source named {bad:?} must be refused as EINVAL",
        );
        assert_eq!(
            srv.rename(sub, &named("inner"), sub, &named(bad))
                .err()
                .map(|e| e.errno),
            Some(EINVAL),
            "a destination named {bad:?} must be refused as EINVAL",
        );
    }
    // Renaming a name onto itself changes nothing and succeeds, which is POSIX's answer and the
    // one that matters: the alternative unlinks the only link and then has nothing to relink.
    srv.rename(sub, &named("inner"), sub, &named("inner"))
        .expect("a no-op");
    assert!(srv.open_file_at(sub, &named("inner")).is_ok());

    let f = srv.open_file_at(sub, &named("inner")).unwrap();
    assert_eq!(
        srv.rename(f, &named("inner"), sub, &named("x"))
            .err()
            .map(|e| e.errno),
        Some(ENOTDIR),
    );
    assert_eq!(
        srv.rename(sub, &named("inner"), f, &named("x"))
            .err()
            .map(|e| e.errno),
        Some(ENOTDIR),
    );
    assert_eq!(
        srv.rename(999, &named("inner"), sub, &named("x"))
            .err()
            .map(|e| e.errno),
        Some(EBADF),
    );
}

/// A rename is on the platter, not only in the handle table: drop the mount the way a dying
/// process does and reopen. Persistence is the half a cache cannot fake, and it is what the
/// crash sweep in `tests/crash_consistency.rs` then cuts into.
#[test]
fn a_rename_survives_a_reopen() {
    let mut disk = image_with_tree();
    {
        let mut srv = Server::open(disk).expect("open");
        let sub = srv
            .open_dir(
                filesystem_contract::fs::ROOT as u32,
                &named("sub"),
                dir::ALL,
            )
            .unwrap();
        srv.rename(sub, &named("inner"), sub, &named("survivor"))
            .unwrap();
        disk = srv.fs.disk;
    }
    let mut srv = Server::open(disk).expect("reopen");
    let sub = srv
        .open_dir(
            filesystem_contract::fs::ROOT as u32,
            &named("sub"),
            dir::ALL,
        )
        .unwrap();
    assert_eq!(
        srv.open_file_at(sub, &named("inner"))
            .err()
            .map(|e| e.errno),
        Some(syscall::error::ENOENT),
    );
    let h = srv
        .open_file_at(sub, &named("survivor"))
        .expect("the new name");
    let mut buf = [0u8; 64];
    let n = srv.read(h, 0, &mut buf).unwrap();
    assert_eq!(&buf[..n], b"inside the grant");
}

/// Everything a directory capability makes survives the mount, which is the half a cache cannot
/// fake: the descent, the created directory, and the file inside it are all on the platter.
#[test]
fn a_subtree_built_through_directory_handles_survives_a_reopen() {
    let mut disk = image_with_tree();
    {
        let mut srv = Server::open(disk).expect("open");
        let sub = srv
            .open_dir(
                filesystem_contract::fs::ROOT as u32,
                &named("sub"),
                dir::ALL,
            )
            .unwrap();
        let made = srv.make_dir(sub, &named("made"), dir::ALL).unwrap();
        let f = srv.create_file_at(made, &named("note")).unwrap();
        srv.write(f, 0, b"written two levels down").unwrap();
        disk = srv.fs.disk; // no unmount, as a dying process leaves it
    }
    let mut srv = Server::open(disk).expect("reopen");
    let sub = srv
        .open_dir(
            filesystem_contract::fs::ROOT as u32,
            &named("sub"),
            dir::ALL,
        )
        .unwrap();
    let made = srv
        .open_dir(sub, &named("made"), dir::ALL)
        .expect("the made directory");
    let f = srv
        .open_file_at(made, &named("note"))
        .expect("the file in it");
    let mut buf = [0u8; 64];
    let n = srv.read(f, 0, &mut buf).unwrap();
    assert_eq!(&buf[..n], b"written two levels down");
}
// --- STATFS (milestone 54) ---

/// **The volume's numbers are the image's, and writing to it moves the free count.**
///
/// The moving part is what makes this a test rather than a tautology: a `statfs` that returned
/// a constant would pass every assertion about units and totals, and it is exactly the constant
/// this verb exists to replace (`smb_proto::share::NOMINAL_VOLUME_BYTES`). So the free count is
/// read, a megabyte is written, and it must have gone down.
#[test]
fn statfs_reports_the_image_and_a_write_takes_space_out_of_it() {
    let mut srv = server_with(&[("small", b"x")]);

    let (block, total, free) = srv.statfs(filesystem_contract::fs::ROOT as u32).unwrap();
    assert_eq!(
        block,
        redoxfs::BLOCK_SIZE,
        "the unit is RedoxFS's own block"
    );
    // `server_with` builds a 16 MiB DiskMemory; the filesystem is that minus the reservation
    // RedoxFS makes at the front, so the assertion is a range rather than an equality.
    let bytes = filesystem_contract::statfs::total_bytes(block, total);
    assert!(
        (8 * 1024 * 1024..=16 * 1024 * 1024).contains(&bytes),
        "a 16 MiB image reported {bytes} bytes",
    );
    assert!(free > 0 && free < total, "free {free} of {total} blocks");

    let h = srv.create_file(&named("big")).unwrap();
    srv.write(h, 0, &[7u8; 1024 * 1024]).unwrap();
    let (_, after_total, after_free) = srv.statfs(filesystem_contract::fs::ROOT as u32).unwrap();
    assert_eq!(after_total, total, "the image did not change size");
    assert!(
        after_free < free,
        "a megabyte was written and free went {free} -> {after_free}",
    );

    // Any handle qualifies, file or directory, because the answer is about the image behind
    // both. A handle nobody minted does not.
    assert_eq!(srv.statfs(h).unwrap().0, block);
    assert_eq!(srv.statfs(9999).err().map(|e| e.errno), Some(EBADF));
}

/// **No right is demanded**, which is the row `filesystem_contract::verb::TABLE` carries and the thing a
/// narrowed capability depends on: a grant that may write but not read still has to be able to
/// find out whether its next write fits.
#[test]
fn statfs_answers_through_a_capability_that_carries_nothing() {
    let mut srv = server_with_tree();
    let root = filesystem_contract::fs::ROOT as u32;
    // A child directory holding one right, and not one of the ones a read would need.
    let narrow = srv.open_dir(root, &named("sub"), dir::DESCEND).unwrap();
    assert!(
        srv.read_dir(narrow, 0, &mut [0u8; 64]).is_err(),
        "the fixture is only meaningful if this capability cannot enumerate",
    );
    assert_eq!(
        srv.statfs(narrow).unwrap().0,
        redoxfs::BLOCK_SIZE,
        "STATFS asks for no right; see filesystem_contract::fs::STATFS",
    );
}

// --- CREATE and TRUNCATE (milestone 31 phase 2) ---

#[test]
fn create_makes_an_empty_file_and_refuses_an_existing_name() {
    let mut srv = server_with(&[("already", b"here")]);

    let h = srv.create_file(&named("fresh")).unwrap();
    assert_eq!(srv.fstat(h).unwrap(), 0, "a created file starts empty");

    // Create is create, not create-or-open. Silently opening an existing file is how a
    // partly-working write comes to look like a working one (the §27 failure mode).
    assert_eq!(
        srv.create_file(&named("already")).err().map(|e| e.errno),
        Some(EEXIST),
        "creating over an existing name must refuse, and must not truncate it either",
    );
    let h2 = srv.open_file(&named("already")).unwrap();
    assert_eq!(
        srv.fstat(h2).unwrap(),
        4,
        "the refused create left the file alone"
    );

    // And the refusal did not half-create anything: the name still resolves to the old file.
    let mut buf = [0u8; 8];
    assert_eq!(srv.read(h2, 0, &mut buf).unwrap(), 4);
    assert_eq!(&buf[..4], b"here");
}

#[test]
fn truncate_shrinks_and_discards_the_tail_which_is_the_bug_it_exists_to_fix() {
    // The exact byte counts from DECISIONS §27: a 64-byte payload from one boot, a 61-byte
    // write from the next, and the three-byte tail that made a good write look like a failure.
    let mut srv = server_with(&[("scratch", b"(placeholder)")]);
    let h = srv.open_file(&named("scratch")).unwrap();
    assert_eq!(srv.write(h, 0, &[b'L'; 64]).unwrap(), 64);

    let short = [b'S'; 61];
    assert_eq!(srv.write(h, 0, &short).unwrap(), 61);
    srv.truncate(h, short.len() as u64).unwrap();

    assert_eq!(srv.fstat(h).unwrap(), 61, "truncate sets the size exactly");
    let mut buf = [0u8; 128];
    let n = srv.read(h, 0, &mut buf).unwrap();
    assert_eq!(n, 61, "and a whole-file read returns only what was written");
    assert_eq!(&buf[..61], &short[..], "with the new bytes intact");
}

#[test]
fn truncate_grows_with_zeroes_and_is_idempotent() {
    let mut srv = server_with(&[("f", b"abc")]);
    let h = srv.open_file(&named("f")).unwrap();

    srv.truncate(h, 8).unwrap();
    assert_eq!(srv.fstat(h).unwrap(), 8);
    let mut buf = [0xffu8; 16];
    assert_eq!(srv.read(h, 0, &mut buf).unwrap(), 8);
    assert_eq!(&buf[..3], b"abc", "the original bytes survive a grow");
    assert_eq!(
        &buf[3..8],
        &[0, 0, 0, 0, 0],
        "and the extension reads as zeroes"
    );

    // Truncating to the current size changes nothing, which matters because std::fs::write
    // opens-creates-truncates unconditionally and would otherwise churn the tree every call.
    srv.truncate(h, 8).unwrap();
    assert_eq!(srv.fstat(h).unwrap(), 8);

    srv.truncate(h, 0).unwrap();
    assert_eq!(
        srv.fstat(h).unwrap(),
        0,
        "and shrinking to empty is allowed"
    );
    assert_eq!(srv.read(h, 0, &mut buf).unwrap(), 0);
}

#[test]
fn create_write_truncate_all_survive_a_close_and_reopen() {
    // The half a cache cannot fake: drop the mount entirely and reopen the same disk. The disk is
    // taken back out of the server rather than cloned, which is also how a dying process leaves
    // it: no unmount, exactly as the cross-boot test does.
    let mut disk = image_with(&[]);
    let payload = b"created, written, and truncated to exactly this";

    {
        let mut srv = Server::open(disk).expect("open");
        let h = srv.create_file(&named("made")).unwrap();
        assert_eq!(srv.write(h, 0, payload).unwrap(), payload.len());
        assert_eq!(srv.write(h, 0, &[b'X'; 200]).unwrap(), 200);
        assert_eq!(srv.write(h, 0, payload).unwrap(), payload.len());
        srv.truncate(h, payload.len() as u64).unwrap();
        disk = srv.fs.disk;
    }

    let mut srv = Server::open(disk).expect("reopen");
    let h = srv
        .open_file(&named("made"))
        .expect("the created name survived the reopen");
    assert_eq!(srv.fstat(h).unwrap(), payload.len() as u64);
    let mut buf = [0u8; 512];
    let n = srv.read(h, 0, &mut buf).unwrap();
    assert_eq!(
        &buf[..n],
        payload,
        "the truncated contents are what persisted"
    );
}

#[test]
fn create_and_truncate_refuse_what_every_other_verb_refuses() {
    let mut srv = server_with(&[("f", b"x")]);

    // A forged handle is EBADF in exactly one place, and the new verb goes through it too.
    assert_eq!(
        srv.truncate(99, 0).err().map(|e| e.errno),
        Some(EBADF),
        "truncate must validate the handle like every other operation",
    );

    // A name is a single component, not a path, and both verbs enforce it. RedoxFS does NOT:
    // its check_name passes `/`, `.` and `..` through, so before this check create_file("../escape")
    // happily made an entry literally named `../escape`. Not a traversal, because nothing walks
    // paths, and a landmine for the first thing that does.
    for bad in [
        "../escape",
        "sub/deep",
        "/absolute",
        ".",
        "..",
        "",
        "with:colon",
        "back\\slash",
    ] {
        assert_eq!(
            srv.create_file(&named(bad)).err().map(|e| e.errno),
            Some(EINVAL),
            "create must refuse {bad:?} as EINVAL: a name is not a path",
        );
        // `OPEN` takes a relative path since ruling A of milestone 606 (a directory walk costs
        // what it does on Linux), so `sub/deep` is a path to open rather than a malformed name,
        // and there is no `sub` here: `ENOENT`. Every other entry still has a component that
        // is not a name.
        let want = if bad == "sub/deep" { ENOENT } else { EINVAL };
        assert_eq!(
            srv.open_file(&named(bad)).err().map(|e| e.errno),
            Some(want),
            "and open must refuse {bad:?} the same way create does, or as a path",
        );
    }
}
/// **What `std::fs::write` actually does, twice, with a shorter payload the second time.**
///
/// Written to chase a livelock: on device this sequence tripped the 90 s per-test ceiling with two
/// userspace threads spinning. If the loop is in the filesystem it shows up here in milliseconds;
/// if this passes, the fault is above the core (the wire, the PAL, or the client's own loops) and
/// that is worth knowing before reading any of them.
///
/// `std::fs::write` is `create(true).truncate(true)`, so each call is: open, or create if absent;
/// truncate to 0; write. The second call is the interesting one, because it truncates a file that
/// already has bytes and then writes fewer than it had.
#[test]
fn the_std_fs_write_sequence_twice_does_not_loop() {
    let mut srv = server_with(&[]);
    let long = b"the first write, deliberately the longer of the two";
    let short = b"the second write, shorter";
    assert!(short.len() < long.len());

    // Call one: the name is absent, so create, then truncate an already-empty file, then write.
    let h = srv.create_file(&named("made-by-std")).unwrap();
    srv.truncate(h, 0).unwrap();
    assert_eq!(srv.write(h, 0, long).unwrap(), long.len());
    srv.close(h).unwrap();

    // Call two: the name exists, so open, truncate away 51 bytes, then write 25.
    let h = srv.open_file(&named("made-by-std")).unwrap();
    srv.truncate(h, 0).unwrap();
    assert_eq!(
        srv.fstat(h).unwrap(),
        0,
        "truncate to 0 must empty the file"
    );
    assert_eq!(srv.write(h, 0, short).unwrap(), short.len());
    assert_eq!(srv.fstat(h).unwrap(), short.len() as u64);
    srv.close(h).unwrap();

    // And the read-back `std::fs::read` does: open, fstat for the size hint, read to EOF. The
    // read loop must terminate, which means a read at or past the size returns 0.
    let h = srv.open_file(&named("made-by-std")).unwrap();
    let size = srv.fstat(h).unwrap() as usize;
    let mut got = Vec::new();
    let mut buf = [0u8; 4096];
    let mut off = 0u64;
    // Bounded so a livelock fails the test instead of hanging it: EOF must arrive well inside this.
    for _ in 0..16 {
        let n = srv.read(h, off, &mut buf).unwrap();
        if n == 0 {
            break;
        }
        got.extend_from_slice(&buf[..n]);
        off += n as u64;
    }
    assert_eq!(
        got.len(),
        size,
        "the read loop did not reach EOF in 16 reads"
    );
    assert_eq!(
        &got[..],
        short,
        "the shorter write did not replace the contents"
    );
    assert_eq!(
        srv.read(h, off, &mut buf).unwrap(),
        0,
        "a read at EOF must return 0, or every read_to_end spins",
    );
}
/// **Create into a directory the image already populated**, which is what the device does and what
/// the empty-root tests above do not.
///
/// The device image is built by the host tool with `motd` and `scratch` already in the root, and
/// `std::fs::write` then adds a third name. On device that hangs: three FS-server instances were
/// caught spinning, one in `create_node` and two in `find_node_inner`, i.e. in the DIRECTORY code
/// rather than the allocator. Every earlier host test created into an *empty* root, so none of
/// them shaped the directory the way the real image does. If the fault is directory-shape
/// dependent it belongs here, in milliseconds.
#[test]
fn create_into_a_populated_root_and_look_it_up() {
    // The same names and contents the shipped image carries, so the root's shape matches.
    let mut srv = server_with(&[
        (
            "motd",
            b"redoxfs served this file to an EL0 client through a capability handle\n",
        ),
        (
            "scratch",
            b"(placeholder overwritten by the redoxfs_server write test)",
        ),
    ]);

    let h = srv.create_file(&named("made-by-std")).unwrap();
    assert_eq!(srv.write(h, 0, b"first").unwrap(), 5);
    srv.close(h).unwrap();

    // The lookups that spin on device: the new name, and crucially the pre-existing ones, since a
    // corrupted directory would take every scan down with it, not just the new entry's.
    for name in ["made-by-std", "motd", "scratch"] {
        let h = srv
            .open_file(&named(name))
            .unwrap_or_else(|e| panic!("{name} no longer resolves: {e:?}"));
        srv.close(h).unwrap();
    }
    assert!(
        srv.open_file(&named("definitely-absent")).is_err(),
        "a miss must still terminate and report, not scan forever",
    );
}
/// CREATE through the EL0 binary's exact block chunking, the one path no other create test takes.
///
/// The other tests drive `DiskMemory`, whose `write_at` writes exactly its buffer; the device drives
/// `IpcDisk` behind [`BlockDisk`], which splits calls into whole blocks and read-modify-writes a
/// short final chunk. `create_node` writes a fresh node block and extends a directory, the shape
/// that would expose a chunking fault, and on device FS servers were seen spinning in `create_node`
/// and `find_node_inner`. Bounded, so a livelock fails the run rather than wedging the suite.
#[test]
fn create_through_the_block_chunking_does_not_loop() {
    let mut fs = FileSystem::create(BlockDisk(VecIo(vec![0u8; 16 * 1024 * 1024])), None, 0, 0)
        .expect("mkfs through BlockDisk");
    // The root the shipped image has when std::fs::write adds a third name to it.
    fs.tx(|tx| {
        for (name, body) in [
            (
                "motd",
                &b"redoxfs served this file to an EL0 client through a capability handle\n"[..],
            ),
            (
                "scratch",
                &b"(placeholder overwritten by the redoxfs_server write test)"[..],
            ),
        ] {
            let ptr = tx
                .create_node(TreePtr::root(), name, Node::MODE_FILE | 0o644, 0, 0)?
                .ptr();
            tx.write_node(ptr, 0, body, 0, 0)?;
        }
        Ok(())
    })
    .expect("populate");
    let image = fs.disk.0.0;

    let mut srv = Server::open(BlockDisk(VecIo(image))).expect("Server::open");

    // What std::fs::write does: create, truncate, write; then again, shorter.
    let h = srv
        .create_file(&named("made-by-std"))
        .expect("create through chunking");
    srv.truncate(h, 0).unwrap();
    let long = b"the first write, deliberately the longer of the two";
    assert_eq!(srv.write(h, 0, long).unwrap(), long.len());
    srv.close(h).unwrap();

    let h = srv
        .open_file(&named("made-by-std"))
        .expect("reopen through chunking");
    srv.truncate(h, 0).unwrap();
    let short = b"the second write, shorter";
    assert_eq!(srv.write(h, 0, short).unwrap(), short.len());
    srv.close(h).unwrap();

    // The lookups that spin on device, including the pre-existing names: a directory damaged by
    // the create would take every scan down with it, not only the new entry's.
    for name in ["made-by-std", "motd", "scratch"] {
        let h = srv
            .open_file(&named(name))
            .unwrap_or_else(|e| panic!("{name} no longer resolves after a chunked create: {e:?}"));
        srv.close(h).unwrap();
    }

    let h = srv.open_file(&named("made-by-std")).unwrap();
    let mut buf = [0u8; 256];
    let n = srv.read(h, 0, &mut buf).unwrap();
    assert_eq!(
        &buf[..n],
        short,
        "the chunked create/truncate/write round trip lost bytes"
    );
}

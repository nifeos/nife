use super::*;

// --- The directory capability (milestone 47) ---

/// The names in a listing, for the memo tests below.
fn names(srv: &mut Server<DiskMemory>, handle: u32) -> Vec<(String, bool)> {
    list(srv, handle, 4096).expect("list")
}

/// **Every namespace change is visible to the next listing and the next lookup** (milestone
/// 606). The listing is memoized after the first `READDIR`, so each step below lists and looks
/// up once to fill the memo, changes the namespace, and checks that neither answer is stale.
#[test]
fn a_memoized_listing_follows_every_namespace_change() {
    let root = filesystem_contract::fs::ROOT as u32;
    let mut srv = server_with(&[("a", b"alpha")]);
    assert_eq!(names(&mut srv, root), [("a".into(), false)]);

    let h = srv.create_file_at(root, &named("b")).unwrap();
    srv.close(h).unwrap();
    assert_eq!(
        names(&mut srv, root),
        [("a".into(), false), ("b".into(), false)]
    );
    let h = srv
        .open_file_at(root, &named("b"))
        .expect("a created name opens");
    srv.close(h).unwrap();

    let d = srv.make_dir(root, &named("d"), dir::ALL).unwrap();
    srv.close(d).unwrap();
    assert!(names(&mut srv, root).contains(&("d".into(), true)));
    let d = srv
        .open_dir(root, &named("d"), dir::ALL)
        .expect("a made directory opens");
    srv.close(d).unwrap();

    srv.rename(root, &named("b"), root, &named("c")).unwrap();
    let after = names(&mut srv, root);
    assert!(after.contains(&("c".into(), false)) && !after.iter().any(|(n, _)| n == "b"));
    assert_eq!(
        srv.open_file_at(root, &named("b")).err().map(|e| e.errno),
        Some(ENOENT)
    );

    srv.unlink(root, &named("c")).unwrap();
    srv.rmdir(root, &named("d")).unwrap();
    assert_eq!(names(&mut srv, root), [("a".into(), false)]);
    assert_eq!(
        srv.open_file_at(root, &named("c")).err().map(|e| e.errno),
        Some(ENOENT)
    );
    assert_eq!(
        srv.open_dir(root, &named("d"), dir::ALL)
            .err()
            .map(|e| e.errno),
        Some(ENOENT)
    );
}

/// **A path opens exactly what the hop-by-hop walk opens, with the same rights** (milestone 606,
/// ruling A). The walk asks each directory for `DESCEND | READ` when a file is opened for
/// reading, so the file it reaches carries `READ` and not `WRITE`, even under a grant that has
/// both; one `OPEN` with the path and `READ` in the second word must give the same handle.
#[test]
fn a_path_opens_what_the_walk_opens() {
    let mut srv = server_with_tree();
    let walked = {
        let sub = srv
            .open_dir(0, &named("sub"), dir::DESCEND | dir::READ)
            .unwrap();
        let deeper = srv
            .open_dir(sub, &named("deeper"), dir::DESCEND | dir::READ)
            .unwrap();
        let f = srv.open_file_at(deeper, &named("leaf")).unwrap();
        srv.close(sub).unwrap();
        srv.close(deeper).unwrap();
        f
    };
    let pathed = srv
        .open_file_path(0, &named("sub/deeper/leaf"), dir::READ)
        .unwrap();
    for h in [walked, pathed] {
        let mut buf = [0u8; 32];
        let n = srv.read(h, 0, &mut buf).unwrap();
        assert_eq!(&buf[..n], b"two levels down");
        assert_eq!(srv.write(h, 0, b"x").err().map(|e| e.errno), Some(EROFS));
    }
    let rw = srv
        .open_file_path(0, &named("sub/deeper/leaf"), dir::READ | dir::WRITE)
        .unwrap();
    assert_eq!(
        srv.write(rw, 0, b"T").unwrap(),
        1,
        "asked for WRITE, got it"
    );
}

/// **A path reaches nothing the walk could not** (ruling A): a step that would come up short
/// is `EPERM`, a directory without `DESCEND` hides what is below it, and every step is one
/// name, so `..`, an empty step, a leading or trailing `/` and a file in the middle are refused.
#[test]
fn a_path_reaches_nothing_the_walk_could_not() {
    let mut srv = server_with_tree();
    let read_only = srv
        .open_dir(0, &named("sub"), dir::DESCEND | dir::READ)
        .unwrap();
    let errno = |r: Result<u32>| r.err().map(|e| e.errno);
    assert_eq!(
        errno(srv.open_file_path(read_only, &named("deeper/leaf"), dir::READ | dir::WRITE)),
        Some(EPERM),
        "a step asking for more than its parent holds is refused, not narrowed"
    );
    assert!(
        srv.open_file_path(read_only, &named("deeper/leaf"), dir::READ)
            .is_ok()
    );

    let no_descend = srv.open_dir(0, &named("sub"), dir::READ).unwrap();
    assert_eq!(
        errno(srv.open_file_path(no_descend, &named("deeper/leaf"), dir::READ)),
        Some(ENOENT)
    );
    assert!(
        srv.open_file_at(no_descend, &named("inner")).is_ok(),
        "one name needs no DESCEND"
    );

    for bad in [
        "sub/../motd",
        "sub//inner",
        "/sub/inner",
        "sub/inner/",
        "sub/./inner",
    ] {
        assert_eq!(
            errno(srv.open_file_path(0, &named(bad), dir::READ)),
            Some(EINVAL),
            "{bad}"
        );
        assert_eq!(
            errno(srv.open_dir(0, &named(bad), dir::READ)),
            Some(EINVAL),
            "{bad}"
        );
    }
    assert_eq!(
        errno(srv.open_file_path(0, &named("motd/x"), dir::READ)),
        Some(ENOTDIR)
    );
    assert_eq!(
        errno(srv.open_file_path(0, &named("sub/deeper"), dir::READ)),
        Some(EISDIR)
    );
}

/// **`OPENDIR` takes a path too**, and the directory it hands back carries exactly what was
/// asked for: `ENUMERATE` lists and does not open.
#[test]
fn opendir_takes_a_path_and_narrows_to_the_request() {
    let mut srv = server_with_tree();
    let h = srv
        .open_dir(0, &named("sub/deeper"), dir::ENUMERATE)
        .unwrap();
    assert_eq!(list(&mut srv, h, 4096).unwrap(), [("leaf".into(), false)]);
    assert_eq!(
        srv.open_file_at(h, &named("leaf")).err().map(|e| e.errno),
        Some(ENOENT)
    );
    assert_eq!(
        srv.open_dir(0, &named("sub/inner"), dir::ENUMERATE)
            .err()
            .map(|e| e.errno),
        Some(ENOTDIR)
    );
}

/// **A badge the server has no window for reaches nothing** (milestone 726 (an unknown badge fails closed in subtree_scope), calef's ruling of
/// 2026-10-03). A nonzero badge at or past `CLIENT_WINDOWS` used to fold onto badge 0 and carry
/// the endpoint's whole authority; now `admit` refuses it `EBADF`, and badge 0 stays open.
#[test]
fn a_badge_with_no_window_is_refused() {
    let mut srv = server_with_tree();
    let outside = srv.open_file(&named("motd")).unwrap();
    let windows = filesystem_contract::fs::CLIENT_WINDOWS as u64;
    for badge in [windows, windows + 1, u64::MAX] {
        assert_eq!(
            srv.admit(badge, outside as u64).err().map(|e| e.errno),
            Some(EBADF),
            "badge {badge}"
        );
        assert!(srv.scoped(badge));
    }
    assert_eq!(srv.admit(0, outside as u64).unwrap(), outside);
    assert_eq!(srv.admit(windows - 1, outside as u64).unwrap(), outside);
}

/// A sync-only badge answers `SYNC` and refuses every other verb (milestone 805 (`reboot` at the
/// prompt), `fs::BIND_SYNC`), through [`Server::handle`], the dispatch the EL0 binary runs: every
/// verb from `OPEN` to `UNBIND`, on `ROOT` and on a real handle, is `EPERM` and reaches no device,
/// and `SYNC` reaches the device once and answers its count. An unbound badge's `SYNC` still needs
/// a handle with `WRITE`, as it always did.
///
/// Falsification: attested 2026-10-06. Removed the dispatch's `EPERM` arm for a sync-only badge,
/// so every other verb fell through to `admit`; the test went red on opcode 0 (`EBADF` from
/// `subtree_scope`'s backstop, not the dispatch's refusal) and green again when restored. Not
/// replayable by `script/falsifications`, which sweeps the root workspace only.
#[test]
fn a_sync_only_badge_answers_sync_and_refuses_everything_else() {
    struct Flushes(i64);
    impl crate::ServeEdges for Flushes {
        fn sync(&mut self) -> i64 {
            self.0 += 1;
            self.0
        }
    }
    let mut srv = server_with_tree();
    let file = srv.open_file(&named("motd")).unwrap() as u64;
    srv.bind_sync_only(0, 3).unwrap();
    assert!(srv.is_sync_only(3));
    let mut window = vec![0u8; filesystem_contract::fs::TRANSFER_MAX];
    window[..4].copy_from_slice(b"motd");
    let mut edges = Flushes(0);
    use filesystem_contract::fs;
    for code in 0..=fs::BIND_SYNC {
        if code == fs::SYNC {
            continue;
        }
        for handle in [fs::ROOT, file] {
            let (r0, _) = srv.handle(3, fs::req(code, handle, 4), 0, &mut window, &mut edges);
            assert_eq!(r0, -(EPERM as i64), "opcode {code} on handle {handle}");
        }
    }
    assert_eq!(edges.0, 0, "a refused verb reached the device");
    let (r0, _) = srv.handle(
        3,
        fs::req(fs::SYNC, fs::ROOT, 0),
        0,
        &mut window,
        &mut edges,
    );
    assert_eq!((r0, edges.0), (1, 1), "the flush reached the device once");
    // The badge binds nothing, as any scoped badge.
    assert!(srv.bind_sync_only(3, 4).is_err());
    // Taken back, it reaches nothing, the sync included.
    srv.unbind(0, 3).unwrap();
    assert!(!srv.is_sync_only(3));
    let (r0, _) = srv.handle(
        3,
        fs::req(fs::SYNC, fs::ROOT, 0),
        0,
        &mut window,
        &mut edges,
    );
    assert!(r0 < 0 && edges.0 == 1, "a revoked badge synced");
}

/// **A bound badge sees its grant as its root and reaches only what it minted** (milestone 606,
/// ruling D). The dispatch in the EL0 binary is `admit` on every request and `claim` on every
/// handle a verb hands back; this drives the same two calls.
#[test]
fn a_bound_badge_reaches_its_grant_and_what_it_minted() {
    let mut srv = server_with_tree();
    let grant = srv
        .open_dir(0, &named("sub"), dir::ENUMERATE | dir::READ | dir::DESCEND)
        .unwrap();
    let outside = srv.open_file(&named("motd")).unwrap();
    srv.bind(0, grant, 3).unwrap();

    let root = srv.admit(3, filesystem_contract::fs::ROOT).unwrap();
    assert_eq!(root, grant, "a bound badge's ROOT is its grant");
    let f = srv
        .open_file_path(root, &named("deeper/leaf"), dir::READ)
        .unwrap();
    srv.claim(f, 3);
    assert_eq!(
        srv.admit(3, f as u64).unwrap(),
        f,
        "what it minted, it may name"
    );
    let errno = |r: Result<u32>| r.err().map(|e| e.errno);
    assert_eq!(
        errno(srv.admit(3, outside as u64)),
        Some(EBADF),
        "another's handle"
    );
    assert_eq!(
        errno(srv.admit(3, grant as u64)),
        Some(EBADF),
        "the root by its number"
    );
    assert_eq!(
        srv.admit(2, outside as u64).unwrap(),
        outside,
        "an unbound badge is open"
    );

    let other = srv.open_dir(0, &named("other"), dir::READ).unwrap();
    assert_eq!(
        srv.bind(3, other, 4).err().map(|e| e.errno),
        Some(EPERM),
        "bound binds"
    );
    assert_eq!(
        srv.bind(0, other, 3).err().map(|e| e.errno),
        Some(EPERM),
        "bound twice"
    );

    srv.unbind(0, 3).unwrap();
    assert_eq!(
        errno(srv.admit(3, filesystem_contract::fs::ROOT)),
        Some(EBADF),
        "revoked"
    );
    assert_eq!(errno(srv.admit(3, f as u64)), Some(EBADF));
    let mut buf = [0u8; 8];
    assert_eq!(
        srv.read(f, 0, &mut buf).err().map(|e| e.errno),
        Some(EBADF),
        "closed"
    );
}

/// A write whose end does not fit in 64 bits is refused `EFBIG`, POSIX's answer, and the server
/// lives (found by the `redoxfs_server_session` fuzz target, `lane/fuzz-service-handlers`). The
/// engine computes `offset + len` unchecked; an offset near `u64::MAX` wrapped it and the
/// inline-data path sliced backwards and panicked, so one `WRITE` killed the file server.
/// Falsification: attested 2026-10-04. Drop the `checked_add` guard from `Server::write`; the
/// engine panicked in `Transaction::write_node_inner` (`vendor/redoxfs/src/transaction.rs`), at its
/// `offset + buf.len()` addition. Not swept (separate workspace, see
/// script/falsifications BUGS); replay by hand with the edit and `cargo test --manifest-path
/// redoxfs_server/Cargo.toml --lib <this test>`.
#[test]
fn a_write_past_the_last_offset_is_refused() {
    let mut srv = server_with_tree();
    let f = srv
        .open_file_path(0, &named("motd"), dir::READ | dir::WRITE)
        .unwrap();
    for offset in [u64::MAX, u64::MAX - 3, u64::MAX - 6] {
        assert_eq!(
            srv.write(f, offset, b"overflow").err().map(|e| e.errno),
            Some(EFBIG),
            "offset {offset:#x}"
        );
    }
    let mut buf = [0u8; 16];
    let n = srv.read(f, 0, &mut buf).unwrap();
    assert_eq!(
        &buf[..n],
        b"root motd",
        "the file is unchanged and the server serves"
    );
}

/// **A file never reaches past what the engine's node tree can address** (found by the
/// `redoxfs_server_session` fuzz target, `lane/fuzz-service-handlers`). Upstream's
/// `NodeLevel::new` admits twelve level-4 entries where `NodeLevelData::level4` has eight, so a
/// write ending in that band, or a read in it after a truncate grew the file there, indexed past
/// the array and killed the server. Both are refused `EFBIG` at [`MAX_FILE_END`].
/// Falsification: attested 2026-10-04. Define `MAX_FILE_END` but drop its two checks; the engine
/// panicked in `Transaction::sync_node_record_ptr` (`vendor/redoxfs/src/transaction.rs`). Not swept
/// (separate workspace, see script/falsifications BUGS); replay by hand with the edit and `cargo test --manifest-path
/// redoxfs_server/Cargo.toml --lib <this test>`.
#[test]
fn a_file_never_reaches_past_the_tree() {
    let mut srv = server_with_tree();
    let f = srv
        .open_file_path(0, &named("motd"), dir::READ | dir::WRITE)
        .unwrap();
    let errno = |r: Result<()>| r.err().map(|e| e.errno);
    // Just past the tree for this build's 8 KiB records: the band that panicked.
    let band = (MAX_FILE_END / BLOCK as u64) * 8192 + 8192;
    assert_eq!(
        errno(srv.write(f, band, b"x").map(|_| ())),
        Some(EFBIG),
        "a write in the band"
    );
    assert_eq!(
        errno(srv.truncate(f, band)),
        Some(EFBIG),
        "a truncate into it"
    );
    assert_eq!(
        errno(srv.write(f, MAX_FILE_END, b"x").map(|_| ())),
        Some(EFBIG),
        "one byte past the end"
    );
    srv.truncate(f, MAX_FILE_END).unwrap();
    assert_eq!(
        srv.fstat(f).unwrap(),
        MAX_FILE_END,
        "the end itself is a size"
    );
}

/// **A grant's root cannot be closed out from under its binding** (found by the
/// `redoxfs_server_session` fuzz target's content rule, `lane/fuzz-service-handlers`). Handles
/// are per server, not per client, so an open client could `CLOSE` the handle a bound badge's
/// `ROOT` resolves to; the next open anywhere then reused the slot, and the bound badge's `ROOT`
/// named that object instead, outside its grant. Closing it is refused `EINVAL`, as closing
/// `ROOT` is, until `UNBIND` takes the grant back and closes it itself.
/// Falsification: attested 2026-10-04. Drop `|| self.is_grant_root(handle)` from `Server::close`; the test read "root motd" through the bound badge. Not swept (separate workspace, see
/// script/falsifications BUGS); replay by hand with the edit and `cargo test --manifest-path
/// redoxfs_server/Cargo.toml --lib <this test>`.
#[test]
fn a_grant_root_cannot_be_closed_while_bound() {
    let mut srv = server_with_tree();
    let grant = srv
        .open_dir(0, &named("sub"), dir::ENUMERATE | dir::READ | dir::DESCEND)
        .unwrap();
    srv.bind(0, grant, 3).unwrap();
    assert_eq!(
        srv.close(grant).err().map(|e| e.errno),
        Some(EINVAL),
        "an open client closed a bound badge's root"
    );
    let other = srv.open_file(&named("motd")).unwrap();
    assert_ne!(other, grant, "the grant's slot was reused");
    let root = srv.admit(3, filesystem_contract::fs::ROOT).unwrap();
    let mut buf = [0u8; 16];
    assert_ne!(
        srv.read(root, 0, &mut buf).map(|n| buf[..n].to_vec()).ok(),
        Some(b"root motd".to_vec()),
        "the bound badge read a file outside its grant"
    );
    srv.unbind(0, 3).unwrap();
    assert_eq!(
        srv.close(grant).err().map(|e| e.errno),
        Some(EBADF),
        "UNBIND closed the root itself"
    );
}

/// **No path goes through a symbolic link, and no open lands on one** (ruling D): `sub/link`
/// points at `../other`, and following it would leave a `sub` grant.
#[test]
fn a_symbolic_link_is_never_followed_or_opened() {
    let disk = DiskMemory::new(16 * 1024 * 1024);
    let mut fs = FileSystem::create(disk, None, 0, 0).expect("create");
    fs.tx(|tx| {
        let sub = tx.create_node(TreePtr::root(), "sub", Node::MODE_DIR | 0o755, 0, 0)?;
        let other = tx.create_node(TreePtr::root(), "other", Node::MODE_DIR | 0o755, 0, 0)?;
        let secret = tx.create_node(other.ptr(), "secret", Node::MODE_FILE | 0o644, 0, 0)?;
        tx.write_node(secret.ptr(), 0, b"in the sibling", 0, 0)?;
        let link = tx.create_node(sub.ptr(), "link", Node::MODE_SYMLINK | 0o777, 0, 0)?;
        tx.write_node(link.ptr(), 0, b"../other", 0, 0)?;
        Ok(())
    })
    .expect("populate");
    let mut srv = Server::open(fs.disk).expect("open");
    let errno = |r: Result<u32>| r.err().map(|e| e.errno);
    let eloop = subtree_scope::Refusal::Symlink.errno();
    assert_eq!(
        errno(srv.open_file_path(0, &named("sub/link/secret"), dir::READ)),
        Some(eloop)
    );
    assert_eq!(
        errno(srv.open_file_path(0, &named("sub/link"), dir::READ)),
        Some(eloop)
    );
    assert_eq!(
        errno(srv.open_dir(0, &named("sub/link"), dir::READ)),
        Some(eloop)
    );
}

/// **A name found without a listing is forgotten when the namespace changes** (milestone 606):
/// opened once (so the memo holds it), removed, and made again as a directory, it answers as
/// the image does at each step and never as it did.
#[test]
fn a_name_found_without_a_listing_follows_changes() {
    let root = filesystem_contract::fs::ROOT as u32;
    let mut srv = server_with(&[("a", b"alpha")]);
    let h = srv.open_file_at(root, &named("a")).unwrap();
    srv.close(h).unwrap();
    let h = srv
        .open_file_at(root, &named("a"))
        .expect("a remembered name opens again");
    srv.close(h).unwrap();
    srv.unlink(root, &named("a")).unwrap();
    assert_eq!(
        srv.open_file_at(root, &named("a")).err().map(|e| e.errno),
        Some(ENOENT)
    );
    let d = srv.make_dir(root, &named("a"), dir::ALL).unwrap();
    srv.close(d).unwrap();
    assert_eq!(
        srv.open_file_at(root, &named("a")).err().map(|e| e.errno),
        Some(EISDIR)
    );
    let d = srv
        .open_dir(root, &named("a"), dir::ALL)
        .expect("the new directory opens");
    srv.close(d).unwrap();
}

/// **A memoized listing answers a lookup exactly as `find_node` would**: absent is `ENOENT`, a
/// directory is not a file and a file is not a directory.
#[test]
fn a_memoized_listing_answers_lookups_as_the_engine_does() {
    let root = filesystem_contract::fs::ROOT as u32;
    let mut srv = server_with(&[("a", b"alpha")]);
    let d = srv.make_dir(root, &named("d"), dir::ALL).unwrap();
    srv.close(d).unwrap();
    names(&mut srv, root); // memoize
    assert_eq!(
        srv.open_file_at(root, &named("nope"))
            .err()
            .map(|e| e.errno),
        Some(ENOENT)
    );
    assert_eq!(
        srv.open_file_at(root, &named("d")).err().map(|e| e.errno),
        Some(EISDIR)
    );
    assert_eq!(
        srv.open_dir(root, &named("a"), dir::ALL)
            .err()
            .map(|e| e.errno),
        Some(ENOTDIR)
    );
    assert_eq!(
        srv.open_dir(root, &named("nope"), dir::ALL)
            .err()
            .map(|e| e.errno),
        Some(ENOENT)
    );
}

/// **A memoized file's bytes follow every write and truncate**, through another handle too,
/// and `FSTAT` follows with them.
#[test]
fn memoized_bytes_follow_writes_and_truncates() {
    let mut srv = server_with(&[("f", b"hello world")]);
    let reader = srv.open_file(&named("f")).unwrap();
    let mut buf = [0u8; 64];
    let n = srv.read(reader, 0, &mut buf).unwrap();
    assert_eq!(&buf[..n], b"hello world");

    let writer = srv.open_file(&named("f")).unwrap();
    srv.write(writer, 0, b"HELLO").unwrap();
    let n = srv.read(reader, 0, &mut buf).unwrap();
    assert_eq!(
        &buf[..n],
        b"HELLO world",
        "a read after a write saw the old bytes"
    );

    srv.truncate(writer, 4).unwrap();
    assert_eq!(srv.fstat(reader).unwrap(), 4);
    let n = srv.read(reader, 0, &mut buf).unwrap();
    assert_eq!(
        &buf[..n],
        b"HELL",
        "a read after a truncate saw the old length"
    );
    assert_eq!(srv.read(reader, 4, &mut buf).unwrap(), 0);
}

/// **The keystone: descending hands back a capability, and it is a capability to that directory
/// and nothing else.** A handle to `sub` opens `inner` and does not open `motd`, which is in the
/// parent, or `secret`, which is in a sibling. Neither is a permission failure: under `sub`
/// there is no such name.
#[test]
fn a_child_directory_handle_names_only_what_is_in_it() {
    let mut srv = server_with_tree();
    let sub = srv
        .open_dir(
            filesystem_contract::fs::ROOT as u32,
            &named("sub"),
            dir::ALL,
        )
        .unwrap();

    let inner = srv
        .open_file_at(sub, &named("inner"))
        .expect("its own file opens");
    let mut buf = [0u8; 64];
    let n = srv.read(inner, 0, &mut buf).unwrap();
    assert_eq!(&buf[..n], b"inside the grant");

    for absent in ["motd", "other", "secret"] {
        assert_eq!(
            srv.open_file_at(sub, &named(absent)).err().map(|e| e.errno),
            Some(syscall::error::ENOENT),
            "{absent} is not under `sub`, so naming it must find nothing",
        );
    }
    // And the same for descending: the sibling directory is real and is not reachable from here.
    assert_eq!(
        srv.open_dir(sub, &named("other"), dir::ALL)
            .err()
            .map(|e| e.errno),
        Some(syscall::error::ENOENT),
    );
    // `..` never resolves, at any rights, because it is not a component this contract accepts.
    assert_eq!(
        srv.open_dir(sub, &named(".."), dir::ALL)
            .err()
            .map(|e| e.errno),
        Some(EINVAL),
    );
}

/// **A child's rights can never exceed its parent's**, and asking for more is refused rather
/// than quietly granted less. Both halves matter: the refusal is the truthfulness, and the
/// attenuation underneath it is the safety.
#[test]
fn a_child_directory_cannot_carry_a_right_the_parent_lacked() {
    let mut srv = server_with_tree();
    // A parent that may descend and read, and may not create, write or remove.
    let narrow = dir::DESCEND | dir::READ | dir::ENUMERATE;
    let sub = srv
        .open_dir(filesystem_contract::fs::ROOT as u32, &named("sub"), narrow)
        .unwrap();

    assert_eq!(
        srv.open_dir(sub, &named("deeper"), narrow | dir::CREATE)
            .err()
            .map(|e| e.errno),
        Some(EPERM),
        "asking for a right the parent lacks must be refused, not silently narrowed",
    );
    // Asking for what the parent has, or less, works, and the child is bounded either way.
    let deeper = srv.open_dir(sub, &named("deeper"), narrow).unwrap();
    assert_eq!(
        srv.create_file_at(deeper, &named("smuggled"))
            .err()
            .map(|e| e.errno),
        Some(EROFS),
        "a grandchild must not have gained the create right the root grant withheld",
    );
    // And the parent itself still cannot create, so the child did not come from a wider parent.
    assert_eq!(
        srv.create_file_at(sub, &named("smuggled"))
            .err()
            .map(|e| e.errno),
        Some(EROFS),
    );
}

/// The rungs are separable, each gating exactly its own verb. Written as a sweep rather than
/// one test per rung because the interesting failure is a verb that answers to the *wrong*
/// right, and that only shows up when every combination is tried against every verb. Grew from
/// five rungs to eight entries with milestone 47's `touch` (DECISIONS §112): `mtime` proves
/// `READ` gates the getter, `settime_now` proves `WRITE` alone is enough for "now", and
/// `settime_at` proves `WRITE` alone is *not* enough for "arbitrary" even though the previous
/// entry just showed it satisfied for "now"; the pair is what makes DECISIONS §112's "no, they
/// are not the same right" non-vacuous rather than asserted.
#[test]
fn each_rung_of_the_ladder_gates_exactly_its_own_verb() {
    // `withheld` is what the failing capability lacks and `needed` is what the passing one has.
    // They differ for `open`, which takes READ **or** WRITE, so withholding only READ would
    // leave it reachable and the sweep would be asserting the wrong thing (it did, first time).
    for &(withheld, needed, ok, refusal) in &[
        (dir::ENUMERATE, dir::ENUMERATE, "enumerate", EPERM),
        (
            dir::READ | dir::WRITE,
            dir::READ,
            "open",
            syscall::error::ENOENT,
        ),
        (dir::CREATE, dir::CREATE, "create", EROFS),
        (
            dir::DESCEND,
            dir::DESCEND,
            "descend",
            syscall::error::ENOENT,
        ),
        // `rm`'s rung. Until `unlink` existed only `rename` consulted REMOVE, and a rename
        // needs CREATE as well, so this is the first probe that isolates this one.
        (dir::REMOVE, dir::REMOVE, "unlink", EROFS),
        // `touch`'s three rungs (milestone 47, DECISIONS §112). `mtime` is a naming right like
        // `open`'s, so its refusal is `ENOENT` rather than `EROFS`: a holder that may not read
        // a name's time must not learn whether the name is there.
        (dir::READ, dir::READ, "mtime", syscall::error::ENOENT),
        // Bare `touch`: WRITE alone, and nothing more, is what DECISIONS §112 says it should
        // take. `settime_at` below reuses this same "without" (WRITE present, SETTIME absent)
        // to prove the reverse: WRITE does not smuggle in the arbitrary-time authority.
        (dir::WRITE, dir::WRITE, "settime_now", EROFS),
        // `touch -t`: this is the rung DECISIONS §112 added, and the interesting failure this
        // sweep exists to catch is WRITE alone being enough, which the entry above's "without"
        // side already rules out for `settime_now`. Isolating SETTIME here (withheld is just
        // SETTIME, so `without` still carries WRITE) proves the arbitrary half needs the extra
        // grant even when the plain half is satisfied.
        (dir::SETTIME, dir::WRITE | dir::SETTIME, "settime_at", EROFS),
    ] {
        let mut srv = server_with_tree();
        // Everything except the rung under test, so a verb that consults the wrong bit passes
        // when it should fail.
        let without = srv
            .open_dir(
                filesystem_contract::fs::ROOT as u32,
                &named("sub"),
                dir::ALL & !withheld,
            )
            .unwrap();
        let with = srv
            .open_dir(
                filesystem_contract::fs::ROOT as u32,
                &named("sub"),
                needed | dir::READ,
            )
            .unwrap();

        let attempt = |srv: &mut Server<DiskMemory>, h: u32| -> Result<()> {
            match ok {
                "enumerate" => srv.read_dir(h, 0, &mut [0u8; 256]).map(|_| ()),
                "open" => srv.open_file_at(h, &named("inner")).map(|_| ()),
                "create" => srv
                    .create_file_at(h, &named("made-by-the-sweep"))
                    .map(|_| ()),
                "unlink" => srv.unlink(h, &named("inner")),
                "mtime" => srv.mtime(h, &named("inner")).map(|_| ()),
                "settime_now" => srv.set_mtime_now(h, &named("inner")),
                "settime_at" => srv.set_mtime_at(h, &named("inner"), 1_700_000_000),
                _ => srv.open_dir(h, &named("deeper"), 0).map(|_| ()),
            }
        };
        assert_eq!(
            attempt(&mut srv, without).err().map(|e| e.errno),
            Some(refusal),
            "{ok} was allowed by a capability that lacked its own right",
        );
        assert!(
            attempt(&mut srv, with).is_ok(),
            "{ok} was refused by a capability that carried its right, so the refusal above \
             proves nothing",
        );
    }
}

/// **`unlink` removes a name and not an object**, which is the half of `rm` Unix conflated and
/// milestone 47 insists on separating. Three claims in one test because they are one property:
/// the name goes, the holder keeps reading, and the bytes it reads are still the right ones.
///
/// This is the test that decided the implementation. RedoxFS frees a node the instant its last
/// link goes, so the first version of this verb was a *revoke*: the read below returned garbage.
/// Registering an open node with the engine's usage table is what turns it into an unlink, and
/// deleting that one line turns this test red.
#[test]
fn unlink_takes_the_name_and_leaves_the_object_for_whoever_holds_it() {
    let mut srv = server_with_tree();
    let sub = srv
        .open_dir(
            filesystem_contract::fs::ROOT as u32,
            &named("sub"),
            dir::ALL,
        )
        .unwrap();

    let h = srv.create_file_at(sub, &named("doomed")).unwrap();
    let body = b"the bytes outlive the name";
    assert_eq!(srv.write(h, 0, body).unwrap(), body.len());

    srv.unlink(sub, &named("doomed")).expect("unlink");

    // The name is gone: from an open, and from a listing.
    assert_eq!(
        srv.open_file_at(sub, &named("doomed"))
            .err()
            .map(|e| e.errno),
        Some(syscall::error::ENOENT),
        "the name must be gone the moment it is unlinked",
    );
    let names: Vec<String> = list(&mut srv, sub, 256)
        .unwrap()
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    assert!(!names.contains(&"doomed".to_string()), "listing: {names:?}");

    // And the object is not: the handle opened before the unlink still reads its bytes.
    let mut buf = [0u8; 64];
    let n = srv.read(h, 0, &mut buf).expect("a holder keeps reading");
    assert_eq!(
        &buf[..n],
        body,
        "unlink removed the object, not just the name: this verb is a revoke wearing an \
         unlink's name",
    );

    // The close is what finally collects it, and it must not fail on a node whose name is gone.
    srv.close(h).expect("close after unlink");

    // The same property through the OTHER door. `create` and `open` register the node
    // separately, so a test that only creates would leave half the verb unproven; deleting the
    // registration in `open_file_at` alone left this test green the first time it was run.
    let h = srv
        .open_file_at(sub, &named("inner"))
        .expect("open an existing file");
    srv.unlink(sub, &named("inner"))
        .expect("unlink what was opened");
    let n = srv
        .read(h, 0, &mut buf)
        .expect("an opener keeps reading too");
    assert_eq!(&buf[..n], b"inside the grant");
    srv.close(h).unwrap();
}

/// **`RMDIR` takes an empty directory and refuses a non-empty one**, which is the whole safety
/// property: `mkdir` now has an inverse, and no single call on this contract can take a subtree
/// away.
///
/// The pair is what makes it mean anything. A `rmdir` that refused everything would satisfy
/// "non-empty is refused" perfectly, so the same directory is emptied by hand and removed
/// through the same call, and the name really is gone from the listing afterwards.
#[test]
fn rmdir_removes_an_empty_directory_and_refuses_one_with_a_name_in_it() {
    let mut srv = server_with_tree();
    let sub = srv
        .open_dir(
            filesystem_contract::fs::ROOT as u32,
            &named("sub"),
            dir::ALL,
        )
        .unwrap();

    // `deeper` holds `leaf`, so this is exactly the subtree-in-one-word case.
    assert_eq!(
        srv.rmdir(sub, &named("deeper")).err().map(|e| e.errno),
        Some(syscall::error::ENOTEMPTY),
        "a `rmdir` that emptied what it found would put a whole subtree behind one call",
    );
    let names: Vec<String> = list(&mut srv, sub, 256)
        .unwrap()
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    assert!(
        names.contains(&"deeper".to_string()),
        "the refused directory is still there: {names:?}",
    );

    // Empty it the way `rm -r` does: from the bottom, one safe step at a time.
    let deeper = srv.open_dir(sub, &named("deeper"), dir::ALL).unwrap();
    srv.unlink(deeper, &named("leaf")).expect("unlink the leaf");
    srv.close(deeper).expect("close before removing the name");
    srv.rmdir(sub, &named("deeper"))
        .expect("an empty directory goes");

    let names: Vec<String> = list(&mut srv, sub, 256)
        .unwrap()
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    assert!(!names.contains(&"deeper".to_string()), "listing: {names:?}");
    assert_eq!(
        srv.open_dir(sub, &named("deeper"), dir::ALL)
            .err()
            .map(|e| e.errno),
        Some(syscall::error::ENOENT),
        "the name must be gone from the parent, not merely absent from a listing",
    );
}

/// `RMDIR`'s refusals, each of which is a fact about the thing or about the capability.
///
/// A **file** is `ENOTDIR`, the mirror of `unlink`'s `EISDIR`: neither verb removes the kind the
/// other one is for, so there is no spelling of "remove this name whatever it is". And the right
/// is checked before the name is resolved, so a capability that may not remove cannot use this
/// verb as an existence oracle either.
#[test]
fn rmdir_refuses_a_file_a_missing_name_a_path_and_a_capability_without_remove() {
    let mut srv = server_with_tree();
    let sub = srv
        .open_dir(
            filesystem_contract::fs::ROOT as u32,
            &named("sub"),
            dir::ALL,
        )
        .unwrap();

    assert_eq!(
        srv.rmdir(sub, &named("inner")).err().map(|e| e.errno),
        Some(ENOTDIR),
        "`rmdir` of a file must be refused: `unlink` is the verb for one",
    );
    assert_eq!(
        srv.rmdir(sub, &named("never-existed"))
            .err()
            .map(|e| e.errno),
        Some(syscall::error::ENOENT),
    );
    for path in ["..", "a/b", "."] {
        assert_eq!(
            srv.rmdir(sub, &named(path)).err().map(|e| e.errno),
            Some(EINVAL),
            "`{path}` is not a component, so it cannot name a directory to remove",
        );
    }

    let ro = srv
        .open_dir(
            filesystem_contract::fs::ROOT as u32,
            &named("sub"),
            dir::ALL & !dir::REMOVE,
        )
        .unwrap();
    assert_eq!(
        srv.rmdir(ro, &named("deeper")).err().map(|e| e.errno),
        Some(EROFS)
    );
    assert_eq!(
        srv.rmdir(ro, &named("never-existed"))
            .err()
            .map(|e| e.errno),
        Some(EROFS),
        "a name that is there and one that is not must answer the same word",
    );
}

/// **The whole of `rm -r`, driven against the core**: walk, unlink the files, remove the empty
/// directories bottom-up. It is here because that is the shape `components/src/rm.rs` implements over
/// IPC, and proving the *order* is safe belongs next to the verbs rather than only in a guest
/// test that takes an emulator to run.
///
/// The load-bearing assertion is the second one: **the bottom-up order is forced.** Removing
/// `deeper` before its `leaf` is `ENOTEMPTY`, so a walk that got the order wrong cannot pass by
/// accident, and there is no call that would have let it.
#[test]
fn a_recursive_removal_is_a_loop_of_individually_safe_steps() {
    let mut srv = server_with_tree();
    let root = filesystem_contract::fs::ROOT as u32;

    assert_eq!(
        srv.rmdir(root, &named("sub")).err().map(|e| e.errno),
        Some(syscall::error::ENOTEMPTY),
        "top-down is refused at the first step, which is what makes the walk necessary",
    );

    // Depth first, exactly as the program does it.
    let sub = srv.open_dir(root, &named("sub"), dir::ALL).unwrap();
    let deeper = srv.open_dir(sub, &named("deeper"), dir::ALL).unwrap();
    for (name, is_dir) in list(&mut srv, deeper, 256).unwrap() {
        assert!(!is_dir, "the fixture's bottom level holds only files");
        srv.unlink(deeper, &named(&name))
            .expect("unlink at the bottom");
    }
    srv.close(deeper).unwrap();
    srv.rmdir(sub, &named("deeper")).expect("now it is empty");
    for (name, is_dir) in list(&mut srv, sub, 256).unwrap() {
        assert!(!is_dir, "`deeper` was the only directory in `sub`");
        srv.unlink(sub, &named(&name)).expect("unlink one level up");
    }
    srv.close(sub).unwrap();
    srv.rmdir(root, &named("sub"))
        .expect("and the top goes last");

    let names: Vec<String> = list(&mut srv, root, 256)
        .unwrap()
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    assert!(!names.contains(&"sub".to_string()), "root holds {names:?}");
    // And nothing outside the walk moved. The walk removes what it enumerated and nothing else,
    // which is the property the guest test then makes about a *capability* rather than a loop.
    assert!(names.contains(&"other".to_string()));
    assert!(names.contains(&"motd".to_string()));
}

/// The refusals, which are what stop `rm` from meaning more than it says. A directory is
/// `EISDIR` ([`Server::rmdir`] is the verb for one, and it takes only an empty one), a
/// missing name is `ENOENT`, and a path is `EINVAL` exactly as it is everywhere else here.
#[test]
fn unlink_refuses_a_directory_a_missing_name_and_a_path() {
    let mut srv = server_with_tree();
    let sub = srv
        .open_dir(
            filesystem_contract::fs::ROOT as u32,
            &named("sub"),
            dir::ALL,
        )
        .unwrap();

    assert_eq!(
        srv.unlink(sub, &named("deeper")).err().map(|e| e.errno),
        Some(EISDIR),
        "`rm` of a directory must be refused, not silently recursive",
    );
    assert_eq!(
        srv.unlink(sub, &named("never-existed"))
            .err()
            .map(|e| e.errno),
        Some(syscall::error::ENOENT),
    );
    for path in ["..", "a/b", "."] {
        assert_eq!(
            srv.unlink(sub, &named(path)).err().map(|e| e.errno),
            Some(EINVAL),
            "`{path}` is not a component, so it cannot name anything to remove",
        );
    }
    // The right is checked BEFORE the name is resolved, so a capability that may not remove
    // cannot use this verb as an existence oracle: a name that is there and one that is not
    // must answer the same word.
    let ro = srv
        .open_dir(
            filesystem_contract::fs::ROOT as u32,
            &named("sub"),
            dir::ALL & !dir::REMOVE,
        )
        .unwrap();
    assert_eq!(
        srv.unlink(ro, &named("inner")).err().map(|e| e.errno),
        Some(EROFS),
    );
    assert_eq!(
        srv.unlink(ro, &named("never-existed"))
            .err()
            .map(|e| e.errno),
        Some(EROFS),
    );
}

/// **A file handle inherits its directory's direction**, which is the "open (read versus write)"
/// rung. A directory granted for writing yields a file that writes and, deliberately, does not
/// read; the reverse yields one that reads and refuses to write or truncate.
#[test]
fn a_file_handle_carries_the_direction_of_the_directory_it_came_from() {
    let mut srv = server_with_tree();

    let ro = srv
        .open_dir(
            filesystem_contract::fs::ROOT as u32,
            &named("sub"),
            dir::DESCEND | dir::READ,
        )
        .unwrap();
    let h = srv.open_file_at(ro, &named("inner")).unwrap();
    let mut buf = [0u8; 32];
    assert!(srv.read(h, 0, &mut buf).is_ok());
    assert_eq!(
        srv.write(h, 0, b"x").err().map(|e| e.errno),
        Some(EROFS),
        "a file opened under a read-only directory must not write",
    );
    assert_eq!(
        srv.truncate(h, 0).err().map(|e| e.errno),
        Some(EROFS),
        "and must not truncate either: a truncate is a write that carries no bytes",
    );

    let wo = srv
        .open_dir(
            filesystem_contract::fs::ROOT as u32,
            &named("sub"),
            dir::DESCEND | dir::WRITE,
        )
        .unwrap();
    let h = srv.open_file_at(wo, &named("inner")).unwrap();
    assert!(srv.write(h, 0, b"appended by a write-only grant").is_ok());
    assert_eq!(
        srv.read(h, 0, &mut buf).err().map(|e| e.errno),
        Some(EBADF),
        "a write-only handle is not a readable one, which is POSIX's own answer",
    );
    assert!(srv.fstat(h).is_ok(), "but it can still find the end");
}

/// A listing is a rendering of authority: it holds exactly the granted directory's names, never
/// the parent's or the sibling's, and it says which are directories so a reader need not open
/// every one to find out.
#[test]
fn a_listing_holds_exactly_the_names_in_that_directory() {
    let mut srv = server_with_tree();
    let sub = srv
        .open_dir(
            filesystem_contract::fs::ROOT as u32,
            &named("sub"),
            dir::ALL,
        )
        .unwrap();

    let mut got = list(&mut srv, sub, 256).expect("enumerate");
    got.sort();
    assert_eq!(
        got,
        vec![("deeper".to_string(), true), ("inner".to_string(), false)],
        "the listing must be the granted directory's names and their kinds",
    );

    // The root's own listing has the sibling in it, which is what makes the assertion above a
    // statement about the capability rather than about an empty directory.
    let mut root =
        list(&mut srv, filesystem_contract::fs::ROOT as u32, 256).expect("enumerate the root");
    root.sort();
    assert!(root.iter().any(|(n, d)| n == "other" && *d));
}

/// **A listing bigger than the buffer comes back across several calls, whole records only.**
/// The cursor is what makes that work, and a buffer sized to cut a record in half is exactly the
/// case that would otherwise hand a client half a name.
#[test]
fn a_listing_larger_than_the_buffer_is_paged_without_tearing_a_name() {
    let mut srv = server_with_tree();
    let root = filesystem_contract::fs::ROOT as u32;
    for i in 0..12 {
        srv.create_file_at(root, &named(&format!("entry-{i:02}-with-a-longish-name")))
            .unwrap();
    }
    let whole = list(&mut srv, root, 4096).expect("one big listing");
    // Ten bytes fits one short record and no long one, so most calls return one entry and the
    // walk only terminates if the cursor advances correctly.
    let paged = list(&mut srv, root, 40).expect("paged listing");
    assert_eq!(paged, whole, "paging changed the listing");
    assert_eq!(whole.len(), 15, "3 fixture entries plus the 12 made here");
}

/// `mkdir` is descend-with-creation: it mints a directory node and hands back a capability to it,
/// attenuated exactly as a descent is, so making a directory is not a way to mint authority.
#[test]
fn mkdir_returns_a_capability_no_wider_than_the_one_that_made_it() {
    let mut srv = server_with_tree();
    // No REMOVE, so nothing made below may remove either.
    let held = dir::DESCEND | dir::CREATE | dir::READ | dir::WRITE | dir::ENUMERATE;
    let sub = srv
        .open_dir(filesystem_contract::fs::ROOT as u32, &named("sub"), held)
        .unwrap();

    let made = srv.make_dir(sub, &named("logs"), held).expect("mkdir");
    assert_eq!(
        srv.make_dir(sub, &named("logs"), held)
            .err()
            .map(|e| e.errno),
        Some(EEXIST),
        "mkdir is create, not create-or-open",
    );
    // The capability it handed back really is a directory capability: it takes names.
    let f = srv
        .create_file_at(made, &named("today"))
        .expect("create in it");
    assert_eq!(srv.write(f, 0, b"a log line\n").unwrap(), 11);
    assert_eq!(
        srv.make_dir(sub, &named("wider"), dir::ALL)
            .err()
            .map(|e| e.errno),
        Some(EPERM),
        "and it cannot be made wider than the directory it was made in",
    );

    // The new directory shows up in its parent's listing, as a directory.
    let mut got = list(&mut srv, sub, 512).unwrap();
    got.sort();
    assert!(got.contains(&("logs".to_string(), true)));
}

/// A directory capability granted without `CREATE` is the milestone's motivating sentence: a
/// program may write into what is there and may not make or destroy anything.
#[test]
fn a_write_grant_without_create_can_write_and_cannot_make_a_name() {
    let mut srv = server_with_tree();
    let logs = srv
        .open_dir(
            filesystem_contract::fs::ROOT as u32,
            &named("sub"),
            dir::DESCEND | dir::READ | dir::WRITE,
        )
        .unwrap();
    let h = srv.open_file_at(logs, &named("inner")).unwrap();
    assert!(srv.write(h, 0, b"the program's own bytes").is_ok());
    assert_eq!(
        srv.create_file_at(logs, &named("new"))
            .err()
            .map(|e| e.errno),
        Some(EROFS),
    );
    assert_eq!(
        srv.make_dir(logs, &named("new"), 0).err().map(|e| e.errno),
        Some(EROFS),
    );
}

/// The two typed handles do not answer for each other, and the root cannot be closed. Each of
/// these is a way for a client to make the server confuse one kind of authority for another.
#[test]
fn a_directory_handle_and_a_file_handle_refuse_each_others_verbs() {
    let mut srv = server_with_tree();
    let root = filesystem_contract::fs::ROOT as u32;
    let sub = srv.open_dir(root, &named("sub"), dir::ALL).unwrap();
    let file = srv.open_file_at(sub, &named("inner")).unwrap();
    let mut buf = [0u8; 32];

    assert_eq!(
        srv.read(sub, 0, &mut buf).err().map(|e| e.errno),
        Some(EISDIR)
    );
    assert_eq!(srv.write(sub, 0, b"x").err().map(|e| e.errno), Some(EISDIR));
    assert_eq!(srv.truncate(sub, 0).err().map(|e| e.errno), Some(EISDIR));
    assert_eq!(
        srv.open_file_at(file, &named("inner"))
            .err()
            .map(|e| e.errno),
        Some(ENOTDIR),
    );
    assert_eq!(
        srv.read_dir(file, 0, &mut buf).err().map(|e| e.errno),
        Some(ENOTDIR),
    );
    assert_eq!(
        srv.open_dir(file, &named("deeper"), 0)
            .err()
            .map(|e| e.errno),
        Some(ENOTDIR),
    );
    // Opening a directory as a file is EISDIR, not a handle to bytes that mean nothing.
    assert_eq!(
        srv.open_file_at(root, &named("sub")).err().map(|e| e.errno),
        Some(EISDIR),
    );
    assert_eq!(
        srv.close(root).err().map(|e| e.errno),
        Some(EINVAL),
        "the root of a namespace is not something the client opened",
    );
    // And a forged handle is still EBADF, from the same one check every verb goes through.
    for h in [99u32, 1000] {
        assert_eq!(
            srv.read_dir(h, 0, &mut buf).err().map(|e| e.errno),
            Some(EBADF)
        );
        assert_eq!(
            srv.open_dir(h, &named("sub"), 0).err().map(|e| e.errno),
            Some(EBADF)
        );
    }
}

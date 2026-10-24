use super::*;

// --- Extended attributes (milestone 57) ---

/// The node id behind a name, so a test can talk about the thing the store keys on rather than
/// about the name that happens to point at it. This is the whole trick of the layer, so the
/// tests have to be able to see it.
fn node_id_of(srv: &mut Server<DiskMemory>, parent: TreePtr<Node>, name: &str) -> u32 {
    srv.fs
        .tx(|tx| Ok(tx.find_node(parent, name)?.id()))
        .unwrap_or_else(|e| panic!("no node behind {name}: {e:?}"))
}

/// Whether the store still holds a blob for `node_id`. Reaches past the contract deliberately:
/// a client cannot name the store, so a test that could only use the contract could not tell
/// "the attributes were purged" from "the attributes are unreachable".
fn store_holds(srv: &mut Server<DiskMemory>, node_id: u32) -> bool {
    srv.fs
        .tx(|tx| {
            let Some(dir) = find_store(tx)? else {
                return Ok(false);
            };
            let bytes = xattr::store::file_name(node_id);
            let name = core::str::from_utf8(&bytes).expect("hex is ASCII");
            Ok(tx.find_node(dir, name).is_ok())
        })
        .unwrap()
}

/// The attribute names on a handle, through the contract, in the order the store holds them.
fn attr_names(srv: &mut Server<DiskMemory>, handle: u32) -> Vec<Vec<u8>> {
    let mut page = [0u8; 4096];
    let n = srv.list_xattr(handle, &mut page).expect("list");
    filesystem_contract::xattr::list::iter(&page[..n])
        .map(|s| s.to_vec())
        .collect()
}

/// One attribute's value through the contract.
fn attr(srv: &mut Server<DiskMemory>, handle: u32, name: &[u8]) -> Result<(u32, Vec<u8>)> {
    let mut page = [0u8; 4096];
    let (kind, n) = srv.get_xattr(handle, name, &mut page)?;
    Ok((kind, page[..n].to_vec()))
}

/// **An attribute is on the disk, not in the server.** Set it, drop the mount the way a dying
/// process does, mount the same image again, and read it back. Without this the whole layer
/// could be a map in a `Server` and every other test here would still pass.
#[test]
fn an_attribute_survives_a_mount_it_was_not_written_in() {
    let disk = image_with(&[("motd", b"hello")]);
    let mut srv = Server::open(disk).unwrap();
    let h = srv.open_file(&named("motd")).unwrap();
    srv.set_xattr(h, b"user.DOSATTRIB", xattr::RAW, b"0x20")
        .expect("set");
    srv.set_xattr(h, b"user.com.apple.FinderInfo", 0x4353_5452, &[7u8; 32])
        .expect("set a typed one");
    let disk = srv.fs.disk; // no unmount, exactly as on device

    let mut srv = Server::open(disk).unwrap();
    let h = srv.open_file(&named("motd")).unwrap();
    assert_eq!(
        attr(&mut srv, h, b"user.DOSATTRIB").unwrap(),
        (xattr::RAW, b"0x20".to_vec()),
    );
    assert_eq!(
        attr(&mut srv, h, b"user.com.apple.FinderInfo").unwrap(),
        (0x4353_5452, vec![7u8; 32]),
        "the type code has to survive the platter too, or indexing is foreclosed after all",
    );
    assert_eq!(
        attr_names(&mut srv, h),
        [
            b"user.DOSATTRIB".to_vec(),
            b"user.com.apple.FinderInfo".to_vec()
        ],
    );
}

/// **The headline: a rename carries the attributes, and nothing in the rename path knows they
/// exist.**
///
/// The store keys on the node's `TreePtr`, and a rename changes a directory entry rather than a
/// node, so this works by construction. It is the correctness property that is only available
/// inside the FS server, and the one AppleDouble sidecars (and any path-keyed store) get wrong,
/// so it is pinned rather than reasoned about. The node id is asserted unchanged as well, since
/// that equality is *why* it holds.
#[test]
fn a_rename_carries_the_attributes_because_the_store_keys_on_the_node() {
    let mut srv = server_with_tree();
    let h = srv.open_file(&named("motd")).unwrap();
    srv.set_xattr(h, b"user.tag", 9, b"follow me").unwrap();
    let before = node_id_of(&mut srv, TreePtr::root(), "motd");

    // Rename within the root, then into a subdirectory: neither touches the node.
    srv.rename(
        filesystem_contract::fs::ROOT as u32,
        &named("motd"),
        filesystem_contract::fs::ROOT as u32,
        &named("motd2"),
    )
    .expect("rename in place");
    let sub = srv
        .open_dir(
            filesystem_contract::fs::ROOT as u32,
            &named("sub"),
            dir::ALL,
        )
        .unwrap();
    srv.rename(
        filesystem_contract::fs::ROOT as u32,
        &named("motd2"),
        sub,
        &named("moved"),
    )
    .expect("rename across directories");

    let after = node_id_of(&mut srv, TreePtr::root(), "sub");
    assert_ne!(
        after, before,
        "sanity: the subdirectory is a different node"
    );
    let moved = srv
        .open_file_at(sub, &named("moved"))
        .expect("open at its new name");
    assert_eq!(
        attr(&mut srv, moved, b"user.tag").unwrap(),
        (9, b"follow me".to_vec()),
        "the attribute did not follow the file across two renames",
    );
    assert!(
        store_holds(&mut srv, before),
        "and it is still the same node's blob"
    );
}

/// **Unlink takes the attributes with the file, in the same transaction**, and a node id issued
/// again afterwards inherits nothing.
///
/// This is the correctness bug wearing a housekeeping costume that DECISIONS §34's amendment
/// names: a leaked blob is not wasted space, it is somebody else's metadata attached to a file
/// that never asked for it. The reuse half is the one that makes it a bug rather than a leak, so
/// it is tested by actually provoking the reuse.
#[test]
fn unlink_takes_the_attributes_and_a_reused_node_inherits_nothing() {
    let mut srv = server_with_tree();
    let h = srv.create_file(&named("doomed")).unwrap();
    srv.set_xattr(h, b"user.secret", xattr::RAW, b"not yours")
        .unwrap();
    let doomed = node_id_of(&mut srv, TreePtr::root(), "doomed");
    assert!(store_holds(&mut srv, doomed));

    srv.close(h).unwrap(); // an open handle pins the node; close first so the id is really freed
    srv.unlink(filesystem_contract::fs::ROOT as u32, &named("doomed"))
        .unwrap();
    assert!(
        !store_holds(&mut srv, doomed),
        "the blob outlived the file it belonged to",
    );

    // **Provoke the reuse rather than reasoning about it.** RedoxFS releases a freed node id, so
    // a fresh create gets it back, and today the very first one does. The loop is bounded and
    // the miss is asserted rather than skipped: without a reuse the interesting half of this
    // test is vacuous, and a silent skip is how a test stops proving anything without saying so.
    // If a future engine stops recycling ids promptly, widen the bound; do not delete the
    // assertion.
    let mut reused = None;
    for i in 0..8 {
        let name = format!("fresh{i}");
        let nh = srv.create_file(&named(&name)).unwrap();
        if node_id_of(&mut srv, TreePtr::root(), &name) == doomed {
            reused = Some(nh);
            break;
        }
    }
    let nh = reused.expect("no create reused the freed node id, so the reuse case went untested");
    assert!(
        attr_names(&mut srv, nh).is_empty(),
        "a new file was handed the previous occupant's attributes",
    );
    assert_eq!(
        attr(&mut srv, nh, b"user.secret").unwrap_err().errno,
        xattr::ENODATA,
    );
}

/// A rename that **replaces** a destination takes the replaced file's attributes too. RedoxFS's
/// `rename_node` removes that node and throws the freed id away, so this is the one removal the
/// engine cannot tell us about and the server has to notice for itself.
#[test]
fn a_rename_over_a_file_takes_the_replaced_files_attributes() {
    let mut srv = server_with_tree();
    let victim = srv.create_file(&named("victim")).unwrap();
    srv.set_xattr(victim, b"user.doomed", xattr::RAW, b"x")
        .unwrap();
    srv.close(victim).unwrap();
    let victim_id = node_id_of(&mut srv, TreePtr::root(), "victim");

    let winner = srv.create_file(&named("winner")).unwrap();
    srv.set_xattr(winner, b"user.kept", xattr::RAW, b"y")
        .unwrap();
    let winner_id = node_id_of(&mut srv, TreePtr::root(), "winner");

    srv.rename(
        filesystem_contract::fs::ROOT as u32,
        &named("winner"),
        filesystem_contract::fs::ROOT as u32,
        &named("victim"),
    )
    .expect("replace");

    assert!(
        !store_holds(&mut srv, victim_id),
        "the replaced file's attributes outlived it",
    );
    assert!(
        store_holds(&mut srv, winner_id),
        "and the survivor kept its own"
    );
    let h = srv.open_file(&named("victim")).unwrap();
    assert_eq!(attr_names(&mut srv, h), [b"user.kept".to_vec()]);
}

/// `rmdir` purges exactly as `unlink` does, because a directory carries attributes exactly as a
/// file does. A verb that purged one kind and not the other would leak on whichever half its
/// author was not thinking about.
#[test]
fn a_directory_carries_attributes_and_rmdir_takes_them_with_it() {
    let mut srv = server_with_tree();
    let d = srv
        .make_dir(
            filesystem_contract::fs::ROOT as u32,
            &named("attrdir"),
            dir::ALL,
        )
        .unwrap();
    srv.set_xattr(d, b"user.on-a-directory", 3, b"yes").unwrap();
    let id = node_id_of(&mut srv, TreePtr::root(), "attrdir");
    assert_eq!(
        attr(&mut srv, d, b"user.on-a-directory").unwrap(),
        (3, b"yes".to_vec()),
    );

    srv.rmdir(filesystem_contract::fs::ROOT as u32, &named("attrdir"))
        .unwrap();
    assert!(
        !store_holds(&mut srv, id),
        "an empty directory left its attributes behind"
    );
}

/// The last attribute takes the store directory with it, so a filesystem holding no attributes is
/// byte-for-byte one that never held any (`redoxfs_host extract` would otherwise recover an
/// unexplained empty `.nife-attrs`).
///
/// Both ways of emptying it are exercised: `remove_xattr` writes an empty blob and `unlink` purges
/// one, and only the first goes through `write_attrs`. The middle assertion keeps it honest: while a
/// second node still has attributes the directory must stay, or an implementation that removed the
/// store every time would pass too.
#[test]
fn the_last_attribute_takes_the_store_with_it() {
    /// Whether the store directory exists at all. Reaches past the contract on purpose: no
    /// client can name it, so nothing inside the contract can tell "gone" from "hidden".
    fn store_exists(srv: &mut Server<DiskMemory>) -> bool {
        srv.fs.tx(|tx| Ok(find_store(tx)?.is_some())).unwrap()
    }

    let mut srv = server_with_tree();
    assert!(
        !store_exists(&mut srv),
        "a fresh image must not carry a store before anything sets an attribute",
    );

    let motd = srv.open_file(&named("motd")).unwrap();
    let second = srv.create_file(&named("second")).unwrap();
    srv.set_xattr(motd, b"user.a", xattr::RAW, b"1").unwrap();
    srv.set_xattr(second, b"user.b", xattr::RAW, b"2").unwrap();
    assert!(store_exists(&mut srv), "setting one must create the store");

    srv.remove_xattr(motd, b"user.a").unwrap();
    assert!(
        store_exists(&mut srv),
        "the store must stay while another node still has attributes",
    );

    srv.remove_xattr(second, b"user.b").unwrap();
    assert!(
        !store_exists(&mut srv),
        "the last attribute must take the store directory with it",
    );

    // And again by the other route: `unlink` purges a blob without going through `write_attrs`,
    // so the two paths need separate evidence.
    srv.set_xattr(motd, b"user.c", xattr::RAW, b"3").unwrap();
    assert!(store_exists(&mut srv));
    srv.unlink(filesystem_contract::fs::ROOT as u32, &named("motd"))
        .unwrap();
    srv.close(motd).unwrap();
    assert!(
        !store_exists(&mut srv),
        "unlinking the last node with attributes must take the store too",
    );
}

/// **The store is not in the namespace.** It cannot be listed, opened, descended into, created,
/// renamed or removed, in any directory, at full rights. If a client could name it, "the
/// attributes of a file" would be reachable as ordinary bytes by anything holding the directory.
#[test]
fn the_attribute_store_is_not_in_the_namespace() {
    let mut srv = server_with_tree();
    let h = srv.open_file(&named("motd")).unwrap();
    srv.set_xattr(h, b"user.x", xattr::RAW, b"1").unwrap();

    // It is really there: this test would be vacuous against a filesystem with no store at all.
    assert!(
        srv.fs.tx(|tx| Ok(find_store(tx)?.is_some())).unwrap(),
        "nothing was created, so nothing is being hidden",
    );

    let root = filesystem_contract::fs::ROOT as u32;
    let listed = list(&mut srv, root, 4096).unwrap();
    assert!(
        !listed.iter().any(|(n, _)| n == xattr::STORE_DIR),
        "a listing named the attribute store: {listed:?}",
    );
    // Every name-taking verb, at every rights this contract can carry, and the answer is always
    // the same one `..` gets: the name is not expressible here.
    assert_eq!(
        srv.open_file_at(root, &named(xattr::STORE_DIR))
            .unwrap_err()
            .errno,
        EINVAL
    );
    assert_eq!(
        srv.open_dir(root, &named(xattr::STORE_DIR), dir::ALL)
            .unwrap_err()
            .errno,
        EINVAL,
    );
    assert_eq!(
        srv.create_file_at(root, &named(xattr::STORE_DIR))
            .unwrap_err()
            .errno,
        EINVAL
    );
    assert_eq!(
        srv.make_dir(root, &named(xattr::STORE_DIR), dir::ALL)
            .unwrap_err()
            .errno,
        EINVAL,
    );
    assert_eq!(
        srv.unlink(root, &named(xattr::STORE_DIR))
            .unwrap_err()
            .errno,
        EINVAL
    );
    assert_eq!(
        srv.rmdir(root, &named(xattr::STORE_DIR)).unwrap_err().errno,
        EINVAL
    );
    assert_eq!(
        srv.rename(root, &named("motd"), root, &named(xattr::STORE_DIR))
            .unwrap_err()
            .errno,
        EINVAL,
    );
    // And it is reserved in a *subdirectory* too, so the rule is one sentence rather than a
    // location-dependent one.
    let sub = srv.open_dir(root, &named("sub"), dir::ALL).unwrap();
    assert_eq!(
        srv.create_file_at(sub, &named(xattr::STORE_DIR))
            .unwrap_err()
            .errno,
        EINVAL
    );
}

/// **An attribute takes the direction the capability carries**, and no new rung was added to the
/// ladder to say so: reading one needs what reading the file needs, changing one needs what
/// writing the file needs, and each is refused with the word that verb already answers.
#[test]
fn attributes_take_the_direction_the_capability_already_carries() {
    let mut srv = server_with_tree();

    // A read-only directory, so the file handle under it inherits read and not write.
    let ro = srv
        .open_dir(
            filesystem_contract::fs::ROOT as u32,
            &named("sub"),
            dir::READ | dir::DESCEND,
        )
        .unwrap();
    let h = srv.open_file_at(ro, &named("inner")).unwrap();
    let mut page = [0u8; 4096];
    assert_eq!(
        srv.list_xattr(h, &mut page).unwrap(),
        0,
        "reading is allowed"
    );
    assert_eq!(
        srv.set_xattr(h, b"user.x", xattr::RAW, b"1")
            .unwrap_err()
            .errno,
        EROFS,
        "through this capability the file is read-only, and so are its attributes",
    );
    assert_eq!(srv.remove_xattr(h, b"user.x").unwrap_err().errno, EROFS);

    // And the mirror: a write-only directory's file handle may set and may not read, which is
    // `EBADF` because it is a fact about the handle rather than about the capability.
    let wo = srv
        .open_dir(
            filesystem_contract::fs::ROOT as u32,
            &named("sub"),
            dir::WRITE | dir::DESCEND,
        )
        .unwrap();
    let h = srv.open_file_at(wo, &named("inner")).unwrap();
    srv.set_xattr(h, b"user.x", xattr::RAW, b"1")
        .expect("writing is allowed");
    assert_eq!(
        srv.get_xattr(h, b"user.x", &mut page).unwrap_err().errno,
        EBADF
    );
    assert_eq!(srv.list_xattr(h, &mut page).unwrap_err().errno, EBADF);
    // A handle that was never minted is EBADF here as it is everywhere else.
    assert_eq!(srv.list_xattr(999, &mut page).unwrap_err().errno, EBADF);
}

/// **Every ceiling refuses at the server boundary with its own word**, so a client can act on
/// which one it hit. DECISIONS §42's rule is that an offered verb fails loudly rather than
/// degrading; a store that clipped a value would hand back a file that looked intact and was not.
#[test]
fn every_ceiling_refuses_loudly_and_with_its_own_word() {
    let mut srv = server_with_tree();
    let h = srv.open_file(&named("motd")).unwrap();
    let mut page = [0u8; 4096];

    assert_eq!(
        srv.set_xattr(h, b"", xattr::RAW, b"v").unwrap_err().errno,
        xattr::ERANGE,
    );
    assert_eq!(
        srv.set_xattr(h, &vec![b'n'; xattr::MAX_NAME + 1], xattr::RAW, b"v")
            .unwrap_err()
            .errno,
        xattr::ERANGE,
    );
    assert_eq!(
        srv.set_xattr(h, b"user.big", xattr::RAW, &vec![0u8; xattr::MAX_VALUE + 1])
            .unwrap_err()
            .errno,
        xattr::E2BIG,
    );
    // The widest value the contract allows must actually work, or the limit above is a limit on
    // nothing and the refusal proves only that the check exists.
    srv.set_xattr(
        h,
        b"user.widest",
        xattr::RAW,
        &vec![0xCDu8; xattr::MAX_VALUE],
    )
    .expect("the widest permitted value");
    assert_eq!(
        attr(&mut srv, h, b"user.widest").unwrap().1.len(),
        xattr::MAX_VALUE,
    );

    // Fill the node and ask for one more.
    for i in 1..xattr::MAX_COUNT {
        srv.set_xattr(h, format!("user.a{i}").as_bytes(), xattr::RAW, b"v")
            .unwrap();
    }
    assert_eq!(
        srv.set_xattr(h, b"user.overflow", xattr::RAW, b"v")
            .unwrap_err()
            .errno,
        xattr::ENOSPC,
    );
    // Replacing at the cap still works: the ceiling is on new names, not on a full file.
    srv.set_xattr(h, b"user.a1", xattr::RAW, b"replaced")
        .unwrap();

    // Absence is ENODATA, from both verbs, and is the only error that means it.
    assert_eq!(
        srv.get_xattr(h, b"user.never-set", &mut page)
            .unwrap_err()
            .errno,
        xattr::ENODATA,
    );
    assert_eq!(
        srv.remove_xattr(h, b"user.never-set").unwrap_err().errno,
        xattr::ENODATA,
    );
    // And a buffer too small for the value is ERANGE, never a short read.
    let mut tiny = [0u8; 4];
    assert_eq!(
        srv.get_xattr(h, b"user.widest", &mut tiny)
            .unwrap_err()
            .errno,
        xattr::ERANGE,
    );
}

/// **A shorter blob does not leave the longer one's tail behind.** The store file is written and
/// then truncated to length, because a write does not truncate; without the truncate the reader
/// walks records nobody wrote. That is DECISIONS §27's four-times-corrected failure met in a
/// place where it would present as corrupted metadata rather than as a longer file.
#[test]
fn a_shrinking_attribute_set_does_not_leave_a_tail() {
    let mut srv = server_with_tree();
    let h = srv.open_file(&named("motd")).unwrap();
    for i in 0..6 {
        srv.set_xattr(h, format!("user.n{i}").as_bytes(), 1, &[b'x'; 200])
            .unwrap();
    }
    assert_eq!(attr_names(&mut srv, h).len(), 6);

    // Take five of them away and shrink the sixth's value: every byte of the old blob is past
    // the end of the new one.
    for i in 0..5 {
        srv.remove_xattr(h, format!("user.n{i}").as_bytes())
            .unwrap();
    }
    srv.set_xattr(h, b"user.n5", 1, b"tiny").unwrap();

    let disk = srv.fs.disk;
    let mut srv = Server::open(disk).unwrap();
    let h = srv.open_file(&named("motd")).unwrap();
    assert_eq!(attr_names(&mut srv, h), [b"user.n5".to_vec()]);
    assert_eq!(
        attr(&mut srv, h, b"user.n5").unwrap(),
        (1, b"tiny".to_vec())
    );

    // And the last one out takes the store file with it, so the store does not accumulate an
    // entry per node that ever held an attribute.
    let id = node_id_of(&mut srv, TreePtr::root(), "motd");
    srv.remove_xattr(h, b"user.n5").unwrap();
    assert!(
        !store_holds(&mut srv, id),
        "an emptied blob left its file behind"
    );
    assert!(attr_names(&mut srv, h).is_empty());
}

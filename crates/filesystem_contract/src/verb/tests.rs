use crate::*;

// The crate is `no_std`, so its own tests have to say where the allocating types come from. They
// are here only to build fixtures; nothing under test allocates.
extern crate std;
use std::vec::Vec;

/// **Every verb in the contract has a row, and the row agrees with the opcode's own doc.**
///
/// The compile-time assertions in [`verb`] already prove the table is complete and in order, so
/// this test proves the part a `const` cannot reach: that the rows say the *right* thing. It is
/// the mitigation for the table's own BUGS entry, which is that one wrong row is wrong in three
/// programs at once.
/// The build-time order check refuses a table with two rows swapped. The build only ever shows
/// it the real table, which passes whether or not the check runs.
#[test]
fn the_order_check_refuses_two_rows_out_of_place() {
    assert!(!verb::in_opcode_order(&[verb::TABLE[1], verb::TABLE[0]]));
}

#[test]
fn every_verb_has_a_row_that_says_what_its_words_mean() {
    for operation in verb::FIRST..=verb::LAST {
        let v = verb::of(operation).unwrap_or_else(|| panic!("opcode {operation} has no row"));
        assert_eq!(
            v.operation, operation,
            "{} is filed under the wrong opcode",
            v.name
        );
        assert!(!v.name.is_empty(), "opcode {operation} has no name");
        // A verb cannot state its rights two ways at once: `Rights::allows` is "all of", so an
        // any-of requirement folded into `needs_all` would demand both and refuse a capability
        // the server would have accepted.
        assert!(
            v.needs_all == 0 || v.needs_any == 0,
            "{} states its rights twice",
            v.name
        );
        assert_eq!(
            v.needs_all & !dir::ALL,
            0,
            "{} names a right this contract does not define",
            v.name
        );
        assert_eq!(
            v.needs_any & !dir::ALL,
            0,
            "{} names a right this contract does not define",
            v.name
        );
    }
    // The one any-of in the contract, pinned so that folding it into `needs_all` fails here.
    let open = verb::of(fs::OPEN).unwrap();
    assert_eq!(open.needs_any, dir::READ | dir::WRITE);
    assert_eq!(open.needs_all, 0);

    // The five verbs that hand back authority, which is what a renumbering proxy must install.
    let mints: Vec<&str> = (verb::FIRST..=verb::LAST)
        .filter_map(verb::of)
        .filter(|v| v.mints_handle)
        .map(|v| v.name)
        .collect();
    assert_eq!(mints, ["OPEN", "CREATE", "OPENDIR", "MKDIR", "NARROW"]);
}

/// **No caretaker can forward a binding** (milestone 606, ruling D). `BIND` and `UNBIND` are
/// outside the verb table, so `verb::of` answers `None` and every caretaker refuses them, and a
/// verb added to the table later cannot land on their numbers by accident.
#[test]
fn bind_and_unbind_are_not_directory_verbs() {
    for code in [fs::BIND, fs::UNBIND, fs::BIND_SYNC] {
        assert!(
            verb::of(code).is_none(),
            "a caretaker would forward opcode {code}"
        );
        assert!(code > verb::LAST);
    }
}

/// **`NARROW` names a handle and a mask, resolves no name and demands no right** (§173 (narrowing the root of the shell's namespace)). A name
/// filter must not see it (nothing in it is a directory entry), a read-only grant must not refuse
/// it (it cannot widen), and a renumbering proxy must install the handle it mints.
#[test]
fn narrow_carries_a_mask_and_a_minted_handle_and_no_name() {
    let v = verb::of(fs::NARROW).unwrap();
    assert_eq!(v.name, "NARROW");
    assert_eq!(fs::NARROW, 23);
    assert_eq!((v.needs_all, v.needs_any), (0, 0));
    assert!(!v.carries_len(), "NARROW resolves no name");
    assert!(v.carries_w1, "the rights mask rides in the second word");
    assert!(v.mints_handle);
    assert!(!v.mutates());
    assert!(!v.takes_name());
}

/// **A name filter refuses exactly the verbs that reissue a handle without a name**, and today that
/// is `NARROW` alone: every other minting verb names an entry the filter can check.
#[test]
fn only_narrow_reissues_a_handle_without_a_name() {
    let reissuing: Vec<&str> = verb::TABLE
        .iter()
        .filter(|v| v.reissues_handle())
        .map(|v| v.name)
        .collect();
    assert_eq!(reissuing, ["NARROW"]);
}

/// **`STATFS` asks for no right and mints no handle**, which is the whole of its row and the
/// part a caretaker acts on. Spelled out rather than derived, for [`verb::TABLE`]'s reason: a
/// row that grew a rights requirement would silently make a write-only grant unable to ask
/// whether its next write fits.
#[test]
fn statfs_names_a_handle_and_nothing_else() {
    let v = verb::of(fs::STATFS).unwrap();
    assert_eq!(v.name, "STATFS");
    assert_eq!(v.needs_all, 0);
    assert_eq!(v.needs_any, 0);
    assert!(!v.carries_len(), "STATFS's length field counts nothing");
    assert!(!v.carries_w1, "STATFS has no second word");
    assert!(!v.mints_handle);
    assert!(!v.mutates(), "asking how full a disk is changes nothing");
    assert!(!v.takes_name(), "a name filter has nothing to filter here");
    // Forwarded through a per-file grant: see the policy row's own comment.
    assert_eq!(
        verb::file_grant::of(fs::STATFS),
        Some(verb::file_grant::Policy::Forward)
    );
}

/// **A name filter must see every verb that names a directory entry, and no others.**
///
/// This is the row that decides a security property rather than a formatting one:
/// `fs_nameset_caretaker` asks "is this name in the set" exactly when `takes_name` is true, so a
/// name-taking verb whose row says otherwise walks straight past the filter. The list is spelled
/// out rather than derived, because deriving it from the field it is checking would assert
/// nothing.
#[test]
fn the_filter_sees_exactly_the_verbs_that_name_a_directory_entry() {
    let filtered: Vec<&str> = (verb::FIRST..=verb::LAST)
        .filter_map(verb::of)
        .filter(|v| v.takes_name())
        .map(|v| v.name)
        .collect();
    assert_eq!(
        filtered,
        [
            "OPEN",
            "CREATE",
            "OPENDIR",
            "MKDIR",
            "RENAME",
            "UNLINK",
            "RMDIR",
            "GETMTIME",
            "SETMTIME",
            "SETMTIME_AT",
        ],
        "a verb that resolves a name in the granted directory is missing from the filter, or a \
         verb that does not resolve one is being filtered"
    );
    // The attribute verbs carry a name in the page and it is NOT a directory name, which is the
    // distinction `Operand::Payload` exists to make. Filtering it would refuse a program its own
    // file's attributes on the grounds that the attribute is not in the directory.
    for operation in [fs::GETXATTR, fs::SETXATTR, fs::LISTXATTR, fs::REMOVEXATTR] {
        assert!(
            !verb::of(operation).unwrap().takes_name(),
            "an attribute name is not a name in the namespace a capability narrows"
        );
    }
}

/// **Every mutating verb is refused by a read-only per-file grant**, which is what makes the
/// attribute forwarding safe: `SETXATTR` and `REMOVEXATTR` mutate, so a read grant must not pass
/// them on, and `fs_file_caretaker` decides that from `mutates()` rather than from a list of
/// verbs somebody maintains.
#[test]
fn a_read_only_file_grant_refuses_every_verb_that_mutates() {
    use verb::file_grant::Policy;
    let mutating: Vec<&str> = (verb::FIRST..=verb::LAST)
        .filter_map(verb::of)
        .filter(|v| v.mutates())
        .map(|v| v.name)
        .collect();
    assert_eq!(
        mutating,
        [
            "WRITE",
            "CREATE",
            "TRUNCATE",
            "MKDIR",
            "RENAME",
            "UNLINK",
            "RMDIR",
            "SETXATTR",
            "REMOVEXATTR",
            "SYNC",
            "SETMTIME",
            "SETMTIME_AT",
        ]
    );
    // Nothing that mutates may be answered locally: `Forward` is the only policy that consults
    // the grant's direction, and `Refused` never reaches the server at all.
    for operation in verb::FIRST..=verb::LAST {
        let v = verb::of(operation).unwrap();
        let p = verb::file_grant::of(operation).unwrap();
        if v.mutates() {
            assert!(
                matches!(p, Policy::Forward | Policy::Refused(_)),
                "{} mutates and must be refused or direction-gated, never answered locally",
                v.name
            );
        }
    }
}

/// **The four attribute verbs are forwarded through a per-file grant** (milestone 61's gap).
///
/// Pinned as its own test because the old behaviour was uniform and defensible: all four
/// answered `EOPNOTSUPP`, which was honest about a capability that did not carry them. What
/// makes forwarding right is `xattr`'s own rule, that an attribute is part of what a file *is*,
/// so a capability that may read the file may read what is attached to it. A future edit that
/// quietly reverted this would pass every other test here.
#[test]
fn a_per_file_grant_carries_its_files_attributes() {
    use verb::file_grant::Policy;
    for operation in [fs::GETXATTR, fs::SETXATTR, fs::LISTXATTR, fs::REMOVEXATTR] {
        assert_eq!(
            verb::file_grant::of(operation),
            Some(Policy::Forward),
            "{} is not forwarded, so a program behind a per-file grant cannot reach its own \
             file's attributes",
            verb::of(operation).unwrap().name
        );
    }
    // The read half must be reachable through a read-only grant, which is the case that
    // motivated the milestone: `wc report.txt` should be able to ask what is attached to it.
    assert!(!verb::of(fs::GETXATTR).unwrap().mutates());
    assert!(!verb::of(fs::LISTXATTR).unwrap().mutates());
}

/// **A file capability is not a directory, and the refusals still say which kind of "no" it is.**
///
/// The distinction the table was most at risk of flattening. `CREATE` answers `ENOTDIR` because
/// the request does not mean anything here, not `EACCES` and not a generic "denied"; the other
/// directory verbs answer it too, since 2026-08-01. All seven are pinned, so a change to any of
/// them is a change somebody made on purpose, and `EBADF` stays available to mean the one thing
/// it should: the caller named a handle this program never minted.
#[test]
fn a_file_grants_refusals_keep_their_separate_meanings() {
    use verb::file_grant::Policy;
    assert_eq!(
        verb::file_grant::of(fs::CREATE),
        Some(Policy::Refused(grant::ENOTDIR)),
        "CREATE must say `this is not a directory`, never `you were denied`"
    );
    // Every directory verb answers what CREATE answers. `EBADF` would be a claim about the
    // caller's handle; these are refusals about the REQUEST, which a file capability cannot
    // give meaning to. The two must stay distinguishable, because `EBADF` is also what a
    // forged handle gets, and a client cannot act on a word that means both.
    for operation in [
        fs::OPENDIR,
        fs::READDIR,
        fs::MKDIR,
        fs::RENAME,
        fs::UNLINK,
        fs::RMDIR,
        fs::NARROW,
    ] {
        assert_eq!(
            verb::file_grant::of(operation),
            Some(Policy::Refused(grant::ENOTDIR)),
            "a directory verb through a file grant is ENOTDIR, not EBADF",
        );
    }
    // The two the caretaker answers out of what it holds rather than by asking the server.
    assert_eq!(verb::file_grant::of(fs::OPEN), Some(Policy::Local));
    assert_eq!(verb::file_grant::of(fs::CLOSE), Some(Policy::Local));
    // An opcode the contract does not carry has no row and no policy, in both tables at once, so
    // a caretaker's one refusal site covers a forged opcode as well as a forged handle.
    assert!(verb::of(verb::LAST + 1).is_none());
    assert!(verb::file_grant::of(verb::LAST + 1).is_none());
    assert!(verb::of(0).is_none(), "0 is not a verb");
}

// The tests from here down each pin something the run of milestone 85 (mutation testing over the
// host crates) showed nothing pinned. The equivalences (every `|` over disjoint masked fields,
// where `^` cannot differ) are recorded in notes/mutation-testing.md rather than tested.

/// The two Verb predicates the caretakers key off, pinned per verb rather than by example. A
/// `carries_len` that answered true (or false) for everything, and a `mutates` that lost a
/// bit, would misroute requests in three programs at once, which is the table's own BUGS
/// entry.
#[test]
fn the_verb_predicates_partition_the_table_as_documented() {
    // Operand::None: the length field means nothing.
    let no_len = [
        "CLOSE",
        "FSTAT",
        "TRUNCATE",
        "READDIR",
        "LISTXATTR",
        "STATFS",
        "SYNC",
        "NARROW",
    ];
    // needs_all intersects WRITE | CREATE | REMOVE. SETMTIME_AT's needs_all also carries
    // SETTIME, but it mutates because it carries WRITE too; SETTIME alone never appears
    // without WRITE, by this table's own construction.
    let mutating = [
        "WRITE",
        "CREATE",
        "TRUNCATE",
        "MKDIR",
        "RENAME",
        "UNLINK",
        "RMDIR",
        "SETXATTR",
        "REMOVEXATTR",
        "SYNC",
        "SETMTIME",
        "SETMTIME_AT",
    ];
    for v in &verb::TABLE {
        assert_eq!(
            v.carries_len(),
            !no_len.contains(&v.name),
            "{}: carries_len",
            v.name
        );
        assert_eq!(
            v.mutates(),
            mutating.contains(&v.name),
            "{}: mutates",
            v.name
        );
    }
}

/// **A row the compiler built through `row` is the row `row` builds at run time.**
///
/// `row` is only ever evaluated by the compiler, building [`verb::TABLE`], so a coverage run counts
/// none of its lines; inside the old single-file crate root that went unnoticed, and in its own file
/// it put `verb.rs` under the per-file coverage floor (milestone 845 (the filesystem-protocol crate
/// root is split along its seams)). This runs it, and pins the one thing it decides on its own: a
/// row spelled through it asks for no any-of right.
#[test]
fn a_row_built_at_run_time_is_the_row_the_compiler_built() {
    let mut rebuilt = 0;
    for v in verb::TABLE.iter().filter(|v| v.needs_any == 0) {
        let r = super::row(
            v.operation,
            v.name,
            v.operand,
            v.carries_w1,
            v.mints_handle,
            v.needs_all,
        );
        assert_eq!(
            (r.operation, r.name, r.operand, r.carries_w1),
            (v.operation, v.name, v.operand, v.carries_w1)
        );
        assert_eq!(
            (r.mints_handle, r.needs_all, r.needs_any),
            (v.mints_handle, v.needs_all, 0),
            "{}",
            v.name
        );
        rebuilt += 1;
    }
    assert!(rebuilt > 0, "no row was spelled through `row`");
}

//! The host-side checks that run after a boot, against the images the guest wrote.
//!
//! A guest can only report what it believes it did. These open the same disks from outside,
//! with the same parsers, and say what is actually on them.

use crate::disk::{blank_disk_path, crash_disk_path, redoxfs_disk_path};
use crate::host::capture;

/// After a test run, read the blank disk back **from the host** and check what the guest put on it:
/// the partition table with `crates/globally_unique_identifier_partition_table`, and the filesystem
/// inside the data partition, read in place by the pinned engine through `tools/redoxfs_host`.
/// Returns whether both hold.
///
/// This is a different program, on a different operating system, with a different engine build,
/// opening the file the run left behind; if it and the in-guest check ever disagreed, the guest
/// would be the one to doubt. The partition is named by type GUID, not slot number, as the guest's
/// `mkfs` finds it: the type is what the partition is, and the slot is this table's current order.
/// See notes/host-recovery/devices-and-partitions.md.
pub(crate) fn blank_check_after_run() -> bool {
    use filesystem_fixture::blank;
    use globally_unique_identifier_partition_table::GloballyUniqueIdentifierPartitionTable;

    let path = blank_disk_path();
    let Ok(img) = std::fs::read(&path) else {
        eprintln!("BLANK IMAGE CHECK FAILED: cannot read {path}");
        return false;
    };
    let lba = blank::LBA as usize;
    if img.len() < 34 * lba {
        eprintln!("BLANK IMAGE CHECK FAILED: {path} is {} bytes", img.len());
        return false;
    }

    // The table the guest wrote, judged by the parser the guest did not run: this process's own
    // copy, on the host, against the bytes on disk.
    if let Err(e) =
        globally_unique_identifier_partition_table::mbr::validate(&img[..lba], blank::DISK_BLOCKS)
    {
        eprintln!("BLANK IMAGE CHECK FAILED: the protective MBR the guest wrote is bad: {e:?}");
        return false;
    }
    let table = match GloballyUniqueIdentifierPartitionTable::parse(
        &img[lba..2 * lba],
        &img[2 * lba..34 * lba],
    ) {
        Ok(t) => t,
        Err(e) => {
            eprintln!(
                "BLANK IMAGE CHECK FAILED: the guest's partition table does not parse: {e:?}"
            );
            return false;
        }
    };
    let parts: Vec<_> = table.partitions().collect();
    if parts.len() != blank::PARTITIONS {
        eprintln!(
            "BLANK IMAGE CHECK FAILED: the guest wrote {} partitions, expected {}",
            parts.len(),
            blank::PARTITIONS,
        );
        return false;
    }
    // Every unique GUID distinct and version 4. Two partitions with one id is the failure the
    // entropy capability exists to prevent, and it is invisible to everything else here.
    for (i, (_, part)) in parts.iter().enumerate() {
        let text = format!("{:X}", part.unique_guid);
        if text.as_bytes()[14] != b'4' {
            eprintln!(
                "BLANK IMAGE CHECK FAILED: partition {i}'s unique GUID is not version 4: {text}",
            );
            return false;
        }
        for (j, (_, other)) in parts.iter().enumerate() {
            if i != j && part.unique_guid == other.unique_guid {
                eprintln!("BLANK IMAGE CHECK FAILED: partitions {i} and {j} share a unique GUID");
                return false;
            }
        }
    }
    if !parts.iter().any(|(_, p)| {
        p.type_guid == globally_unique_identifier_partition_table::guid::types::NIFE_DATA
    }) {
        eprintln!("BLANK IMAGE CHECK FAILED: no nife data partition on the guest's disk");
        return false;
    }

    // The filesystem inside it, opened by the pinned engine on the host, at the offset the tool
    // works out from the same table this function just checked.
    let data_type = format!(
        "{:X}",
        globally_unique_identifier_partition_table::guid::types::NIFE_DATA
    );
    let out = capture(
        "cargo",
        &[
            "run",
            "--quiet",
            "--manifest-path",
            "tools/redoxfs_host/Cargo.toml",
            "--",
            "cat",
            &path,
            blank::MADE_NAME,
            "--partition-type",
            &data_type,
        ],
    );
    match out.as_deref() {
        Some(s) if s.as_bytes() == blank::MADE_BODY => {
            eprintln!(
                "blank image: the guest partitioned it ({} partitions, distinct v4 GUIDs) and the \
                 host engine reads `{}` out of the filesystem the guest created",
                parts.len(),
                blank::MADE_NAME,
            );
            true
        }
        other => {
            eprintln!(
                "BLANK IMAGE CHECK FAILED: the host tool did not read the guest's file back (got \
                 {:?}). The table is fine, so this is the filesystem `mkfs` made.",
                other.unwrap_or("<host tool error: the partition did not even open>"),
            );
            false
        }
    }
}

/// After a test run, reopen the **crash** image with the host tool and confirm the property holds
/// from outside the guest: `cut` reads back as exactly one of the two payloads, whole.
///
/// This is the half a cache cannot fake. The guest's verifier read the file back through an FS
/// server that had just mounted the damaged disk; this is a different process, on the host, with the
/// pinned engine, opening the image the run left behind. It also proves the image is still a
/// consistent RedoxFS at all, because `cat` cannot succeed on one that is not.
pub(crate) fn redoxfs_crash_check_after_run() -> bool {
    let out = capture(
        "cargo",
        &[
            "run",
            "--quiet",
            "--manifest-path",
            "tools/redoxfs_host/Cargo.toml",
            "--",
            "cat",
            &crash_disk_path(),
            filesystem_fixture::crash::NAME,
        ],
    );
    let want_a = filesystem_fixture::crash::A;
    let want_b = filesystem_fixture::crash::B;
    match out.as_deref() {
        Some(s) if s.as_bytes() == want_a => {
            eprintln!(
                "crash image: `cut` holds payload A, whole (the interrupted write is absent)"
            );
            true
        }
        Some(s) if s.as_bytes() == want_b => {
            eprintln!(
                "crash image: `cut` holds payload B, whole (the interrupted write completed)"
            );
            true
        }
        other => {
            eprintln!(
                "CRASH CONSISTENCY FAILED: after a kill mid-transaction the image's `cut` is \
                 neither payload whole (got {:?}). A write must be wholly present or wholly absent.",
                other.unwrap_or("<host tool error: the image did not even open>"),
            );
            false
        }
    }
}

/// After a test run, reopen the image with the host tool and confirm it still parses, that the file
/// the FS server served reads back byte for byte, and that the write the `std::fs` test performed
/// **reached the disk**. `cat` succeeding at all proves the image is still a consistent RedoxFS
/// after the run (the FS server opened it read-write with cleanup, which advances the header ring);
/// the bytes prove nothing was corrupted.
///
/// The `scratch` half is the on-disk half of the write proof, and it is the part a cache cannot
/// fake: the guest read its own write back through the same FS server, but this reopens the image
/// with a different process and the pinned engine.
pub(crate) fn redoxfs_check_after_run() -> bool {
    redoxfs_reads_back(filesystem_fixture::MOTD_NAME, filesystem_fixture::MOTD)
        && redoxfs_reads_back(
            filesystem_fixture::SCRATCH_NAME,
            filesystem_fixture::WRITE_PATTERN,
        )
        && redoxfs_subtree_was_confined()
        && redoxfs_glob_grant_took_exactly_the_match()
}

/// **The set grant, witnessed from outside the guest** (milestone 47 (navigation and naming)'s
/// globbing lane): the host, with the pinned engine, reading the image the run left behind. Returns
/// whether it holds; an untouched fixture (the guest never ran the set grant) passes.
///
/// Three claims, which only mean anything together. The two matched names are gone, so "the
/// expansion is the grant" is a fact about the disk. The two unmatched names, one entry away and
/// removable by the caretaker a hop up, are still there, so their survival is a fact about the
/// *set*. The unmatched directory still holds its file, since a set capability with no `-r` cannot
/// look inside even one it matched. See notes/glob-grant/tests.md.
fn redoxfs_glob_grant_took_exactly_the_match() -> bool {
    use filesystem_fixture::tree;
    let img = redoxfs_disk_path();
    let Some(globset) = redoxfs_ls(&img, tree::GLOBSET) else {
        eprintln!("milestone-47 glob check: `{}` did not list", tree::GLOBSET);
        return false;
    };
    // No RedoxFS disk, or a boot that never ran the test: the fixture is untouched, and this check
    // has nothing to say. `redoxfs_subtree_was_confined` makes the same allowance for the same
    // reason (a `run` that skipped the FS tests must not fail the gate).
    let matched_gone = ![tree::GLOB_ONE, tree::GLOB_TWO]
        .iter()
        .any(|n| globset.iter().any(|got| got == n));
    if !matched_gone && globset.len() == 4 {
        eprintln!(
            "milestone-47 glob check: `{}` is untouched; the guest never ran the set grant (skipping)",
            tree::GLOBSET
        );
        return true;
    }
    for name in [tree::GLOB_ONE, tree::GLOB_TWO] {
        if globset.iter().any(|got| got == name) {
            eprintln!(
                "MILESTONE-47 GLOB FAILED: `{name}` matched the pattern and is still in `{}` \
                 ({globset:?}). What the expansion showed is not what the grant took away.",
                tree::GLOBSET,
            );
            return false;
        }
    }
    for name in [tree::GLOB_MISS, tree::GLOB_DIR] {
        if !globset.iter().any(|got| got == name) {
            eprintln!(
                "MILESTONE-47 GLOB FAILED: `{name}` did NOT match the pattern and is gone from \
                 `{}` ({globset:?}). A set capability reached a name one directory entry away that \
                 the command line never designated.",
                tree::GLOBSET,
            );
            return false;
        }
    }
    let inner = format!("{}/{}", tree::GLOBSET, tree::GLOB_DIR);
    match redoxfs_ls(&img, &inner) {
        Some(names) if names.iter().any(|n| n == tree::GLOB_INNER) => {
            eprintln!(
                "glob grant: `{}` matched two names in `{}` and exactly those two are gone; \
                 {globset:?} is what the pattern did not designate",
                core::str::from_utf8(tree::GLOB_PATTERN).unwrap_or("?"),
                tree::GLOBSET,
            );
            true
        }
        other => {
            eprintln!(
                "MILESTONE-47 GLOB FAILED: `{inner}` holds {other:?} and should still hold `{}`. \
                 An unmatched directory was walked into.",
                tree::GLOB_INNER,
            );
            false
        }
    }
}

/// **The directory capability's confinement, asserted from outside the confined program**
/// (milestone 47), on the image the run left behind. Returns whether every fixture name is still in
/// the root, none of the attacker's names is, its creations (one renamed, one not) are in `sub`,
/// `other/secret` and `sub/inner` read back unchanged, and both shells pass
/// [`shell_navigation_landed`]. See notes/dir-capability/tests-and-stack-usage.md.
///
/// # BUGS
/// - The root check is containment, not equality, because other tests write the root too
///   (`made-by-std`), and an exact match would manufacture failures (§27 (the filesystem service)).
///   A leaked name matching neither fixture prefix slips past.
fn redoxfs_subtree_was_confined() -> bool {
    use filesystem_fixture::tree;
    let img = redoxfs_disk_path();

    let (Some(root), Some(sub)) = (redoxfs_ls(&img, "/"), redoxfs_ls(&img, tree::SUB)) else {
        eprintln!("milestone-47 confinement check: the image did not list after the run");
        return false;
    };
    for want in tree::ROOT_ENTRIES {
        if !root.iter().any(|n| n == want) {
            eprintln!(
                "MILESTONE-47 CONFINEMENT FAILED: `{want}` is gone from the image root (it holds \
                 {root:?}). Nothing in this run held a capability that could remove it.",
            );
            return false;
        }
    }
    // A run index is appended to each name so three attacker runs sharing one image do not collide,
    // so the prefix is what identifies a creation rather than the whole name.
    let attackers_own = |n: &String| {
        n.starts_with(tree::MADE) || n.starts_with(tree::MADE_DIR) || n.starts_with(tree::MOVED)
    };
    if let Some(leaked) = root.iter().find(|n| attackers_own(n)) {
        eprintln!(
            "MILESTONE-47 CONFINEMENT FAILED: `{leaked}` is in the image ROOT. A program granted a \
             capability to `{}` created a name in its parent.",
            tree::SUB,
        );
        return false;
    }
    let count = |prefix: &str| sub.iter().filter(|n| n.starts_with(prefix)).count();
    let (made_files, made_dirs, moved) =
        (count(tree::MADE), count(tree::MADE_DIR), count(tree::MOVED));
    if made_files == 0 || made_dirs == 0 {
        eprintln!(
            "milestone-47 confinement check: `{}` holds {sub:?}, with {made_files} created files \
             and {made_dirs} created directories. The attacker created nothing, so \"nothing it \
             made escaped to the root\" is true of a capability that reaches nothing at all.",
            tree::SUB,
        );
        return false;
    }
    // `made_files` counts the names that were created and NOT renamed, so requiring both counts to
    // be non-zero is the `REMOVE` rung asserted from out here: a capability carrying it moved a
    // name, and one without it left its own name exactly where it made it.
    if moved == 0 {
        eprintln!(
            "MILESTONE-47 CONFINEMENT FAILED: `{}` holds {sub:?} and nothing was renamed. A \
             capability carrying REMOVE and CREATE must be able to move a name it made, or the \
             refusals the other runs report are refusals of a verb that never works.",
            tree::SUB,
        );
        return false;
    }
    let sibling = format!("{}/{}", tree::OTHER, tree::SECRET);
    let granted = format!("{}/{}", tree::SUB, tree::INNER);
    redoxfs_reads_back(&sibling, tree::SECRET_BODY)
        && redoxfs_reads_back(&granted, tree::INNER_BODY)
        && shell_navigation_landed(&root, &sub)
        && match redoxfs_ls(&img, tree::OTHER) {
            // The **second** shell's leavings, in the sibling it was rooted at. Checking only `sub`
            // would leave the headline property half-witnessed from out here: two shells with two
            // roots each wrote into their own and neither into the other's.
            Some(other) => shell_navigation_landed(&root, &other),
            None => false,
        }
}

/// **`rm` witnessed from outside the guest** (milestone 47's commands). `root` is the image root's
/// listing and `home` the listing of the directory a shell was rooted at, both read on the host by
/// the pinned engine after the run. Returns whether the name the shell removed is absent from
/// `home`, the names it kept, made and touched are present, and none is in `root`.
///
/// The kept name is what makes "the removed name is absent" non-vacuous. `touch`'s content claim (a
/// second `touch` does not truncate) is checked in-guest, since this file has no run index to
/// rebuild the exact name `redoxfs_reads_back` would need. See notes/shell-navigation.md.
fn shell_navigation_landed(root: &[String], home: &[String]) -> bool {
    use filesystem_fixture::tree;
    let count = |dir: &[String], prefix: &str| dir.iter().filter(|n| n.starts_with(prefix)).count();

    if count(home, tree::NAV_KEPT) == 0 || count(home, tree::NAV_DIR) == 0 {
        eprintln!(
            "milestone-47 navigation check: a shell's root holds {home:?}, with nothing a \
             navigating shell made in it. \"what it removed is gone\" is true of a shell that \
             created nothing, so this proves nothing without it.",
        );
        return false;
    }
    if count(home, tree::NAV_TOUCH) == 0 {
        eprintln!(
            "MILESTONE-47 NAVIGATION FAILED: a shell's root holds no `{}` name, so its `touch` \
             never reached the platter: {home:?}",
            tree::NAV_TOUCH,
        );
        return false;
    }
    if count(home, tree::NAV_GONE) != 0 {
        eprintln!(
            "MILESTONE-47 NAVIGATION FAILED: a shell's root still holds a `{}` name, so its `rm` \
             reported success and never reached the platter: {home:?}",
            tree::NAV_GONE,
        );
        return false;
    }
    let leaked = |n: &&String| {
        n.starts_with(tree::NAV_KEPT)
            || n.starts_with(tree::NAV_GONE)
            || n.starts_with(tree::NAV_DIR)
            || n.starts_with(tree::NAV_TOUCH)
    };
    if let Some(name) = root.iter().find(leaked) {
        eprintln!(
            "MILESTONE-47 NAVIGATION FAILED: `{name}` is in the image ROOT. A shell rooted at a \
             subtree made a name in its parent.",
        );
        return false;
    }
    true
}

/// The names in one directory of the post-run image, sorted, via the host tool's `ls`. Its output is
/// `kind size name` per line and the fixture's names carry no spaces, so the name is the last field.
fn redoxfs_ls(image: &str, path: &str) -> Option<Vec<String>> {
    let out = capture(
        "cargo",
        &[
            "run",
            "--quiet",
            "--manifest-path",
            "tools/redoxfs_host/Cargo.toml",
            "--",
            "ls",
            image,
            path,
        ],
    )?;
    let mut names: Vec<String> = out
        .lines()
        .filter_map(|l| l.split_whitespace().last().map(str::to_string))
        .collect();
    names.sort();
    Some(names)
}

/// `cat` one file out of the post-run image with the host tool and compare it byte for byte.
fn redoxfs_reads_back(name: &str, want: &[u8]) -> bool {
    let out = capture(
        "cargo",
        &[
            "run",
            "--quiet",
            "--manifest-path",
            "tools/redoxfs_host/Cargo.toml",
            "--",
            "cat",
            &redoxfs_disk_path(),
            name,
        ],
    );
    match out {
        Some(s) if s.as_bytes() == want => true,
        other => {
            eprintln!(
                "redoxfs consistency check failed: {name} did not read back after the run (got {:?})",
                other.as_deref().unwrap_or("<host tool error>")
            );
            false
        }
    }
}

use crate::*;

#[test]
fn file_request_packs_and_unpacks() {
    // Every field survives the pack, at its extremes, without bleeding into another.
    for &(o, h, l) in &[
        (fs::OPEN, 0u64, 7u64),
        (fs::READ, fs::MAX_HANDLE, fs::MAX_LEN),
        (fs::WRITE, 1, 4096),
        (fs::CLOSE, 65535, 0),
        (fs::FSTAT, 3, 0),
        (fs::CREATE, 0, 11),
        (fs::TRUNCATE, 42, 0),
    ] {
        let w = fs::req(o, h, l);
        assert_eq!(operation(w), o, "opcode");
        assert_eq!(fs::req_handle(w), h, "handle");
        assert_eq!(fs::req_len(w) as u64, l, "len");
    }
}

#[test]
fn a_handle_never_collides_with_the_opcode_or_length() {
    // A big handle must not spill into the opcode field, and a max length must not spill into
    // the handle: the READ path packs all three and reads them back independently.
    let w = fs::req(fs::READ, fs::MAX_HANDLE, fs::MAX_LEN);
    assert_eq!(operation(w), fs::READ);
    assert_eq!(fs::req_handle(w), fs::MAX_HANDLE);
    assert_eq!(fs::req_len(w) as u64, fs::MAX_LEN);
}

// --- The directory capability (milestone 47 (navigation and naming)) ---

/// The three new verbs must not collide with the seven that were already on the wire, and
/// `ROOT` must stay 0 because every client that ever sent an `OPEN` sent 0 in that field.
#[test]
fn the_directory_verbs_are_distinct_from_every_other_one() {
    let operations = [
        ("OPEN", fs::OPEN),
        ("READ", fs::READ),
        ("WRITE", fs::WRITE),
        ("CLOSE", fs::CLOSE),
        ("FSTAT", fs::FSTAT),
        ("CREATE", fs::CREATE),
        ("TRUNCATE", fs::TRUNCATE),
        ("OPENDIR", fs::OPENDIR),
        ("READDIR", fs::READDIR),
        ("MKDIR", fs::MKDIR),
        ("RENAME", fs::RENAME),
        ("UNLINK", fs::UNLINK),
        ("RMDIR", fs::RMDIR),
        ("GETXATTR", fs::GETXATTR),
        ("SETXATTR", fs::SETXATTR),
        ("LISTXATTR", fs::LISTXATTR),
        ("REMOVEXATTR", fs::REMOVEXATTR),
        ("SYNC", fs::SYNC),
        ("GETMTIME", fs::GETMTIME),
        ("SETMTIME", fs::SETMTIME),
        ("SETMTIME_AT", fs::SETMTIME_AT),
    ];
    for (i, (na, a)) in operations.iter().enumerate() {
        assert!(*a <= 0xff, "{na} does not fit the 8-bit opcode field");
        assert_ne!(*a, 0, "0 is not a verb");
        for (nb, b) in &operations[i + 1..] {
            assert_ne!(a, b, "{na} and {nb} share an opcode");
        }
    }
    assert_eq!(fs::ROOT, 0, "every existing client sends 0 and means this");
}

/// **A rename names two directories, so its second word is a packed pair rather than a scalar**,
/// and the two halves must not bleed into each other or a rename lands somewhere nobody asked
/// for. The destination fields use the same bit positions as the request word's, which is the
/// point of reusing them, so this also pins that they stay in step.
#[test]
fn a_rename_carries_two_directories_and_two_lengths_without_them_bleeding() {
    for &(sh, sl, dh, dl) in &[
        (fs::ROOT, 3u64, fs::ROOT, 4u64),
        (1, 16, 2, 1),
        (fs::MAX_HANDLE, fs::MAX_LEN, fs::MAX_HANDLE, fs::MAX_LEN),
        (fs::MAX_HANDLE, 1, 0, fs::MAX_LEN),
    ] {
        let w0 = fs::req(fs::RENAME, sh, sl);
        let w1 = fs::rename_dst(dh, dl);
        assert_eq!(operation(w0), fs::RENAME);
        assert_eq!(fs::req_handle(w0), sh, "source handle");
        assert_eq!(fs::req_len(w0) as u64, sl, "source name length");
        assert_eq!(fs::dst_handle(w1), dh, "destination handle");
        assert_eq!(fs::dst_len(w1) as u64, dl, "destination name length");
    }
    // A destination in the same directory is the ordinary rename, and it must not be confusable
    // with "no destination given": handle 0 IS a directory (the bound one), so a zero second
    // word means "rename inside your root", which is exactly what a plain `mv a b` wants.
    assert_eq!(fs::dst_handle(0), fs::ROOT);
}

// The tests from here down each pin something the run of milestone 85 (mutation testing over the
// host crates) showed nothing pinned. The equivalences (every `|` over disjoint masked fields,
// where `^` cannot differ) are recorded in notes/mutation-testing.md rather than tested.

/// **A single-page request still means one page**, which is the compatibility claim
/// [`fs::TRANSFER_PAGES`] makes in prose, checked through the packing rather than beside it.
///
/// The arithmetic the channel has to satisfy is a `const` block next to the constants
/// themselves, because a build is the only artifact that can be wrong about it and a test does
/// not run in the EL0 build. What is left here is the round trip, which needs the packing.
#[test]
fn a_single_page_request_still_means_one_page() {
    let w0 = fs::req(fs::READ, 1, PAGE as u64);
    assert_eq!(fs::req_len(w0), PAGE);
    assert_eq!(fs::req_handle(w0), 1);
    assert_eq!(operation(w0), fs::READ);

    // And the largest the channel permits survives the same 40-bit field.
    let w0 = fs::req(fs::WRITE, 1, fs::TRANSFER_MAX as u64);
    assert_eq!(fs::req_len(w0), fs::TRANSFER_MAX);
    assert_eq!(operation(w0), fs::WRITE);
}

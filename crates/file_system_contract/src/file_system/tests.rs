use crate::*;

#[test]
fn file_request_packs_and_unpacks() {
    // Every field survives the pack, at its extremes, without bleeding into another.
    for &(o, h, l) in &[
        (file_system::OPEN, 0u64, 7u64),
        (
            file_system::READ,
            file_system::MAXIMUM_HANDLE,
            file_system::MAXIMUM_LENGTH_BYTES,
        ),
        (file_system::WRITE, 1, 4096),
        (file_system::CLOSE, 65535, 0),
        (file_system::FSTAT, 3, 0),
        (file_system::CREATE, 0, 11),
        (file_system::TRUNCATE, 42, 0),
    ] {
        let w = file_system::request(o, h, l);
        assert_eq!(operation(w), o, "opcode");
        assert_eq!(file_system::request_handle(w), h, "handle");
        assert_eq!(file_system::request_length_bytes(w) as u64, l, "len");
    }
}

#[test]
fn a_handle_never_collides_with_the_opcode_or_length() {
    // A big handle must not spill into the opcode field, and a max length must not spill into
    // the handle: the READ path packs all three and reads them back independently.
    let w = file_system::request(
        file_system::READ,
        file_system::MAXIMUM_HANDLE,
        file_system::MAXIMUM_LENGTH_BYTES,
    );
    assert_eq!(operation(w), file_system::READ);
    assert_eq!(file_system::request_handle(w), file_system::MAXIMUM_HANDLE);
    assert_eq!(
        file_system::request_length_bytes(w) as u64,
        file_system::MAXIMUM_LENGTH_BYTES
    );
}

#[test]
#[should_panic]
fn a_request_refuses_a_handle_past_the_field() {
    let _ = file_system::request(file_system::READ, file_system::MAXIMUM_HANDLE + 1, 0);
}

#[test]
#[should_panic]
fn a_request_refuses_a_length_past_the_field() {
    let _ = file_system::request(
        file_system::READ,
        3,
        file_system::MAXIMUM_LENGTH_BYTES + 1,
    );
}

#[test]
#[should_panic]
fn a_rename_destination_refuses_a_handle_past_the_field() {
    let _ = file_system::rename_destination(file_system::MAXIMUM_HANDLE + 1, 1);
}

#[test]
#[should_panic]
fn a_rename_destination_refuses_a_length_past_the_field() {
    let _ = file_system::rename_destination(1, file_system::MAXIMUM_LENGTH_BYTES + 1);
}

// --- The directory capability ---

/// `ROOT` stays 0 because every client that ever sent an `OPEN` sent 0 in that field.
#[test]
fn the_root_handle_stays_zero() {
    assert_eq!(
        file_system::ROOT,
        0,
        "every existing client sends 0 and means this"
    );
}

/// **A rename names two directories, so its second word is a packed pair rather than a scalar**,
/// and the two halves must not bleed into each other or a rename lands somewhere nobody asked
/// for. The destination fields use the same bit positions as the request word's, which is the
/// point of reusing them, so this also pins that they stay in step.
#[test]
fn a_rename_carries_two_directories_and_two_lengths_without_them_bleeding() {
    for &(sh, sl, dh, dl) in &[
        (file_system::ROOT, 3u64, file_system::ROOT, 4u64),
        (1, 16, 2, 1),
        (
            file_system::MAXIMUM_HANDLE,
            file_system::MAXIMUM_LENGTH_BYTES,
            file_system::MAXIMUM_HANDLE,
            file_system::MAXIMUM_LENGTH_BYTES,
        ),
        (
            file_system::MAXIMUM_HANDLE,
            1,
            0,
            file_system::MAXIMUM_LENGTH_BYTES,
        ),
    ] {
        let w0 = file_system::request(file_system::RENAME, sh, sl);
        let w1 = file_system::rename_destination(dh, dl);
        assert_eq!(operation(w0), file_system::RENAME);
        assert_eq!(file_system::request_handle(w0), sh, "source handle");
        assert_eq!(
            file_system::request_length_bytes(w0) as u64,
            sl,
            "source name length"
        );
        assert_eq!(
            file_system::destination_handle(w1),
            dh,
            "destination handle"
        );
        assert_eq!(
            file_system::destination_length_bytes(w1) as u64,
            dl,
            "destination name length"
        );
    }
    // A destination in the same directory is the ordinary rename, and it must not be confusable
    // with "no destination given": handle 0 IS a directory (the bound one), so a zero second
    // word means "rename inside your root", which is exactly what a plain `mv a b` wants.
    assert_eq!(file_system::destination_handle(0), file_system::ROOT);
}

// The tests from here down each pin something a mutation run over this crate showed nothing
// pinned. The equivalences (every `|` over disjoint masked fields, where `^` cannot differ) are
// recorded as such rather than tested.

/// **A single-page request still means one page**, which is the compatibility claim
/// [`file_system::TRANSFER_PAGES`] makes in prose, checked through the packing rather than beside it.
///
/// The arithmetic the channel has to satisfy is a `const` block next to the constants
/// themselves, because a build is the only artifact that can be wrong about it and a test does
/// not run in the unprivileged build. What is left here is the round trip, which needs the packing.
#[test]
fn a_single_page_request_still_means_one_page() {
    let w0 = file_system::request(file_system::READ, 1, PAGE_BYTES as u64);
    assert_eq!(file_system::request_length_bytes(w0), PAGE_BYTES);
    assert_eq!(file_system::request_handle(w0), 1);
    assert_eq!(operation(w0), file_system::READ);

    // And the largest the channel permits survives the same 40-bit field.
    let w0 = file_system::request(
        file_system::WRITE,
        1,
        file_system::MAXIMUM_TRANSFER_BYTES as u64,
    );
    assert_eq!(
        file_system::request_length_bytes(w0),
        file_system::MAXIMUM_TRANSFER_BYTES
    );
    assert_eq!(operation(w0), file_system::WRITE);
}

#[test]
fn the_packed_words_are_the_wire_bytes_they_always_were() {
    // Known inputs, spelled as bit fields: operation in bits 63:56, handle in 55:40, length in 39:0.
    let w0 = file_system::request(file_system::READ, 3, 1024);
    assert_eq!(w0, (file_system::READ << 56) | (3 << 40) | 1024);
    assert_eq!(file_system::request_handle(w0), 3);
    assert_eq!(file_system::request_length_bytes(w0), 1024);

    // A rename's second word is the same two fields without the opcode.
    let w1 = file_system::rename_destination(5, 12);
    assert_eq!(w1, (5 << 40) | 12);
    assert_eq!(file_system::destination_handle(w1), 5);
    assert_eq!(file_system::destination_length_bytes(w1), 12);

    // A block request stores the count less one in the low byte.
    let w0 = block::request(block::READ, 4);
    assert_eq!(w0, (block::READ << 56) | 3);
    assert_eq!(block::request_block_count(w0), Some(4));
}

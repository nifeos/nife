//! **Machine-checked proofs (Kani) of the packers and decoders**, beside the rights proofs in
//! [`directory`]: every field a word carries comes back out unchanged, no field bleeds into another, and
//! the verb table is total over the opcode space. Behind `#[cfg(kani)]`, so only `cargo kani`
//! compiles them.
//!
//! Name: provisional, after the module every other crate's harnesses live in.

use super::*;

/// **A file request word round-trips for every in-range opcode, handle and length, and no field
/// bleeds into another.** The destination half of a rename uses the same fields and is read back
/// the same way.
/// Falsification: replayable `crates/file_system_contract/falsifications/proofs.a_file_request_round_trips.patch`
#[kani::proof]
fn a_file_request_round_trips() {
    let opcode: u64 = kani::any();
    let handle: u64 = kani::any();
    let length: u64 = kani::any();
    kani::assume(opcode <= 0xff);
    kani::assume(handle <= file_system::MAXIMUM_HANDLE);
    kani::assume(length <= file_system::MAXIMUM_LENGTH_BYTES);
    let w0 = file_system::request(opcode, handle, length);
    assert!(operation(w0) == opcode);
    assert!(file_system::request_handle(w0) == handle);
    assert!(file_system::request_length_bytes(w0) as u64 == length);
    let w1 = file_system::rename_destination(handle, length);
    assert!(file_system::destination_handle(w1) == handle);
    assert!(file_system::destination_length_bytes(w1) as u64 == length);
}

/// **An attribute reply is non-negative for every kind and length, and round-trips.** A negative
/// reply word is an errno, so a kind that set the sign bit would read as an error.
/// Falsification: replayable `crates/file_system_contract/falsifications/proofs.an_attribute_reply_is_never_an_error.patch`
#[kani::proof]
fn an_attribute_reply_is_never_an_error() {
    let kind: u32 = kani::any();
    let value_len: u32 = kani::any();
    kani::assume(kind <= extended_attribute::MAXIMUM_KIND);
    let r0 = extended_attribute::reply(kind, value_len as usize);
    assert!(r0 >= 0);
    assert!(extended_attribute::reply_kind(r0) == kind);
    assert!(extended_attribute::reply_value_len(r0) == value_len as usize);
}

/// **A block request round-trips its count for every count in `1..=TRANSFER_BLOCKS`.**
/// Falsification: replayable `crates/file_system_contract/falsifications/proofs.a_block_request_round_trips.patch`
#[kani::proof]
fn a_block_request_round_trips() {
    let opcode: u64 = kani::any();
    let blocks: usize = kani::any();
    kani::assume(opcode <= 0xff);
    kani::assume((1..=block::TRANSFER_BLOCKS).contains(&blocks));
    let w0 = block::request(opcode, blocks);
    assert!(operation(w0) == opcode);
    assert!(block::request_block_count(w0) == Some(blocks));
}

/// **A packed name comes back whole at every length the two words hold.** Eight bytes are all the
/// proof needs per word, so the name is symbolic in full.
/// Falsification: replayable `crates/file_system_contract/falsifications/proofs.a_packed_name_comes_back_whole.patch`
#[kani::proof]
#[kani::unwind(18)]
fn a_packed_name_comes_back_whole() {
    let bytes: [u8; grant::MAXIMUM_WORDS_NAME_BYTES] = kani::any();
    let length: usize = kani::any();
    kani::assume((1..=grant::MAXIMUM_WORDS_NAME_BYTES).contains(&length));
    let name = &bytes[..length];
    let (lo, hi) = grant::pack_name(name).unwrap();
    let mut out = [0u8; grant::MAXIMUM_WORDS_NAME_BYTES];
    let n = grant::unpack_name(lo, hi, length, &mut out);
    assert!(n == length);
    assert!(&out[..n] == name);
}

/// **`verb::of` answers `None` outside `FIRST..=LAST` and the row filed under the opcode inside
/// it, for every opcode, without indexing out of bounds.**
/// Falsification: replayable `crates/file_system_contract/falsifications/proofs.the_verb_table_is_total.patch`
#[kani::proof]
fn the_verb_table_is_total() {
    let opcode: u64 = kani::any();
    match verb::of(opcode) {
        Some(row) => {
            assert!((verb::FIRST..=verb::LAST).contains(&opcode));
            assert!(row.operation == opcode);
        }
        None => assert!(!(verb::FIRST..=verb::LAST).contains(&opcode)),
    }
}

/// **The server's path never widens: whatever a request says, including the sentinel that means
/// "the parent's own rights", the child is a subset of the parent.** `attenuate_never_widens` in
/// [`directory`] covers `attenuate`; this adds [`directory::Rights::substitute_parent_rights`] in front of it.
/// Falsification: replayable `crates/file_system_contract/falsifications/proofs.a_resolved_request_never_widens.patch`
#[kani::proof]
fn a_resolved_request_never_widens() {
    let parent = directory::Rights::root(kani::any());
    let requested: u64 = kani::any();
    let child = parent.attenuate(parent.substitute_parent_rights(requested));
    assert!(child.bits() & !parent.bits() == 0);
    // `attenuate` masks whatever `substitute_parent_rights` answers, so the line above alone cannot see `substitute_parent_rights`
    // widen. The sentinel is the one word `substitute_parent_rights` rewrites, and it must become the parent's own.
    assert!(parent.substitute_parent_rights(directory::rights::PARENT_RIGHTS) == parent.bits());
}

/// **A directory listing's walk ends, and every entry it yields is a whole record inside the
/// buffer.** The records it yields take at most the buffer's bytes, and each takes at least its
/// two-byte header, so a buffer of `n` bytes yields at most `n / 2` entries. Kani's unwinding check
/// proves the loop ends on every buffer of up to eight bytes. Nothing in `next` reads the buffer's
/// length except through a bounds check, and each step moves forward by at least two bytes, so the
/// same count bounds a page; `a_page_of_the_smallest_records_walks_to_its_end` in the tests walks one.
/// Falsification: replayable `crates/file_system_contract/falsifications/proofs.the_directory_entry_walk_ends.patch`
#[kani::proof]
#[kani::unwind(6)]
fn the_directory_entry_walk_ends() {
    let bytes: [u8; 8] = kani::any();
    let length: usize = kani::any();
    kani::assume(length <= bytes.len());
    let (mut consumed, mut records) = (0, 0);
    for (name, _) in directory_entry::iter(&bytes[..length]) {
        consumed += directory_entry::record_len(name.len());
        records += 1;
    }
    assert!(consumed <= length);
    assert!(records * directory_entry::record_len(0) <= length);
}

/// **A name set's walk ends, and every name it yields is a whole record inside the buffer.** Each
/// record takes at least three bytes (a length, a flags byte and one byte of name), for the
/// directory listing's reason above. The walk would end after [`nameset::MAXIMUM_NAMES`] records
/// even without that, so the proof checks the stronger bound: the bytes, not the cap.
/// Falsification: replayable `crates/file_system_contract/falsifications/proofs.the_name_set_walk_ends.patch`
#[kani::proof]
#[kani::unwind(6)]
fn the_name_set_walk_ends() {
    let bytes: [u8; 8] = kani::any();
    let length: usize = kani::any();
    kani::assume(length <= bytes.len());
    let (mut consumed, mut records) = (0, 0);
    for (name, _) in nameset::iter(&bytes[..length]) {
        consumed += 2 + name.len();
        records += 1;
    }
    assert!(consumed <= length);
    assert!(records * 3 <= length);
}

/// **An attribute listing's walk ends, and every name it yields is a whole record inside the
/// buffer.** Each record takes at least two bytes, since a zero length ends the walk, for the
/// directory listing's reason above.
/// Falsification: replayable `crates/file_system_contract/falsifications/proofs.the_attribute_list_walk_ends.patch`
#[kani::proof]
#[kani::unwind(6)]
fn the_attribute_list_walk_ends() {
    let bytes: [u8; 8] = kani::any();
    let length: usize = kani::any();
    kani::assume(length <= bytes.len());
    let (mut consumed, mut records) = (0, 0);
    for name in extended_attribute::list::iter(&bytes[..length]) {
        consumed += extended_attribute::list::record_len(name.len());
        records += 1;
    }
    assert!(consumed <= length);
    assert!(records * extended_attribute::list::record_len(1) <= length);
}

/// **An attribute store's walk ends, and every attribute it yields is a whole record inside the
/// blob.** Each record takes at least its header and one byte of name, for the directory listing's
/// reason above. Sixteen bytes hold two of the smallest records.
/// Falsification: replayable `crates/file_system_contract/falsifications/proofs.the_attribute_store_walk_ends.patch`
#[kani::proof]
#[kani::unwind(4)]
fn the_attribute_store_walk_ends() {
    let bytes: [u8; 16] = kani::any();
    let length: usize = kani::any();
    kani::assume(length <= bytes.len());
    let (mut consumed, mut records) = (0, 0);
    for (name, _, value) in extended_attribute::store::iter(&bytes[..length]) {
        consumed += extended_attribute::store::record_len(name.len(), value.len());
        records += 1;
    }
    assert!(consumed <= length);
    assert!(records * extended_attribute::store::record_len(1, 0) <= length);
}

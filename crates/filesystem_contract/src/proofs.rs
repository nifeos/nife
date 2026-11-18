//! **Machine-checked proofs (Kani) of the packers and decoders**, beside the rights proofs in
//! [`dir`]: every field a word carries comes back out unchanged, no field bleeds into another, and
//! the verb table is total over the opcode space. Behind `#[cfg(kani)]`, so only `cargo kani`
//! (`script/verify`) compiles them. See notes/verification.md.
//!
//! Name: provisional, after the module every other crate's harnesses live in.

use super::*;

/// **A file request word round-trips for every in-range opcode, handle and length, and no field
/// bleeds into another.** The destination half of a rename uses the same fields and is read back
/// the same way.
/// Falsification: replayable `crates/filesystem_contract/falsifications/proofs.a_file_request_round_trips.patch`
#[kani::proof]
fn a_file_request_round_trips() {
    let opcode: u64 = kani::any();
    let handle: u64 = kani::any();
    let length: u64 = kani::any();
    kani::assume(opcode <= 0xff);
    kani::assume(handle <= fs::MAXIMUM_HANDLE);
    kani::assume(length <= fs::MAXIMUM_LENGTH_BYTES);
    let w0 = fs::req(opcode, handle, length);
    assert!(operation(w0) == opcode);
    assert!(fs::req_handle(w0) == handle);
    assert!(fs::req_len(w0) as u64 == length);
    let w1 = fs::rename_dst(handle, length);
    assert!(fs::dst_handle(w1) == handle);
    assert!(fs::dst_len(w1) as u64 == length);
}

/// **An attribute reply is non-negative for every kind and length, and round-trips.** A negative
/// reply word is an errno, so a kind that set the sign bit would read as an error.
/// Falsification: replayable `crates/filesystem_contract/falsifications/proofs.an_attribute_reply_is_never_an_error.patch`
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
/// Falsification: replayable `crates/filesystem_contract/falsifications/proofs.a_block_request_round_trips.patch`
#[kani::proof]
fn a_block_request_round_trips() {
    let opcode: u64 = kani::any();
    let blocks: usize = kani::any();
    kani::assume(opcode <= 0xff);
    kani::assume((1..=block::TRANSFER_BLOCKS).contains(&blocks));
    let w0 = block::req(opcode, blocks);
    assert!(operation(w0) == opcode);
    assert!(block::req_blocks(w0) == blocks);
}

/// **A packed name comes back whole at every length the two words hold.** Eight bytes are all the
/// proof needs per word, so the name is symbolic in full.
/// Falsification: replayable `crates/filesystem_contract/falsifications/proofs.a_packed_name_comes_back_whole.patch`
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
/// Falsification: replayable `crates/filesystem_contract/falsifications/proofs.the_verb_table_is_total.patch`
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
/// [`dir`] covers `attenuate`; this adds [`directory::Rights::resolve`] in front of it.
/// Falsification: replayable `crates/filesystem_contract/falsifications/proofs.a_resolved_request_never_widens.patch`
#[kani::proof]
fn a_resolved_request_never_widens() {
    let parent = directory::Rights::root(kani::any());
    let requested: u64 = kani::any();
    let child = parent.attenuate(parent.resolve(requested));
    assert!(child.bits() & !parent.bits() == 0);
    // `attenuate` masks whatever `resolve` answers, so the line above alone cannot see `resolve`
    // widen. The sentinel is the one word `resolve` rewrites, and it must become the parent's own.
    assert!(parent.resolve(directory::PARENT_RIGHTS) == parent.bits());
}

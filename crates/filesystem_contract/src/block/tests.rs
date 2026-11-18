use crate::*;

#[test]
fn block_request_roundtrips() {
    let w0 = req(block::READ);
    assert_eq!(operation(w0), block::READ);
    // The block index rides in the second word untouched, so any u64 survives.
    for block in [0u64, 1, 42, u64::MAX] {
        let (rw0, rw1) = (req(block::WRITE), block);
        assert_eq!(operation(rw0), block::WRITE);
        assert_eq!(rw1, block);
    }
}

/// **The block count rides the request**: a caller that packs a block count survives the round trip, and a
/// caller that never heard of one (every caller through step 3, via the plain [`req`]) still
/// decodes as exactly one block, which is the whole of the compatibility claim
/// `block::TRANSFER_BLOCKS`'s doc makes.
#[test]
fn blk_request_carries_a_block_count_and_defaults_to_one() {
    for blocks in [1usize, 2, block::TRANSFER_BLOCKS] {
        let w0 = block::req(block::READ, blocks);
        assert_eq!(operation(w0), block::READ);
        assert_eq!(block::req_blocks(w0), blocks);
    }
    // A single-block caller's word (the plain, one-argument `req`) decodes as one block.
    assert_eq!(block::req_blocks(req(block::READ)), 1);
    // `block::req(operation, 1)` and `req(operation)` are wire-identical, which is the compatibility property
    // stated: a caller that upgrades to naming one block explicitly changes nothing on the wire.
    assert_eq!(block::req(block::READ, 1), req(block::READ));
}

#[test]
#[should_panic]
fn blk_request_refuses_a_count_of_zero() {
    block::req(block::READ, 0);
}

#[test]
#[should_panic]
fn blk_request_refuses_a_count_past_the_transfer_limit() {
    block::req(block::READ, block::TRANSFER_BLOCKS + 1);
}

#[test]
fn a_filesystem_block_is_eight_sectors() {
    assert_eq!(block::BLOCK_BYTES, PAGE_BYTES);
    assert_eq!(block::SECTORS_PER_BLOCK, 8);
}

use crate::*;

#[test]
fn block_request_roundtrips() {
    let w0 = request(block::READ);
    assert_eq!(operation(w0), block::READ);
    // The block index rides in the second word untouched, so any u64 survives.
    for block in [0u64, 1, 42, u64::MAX] {
        let (rw0, rw1) = (request(block::WRITE), block);
        assert_eq!(operation(rw0), block::WRITE);
        assert_eq!(rw1, block);
    }
}

/// **The block count rides the request**: a caller that packs a block count survives the round trip, and a
/// caller that never heard of one (every caller through step 3, via the plain [`request`]) still
/// decodes as exactly one block, which is the whole of the compatibility claim
/// `block::TRANSFER_BLOCKS`'s doc makes.
#[test]
fn blk_request_carries_a_block_count_and_defaults_to_one() {
    for blocks in [1usize, 2, block::TRANSFER_BLOCKS] {
        let w0 = block::request(block::READ, blocks);
        assert_eq!(operation(w0), block::READ);
        assert_eq!(block::request_block_count(w0), Some(blocks));
    }
    // A single-block caller's word (the plain, one-argument `request`) decodes as one block.
    assert_eq!(block::request_block_count(request(block::READ)), Some(1));
    // `block::request(operation, 1)` and `request(operation)` are wire-identical, which is the compatibility property
    // stated: a caller that upgrades to naming one block explicitly changes nothing on the wire.
    assert_eq!(block::request(block::READ, 1), request(block::READ));
}

#[test]
#[should_panic]
fn blk_request_refuses_a_count_of_zero() {
    block::request(block::READ, 0);
}

#[test]
#[should_panic]
fn blk_request_refuses_a_count_past_the_transfer_limit() {
    block::request(block::READ, block::TRANSFER_BLOCKS + 1);
}

#[test]
fn a_filesystem_block_is_eight_logical_blocks() {
    assert_eq!(block::BLOCK_BYTES, PAGE_BYTES);
    assert_eq!(block::BLOCK_BYTES as u64 / block::LOGICAL_BLOCK_BYTES, 8);
}

/// **A count the shared region cannot hold is refused, not clamped**: the eight-bit field spells up
/// to 256 blocks, and every count past [`block::TRANSFER_BLOCKS`] decodes as `None`.
#[test]
fn a_block_count_past_the_region_is_refused() {
    let last = (block::READ << 56) | (block::TRANSFER_BLOCKS as u64 - 1);
    assert_eq!(
        block::request_block_count(last),
        Some(block::TRANSFER_BLOCKS)
    );
    assert_eq!(
        block::request_block_count(last + 1),
        None,
        "one block past the region"
    );
    assert_eq!(
        block::request_block_count((block::READ << 56) | 0xff),
        None,
        "the field's largest count"
    );
}

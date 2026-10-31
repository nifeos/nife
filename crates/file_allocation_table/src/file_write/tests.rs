//! The sector merge at its edges, and a write that crosses from a full volume into a refusal.
//!
//! Name: provisional.

use super::*;

#[test]
fn a_merge_keeps_the_head_zeroes_the_slack_and_lands_the_bytes() {
    let mut sector = [0xEEu8; 8];
    merge_partial(&mut sector, 5, 2, &[1, 2]);
    assert_eq!(sector, [0xEE, 0xEE, 1, 2, 0xEE, 0, 0, 0]);
    let mut fresh = [0xEEu8; 8];
    merge_partial(&mut fresh, 0, 0, &[9]);
    assert_eq!(fresh, [9, 0, 0, 0, 0, 0, 0, 0]);
    // A kept length past the buffer clamps rather than panics.
    let mut full = [3u8; 4];
    merge_partial(&mut full, 99, 3, &[7]);
    assert_eq!(full, [3, 3, 3, 7]);
}

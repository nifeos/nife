use crate::*;

/// **The `statfs` record survives a round trip and refuses a short one.**
///
/// The refusal is the half worth having: a truncated reply read as zeroes would tell a client
/// the volume is empty and zero bytes big, which is a lie in the direction that loses data.
#[test]
fn the_statfs_record_round_trips_and_a_short_reply_is_not_zeroes() {
    let mut page = [0u8; 64];
    assert_eq!(
        statfs::encode(&mut page, 4096, 16384, 9001),
        Some(statfs::LEN)
    );
    assert_eq!(statfs::decode(&page), Some((4096, 16384, 9001)));
    // Exactly the record is enough; one byte less is not readable at all.
    assert_eq!(
        statfs::decode(&page[..statfs::LEN]),
        Some((4096, 16384, 9001))
    );
    assert_eq!(statfs::decode(&page[..statfs::LEN - 1]), None);
    assert_eq!(statfs::decode(&[]), None);
    // A page too small to hold the record refuses rather than writing a partial one, and a
    // page of exactly the record, what a caller sizing from `statfs::LEN` hands it, is not.
    let mut tiny = [0u8; statfs::LEN - 1];
    assert_eq!(statfs::encode(&mut tiny, 4096, 1, 1), None);
    let mut exact = [0u8; statfs::LEN];
    assert_eq!(
        statfs::encode(&mut exact, 4096, 16384, 9001),
        Some(statfs::LEN)
    );
    assert_eq!(statfs::decode(&exact), Some((4096, 16384, 9001)));

    // The length is the version: a longer reply is a later version, and this version reads its
    // own prefix out of it rather than refusing.
    let mut longer = [0u8; statfs::LEN + 8];
    statfs::encode(&mut longer, 512, 2, 1).unwrap();
    longer[statfs::LEN..].copy_from_slice(&7u64.to_le_bytes());
    assert_eq!(statfs::decode(&longer), Some((512, 2, 1)));

    assert_eq!(statfs::total_bytes(4096, 16384), 64 * 1024 * 1024);
    // A non-zero free count, because zero is the one answer a broken `free_bytes` also gives.
    assert_eq!(statfs::free_bytes(4096, 0), 0);
    assert_eq!(statfs::free_bytes(4096, 9001), 9001 * 4096);
    assert_eq!(statfs::free_bytes(u64::MAX, 2), u64::MAX);
    // Saturating, because these numbers come off a disk: a corrupt pair must stay obviously
    // wrong instead of wrapping into a small plausible answer.
    assert_eq!(statfs::total_bytes(u64::MAX, 2), u64::MAX);
}

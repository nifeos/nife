//! The `FSInfo` sector: each signature, each hint at its edges, and a store that touches only the
//! two hints.
//!
//! Name: provisional.

use super::*;

fn sector(free: u32, next: u32) -> [u8; 512] {
    let mut s = [0u8; 512];
    s[0..4].copy_from_slice(&LEAD_SIGNATURE.to_le_bytes());
    s[484..488].copy_from_slice(&STRUCTURE_SIGNATURE.to_le_bytes());
    s[488..492].copy_from_slice(&free.to_le_bytes());
    s[492..496].copy_from_slice(&next.to_le_bytes());
    s[508..512].copy_from_slice(&TRAIL_SIGNATURE.to_le_bytes());
    s
}

#[test]
fn a_sound_sector_gives_both_hints() {
    let info = FsInfo::parse(&sector(100, 7), 1000).unwrap();
    assert_eq!(info.free_clusters(), Some(100));
    assert_eq!(info.next_free(), Some(7));
}

#[test]
fn a_hint_that_cannot_be_true_is_dropped() {
    let unknown = FsInfo::parse(&sector(u32::MAX, u32::MAX), 1000).unwrap();
    assert_eq!((unknown.free_clusters(), unknown.next_free()), (None, None));
    let past = FsInfo::parse(&sector(1001, 1002), 1000).unwrap();
    assert_eq!((past.free_clusters(), past.next_free()), (None, None));
    let edge = FsInfo::parse(&sector(1000, 1001), 1000).unwrap();
    assert_eq!(
        (edge.free_clusters(), edge.next_free()),
        (Some(1000), Some(1001))
    );
    let low = FsInfo::parse(&sector(0, 1), 1000).unwrap();
    assert_eq!((low.free_clusters(), low.next_free()), (Some(0), None));
    assert_eq!(
        FsInfo::parse(&sector(0, 2), 1000).unwrap().next_free(),
        Some(2)
    );
}

#[test]
fn each_signature_is_checked_and_a_short_sector_is_refused() {
    for at in [0, 484, 508] {
        let mut s = sector(1, 2);
        s[at] ^= 1;
        assert_eq!(FsInfo::parse(&s, 10), Err(FsInfoError::BadSignature));
    }
    assert_eq!(
        FsInfo::parse(&sector(1, 2)[..511], 10),
        Err(FsInfoError::TooShort)
    );
    assert_eq!(FsInfo::parse(&[], 10), Err(FsInfoError::TooShort));
}

#[test]
fn a_store_changes_the_two_hints_and_nothing_else() {
    let mut s = sector(100, 7);
    s[100] = 0x5A;
    let before = s;
    assert!(FsInfo::store(&mut s, Some(55), Some(9)));
    let info = FsInfo::parse(&s, 1000).unwrap();
    assert_eq!(
        (info.free_clusters(), info.next_free()),
        (Some(55), Some(9))
    );
    for (at, (a, b)) in before.iter().zip(&s).enumerate() {
        if !(488..496).contains(&at) {
            assert_eq!(a, b, "byte {at}");
        }
    }
    assert!(FsInfo::store(&mut s, None, None));
    assert_eq!(FsInfo::parse(&s, 1000).unwrap().free_clusters(), None);
    s[0] = 0;
    let broken = s;
    assert!(!FsInfo::store(&mut s, Some(1), Some(2)));
    assert_eq!(s, broken);
}

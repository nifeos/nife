//! **Machine-checked proofs (Kani) of the `FSInfo` parse and store**, over every 512-byte sector.
//! Behind `#[cfg(kani)]`, so only `cargo kani` compiles them.
//!
//! Both are straight-line code over fixed offsets, so each harness covers every sector and every
//! cluster count, not a bounded family.
//!
//! Name: ratified 2026-11-01 (UTC), by the class rule on the `tests` and `verification` modules of this crate.

use super::*;

/// **A parsed `FSInfo` is sound for its volume.** For every 512-byte sector and cluster count:
/// the parse succeeds only when the three signatures are the specification's, a free count it
/// reports is at most the cluster count, and a next-free cluster it reports is a data cluster.
/// Falsification: replayable `crates/file_allocation_table/falsifications/fs_info.verification.every_sector_parses_to_hints_inside_the_volume.patch`
#[kani::proof]
fn every_sector_parses_to_hints_inside_the_volume() {
    let sector: [u8; 512] = kani::any();
    let clusters: u32 = kani::any();
    match FsInfo::parse(&sector, clusters) {
        Err(error) => assert_eq!(error, FsInfoError::BadSignature),
        Ok(info) => {
            assert_eq!(sector[0..4], *b"RRaA");
            assert_eq!(sector[484..488], *b"rrAa");
            assert_eq!(sector[508..512], [0x00, 0x00, 0x55, 0xAA]);
            if let Some(free) = info.free_clusters() {
                assert!(free <= clusters);
            }
            if let Some(next) = info.next_free() {
                assert!(next >= 2 && next - 2 < clusters);
            }
        }
    }
}

/// **A store changes the two hints and nothing else, and only in an `FSInfo` sector.** For every
/// sector and both hints: a sector with wrong signatures is refused and left as it was; otherwise
/// every byte outside 488 to 495 is as it was and the hints read back (or read as unknown).
/// Falsification: replayable `crates/file_allocation_table/falsifications/fs_info.verification.a_store_changes_only_the_two_hints.patch`
#[kani::proof]
fn a_store_changes_only_the_two_hints() {
    let before: [u8; 512] = kani::any();
    let free: Option<u32> = kani::any();
    let next: Option<u32> = kani::any();
    let mut after = before;
    let stored = FsInfo::store(&mut after, free, next);
    if !stored {
        assert_eq!(after, before);
        return;
    }
    let mut at = 0;
    while at < 512 {
        if !(488..496).contains(&at) {
            assert_eq!(after[at], before[at]);
        }
        at += 1;
    }
    assert_eq!(
        u32::from_le_bytes([after[488], after[489], after[490], after[491]]),
        free.unwrap_or(0xFFFF_FFFF)
    );
    assert_eq!(
        u32::from_le_bytes([after[492], after[493], after[494], after[495]]),
        next.unwrap_or(0xFFFF_FFFF)
    );
    assert!(FsInfo::parse(&after, u32::MAX - 1).is_ok());
}

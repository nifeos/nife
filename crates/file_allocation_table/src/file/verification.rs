//! **Machine-checked proof (Kani) of the file read arithmetic**: the spans a read visits lie
//! inside the file and inside the volume's data clusters, over a table whose every entry the
//! solver chooses. Behind `#[cfg(kani)]`, so only `cargo kani` compiles it.
//!
//! The volume holds one to three clusters of one, two or four sectors, the bound Kani needs to
//! unroll the chain walk; the walk's own bound is the cluster count whatever it is, and the fuzz
//! target reads whole images. What a span's bytes are is the device's: the read copies the span
//! out of one sector, and that a span lies inside one sector is proved here.
//!
//! Name: ratified 2026-11-01 (UTC), by the class rule on the `tests` and `verification` modules of this crate.

use super::*;
use crate::directory::verification::fat12;
use crate::reader::verification::HostileTable;

/// **A file read stays inside its size and inside the data clusters.** From any first cluster and
/// any size, over any table contents and device failures, with seeks to any position between
/// reads and any request sizes: every span has at least one byte and lies inside one sector, that
/// sector is one of a data cluster's, the position never passes the size, and a span moves the
/// position forward by exactly its length. After an error the cursor returns only errors.
/// Falsification: replayable `crates/file_allocation_table/falsifications/file.verification.a_file_read_stays_inside_its_size_and_its_clusters.patch`
#[kani::proof]
#[kani::unwind(8)]
fn a_file_read_stays_inside_its_size_and_its_clusters() {
    let clusters: u32 = kani::any();
    kani::assume((1..=3).contains(&clusters));
    let per_cluster: u8 = match kani::any::<u8>() % 3 {
        0 => 1,
        1 => 2,
        _ => 4,
    };
    let boot = fat12(clusters, per_cluster);
    let mut table = HostileTable { clusters };
    let size: u32 = kani::any();
    let mut cursor = FileCursor::new(kani::any(), size);
    let first_data = boot.first_data_sector();
    let data_sectors = clusters * u32::from(per_cluster);
    let mut failed = false;
    for _ in 0..4 {
        if kani::any() {
            cursor.seek(&boot, kani::any());
            assert!(cursor.position() <= size);
        }
        let before = cursor.position();
        let want: usize = kani::any();
        match cursor.next_span(&mut table, &boot, want) {
            Ok(Some(span)) => {
                assert!(!failed);
                assert!(span.len >= 1 && span.len <= want);
                assert!(span.offset + span.len <= 512);
                assert!(span.sector >= first_data && span.sector < first_data + data_sectors);
                assert_eq!(cursor.position(), before + span.len as u32);
                assert!(cursor.position() <= size);
            }
            Ok(None) => {
                assert!(!failed);
                assert!(before >= size || want == 0);
                assert_eq!(cursor.position(), before);
            }
            Err(error) => {
                if failed {
                    assert_eq!(error, FileReadError::Failed);
                }
                failed = true;
            }
        }
    }
}

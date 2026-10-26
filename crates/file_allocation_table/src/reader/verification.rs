//! **Machine-checked proofs (Kani) of the chain walk**, over a table whose every entry the
//! solver chooses, fresh on each read. Behind `#[cfg(kani)]`, so only `cargo kani` compiles them.
//!
//! An entry that changes between reads covers every fixed table as well, and the hostile device
//! a stick can be. The walk reads through [`Links`], so the harness hands it entries directly;
//! that each entry's bytes lie inside the table is `table::verification`'s proof. The volume holds
//! one to eight clusters, the bound Kani needs to unroll the walk; the walk's own bound is the
//! cluster count whatever it is, and the crate's fuzz target runs the same walk over whole images.

use super::*;
use crate::boot_sector::FatType;

/// A table of `clusters` data clusters whose every entry the solver chooses, or a device error.
pub(crate) struct HostileTable {
    pub(crate) clusters: u32,
}

impl Links for HostileTable {
    type Error = ();

    fn clusters(&self) -> u32 {
        self.clusters
    }

    fn entry(&mut self, cluster: u32) -> Result<TableEntry, ChainError<()>> {
        assert!(
            is_data(self.clusters, cluster),
            "the walk read a cluster it does not have"
        );
        if kani::any() {
            return Err(ChainError::Device(()));
        }
        Ok(TableEntry::decode(FatType::Fat32, kani::any()))
    }
}

/// **No chain walk outlasts the volume.** From any first cluster, over any table contents and
/// any device failures, the walk reads only data clusters' entries, yields only data clusters, at
/// most as many as the volume holds, and has ended by the step after that; once ended it stays
/// ended.
/// Falsification: replayable `crates/file_allocation_table/falsifications/reader.verification.no_chain_walk_outlasts_the_volume.patch`
#[kani::proof]
#[kani::unwind(11)]
fn no_chain_walk_outlasts_the_volume() {
    let clusters: u32 = kani::any();
    kani::assume((1..=8).contains(&clusters));
    let mut table = HostileTable { clusters };
    let mut walk = Walk::new(kani::any());
    let mut yielded = 0;
    let mut ended = false;
    for _ in 0..clusters + 2 {
        match walk.step(&mut table) {
            Some(Ok(cluster)) => {
                assert!(cluster >= 2 && cluster <= clusters + 1);
                yielded += 1;
            }
            Some(Err(_)) | None => {
                ended = true;
                break;
            }
        }
    }
    assert!(ended);
    assert!(yielded <= clusters);
    assert!(walk.step(&mut table).is_none());
}

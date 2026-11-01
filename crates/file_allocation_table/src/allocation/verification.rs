//! **Machine-checked proofs (Kani) of allocation and release**, over a model table of one to
//! four data clusters whose every entry the solver chooses. Behind `#[cfg(kani)]`, so only
//! `cargo kani` compiles them.
//!
//! The operations read and write the table through [`LinksMut`], so the harnesses hand them a
//! model that asserts every access lies on a data cluster and every write is one a table may
//! hold. That each write lands in every live copy of the table, and that a FAT12 entry keeps its
//! neighbor, are `table::verification`'s proof and the unit tests'. The bound is the unrolling
//! Kani needs: the operations' own loops are bounded by the cluster count whatever it is, and the
//! oracle tests run them over volumes of tens of thousands.
//!
//! Name: ratified 2026-11-01 (UTC), by the class rule on the `tests` and `verification` modules of this crate.

use super::*;

/// Most data clusters the model holds.
const MOST: usize = 4;

/// A table of `clusters` data clusters held as entries.
pub(crate) struct Model {
    pub(crate) clusters: u32,
    pub(crate) entries: [TableEntry; MOST],
}

impl Links for Model {
    type Error = ();

    fn clusters(&self) -> u32 {
        self.clusters
    }

    fn table_entry(&mut self, cluster: u32) -> Result<TableEntry, ChainError<()>> {
        assert!(is_data(self.clusters, cluster), "a read outside the table");
        Ok(self.entries[(cluster - 2) as usize])
    }
}

impl LinksMut for Model {
    fn set_table_entry(&mut self, cluster: u32, entry: TableEntry) -> Result<(), ChainError<()>> {
        assert!(is_data(self.clusters, cluster), "a write outside the table");
        assert!(
            !matches!(entry, TableEntry::Reserved | TableEntry::Bad),
            "a write of an entry the crate never stores"
        );
        self.entries[(cluster - 2) as usize] = entry;
        Ok(())
    }

    fn append(&mut self, last: Option<u32>) -> Result<Option<u32>, ChainError<()>> {
        let Some(cluster) = allocate(self, 2)? else {
            return Ok(None);
        };
        if let Some(last) = last {
            self.set_table_entry(last, TableEntry::Next(cluster))?;
        }
        Ok(Some(cluster))
    }
}

/// Any entry a table can decode to, with cluster numbers the solver chooses.
fn any_entry() -> TableEntry {
    let next: u32 = kani::any();
    kani::assume(next >= 2);
    match kani::any::<u8>() % 5 {
        0 => TableEntry::Free,
        1 => TableEntry::Reserved,
        2 => TableEntry::Next(next),
        3 => TableEntry::Bad,
        _ => TableEntry::EndOfChain,
    }
}

/// A model of one to [`MOST`] clusters with solver-chosen entries.
fn any_model() -> Model {
    let clusters: u32 = kani::any();
    kani::assume((1..=MOST as u32).contains(&clusters));
    Model {
        clusters,
        entries: [any_entry(), any_entry(), any_entry(), any_entry()],
    }
}

/// **Allocation takes one free cluster and changes nothing else.** From any hint over any table:
/// it returns a data cluster that was free and is now end-of-chain with every other entry as it
/// was, or it returns none and no entry was free.
/// Falsification: replayable `crates/file_allocation_table/falsifications/allocation.verification.allocation_takes_one_free_cluster_and_nothing_else.patch`
#[kani::proof]
#[kani::unwind(6)]
fn allocation_takes_one_free_cluster_and_nothing_else() {
    let mut model = any_model();
    let before = model.entries;
    let clusters = model.clusters;
    let hint: u32 = kani::any();
    match allocate(&mut model, hint) {
        Ok(Some(cluster)) => {
            assert!(is_data(clusters, cluster));
            let taken = (cluster - 2) as usize;
            assert_eq!(before[taken], TableEntry::Free);
            assert_eq!(model.entries[taken], TableEntry::EndOfChain);
            let mut at = 0;
            while at < clusters as usize {
                if at != taken {
                    assert_eq!(model.entries[at], before[at]);
                }
                at += 1;
            }
        }
        Ok(None) => {
            let mut at = 0;
            while at < clusters as usize {
                assert_ne!(before[at], TableEntry::Free);
                assert_eq!(model.entries[at], before[at]);
                at += 1;
            }
        }
        Err(_) => unreachable!("the model never fails"),
    }
}

/// **A release frees only clusters of the chain it was given.** From any first cluster over any
/// table: every entry the release changed was a link or an end marker and is now free, a bad or
/// reserved cluster is never changed, and a successful release freed exactly the clusters it
/// reports, at most the volume's cluster count.
/// Falsification: replayable `crates/file_allocation_table/falsifications/allocation.verification.a_release_frees_only_its_own_chain.patch`
#[kani::proof]
#[kani::unwind(8)]
fn a_release_frees_only_its_own_chain() {
    let mut model = any_model();
    let before = model.entries;
    let clusters = model.clusters;
    let first: u32 = kani::any();
    let result = free_chain(&mut model, first);
    let mut changed = 0;
    let mut at = 0;
    while at < clusters as usize {
        if model.entries[at] != before[at] {
            assert_eq!(model.entries[at], TableEntry::Free);
            assert!(matches!(
                before[at],
                TableEntry::Next(_) | TableEntry::EndOfChain
            ));
            changed += 1;
        }
        at += 1;
    }
    if let Ok(freed) = result {
        assert_eq!(freed, changed);
        assert!(freed >= 1 && freed <= clusters);
    } else {
        assert!(changed <= clusters);
    }
}

/// How many clusters the chain from `first` has in `model`, counting to the end marker; `None`
/// when it breaks or loops.
fn length(model: &mut Model, first: u32) -> Option<u32> {
    let mut walk = Walk::new(first);
    let mut count = 0;
    loop {
        match walk.step(model) {
            Some(Ok(_)) => count += 1,
            Some(Err(_)) => return None,
            None => return Some(count),
        }
    }
}

/// **A cut keeps exactly the head of the chain.** From any first cluster, any `keep` and any
/// table whose chain is whole: afterwards the chain from `first` has `min(keep, length)` clusters
/// and ends properly, and the release reports the clusters it dropped. A `keep` of 0 leaves the
/// chain freed.
/// Falsification: replayable `crates/file_allocation_table/falsifications/allocation.verification.a_cut_keeps_exactly_the_head_of_the_chain.patch`
#[kani::proof]
#[kani::unwind(8)]
fn a_cut_keeps_exactly_the_head_of_the_chain() {
    let mut model = any_model();
    let first: u32 = kani::any();
    kani::assume(is_data(model.clusters, first));
    let Some(whole) = length(&mut model, first) else {
        return;
    };
    let keep: u32 = kani::any();
    kani::assume(keep <= MOST as u32 + 1);
    let freed = truncate_chain(&mut model, first, keep).expect("the model never fails");
    if keep == 0 {
        assert_eq!(freed, whole);
        assert_eq!(model.entries[(first - 2) as usize], TableEntry::Free);
    } else {
        let kept = whole.min(keep);
        assert_eq!(freed, whole - kept);
        assert_eq!(length(&mut model, first), Some(kept));
    }
}

/// A model of `clusters` clusters that are all free.
pub(crate) fn free_model(clusters: u32) -> Model {
    assert!(clusters as usize <= MOST);
    Model {
        clusters,
        entries: [TableEntry::Free; MOST],
    }
}

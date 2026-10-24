use super::test_programs::spawn_progenitor_shaped;
use crate::cap::Rights;
use crate::sched::RendezvousId;
use crate::thread::ThreadId;

/// The witness's budget, in pages. Counted rather than measured, from what it retypes: a
/// rendezvous, the child's address space and its tables, the child's two image pages and four of
/// stack, the timebase page on x86_64 and riscv64, a thread control block, the probe frame and the
/// tables under it, and the tables under the witness's own scratch window. That comes to about
/// thirty; the rest is headroom, so a failure to build reads as a defect rather than as a budget a
/// release build outgrew.
const BUDGET_PAGES: u64 = 64;

/// What the test needs back: the verdict line, and what to reap and reclaim once it has spoken.
pub struct Witness {
    /// The report endpoint carrying the verdict bits.
    pub report: RendezvousId,
    /// The region `report` was carved from, reclaimed by the test once it has the verdict.
    pub report_region: u64,
    /// The witness's own thread. Its space is bound to it and goes when it is reaped.
    pub tid: ThreadId,
    /// The region the witness's thread control block was retyped from.
    pub thread_control_block_region: u64,
    /// The witness's budget: the child's space, its thread, the rendezvous and the probe frame.
    pub budget: u64,
}

/// Spawn the witness (`fixtures/src/process_composition_witness.rs`); returns its verdict line and
/// what it holds.
///
/// Its endowment is the progenitor's shape, which is `builder`'s: the archive read-only at
/// `INITIAL_RAMDISK_VA` with its length in `x1`, a memory region in slot 0, and a `WRITE`-only
/// report line in slot 1, narrower than the progenitor's `WRITE|GRANT` because the witness never
/// delegates it. The region is the lending budget, every right: the witness endows its child's slot
/// 0 through `CAP_INSERT`, which gates on `GRANT`, and milestone 824 (a retype mints no right its
/// budget lacks) means the budget must carry `GRANT` itself, as `memory_region_lending_capability`
/// mints it.
pub fn wire() -> Witness {
    let report_region = crate::memory_region::create(1).expect("no region for the report line");
    let report = crate::sched::create_rendezvous_from(report_region).expect("no report rendezvous");
    let started = spawn_progenitor_shaped(
        "process_composition_witness",
        BUDGET_PAGES,
        Rights::ALL,
        report,
        Rights::WRITE,
        &[],
        None,
        0,
    );
    Witness {
        report,
        report_region,
        tid: started.tid,
        thread_control_block_region: started.thread_control_block_region,
        budget: started.budget,
    }
}

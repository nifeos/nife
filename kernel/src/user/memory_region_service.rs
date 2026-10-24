use super::*;
use crate::cap::{Rights, memory_region_cap, rendezvous_cap};
use crate::sched::RendezvousId;

/// The program this wires, `fixtures/src/memory_region_depleter.rs`, which reads nothing from
/// `x0`.
pub fn depleter_image() -> &'static [u8] {
    program("memory_region_depleter").expect("no memory_region_depleter program in the archive")
}

/// Carve `pages` of memory into an untyped region, hand it to a fresh process, and return the
/// region id, the endpoint the process reports on, and the thread it runs as. The kernel's ONE
/// allocation is the untyped itself; everything the process maps afterward spends that, not the
/// allocator.
///
/// The `ThreadId` is returned because this process does not exit: it reports and then spins, so
/// the free-frame count its caller reads is the measurement's, not a teardown's. A caller that
/// drops it leaks a spinning thread onto every later test.
pub fn start(
    image: &'static [u8],
    pages: u64,
) -> Option<(u64, RendezvousId, crate::thread::ThreadId)> {
    let region = crate::memory_region::create(pages)?;
    let report = crate::sched::create_rendezvous();

    let tid = crate::sched::spawn(move || {
        run(
            image,
            Spawn {
                arg0: 0, // one job, no role selector
                arg1: 0,
                arg2: 0,
                grants: &[
                    memory_region_cap(region),             // slot 0: the memory budget
                    rendezvous_cap(report, Rights::WRITE), // slot 1: report the result
                ],
                maps: &[],
            },
        )
    })
    .expect("could not spawn the untyped demo");

    Some((region, report, tid))
}

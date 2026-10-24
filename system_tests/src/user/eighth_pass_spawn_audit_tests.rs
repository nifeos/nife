//! What a spawned child holds, and what a child with nothing can still build: nothing (the
//! eighth outsider pass at the confinement claim, claims 23, 31 and 32).
//!
//! It attacks `script/swish-check`'s unvouched census and `authority_tests`' drop-and-retry from
//! the kernel side, where the census is exact rather than printed. Claim 31: a child granted the
//! unvouched shape holds those three capabilities and nothing else. Claim 32: no device capability
//! appears in it; the boot half (the spawn service delegates the gpu's four rather than parking
//! them in the shell) is `swish-check`'s census, cited. Claim 23: a child born with one report
//! endpoint gets `NoSuchSlot` for every mint. Portable, per DECISIONS §19 (architectural parity is
//! a tenet). See notes/confinement-outsider-pass-8.md.

use super::test_programs::kernel_test_subject_image;
use crate::cap::{
    Object, Rights, page_frame_run_cap as page_frame_run_capability, page_frame_run_len,
    rendezvous_cap as rendezvous_capability,
};
use crate::sched;
use crate::user::{KERNEL_TEST_SUBJECT_BUILD_WITH_NOTHING, Spawn, run};

/// The interrupt ignorer's ELF bytes: a `_start` that is nothing but a loop, so the table census
/// below reads a live, settled child. `el0_boundary_tests` has the same helper; this file keeps
/// its own because that one is private and it is one line either way.
fn interrupt_ignorer_image() -> &'static [u8] {
    crate::user::program("interrupt_ignorer")
        .expect("no interrupt_ignorer program in the initial ramdisk")
}

/// The objects a thread's table holds, as `(slot, object, rights)` for the occupied slots and how
/// many there are. The array's tail beyond the count is padding, not data.
fn census(tid: u64) -> ([(u64, Object, Rights); 8], usize) {
    let mut out = [(0, Object::Rendezvous(0, 0), Rights::NONE); 8];
    let mut n = 0;
    sched::with_capability_table(tid, |t| {
        for slot in 0..crate::cap::CAPABILITY_TABLE_SLOTS as u64 {
            if let Ok(held) = t.get(slot) {
                assert!(
                    n < out.len(),
                    "the child holds more capabilities than the census buffer holds"
                );
                out[n] = (slot, held.object, held.rights);
                n += 1;
            }
        }
    })
    .expect("the child's table could not be read");
    (out, n)
}

/// A child granted one endpoint and two read-only pages holds exactly those, and no device.
///
/// The interrupt ignorer spins forever, so the census reads a live table. All sixty-four slots are
/// read: the three grants must be the only entries, with exactly the rights handed in. No untyped
/// budget (what separates unvouched from endowed), no second endpoint, and no device capability
/// (`DeviceFrame`, or a port range on `x86_64`), which is claim 32's half.
///
/// Falsification: unfalsified. Written by the eighth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn a_child_holds_exactly_what_its_caller_granted_and_no_device() {
    let region = crate::memory_region::create(4).expect("no region");
    let report = sched::create_rendezvous_from(region).expect("no report rendezvous");
    let page_a = crate::memory_region::retype_page(region).expect("no first page");
    let page_b = crate::memory_region::retype_page(region).expect("no second page");

    let child = sched::spawn(move || {
        run(
            interrupt_ignorer_image(),
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_capability(report, Rights::WRITE),
                    page_frame_run_capability(page_a, page_frame_run_len(1), Rights::READ),
                    page_frame_run_capability(page_b, page_frame_run_len(1), Rights::READ),
                ],
                maps: &[],
            },
        );
    })
    .expect("no child thread");

    // The grants land while the child loads, so the census waits for the table to reach the
    // expected three before asserting anything about exactness: reading a half-filled table
    // would report the spawn path's own progress rather than the child's endowment.
    assert!(
        super::wait_for(|| {
            sched::with_capability_table(child, |t| {
                (0..crate::cap::CAPABILITY_TABLE_SLOTS as u64)
                    .filter(|s| t.get(*s).is_ok())
                    .count()
                    == 3
            })
            .unwrap_or(false)
        }),
        "the child's table never settled on the three granted capabilities",
    );

    let (held, occupied) = census(child);
    assert_eq!(
        occupied, 3,
        "the child's table holds {occupied} capabilities, not the three granted"
    );
    assert_eq!(
        held[0],
        (0, Object::Rendezvous(report, 0), Rights::WRITE),
        "slot 0 did not hold the report endpoint with exactly WRITE",
    );
    assert_eq!(
        held[1].0, 1,
        "the read-only pages did not land in the slots after the endpoint"
    );
    assert_eq!(
        held[1].2,
        Rights::READ,
        "a granted read-only page carried more than READ",
    );
    assert_eq!(
        held[2],
        (
            2,
            Object::PageFrame(page_b, page_frame_run_len(1)),
            Rights::READ
        ),
        "the second read-only page was not at slot 2 with exactly READ",
    );
    assert_eq!(
        held[1].1,
        Object::PageFrame(page_a, page_frame_run_len(1)),
        "the first read-only page was not at slot 1",
    );
    // Claim 32's half, said once more in the claim's own words: no device capability of any kind.
    for (_slot, object, _rights) in held.iter().take(occupied) {
        let is_a_device = matches!(object, Object::DeviceFrame(..));
        #[cfg(target_arch = "x86_64")]
        let is_a_device = is_a_device || matches!(object, Object::PortRange(..));
        assert!(
            !is_a_device,
            "a spawned child with no device grant holds a device capability: {object:?}",
        );
    }

    sched::kill_thread(child);
    assert!(
        super::wait_for(|| !sched::is_thread_present(child)),
        "the child never died after the kill",
    );
    sched::reclaim_region(region).expect("the region did not come back");
}

/// A program with no construction authority cannot build, from inside, through the real syscall.
///
/// The `kernel_test_subject`'s `BUILD_WITH_NOTHING` role reports the errnos of a `RETYPE`, a
/// `SEND` and a `RETYPE_OBJ` on ungranted slots. Each must be `NoSuchSlot` ("nothing there"), not
/// "you may not use it": authority is a capability in a slot, and no slot was filled. That is claim
/// 23's zero-authority corner beside the cited drop-and-retry.
///
/// Falsification: unfalsified. Written by the eighth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn a_child_with_no_construction_authority_cannot_build() {
    let region = crate::memory_region::create(4).expect("no region");
    let report = sched::create_rendezvous_from(region).expect("no report rendezvous");

    let kernel_test_subject = sched::spawn(move || {
        run(
            kernel_test_subject_image(),
            Spawn {
                arg0: KERNEL_TEST_SUBJECT_BUILD_WITH_NOTHING,
                arg1: 0,
                arg2: 0,
                grants: &[rendezvous_capability(report, Rights::WRITE)],
                maps: &[],
            },
        );
    })
    .expect("no kernel_test_subject thread");

    let answer = sched::ipc_receive(report);
    assert_eq!(
        answer[0],
        abi::Error::NoSuchSlot as u64,
        "a RETYPE on an ungranted slot answered something other than NoSuchSlot",
    );
    assert_eq!(
        answer[1],
        abi::Error::NoSuchSlot as u64,
        "a SEND on an ungranted slot answered something other than NoSuchSlot",
    );
    assert_eq!(
        answer[2],
        abi::Error::NoSuchSlot as u64,
        "a RETYPE_OBJ on an ungranted slot answered something other than NoSuchSlot",
    );

    assert!(
        super::wait_for(|| !sched::is_thread_present(kernel_test_subject)),
        "the kernel_test_subject never exited after its report",
    );
    sched::reclaim_region(region).expect("the region did not come back");
}

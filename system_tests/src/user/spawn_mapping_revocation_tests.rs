//! Revocation against a mapping the kernel wired, rather than one a capability made.
//!
//! Every unmap sweep in `crate::revoke` is driven by the mapping log, which
//! [`crate::revoke::MappingHold::record_mapping`] writes. [`AddressSpace::map_physical`] records
//! too, taking [`crate::revoke::PageMapSource`] as a required argument, so a page the kernel wires
//! into a process it builds (a [`super::Spawn`]`::maps` entry, a [`super::DeviceRun`], the initial
//! ramdisk, the `x86_64` timebase page) is unmapped by `PageFrame::REVOKE`, `DeviceFrame::REVOKE`
//! and `MemoryRegion::DESTROY`. The module is named to sort before `thread_leak_police`, whose
//! header says why. See notes/confinement-claims/adversarial-pass-2026-09-21.md.

//! # BUGS
//!
//! - The test drives `PageFrame::REVOKE`'s sweep; [`DeviceFrame::REVOKE`] and
//!   [`MemoryRegion::DESTROY`] read the same log and are reasoned from the code.
//! - It proves the mapping is gone, not that the process faults on it.
//! - `AddressSpace::map_new` does not record, deliberately: no capability names its frames.

use paging::Flags;

use super::{
    AddressSpace, take_user_address_space, user_address_space_create, user_address_space_map,
    user_address_space_root,
};
use crate::arch::mmu;
use crate::revoke::PageMapSource;

/// Where the recorded mapping lands. Two distinct low addresses, so a walk of one space's tables
/// can never be mistaken for a walk of the other's.
const RECORDED_VA: u64 = address_space_map::pair_page(0x0050_0000);
/// Where the kernel-wired mapping lands.
const WIRED_VA: u64 = address_space_map::pair_page(0x0060_0000);

/// A page wired into an address space by the kernel must be unmapped when its frame is revoked.
///
/// One frame is mapped into two live spaces, by [`user_address_space_map`] (the `MAP_INTO` engine)
/// and by [`AddressSpace::map_physical`] (as `Spawn::maps` does). `PageFrame::REVOKE` then runs
/// whole-machine ([`crate::revoke::revoke_page_frame`], the single-page case of
/// `revoke_page_frame_run`), and neither space may still map it. The headline is the last assertion,
/// reached on every path; a vacuity guard (mapped in both places) and a premise check (the revoke
/// reached the recorded mapping) above it each say, failing, that the test proved nothing.
///
/// Falsification: replayable `system_tests/falsifications/user.spawn_mapping_revocation_tests.a_page_the_kernel_wired_is_unmapped_when_its_frame_is_revoked.patch`
#[test_case]
fn a_page_the_kernel_wired_is_unmapped_when_its_frame_is_revoked() {
    // Out of the frame allocator rather than out of a region, which is where every real
    // `Spawn::maps` frame comes from too (`fs_service::page_frame`, `credential_service::page_frame`
    // and the stack-page loops all call `memory::alloc_zeroed`). It also keeps the revoke below a
    // pure capability question: `PageFrame::REVOKE` reclaims nothing, so nobody's budget moves.
    let shared = crate::memory::alloc().expect("no frame to share").addr();

    // The recorded half: a user-built address space, mapped through the `MAP_INTO` engine. Thirty-two
    // pages is the root, its tables, and its log page, with room to spare.
    let region = crate::memory_region::create(32).expect("no region for the recorded space");
    let recorded = user_address_space_create(region).expect("no recorded address space");
    user_address_space_map(
        recorded,
        RECORDED_VA,
        shared,
        Flags::user_data(),
        // The frame is named by no capability here, exactly as it is named by none at boot. The
        // record is filed under the page itself, which is what a single-page `REVOKE` looks for.
        PageMapSource::NoCapability,
    )
    .expect("the recorded space could not map the shared frame");
    let recorded_root = user_address_space_root(recorded).expect("the recorded space has no root");

    // The wired half: a space the kernel built and mapped into directly, which is `Spawn::maps`.
    let mut wired = AddressSpace::new(4).expect("no wired address space");
    wired
        .map_physical(
            WIRED_VA,
            shared,
            Flags::user_data(),
            PageMapSource::NoCapability,
        )
        .expect("the wired space could not map the shared frame");
    let wired_root = wired.root();

    assert!(
        mmu::translate_at(recorded_root, RECORDED_VA).is_some()
            && mmu::translate_at(wired_root, WIRED_VA).is_some(),
        "the shared frame was not mapped in both spaces before the revoke, so there was nothing \
         for the revoke to miss and this test proves nothing",
    );

    crate::revoke::revoke_page_frame(shared);

    assert!(
        mmu::translate_at(recorded_root, RECORDED_VA).is_none(),
        "the revoke did not reach even the recorded mapping, so this test's premise is false and \
         its result means nothing",
    );

    assert!(
        mmu::translate_at(wired_root, WIRED_VA).is_none(),
        "a page the kernel wired into an address space survived the revoke of its frame: \
         `map_physical` filed no record, so every unmap sweep in `crate::revoke` walked past a \
         live mapping of a frame whose capabilities are all gone",
    );

    // Teardown, innermost first: take the recorded space out of the registry so its `Drop` runs
    // (it forgets its root and frees its ASID; its backing is `Lent`, so the region survives),
    // then the wired space's own `Drop`, then the region and the frame.
    drop(take_user_address_space(recorded));
    drop(wired);
    crate::memory_region::destroy(region);
    crate::memory::free(page_frames::PageFrame::from_addr(shared));
}

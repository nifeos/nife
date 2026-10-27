//! `pmap_tests`: `abi::address_space::LIST` proved against a real `Object::AddressSpace`, driven by
//! `pmap::collect`, the real program's real loop. `survey_tests`'s shape, one object type over;
//! see that module's header for the discipline this one repeats rather than restates.

use abi::Error;

use crate::arch::exceptions::TrapFrame;
use crate::capability_object::Rights;
use crate::sched;
use crate::syscall::invoke;

/// The space's own budget: the root, its intermediate tables, and the mapping-record log. Nothing
/// here maps more than three pages, so this is generous slack rather than a tight bound.
const SPACE_PAGES: u64 = 24;

/// A fresh address space, backed by its own region, the `RETYPE_OBJ(ADDRESS_SPACE)` engine driven
/// directly rather than through a spawned builder thread: this kernel test *is* the builder.
/// Returns the space's `name` and the region backing it, so [`tidy`] can give the region back.
fn arena_space() -> (u64, u64) {
    let region = crate::memory_region::create(SPACE_PAGES).expect("no space region");
    let name = crate::user::user_address_space_create(region).expect("no address space");
    (name, region)
}

/// Hold the space **the way a builder holds it**: `WRITE`, the authority `MAP_INTO` takes.
fn hold_builder(name: u64) -> u64 {
    sched::grant(crate::capability_object::address_space_capability(
        name,
        Rights::WRITE,
    ))
    .expect("grant the address space")
}

/// Hold the space **the way `pmap` holds it**: `ENUMERATE` and nothing else. Not that a viewer is
/// refused `MAP_INTO`; it cannot name it, because that takes `WRITE` and this capability does not
/// carry it.
fn hold_viewer(name: u64) -> u64 {
    sched::grant(crate::capability_object::address_space_capability(
        name,
        Rights::ENUMERATE,
    ))
    .expect("grant the address space")
}

/// A fresh frame, from its own one-page region, granted into a fresh capability table slot with `rights`.
/// Returns the slot and the region, so [`tidy`] can give the region back.
fn page_frame(rights: Rights) -> (u64, u64) {
    let region = crate::memory_region::create(1).expect("no frame region");
    let phys = crate::memory_region::retype_page(region).expect("no frame");
    let slot = sched::grant(crate::capability_object::page_frame_capability(
        phys, rights,
    ))
    .expect("grant the frame");
    (slot, region)
}

/// **Give everything back**, `survey_tests::tidy`'s discipline verbatim, one object type over:
/// every granted slot deleted first, then every region this test carved reclaimed. Skipping this
/// is not cosmetic -- kernel tests share one running kernel across the whole suite, so a slot left
/// occupied here is a slot `reap_tests::assert_can_only_supervise` (or any later test walking its
/// own capability table) finds unexpectedly full, and a region left unreclaimed is budget no later test gets
/// back. This module's first draft omitted this entirely and cost `reap_tests` a failure two tests
/// later in the same run, which is what this comment is here to stop recurring.
fn tidy(slots: &[u64], regions: &[u64]) {
    for &s in slots {
        let _ = sched::delete_current_capability(s);
    }
    for &r in regions {
        sched::reclaim_region(r).expect("a region this test carved did not come back");
    }
}

/// `invoke(capability, MAP_INTO, va, frame_slot, mode)`, through the real dispatcher.
fn map_into(aspace_slot: u64, va: u64, frame_slot: u64, mode: u64) -> Result<i64, Error> {
    let mut fr = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    invoke(
        &mut fr,
        aspace_slot,
        abi::address_space::MAP_INTO,
        va,
        frame_slot,
        mode,
    )
}

/// `invoke(capability, LIST, cursor, _, _)`, through the real dispatcher, returning the three words a
/// userspace caller would read out of its registers.
fn list(slot: u64, cursor: u64) -> (i64, u64, u64) {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    match invoke(&mut frame, slot, abi::address_space::LIST, cursor, 0, 0) {
        Ok(next) => (next, frame.arg(1), frame.arg(2)),
        Err(e) => (e as i64, 0, 0),
    }
}

/// **The whole address space, walked by the real program's real loop.** `pmap::collect` is what
/// `components/src/pmap.rs` runs; driving it here rather than reimplementing the cursor walk is
/// `survey_tests::walk`'s discipline verbatim.
const TEST_ROWS: usize = 8;

fn walk(slot: u64, rows: &mut [pmap::Row; TEST_ROWS]) -> pmap::Listing<'_> {
    let l = pmap::collect(rows, &mut |cursor| list(slot, cursor));
    assert!(
        l.is_complete() || l.is_refused(),
        "the listing outgrew this test's row buffer, so what it reported is not the space",
    );
    l
}

/// **A viewer holding `ENUMERATE` alone sees every mapping and can change none of them.**
///
/// This is the deliverable: three mappings, three different `MAP_INTO` modes, and the listing
/// reports every `va` with the permission `MAP_INTO` actually granted it, read back through
/// `arch::mmu::translate_at` rather than asserted from the call that made them.
#[test_case]
fn a_viewer_sees_every_mapping_and_can_touch_none_of_it() {
    let (name, space_region) = arena_space();
    let builder = hold_builder(name);
    let viewer = hold_viewer(name);

    const CODE_VA: u64 = address_space_map::pair_page(0x0040_0000);
    const RW_VA: u64 = address_space_map::pair_page(0x0050_0000);
    const RO_VA: u64 = address_space_map::pair_page(0x0060_0000);

    let (code_frame, code_region) = page_frame(Rights::READ);
    assert_eq!(
        map_into(builder, CODE_VA, code_frame, abi::address_space::MAP_CODE),
        Ok(0)
    );
    let (rw_frame, rw_region) = page_frame(Rights::WRITE);
    assert_eq!(
        map_into(builder, RW_VA, rw_frame, abi::address_space::MAP_RW),
        Ok(0)
    );
    let (ro_frame, ro_region) = page_frame(Rights::READ);
    assert_eq!(
        map_into(builder, RO_VA, ro_frame, abi::address_space::MAP_RO),
        Ok(0)
    );

    let mut rows = [pmap::Row::default(); TEST_ROWS];
    let l = walk(viewer, &mut rows);
    assert!(!l.is_refused());
    assert_eq!(l.rows().len(), 3, "{:?}", l.rows());
    assert!(
        l.rows().contains(&pmap::Row {
            va: CODE_VA,
            kind: abi::address_space::MAP_CODE
        }),
        "{:?}",
        l.rows()
    );
    assert!(
        l.rows().contains(&pmap::Row {
            va: RW_VA,
            kind: abi::address_space::MAP_RW
        }),
        "{:?}",
        l.rows()
    );
    assert!(
        l.rows().contains(&pmap::Row {
            va: RO_VA,
            kind: abi::address_space::MAP_RO
        }),
        "{:?}",
        l.rows()
    );

    // **The other half of the claim.** The same capability that just listed every mapping is
    // refused the one thing this test's builder capability can do: change one.
    let (spare, spare_region) = page_frame(Rights::READ);
    assert_eq!(
        map_into(viewer, 0x0070_0000, spare, abi::address_space::MAP_RO),
        Err(Error::NotPermitted),
        "ENUMERATE let a viewer map a page; that is the exact power SURVEY's split exists to deny",
    );

    tidy(
        &[builder, viewer, code_frame, rw_frame, ro_frame, spare],
        &[
            space_region,
            code_region,
            rw_region,
            ro_region,
            spare_region,
        ],
    );
}

/// **The negative control's other half: `WRITE` alone cannot list.** A builder capability, minted
/// with full `MAP_INTO` authority, does not carry the right `LIST` needs, symmetric to
/// `survey_tests::a_viewer_without_the_domain_is_refused_rather_than_shown_an_empty_list`.
#[test_case]
fn a_builder_without_enumerate_is_refused_the_listing() {
    let (name, space_region) = arena_space();
    let builder = hold_builder(name);

    assert_eq!(list(builder, 0), (Error::NotPermitted as i64, 0, 0));

    tidy(&[builder], &[space_region]);
}

/// **A space with nothing mapped answers, rather than refusing.** Neither claim above means
/// anything without this one: an empty result must not be indistinguishable from a refusal.
#[test_case]
fn an_empty_space_is_a_real_answer_not_a_refusal() {
    let (name, space_region) = arena_space();
    let viewer = hold_viewer(name);

    let mut rows = [pmap::Row::default(); TEST_ROWS];
    let l = walk(viewer, &mut rows);
    assert!(!l.is_refused());
    assert!(l.rows().is_empty());
    assert_eq!(l.complaint(), Some("this address space has nothing mapped"));

    tidy(&[viewer], &[space_region]);
}

/// **An empty slot is `NoSuchSlot`, not `NotPermitted`.** No-ambient-authority feels like "there is
/// nothing there", not "permission denied": `survey_tests`'s distinction, one object type over.
#[test_case]
fn an_empty_slot_holds_no_capability_at_all() {
    // A slot nothing has ever granted into: `sched::current_capability` finds nothing there, the same
    // fact `abi::address_space::LIST`'s dispatch checks before it ever looks at rights. Nothing to tidy:
    // this test grants nothing and carves no region.
    let mut fr = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    let empty_slot = 15; // the highest capability table slot; nothing in this test has granted into it
    assert_eq!(
        invoke(&mut fr, empty_slot, abi::address_space::LIST, 0, 0, 0),
        Err(Error::NoSuchSlot),
    );
}

/// **A capability that outlived its space reads as empty, not refused.**
///
/// The space is ended here the way its thread's reaper ends it since §249 (a running address space
/// stays nameable): `take_user_address_space` takes it out of the registry, and it drops. A
/// capability minted before that still decodes to the same `name`; the syscall handler's
/// `user_address_space_root` lookup is what actually notices the space is gone, and the documented
/// answer is `DONE`, symmetric to `sched::survey_supervised`'s "before the scheduler exists there
/// is no domain to report."
#[test_case]
fn a_capability_outliving_its_space_reads_as_empty() {
    let (name, space_region) = arena_space();
    let viewer = hold_viewer(name);

    // Map one page first, so a bug that kept reading the (freed) log would show a row here rather
    // than accidentally passing.
    let builder = hold_builder(name);
    let (f, frame_region) = page_frame(Rights::READ);
    assert_eq!(
        map_into(builder, 0x0040_0000, f, abi::address_space::MAP_RO),
        Ok(0)
    );

    // End the space the way its thread's reaper would, without building a thread: the registry
    // entry goes away exactly as it would there. `take_user_address_space`'s removal from
    // `USER_SPACES` is what `user_address_space_root` (and so `LIST`) actually notices. Dropped
    // explicitly, before `tidy` reclaims the space's own region, so the ASID and revocation
    // bookkeeping its `Drop` does (`Backing::Lent`, so it frees no memory) happens in the order a
    // thread's death would give it, rather than at the end of scope.
    let space = crate::user::take_user_address_space(name).expect("the space was still registered");
    drop(space);

    assert_eq!(list(viewer, 0), (abi::survey::DONE as i64, 0, 0));

    tidy(&[viewer, builder, f], &[space_region, frame_region]);
}

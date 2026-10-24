//! Build real aarch64 page tables, in real memory, on the host.
//!
//! The trick: a `Box<PageTable>` is 4 KiB-aligned (the type says so) and has a real
//! address. So we hand those addresses to the mapper as "physical" frames, and
//! `phys_to_ptr` is the identity cast. **The pointer arithmetic is bit-for-bit what the
//! kernel does.** We are testing the actual code path, not a model of it.
//!
//! Runs in milliseconds. No emulator, no hardware, no MMU.

use std::cell::{Cell, RefCell};

use paging::{
    Aarch64, Flags, Half, MapError, Mapper, PAGE_SIZE, PageFormat, PageTable, TlbFlush, WalkEnd,
};

/// **Discharge `flush` without invalidating**, and the one place this file argues
/// [`TlbFlush::assume_no_stale_entry`]'s contract. Every table here lives on the host heap and is
/// never installed in a TTBR, `satp` or CR3, so no hardware has walked it and no TLB can hold an
/// entry for it.
fn never_installed(flush: TlbFlush) {
    // SAFETY: see this function's doc; a host test's tables are never walked by hardware.
    unsafe { flush.assume_no_stale_entry() }
}

thread_local! {
    /// Every table the pretend frame allocator hands out, so `TableGuard` can give them back.
    /// Thread-local because tests run in parallel and each test's tables are its own.
    static TABLES: RefCell<Vec<*mut PageTable>> = const { RefCell::new(Vec::new()) };
}

/// Frees the test's tables when it ends, so Miri's leak check stays meaningful
/// (notes/undefined-behavior.md). Declared FIRST in each test that builds a mapper, so it drops
/// last, after the mapper that reads the tables is gone.
///
/// # BUGS
///
/// Binding this is opt-in: a test added here must declare `let _tables = TableGuard;` first, and
/// nothing enforces it. `src/domain.rs` and `tests/blocks.rs` hang allocation off the pool, so
/// forgetting is a compile error there; this file's twenty-one call sites are not converted.
struct TableGuard;
impl Drop for TableGuard {
    fn drop(&mut self) {
        TABLES.with(|t| {
            for p in t.borrow_mut().drain(..) {
                // SAFETY: `p` came from `Box::into_raw` in this thread's allocator below,
                // registered exactly once; the drain removes it so nothing frees it twice.
                unsafe { drop(Box::from_raw(p)) };
            }
        });
    }
}

/// `Box::into_raw` with a receipt: the guard above owns the eventual free.
fn fresh_table() -> u64 {
    let p = Box::into_raw(Box::new(PageTable::new()));
    TABLES.with(|t| t.borrow_mut().push(p));
    p as u64
}

/// This host test suite drives the aarch64 format specifically (the format-neutral `Mapper`
/// behaviour is identical for Sv39; the aarch64 encoding is what these older tests pin). The Sv39
/// format's own arithmetic and round-trips are proved in `paging::sv39`.
type Fmt = Aarch64;

/// The `index` free function moved onto the format; alias it so the arithmetic tests read unchanged.
fn index(va: u64, level: usize) -> usize {
    Fmt::index(va, level)
}

/// A pretend physical frame allocator backed by the host heap. The tables outlive the mapper
/// (they are freed by the test's `TableGuard`, not here), which is the ownership shape the real
/// kernel has too: the mapper borrows tables, the frame allocator owns them.
fn page_frame_source(budget: &Cell<usize>) -> impl FnMut() -> Option<u64> + '_ {
    move || {
        if budget.get() == 0 {
            return None;
        }
        budget.set(budget.get() - 1);
        Some(fresh_table())
    }
}

fn phys_to_ptr(pa: u64) -> *mut PageTable {
    pa as *mut PageTable
}

#[allow(clippy::type_complexity)] // an opaque closure + fn pointer + format; no `type` alias without TAIT
fn mapper_in(
    half: Half,
    budget: &Cell<usize>,
) -> Mapper<impl FnMut() -> Option<u64> + '_, fn(u64) -> *mut PageTable, Fmt> {
    let root = fresh_table();
    // SAFETY: `root` is a fresh, zeroed, 4 KiB-aligned table, and `phys_to_ptr` is the
    // identity, which is correct because these "physical" addresses ARE host addresses.
    unsafe {
        Mapper::new(
            root,
            half,
            page_frame_source(budget),
            phys_to_ptr as fn(u64) -> *mut PageTable,
        )
    }
}

#[allow(clippy::type_complexity)]
fn mapper(
    budget: &Cell<usize>,
) -> Mapper<impl FnMut() -> Option<u64> + '_, fn(u64) -> *mut PageTable, Fmt> {
    mapper_in(Half::Low, budget)
}

/// Each level takes a 9-bit slice. L0 = 47:39, L1 = 38:30, L2 = 29:21, L3 = 20:12.
#[test]
fn index_slices_the_address_correctly() {
    let va = (1u64 << 39) | (2 << 30) | (3 << 21) | (4 << 12) | 0xabc;

    assert_eq!(index(va, 0), 1);
    assert_eq!(index(va, 1), 2);
    assert_eq!(index(va, 2), 3);
    assert_eq!(index(va, 3), 4);
}

/// 9 bits. Anything above bit 47 belongs to the sign-extension, not to L0.
#[test]
fn index_wraps_at_512() {
    assert_eq!(index(0x1ff << 39, 0), 511);
    assert_eq!(index(0xfff << 12, 3), 511);
}

#[test]
fn map_then_translate_round_trips() {
    let _tables = TableGuard;
    let budget = Cell::new(16);
    let mut m = mapper(&budget);

    m.map(0x4008_0000, 0x4008_0000, Flags::kernel_code())
        .unwrap();

    let (pa, flags) = m.translate(0x4008_0000).expect("should be mapped");
    assert_eq!(pa, 0x4008_0000);
    assert_eq!(flags, Flags::kernel_code());
}

#[test]
fn translate_carries_the_offset_within_the_page() {
    let _tables = TableGuard;
    let budget = Cell::new(16);
    let mut m = mapper(&budget);

    m.map(0x1000, 0x4000_0000, Flags::kernel_data()).unwrap();

    // The low 12 bits never go through translation. They are the offset.
    let (pa, _) = m.translate(0x1abc).unwrap();
    assert_eq!(pa, 0x4000_0abc);
}

#[test]
fn unmapped_addresses_translate_to_nothing() {
    let _tables = TableGuard;
    let budget = Cell::new(16);
    let mut m = mapper(&budget);
    m.map(0x1000, 0x4000_0000, Flags::kernel_data()).unwrap();

    assert_eq!(m.translate(0x2000), None);
    assert_eq!(m.translate(0xffff_0000_0000_0000), None);
}

/// The entire point of the exercise, and what milestone 4 (MMU on: page tables, the kernel heap,
/// the high half) step 4 depends on.
#[test]
fn a_virtual_address_can_differ_from_its_physical_one() {
    let _tables = TableGuard;
    let budget = Cell::new(16);
    let mut m = mapper(&budget);

    m.map(0x1000, 0x4008_0000, Flags::kernel_code()).unwrap();

    assert_eq!(m.translate(0x1000).unwrap().0, 0x4008_0000);
    assert_eq!(
        m.translate(0x4008_0000),
        None,
        "the physical address itself is not mapped; only the VA we chose is"
    );
}

// --- the halves: the thing a failing test taught us ---

#[test]
// "TABLE" is shouted on purpose: bits 63:48 choose which table, not an index within one.
#[allow(non_snake_case)]
fn the_top_16_bits_are_not_translated_they_choose_the_TABLE() {
    // This is the crux of how a higher-half kernel works, and it is not what you'd guess.
    //
    // Bits 63:48 are NOT part of any index. `index()` reads bits 47:12 and nothing else. So
    // within a single table, these two addresses are THE SAME ENTRY:
    let high = 0xffff_0000_4008_0000u64;
    let low = 0x0000_0000_4008_0000u64;

    for level in 0..4 {
        assert_eq!(
            index(high, level),
            index(low, level),
            "level {level} indices differ, but they must not"
        );
    }

    // Which means the kernel does not live in the high half because high addresses index
    // somewhere else. It lives there because TTBR1 IS A DIFFERENT SET OF TABLES, and the
    // hardware picks between TTBR0 and TTBR1 using exactly those untranslated top bits.
    assert!(Fmt::is_in_half(Half::High, high));
    assert!(Fmt::is_in_half(Half::Low, low));
    assert!(!Fmt::is_in_half(Half::Low, high));
    assert!(!Fmt::is_in_half(Half::High, low));
}

/// Without this check, mapping a kernel address into the userspace tables silently
/// builds a mapping the CPU will never consult, because it would pick TTBR1 for that
/// address and never look at this table at all. You would then chase the ghost for
/// hours.
#[test]
fn mapping_into_the_wrong_half_is_refused() {
    let _tables = TableGuard;
    let budget = Cell::new(16);

    let mut low = mapper_in(Half::Low, &budget);
    assert_eq!(
        low.map(0xffff_0000_0000_0000, 0x1000, Flags::kernel_data()),
        Err(MapError::WrongHalf)
    );

    let mut high = mapper_in(Half::High, &budget);
    assert_eq!(
        high.map(0x1000, 0x1000, Flags::kernel_data()),
        Err(MapError::WrongHalf)
    );
}

/// Top bits neither all-zero nor all-one. There is no memory there and there never can
/// be: the hardware faults before it consults any table.
#[test]
fn non_canonical_addresses_belong_to_neither_half() {
    let junk = 0x0001_0000_0000_0000u64;
    assert!(!Fmt::is_in_half(Half::Low, junk));
    assert!(!Fmt::is_in_half(Half::High, junk));
}

#[test]
fn the_high_half_maps_normally_once_you_are_in_it() {
    let _tables = TableGuard;
    let budget = Cell::new(16);
    let mut m = mapper_in(Half::High, &budget);

    let va = 0xffff_0000_4008_0000;
    m.map(va, 0x4008_0000, Flags::kernel_code()).unwrap();

    assert_eq!(m.translate(va).unwrap().0, 0x4008_0000);
}

/// Two pages in the same 2 MiB region differ only in their L3 index, so the walk should
/// create L1/L2/L3 once and reuse them. If it doesn't, we burn a frame per page and run
/// out of memory mapping a kernel.
#[test]
fn nearby_pages_share_their_intermediate_tables() {
    let _tables = TableGuard;
    let budget = Cell::new(3); // exactly enough for ONE chain of L1+L2+L3
    let mut m = mapper(&budget);

    m.map(0x4000_0000, 0x4000_0000, Flags::kernel_data())
        .unwrap();
    assert_eq!(budget.get(), 0, "first mapping should consume L1+L2+L3");

    // The next page needs no new tables at all.
    m.map(0x4000_1000, 0x4000_1000, Flags::kernel_data())
        .expect("should reuse the existing tables");
    assert_eq!(budget.get(), 0);
}

#[test]
fn running_out_of_page_frames_is_an_error_not_a_panic() {
    let _tables = TableGuard;
    let budget = Cell::new(0);
    let mut m = mapper(&budget);

    assert_eq!(
        m.map(0x1000, 0x1000, Flags::kernel_data()),
        Err(MapError::OutOfPageFrames)
    );
}

#[test]
fn misaligned_addresses_are_rejected() {
    let _tables = TableGuard;
    let budget = Cell::new(16);
    let mut m = mapper(&budget);

    assert_eq!(
        m.map(0x1001, 0x1000, Flags::kernel_data()),
        Err(MapError::Misaligned)
    );
    assert_eq!(
        m.map(0x1000, 0x1001, Flags::kernel_data()),
        Err(MapError::Misaligned)
    );
}

/// Silently overwriting is how you lose a page and never find out: the old physical
/// frame is still marked used by the allocator, and nothing references it any more.
#[test]
fn mapping_over_an_existing_mapping_is_an_error() {
    let _tables = TableGuard;
    let budget = Cell::new(16);
    let mut m = mapper(&budget);

    m.map(0x1000, 0x4000_0000, Flags::kernel_data()).unwrap();
    assert_eq!(
        m.map(0x1000, 0x5000_0000, Flags::kernel_data()),
        Err(MapError::AlreadyMapped)
    );
}

#[test]
fn map_range_maps_every_page() {
    let _tables = TableGuard;
    let budget = Cell::new(16);
    let mut m = mapper(&budget);

    m.map_range(0x4000_0000, 0x8000_0000, 4, Flags::kernel_data())
        .unwrap();

    for i in 0..4u64 {
        let (pa, _) = m.translate(0x4000_0000 + i * PAGE_SIZE).unwrap();
        assert_eq!(pa, 0x8000_0000 + i * PAGE_SIZE);
    }
    assert_eq!(m.translate(0x4000_0000 + 4 * PAGE_SIZE), None);
}

// --- the portable access-flag behaviour (the aarch64 bit encodings, AF and the MAIR slot, are
// pinned by unit tests in paging::aarch64 where those constants are in scope) ---

/// W^X. A page that is both writable and executable is how a buffer overflow becomes
/// code execution. There is deliberately no constructor that produces one, and this
/// test exists so that adding one is a build failure rather than a security hole.
#[test]
fn nothing_is_both_writable_and_executable() {
    for flags in [
        Flags::kernel_code(),
        Flags::kernel_rodata(),
        Flags::kernel_data(),
        Flags::device(),
        Flags::user_code(),
        Flags::user_data(),
    ] {
        let executable = flags.is_kernel_executable() || flags.is_user_executable();
        assert!(
            !(flags.is_writable() && executable),
            "{flags:?} is writable AND executable"
        );
    }
}

#[test]
fn kernel_code_is_executable_by_the_kernel_and_nobody_else() {
    let f = Flags::kernel_code();
    assert!(f.is_kernel_executable());
    assert!(!f.is_user_executable());
    assert!(!f.is_writable());
    assert!(!f.is_user_accessible());
}

/// PXN is not paranoia. Without it, a bug that jumps the kernel into a user page
/// executes USER-CONTROLLED INSTRUCTIONS AT EL1. Total compromise, and the defence is
/// one bit.
#[test]
fn user_code_is_never_executable_by_the_kernel() {
    let f = Flags::user_code();
    assert!(f.is_user_executable());
    assert!(!f.is_kernel_executable(), "PXN is not set on user code");
    assert!(f.is_user_accessible());
}

/// Mapping MMIO as *normal* memory lets the CPU cache it, reorder writes to it, merge
/// two writes into one, and speculatively read it. Every one of those is catastrophic
/// for a device, because reading a FIFO register HAS A SIDE EFFECT.
#[test]
fn device_memory_is_typed_as_device_and_is_never_executable() {
    let f = Flags::device();

    assert!(f.is_device(), "MMIO is not typed as device memory");
    assert!(!f.is_kernel_executable());
    assert!(!f.is_user_executable());
    assert!(f.is_writable(), "we do need to write to the UART");
}

// --- unmap, and the TLB obligation you cannot forget ---

#[test]
fn unmap_removes_the_mapping_and_returns_the_page_frame() {
    let _tables = TableGuard;
    let budget = Cell::new(16);
    let mut m = mapper(&budget);

    m.map(0x1000, 0x4000_0000, Flags::kernel_data()).unwrap();
    assert!(m.translate(0x1000).is_some());

    let (pa, flush) = m.unmap(0x1000).unwrap();

    // The frame comes BACK, it isn't dropped. The mapper doesn't own it: the caller took it
    // from the frame allocator and the caller must give it back. Silently dropping it would
    // leak a page per unmap, which at process teardown is a leak per page of every process
    // that ever exits.
    assert_eq!(pa, 0x4000_0000);
    assert_eq!(m.translate(0x1000), None, "the mapping survived unmap");

    never_installed(flush);
}

#[test]
fn the_tlb_obligation_names_the_address_it_is_about() {
    let _tables = TableGuard;
    let budget = Cell::new(16);
    let mut m = mapper(&budget);

    m.map(0x8000, 0x4000_0000, Flags::kernel_data()).unwrap();
    let (_, flush) = m.unmap(0x8000).unwrap();

    assert_eq!(flush.address(), 0x8000);

    // And `flush()` hands the address to whatever the architecture wants to do with it. The
    // paging crate stays pure: it emits no instructions.
    let mut invalidated = None;
    flush.flush(|va| invalidated = Some(va));
    assert_eq!(invalidated, Some(0x8000));
}

#[test]
fn unmapping_nothing_is_an_error_not_a_silent_success() {
    let _tables = TableGuard;
    let budget = Cell::new(16);
    let mut m = mapper(&budget);

    match m.unmap(0x1000) {
        Err(MapError::NotMapped) => {}
        other => panic!("expected NotMapped, got {other:?}"),
    }
}

/// On aarch64, changing a VALID descriptor directly into a different VALID descriptor is
/// architecturally unsafe: it can raise a TLB conflict abort, and the hardware is permitted
/// to do essentially anything.
///
/// `AlreadyMapped` is what forces the legal sequence. You CANNOT overwrite; you must unmap
/// (which hands you a `TlbFlush` you cannot ignore) and then map. The API cannot be used
/// incorrectly, rather than merely documenting the rule and hoping.
#[test]
fn changing_a_mapping_is_forced_through_break_before_make() {
    let _tables = TableGuard;
    let budget = Cell::new(16);
    let mut m = mapper(&budget);

    m.map(0x1000, 0x4000_0000, Flags::kernel_data()).unwrap();

    // The illegal move is refused.
    assert_eq!(
        m.map(0x1000, 0x5000_0000, Flags::kernel_data()),
        Err(MapError::AlreadyMapped),
    );

    // The legal one: BREAK...
    let (old, flush) = m.unmap(0x1000).unwrap();
    assert_eq!(old, 0x4000_0000);

    // ...invalidate...
    never_installed(flush);

    // ...then MAKE.
    m.map(0x1000, 0x5000_0000, Flags::kernel_data()).unwrap();
    assert_eq!(m.translate(0x1000).unwrap().0, 0x5000_0000);
}

/// unmap only clears the leaf. The L1/L2/L3 tables stay, so re-mapping into the same region
/// costs no new frames.
///
/// The flip side: tearing down a whole address space by `unmap` would leak these tables. The
/// kernel never does that; it frees the recorded frame set wholesale (notes/teardown.md).
#[test]
fn unmap_then_map_reuses_the_intermediate_tables() {
    let _tables = TableGuard;
    let budget = Cell::new(3); // exactly one chain of L1+L2+L3
    let mut m = mapper(&budget);

    m.map(0x1000, 0x4000_0000, Flags::kernel_data()).unwrap();
    assert_eq!(budget.get(), 0);

    let (_, flush) = m.unmap(0x1000).unwrap();
    never_installed(flush);

    // No frames left in the budget, and this still works: the tables were kept.
    m.map(0x1000, 0x5000_0000, Flags::kernel_data())
        .expect("intermediate tables were thrown away");
}

/// `#[must_use]` catches `m.unmap(va);` as a bare statement. It does NOT catch
/// `let (pa, _) = m.unmap(va)?;`, which is exactly the shape the mistake takes in real
/// code. Rust has no linear types, so the only way to make "you must consume this"
/// enforceable is to make NOT consuming it fail loudly.
#[test]
#[should_panic(expected = "TLB was never invalidated")]
fn dropping_the_tlb_obligation_is_fatal() {
    let _tables = TableGuard;
    let budget = Cell::new(16);
    let mut m = mapper(&budget);
    m.map(0x1000, 0x4000_0000, Flags::kernel_data()).unwrap();

    let (_pa, _flush) = m.unmap(0x1000).unwrap();
    // _flush drops here, un-discharged. Boom.
}

/// **Cutting a table takes its whole span out of the walk, and nothing beside it.** The kernel
/// cuts a table whose page is about to be handed back (`revoke::revoke_region`); this pins what
/// the cut does to the walk.
///
/// The second chain's L2 is cut. Both pages under it stop translating, the page under the first
/// chain's L2 keeps translating, the span reported is the 1 GiB that L2 covered, and a table below
/// the cut one is no longer reachable to cut. A later map rebuilds the walk from fresh tables
/// rather than descending into the cut one, which is the property the kernel needs: after the cut,
/// no store goes into a page that has been given back.
#[test]
fn unlinking_a_table_cuts_its_whole_span_and_nothing_else() {
    let _tables = TableGuard;
    let taken = RefCell::new(Vec::new());
    let alloc = || {
        let t = fresh_table();
        taken.borrow_mut().push(t);
        Some(t)
    };
    // SAFETY: a fresh, zeroed, aligned root; the identity is right for host addresses.
    let mut m = unsafe {
        Mapper::<_, _, Fmt>::new(
            fresh_table(),
            Half::Low,
            alloc,
            phys_to_ptr as fn(u64) -> *mut PageTable,
        )
    };

    const GIB: u64 = 1 << 30;
    m.map(0x1000, 0x4000_0000, Flags::user_data()).unwrap();
    let first_chain = taken.borrow().len();
    m.map(2 * GIB, 0x4000_1000, Flags::user_data()).unwrap();
    m.map(2 * GIB + 0x1000, 0x4000_2000, Flags::user_data())
        .unwrap();
    // The second map reused L1 and took a fresh L2 and L3.
    let (l2, l3) = {
        let t = taken.borrow();
        assert_eq!(t.len(), first_chain + 2, "the second chain should share L1");
        (t[first_chain], t[first_chain + 1])
    };

    let (base, span, flush) = m
        .unlink_table(2 * GIB + 0x1000, l2)
        .expect("the walk to that page passes through its L2");
    never_installed(flush);
    assert_eq!((base, span), (2 * GIB, GIB));

    assert!(m.translate(2 * GIB).is_none());
    assert!(m.translate(2 * GIB + 0x1000).is_none());
    assert_eq!(m.translate(0x1000).map(|(pa, _)| pa), Some(0x4000_0000));
    assert!(
        m.unlink_table(2 * GIB, l3).is_none(),
        "a table below the cut is no longer on any walk"
    );
    assert!(
        m.unlink_table(2 * GIB, l2).is_none(),
        "a table is cut once; the second ask finds nothing"
    );

    let before = taken.borrow().len();
    m.map(2 * GIB, 0x4000_3000, Flags::user_data()).unwrap();
    assert_eq!(
        taken.borrow().len(),
        before + 2,
        "a map after the cut must build fresh tables, not write into the cut one"
    );
}

/// **A walk of a mapped address reads one entry per level and ends at a present leaf**, and its
/// steps name the tables `translate` would have descended: the root first, each next table the
/// previous entry's address. The leaf entry decodes to the frame that was mapped.
#[test]
fn a_walk_of_a_mapped_address_reports_every_level_down_to_the_leaf() {
    let _tables = TableGuard;
    let budget = Cell::new(16);
    let mut m = mapper(&budget);
    m.map(0x40_2000, 0x4000_0000, Flags::user_data()).unwrap();

    let walk = m.walk(0x40_2000, |_| true);
    let steps = walk.steps();
    assert_eq!(walk.end(), WalkEnd::Mapped);
    assert_eq!(steps.len(), Fmt::LEVELS);
    assert_eq!(steps[0].table, m.root());
    for (level, pair) in steps.windows(2).enumerate() {
        assert_eq!(pair[1].table, Fmt::entry_pa(pair[0].entry), "level {level}");
    }
    for (level, step) in steps.iter().enumerate() {
        assert_eq!(step.index, index(0x40_2000, level));
    }
    assert_eq!(Fmt::entry_pa(steps[Fmt::LEVELS - 1].entry), 0x4000_0000);
}

/// **A cleared leaf stops the walk at the last level with the zero it found**, while the levels
/// above it still read as present. That shape is the one a kill report has to tell apart from a
/// table that was never built (which stops higher up).
#[test]
fn a_walk_stops_at_the_entry_that_is_not_present_and_keeps_what_it_read() {
    let _tables = TableGuard;
    let budget = Cell::new(16);
    let mut m = mapper(&budget);
    m.map(0x40_2000, 0x4000_0000, Flags::user_data()).unwrap();
    let (_, flush) = m.unmap(0x40_2000).unwrap();
    flush.flush(|_| {});

    let walk = m.walk(0x40_2000, |_| true);
    assert_eq!(walk.end(), WalkEnd::NotPresent);
    assert_eq!(walk.steps().len(), Fmt::LEVELS);
    assert_eq!(walk.steps()[Fmt::LEVELS - 1].entry, 0);

    let never = m.walk(0x80_0000_0000, |_| true);
    assert_eq!(never.end(), WalkEnd::NotPresent);
    assert_eq!(
        never.steps().len(),
        1,
        "nothing was ever built under the root's second entry"
    );
}

/// **A table the caller will not let it read ends the walk without reading it**, named by its
/// address. This is what keeps a fault handler from following a corrupted entry into memory the
/// direct map does not cover.
#[test]
fn a_walk_never_reads_a_table_the_caller_refuses() {
    let _tables = TableGuard;
    let budget = Cell::new(16);
    let mut m = mapper(&budget);
    m.map(0x40_2000, 0x4000_0000, Flags::user_data()).unwrap();
    let second = Fmt::entry_pa(m.walk(0x40_2000, |_| true).steps()[0].entry);

    let walk = m.walk(0x40_2000, |table| table != second);
    assert_eq!(walk.end(), WalkEnd::Unreadable(second));
    assert_eq!(walk.steps().len(), 1);
    assert_eq!(m.walk(0x40_2000, |_| false).steps().len(), 0);
    assert_eq!(
        m.walk(0xffff_0000_0000_0000, |_| true).end(),
        WalkEnd::OtherHalf
    );
}

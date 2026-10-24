//! Blocks (2 MiB and 1 GiB leaves) in real tables, on the host, for every CPU format.
//!
//! The same trick as `mapping.rs`: a `Box<PageTable>` is 4 KiB-aligned and its address serves as a
//! "physical" frame. Blocks never dereference the address they map, so the mapped `pa`s here are
//! plain numbers; only the tables are real memory.
//!
//! Generic over the format, because the point of `PageFormat::block_entry` and `is_block` is that
//! the one walk in `Mapper` handles blocks for all three, and a test that ran on one format would
//! say nothing about the other two encodings, which differ (aarch64 clears a bit, x86 sets one,
//! Sv39 changes nothing but the level).

use std::cell::{Cell, RefCell};

use paging::{
    Aarch64, Flags, Half, Ia32e, MapError, Mapper, PAGE_SIZE, PageFormat, PageSize, PageTable,
    Sv39, Sv39Mae, TlbFlush, Vtd,
};

/// **Discharge `flush` without invalidating**, and the one place this file argues
/// [`TlbFlush::assume_no_stale_entry`]'s contract. Every table here lives on the host heap and is
/// never installed in a TTBR, `satp` or CR3, so no hardware has walked it and no TLB can hold an
/// entry for it.
fn never_installed(flush: TlbFlush) {
    // SAFETY: see this function's doc; a host test's tables are never walked by hardware.
    unsafe { flush.assume_no_stale_entry() }
}

/// The pretend frame allocator. **The mapper borrows it**, so the borrow checker, not a guard a
/// test has to remember to bind, is what keeps the tables alive until the mapper is gone and frees
/// them afterwards (the shape of `src/domain.rs`'s fixture).
struct Pool {
    tables: RefCell<Vec<*mut PageTable>>,
    budget: Cell<usize>,
}

impl Pool {
    fn new(budget: usize) -> Self {
        Pool {
            tables: RefCell::new(Vec::new()),
            budget: Cell::new(budget),
        }
    }

    fn fresh(&self) -> Option<u64> {
        if self.budget.get() == 0 {
            return None;
        }
        self.budget.set(self.budget.get() - 1);
        let p = Box::into_raw(Box::new(PageTable::new()));
        self.tables.borrow_mut().push(p);
        Some(p as u64)
    }

    /// How many tables the mapper has taken, root included.
    fn used(&self) -> usize {
        self.tables.borrow().len()
    }

    #[allow(clippy::type_complexity)]
    fn mapper<F: PageFormat>(
        &self,
        half: Half,
    ) -> Mapper<impl FnMut() -> Option<u64> + '_, fn(u64) -> *mut PageTable, F> {
        let root = self.fresh().expect("no budget for a root");
        // SAFETY: `root` is fresh, zeroed and 4 KiB-aligned; `phys_to_ptr` is the identity, which
        // is right because these "physical" addresses are host addresses.
        unsafe {
            Mapper::new(
                root,
                half,
                move || self.fresh(),
                (|pa| pa as *mut PageTable) as fn(u64) -> *mut PageTable,
            )
        }
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        for p in self.tables.borrow_mut().drain(..) {
            // SAFETY: each `p` came from `Box::into_raw` in `fresh`, registered exactly once.
            unsafe { drop(Box::from_raw(p)) };
        }
    }
}

const MIB2: u64 = 2 << 20;
const GIB1: u64 = 1 << 30;

/// A low-half address every format can name: 1 GiB aligned, well under Sv39's 256 GiB half.
const VA: u64 = 4 * GIB1;
/// A physical address different from `VA`, so a translation that forgot the offset or used the
/// virtual address shows up.
const PA: u64 = 7 * GIB1;

fn a_block_translates_with_its_offset<F: PageFormat>() {
    let pool = Pool::new(8);
    let mut m = pool.mapper::<F>(Half::Low);
    for (size, flags) in [
        (PageSize::Size2MiB, Flags::user_data()),
        (PageSize::Size1GiB, Flags::user_rodata()),
    ] {
        let va = VA + if size == PageSize::Size2MiB { 0 } else { GIB1 };
        m.map_block(va, PA, size, flags).unwrap();
        for off in [0, PAGE_SIZE, size.bytes() / 2 + 0x123, size.bytes() - 1] {
            assert_eq!(
                m.translate(va + off),
                Some((PA + off, flags)),
                "{size:?} at offset {off:#x}"
            );
        }
        assert_eq!(m.translate(va + size.bytes()), None, "{size:?} overran");
    }
}

#[test]
fn a_block_translates_with_its_offset_on_every_format() {
    a_block_translates_with_its_offset::<Aarch64>();
    a_block_translates_with_its_offset::<Sv39>();
    a_block_translates_with_its_offset::<Ia32e>();
}

/// **Only a table is ever cut.** `unlink_table` asked for the frame a leaf or a block maps
/// answers `None` and leaves the mapping in force: the walk stops at a block, and it never reads
/// the last level, where every entry is a leaf.
fn a_leaf_or_a_block_is_never_cut_as_a_table<F: PageFormat>() {
    let pool = Pool::new(8);
    let mut m = pool.mapper::<F>(Half::Low);
    let flags = Flags::user_data();
    m.map(VA, PA, flags).unwrap();
    m.map_block(VA + GIB1, PA, PageSize::Size1GiB, flags)
        .unwrap();
    assert!(m.unlink_table(VA, PA).is_none(), "a 4 KiB leaf");
    assert!(m.unlink_table(VA + GIB1, PA).is_none(), "a 1 GiB block");
    assert_eq!(m.translate(VA), Some((PA, flags)));
    assert_eq!(m.translate(VA + GIB1), Some((PA, flags)));
}

#[test]
fn a_leaf_or_a_block_is_never_cut_as_a_table_on_every_format() {
    a_leaf_or_a_block_is_never_cut_as_a_table::<Aarch64>();
    a_leaf_or_a_block_is_never_cut_as_a_table::<Sv39>();
    a_leaf_or_a_block_is_never_cut_as_a_table::<Sv39Mae>();
    a_leaf_or_a_block_is_never_cut_as_a_table::<Ia32e>();
}

/// **A page cannot be mapped inside a block**, and the refusal happens before anything is
/// written: the walk would otherwise take the block's 2 MiB frame for a page table.
fn a_page_inside_a_block_is_refused<F: PageFormat>() {
    let pool = Pool::new(8);
    let mut m = pool.mapper::<F>(Half::Low);
    m.map_block(VA, PA, PageSize::Size2MiB, Flags::user_data())
        .unwrap();
    let used = pool.used();
    assert_eq!(
        m.map(VA + 5 * PAGE_SIZE, 0x1000, Flags::user_data()),
        Err(MapError::AlreadyMapped)
    );
    assert_eq!(pool.used(), used, "the refusal allocated a table");
    assert_eq!(
        m.translate(VA + 5 * PAGE_SIZE),
        Some((PA + 5 * PAGE_SIZE, Flags::user_data())),
        "the block changed"
    );
    // And a block over a block, and a 2 MiB block under a 1 GiB one.
    assert_eq!(
        m.map_block(VA, PA, PageSize::Size2MiB, Flags::user_data()),
        Err(MapError::AlreadyMapped)
    );
    m.map_block(VA + GIB1, PA, PageSize::Size1GiB, Flags::user_data())
        .unwrap();
    assert_eq!(
        m.map_block(VA + GIB1 + MIB2, PA, PageSize::Size2MiB, Flags::user_data()),
        Err(MapError::AlreadyMapped)
    );
}

#[test]
fn a_page_inside_a_block_is_refused_on_every_format() {
    a_page_inside_a_block_is_refused::<Aarch64>();
    a_page_inside_a_block_is_refused::<Sv39>();
    a_page_inside_a_block_is_refused::<Ia32e>();
}

/// **`unmap` does not take a 4 KiB bite out of a block**, and says why rather than `NotMapped`.
fn unmapping_inside_a_block_is_refused<F: PageFormat>() {
    let pool = Pool::new(8);
    let mut m = pool.mapper::<F>(Half::Low);
    m.map_block(VA, PA, PageSize::Size2MiB, Flags::user_data())
        .unwrap();
    assert_eq!(m.unmap(VA).map(|_| ()), Err(MapError::InsideBlock));
    assert_eq!(
        m.unmap(VA + PAGE_SIZE).map(|_| ()),
        Err(MapError::InsideBlock)
    );
    assert!(m.translate(VA).is_some());
}

#[test]
fn unmapping_inside_a_block_is_refused_on_every_format() {
    unmapping_inside_a_block_is_refused::<Aarch64>();
    unmapping_inside_a_block_is_refused::<Sv39>();
    unmapping_inside_a_block_is_refused::<Ia32e>();
}

/// **A block does not replace a table**, even one whose pages have all been unmapped: the table
/// frame would be orphaned, and the mapper does not own frames to free them.
fn a_block_over_a_table_is_refused<F: PageFormat>() {
    let pool = Pool::new(8);
    let mut m = pool.mapper::<F>(Half::Low);
    m.map(VA + PAGE_SIZE, 0x1000, Flags::user_data()).unwrap();
    let (_, flush) = m.unmap(VA + PAGE_SIZE).unwrap();
    never_installed(flush);
    assert_eq!(
        m.map_block(VA, PA, PageSize::Size2MiB, Flags::user_data()),
        Err(MapError::AlreadyMapped)
    );
}

#[test]
fn a_block_over_a_table_is_refused_on_every_format() {
    a_block_over_a_table_is_refused::<Aarch64>();
    a_block_over_a_table_is_refused::<Sv39>();
    a_block_over_a_table_is_refused::<Ia32e>();
}

#[test]
fn a_misaligned_block_is_refused() {
    let pool = Pool::new(8);
    let mut m = pool.mapper::<Ia32e>(Half::Low);
    assert_eq!(
        m.map_block(VA + PAGE_SIZE, PA, PageSize::Size2MiB, Flags::user_data()),
        Err(MapError::Misaligned)
    );
    assert_eq!(
        m.map_block(VA, PA + MIB2, PageSize::Size1GiB, Flags::user_data()),
        Err(MapError::Misaligned)
    );
    assert_eq!(pool.used(), 1, "a refused block allocated a table");
}

/// **VT-d declines blocks** rather than writing a bit its reserved-bit check forbids.
#[test]
fn vtd_declines_blocks() {
    let pool = Pool::new(8);
    let mut m = pool.mapper::<Vtd>(Half::Low);
    assert_eq!(
        m.map_block(VA, PA, PageSize::Size2MiB, Flags::user_data()),
        Err(MapError::UnsupportedSize)
    );
    assert_eq!(pool.used(), 1);
    // And a 4 KiB "block" is a page, on every format, this one included.
    m.map_block(VA, PA, PageSize::Size4KiB, Flags::user_data())
        .unwrap();
    assert!(m.translate(VA).is_some());
}

/// **`map_span` maps exactly the pages `map_range` would**, a page head, blocks through the middle
/// and a page tail, and nothing either side of the span. Checked page by page over the whole span
/// against the arithmetic, which is the property the direct map rests on.
fn a_span_maps_exactly_its_pages<F: PageFormat>() {
    // Starts one page below a 2 MiB boundary and ends three pages past the second one after it, so
    // there is a one-page head, two blocks, and a three-page tail.
    let va = VA + MIB2 - PAGE_SIZE;
    let pa = PA + MIB2 - PAGE_SIZE;
    let len = PAGE_SIZE + 2 * MIB2 + 3 * PAGE_SIZE;

    let blocks = Pool::new(16);
    let mut m = blocks.mapper::<F>(Half::Low);
    m.map_span(va, pa, len, Flags::kernel_rodata(), PageSize::Size1GiB)
        .unwrap();
    for off in (0..len).step_by(PAGE_SIZE as usize) {
        assert_eq!(
            m.translate(va + off),
            Some((pa + off, Flags::kernel_rodata())),
            "page at offset {off:#x}"
        );
    }
    assert_eq!(m.translate(va - PAGE_SIZE), None, "mapped below the span");
    assert_eq!(m.translate(va + len), None, "mapped above the span");
    // The head is a page, so `unmap` takes it; the first block starts one page later, so `unmap`
    // refuses there. That is the leaf sizes checked directly rather than through `translate`.
    assert_eq!(
        m.unmap(va + PAGE_SIZE).map(|_| ()),
        Err(MapError::InsideBlock)
    );
    let (_, flush) = m.unmap(va).unwrap();
    never_installed(flush);

    // The same span in pages alone costs strictly more tables: the two blocks' worth of bottom
    // tables that blocks do not need.
    let pages = Pool::new(16);
    let mut p = pages.mapper::<F>(Half::Low);
    p.map_span(va, pa, len, Flags::kernel_rodata(), PageSize::Size4KiB)
        .unwrap();
    assert!(
        pages.used() > blocks.used(),
        "pages {} blocks {}",
        pages.used(),
        blocks.used()
    );
}

#[test]
fn a_span_maps_exactly_its_pages_on_every_format() {
    a_span_maps_exactly_its_pages::<Aarch64>();
    a_span_maps_exactly_its_pages::<Sv39>();
    a_span_maps_exactly_its_pages::<Ia32e>();
}

/// **A block needs both addresses aligned, not either.** When the virtual and physical addresses
/// sit at different offsets within a 2 MiB region, no leaf in the span can be a block, however
/// long it is: a 2 MiB leaf at a misaligned `va` faults, and at a misaligned `pa` the hardware
/// (or the format's mask) maps a different 2 MiB than the one granted. A span with the two at
/// the same offset cannot tell `map_span` computing either address at the wrong offset.
fn skewed_spans_are_mapped_in_pages<F: PageFormat>() {
    for (va, pa) in [(VA + PAGE_SIZE, PA), (VA, PA + PAGE_SIZE)] {
        let len = 2 * MIB2;
        let pool = Pool::new(16);
        let mut m = pool.mapper::<F>(Half::Low);
        m.map_span(va, pa, len, Flags::kernel_data(), PageSize::Size1GiB)
            .unwrap();
        for off in (0..len).step_by(PAGE_SIZE as usize) {
            assert_eq!(
                m.translate(va + off),
                Some((pa + off, Flags::kernel_data())),
                "va {va:#x} pa {pa:#x} offset {off:#x}"
            );
        }
        // A page, not a block, at the one place a block could have started on either side.
        for off in [0, MIB2 - PAGE_SIZE] {
            let (_, flush) = m.unmap(va + off).unwrap();
            never_installed(flush);
        }
    }
}

#[test]
fn skewed_spans_are_mapped_in_pages_on_every_format() {
    skewed_spans_are_mapped_in_pages::<Aarch64>();
    skewed_spans_are_mapped_in_pages::<Sv39>();
    skewed_spans_are_mapped_in_pages::<Ia32e>();
}

/// **`largest` is a ceiling the span honours**: a 1 GiB-aligned gigabyte mapped with a 2 MiB
/// ceiling is 512 blocks under one table, not one 1 GiB leaf. This is the path an x86 without
/// `Page1GB` takes.
#[test]
fn the_largest_size_is_a_ceiling() {
    let two = Pool::new(8);
    let mut m = two.mapper::<Ia32e>(Half::Low);
    m.map_span(VA, PA, GIB1, Flags::kernel_data(), PageSize::Size2MiB)
        .unwrap();
    // Root, PDPT, one PD holding 512 blocks.
    assert_eq!(two.used(), 3);
    assert_eq!(
        m.translate(VA + GIB1 - 1),
        Some((PA + GIB1 - 1, Flags::kernel_data()))
    );

    let one = Pool::new(8);
    let mut g = one.mapper::<Ia32e>(Half::Low);
    g.map_span(VA, PA, GIB1, Flags::kernel_data(), PageSize::Size1GiB)
        .unwrap();
    // Root and a PDPT holding the one leaf.
    assert_eq!(one.used(), 2);
}

#[test]
fn a_span_that_is_not_whole_pages_is_refused() {
    let pool = Pool::new(8);
    let mut m = pool.mapper::<Sv39>(Half::Low);
    assert_eq!(
        m.map_span(
            VA,
            PA,
            PAGE_SIZE + 1,
            Flags::kernel_data(),
            PageSize::Size2MiB
        ),
        Err(MapError::Misaligned)
    );
}

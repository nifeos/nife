//! **radon's SD/MMC controller, the volatile half** (milestone 53 (the board's own peripherals:
//! network and storage on real silicon)).
//!
//! `crates/designware_mobile_storage` is the whole driver: the register map, the controller and
//! card sequences, block I/O, the bench step's two sequences, all host-tested against a simulated
//! controller. This file is the window those sequences run against, and nothing else: one
//! volatile load, one volatile store and a clock, over a base address it is handed. Rule 2 in its
//! smallest form. See notes/designware-mobile-storage.md.
//!
//! **The parity note this milestone must carry**: the JH7110 is a riscv64 part on one board, so
//! this module is riscv64-only and has no aarch64 or x86_64 counterpart. Rule 5's "a scope note
//! records the gap and the plan" applies: the crate is architecture-neutral, and a board with a
//! `DesignWare` Mobile Storage controller on another ISA (several Rockchip arm64 boards have one)
//! would need only this file's twenty lines and a device-tree match.

use designware_mobile_storage::host::Registers;
use designware_mobile_storage::idmac::DmaRegion;

/// A controller's register window in the direct map, and the counter as a microsecond clock.
pub(crate) struct Window {
    base: usize,
    hz: u64,
}

impl Window {
    /// The window whose physical base the tree named.
    ///
    /// # Safety
    ///
    /// `phys` must be a `DesignWare` Mobile Storage controller's base that `mmu::init` mapped
    /// device-typed for at least `regs::WINDOW_USED` bytes (step 10, from the same
    /// `memory::jh7110_storage` answer), and nothing else may drive that controller while this
    /// window is in use.
    pub unsafe fn new(phys: u64) -> Window {
        Window {
            base: crate::arch::mmu::phys_to_virt(phys) as usize,
            hz: crate::arch::timer::frequency().max(1),
        }
    }
}

impl Registers for Window {
    fn read(&mut self, offset: u32) -> u32 {
        debug_assert!(
            offset < designware_mobile_storage::regs::WINDOW_USED && offset.is_multiple_of(4)
        );
        // SAFETY: `new`'s contract: the base is a mapped, device-typed controller window at least
        // `WINDOW_USED` bytes long, and every offset the crate passes is a register below that
        // (asserted above in debug builds), word-aligned.
        unsafe { core::ptr::read_volatile((self.base + offset as usize) as *const u32) }
    }

    fn write(&mut self, offset: u32, value: u32) {
        debug_assert!(
            offset < designware_mobile_storage::regs::WINDOW_USED && offset.is_multiple_of(4)
        );
        // SAFETY: as `read`.
        unsafe { core::ptr::write_volatile((self.base + offset as usize) as *mut u32, value) }
    }

    fn now_us(&mut self) -> u64 {
        // In 128 bits so a counter that has run for days cannot overflow the multiplication.
        (u128::from(crate::arch::timer::now()) * 1_000_000 / u128::from(self.hz)) as u64
    }
}

/// **The IDMAC's region**: contiguous pages in the direct map, ordinary cached memory. radon's DMA
/// is coherent (notes/designware-ethernet.md's probe), so a `fence` is all the ordering the
/// controller needs; a board whose DMA is not would need cache maintenance here.
pub(crate) struct Region {
    phys: u64,
    bytes: u32,
}

impl Region {
    /// `pages` zeroed, contiguous pages, or `None` when the allocator has no such run. Never
    /// freed: the bench boot that uses it halts.
    pub(crate) fn allocate(pages: usize) -> Option<Region> {
        let frame = crate::memory::alloc_contiguous_zeroed(pages)?;
        Some(Region {
            phys: frame.addr(),
            bytes: u32::try_from(pages * 4096).ok()?,
        })
    }

    fn at(&self, off: u32, len: usize) -> usize {
        assert!(u64::from(off) + len as u64 <= u64::from(self.bytes));
        crate::arch::mmu::phys_to_virt(self.phys) as usize + off as usize
    }
}

impl DmaRegion for Region {
    fn bus_address(&self) -> u64 {
        self.phys
    }

    fn bytes(&self) -> u32 {
        self.bytes
    }

    fn read_u32(&self, off: u32) -> u32 {
        // SAFETY: `at` bounds the access to the run `allocate` took, which the direct map covers
        // and nothing else owns; descriptor words are 4-aligned in a page-aligned run. Volatile,
        // because the controller writes these words behind the compiler's back.
        unsafe { core::ptr::read_volatile(self.at(off, 4) as *const u32) }
    }

    fn write_u32(&mut self, off: u32, v: u32) {
        // SAFETY: as `read_u32`.
        unsafe { core::ptr::write_volatile(self.at(off, 4) as *mut u32, v) }
    }

    fn read_bytes(&self, off: u32, out: &mut [u8]) {
        let base = self.at(off, out.len());
        // SAFETY: as `read_u32`, over the bounded span. Called only after `barrier` has ordered
        // the controller's last write before it, and while the IDMAC is stopped.
        unsafe { core::ptr::copy_nonoverlapping(base as *const u8, out.as_mut_ptr(), out.len()) }
    }

    fn write_bytes(&mut self, off: u32, src: &[u8]) {
        let base = self.at(off, src.len());
        // SAFETY: as `read_bytes`; the IDMAC is stopped until the chain that covers these bytes
        // is armed, after `barrier`.
        unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), base as *mut u8, src.len()) }
    }

    /// A full `fence` on riscv64, the one architecture this runs on, so it orders the CPU's reads
    /// after the controller's writes as well as its writes before the register writes.
    fn barrier(&mut self) {
        crate::arch::direct_memory_access_write_barrier();
    }
}

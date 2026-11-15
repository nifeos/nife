//! **Proof that a page table changed and the TLB may now be lying.**
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

/// **Proof that a page table changed and the TLB may now be lying.**
///
/// A changed mapping that is not invalidated leaves the CPU using the old translation, and memory
/// reads back as the previous owner's data. `unmap` returns this; it is `#[must_use]`, and only
/// [`flush`](Self::flush) or the `unsafe` [`assume_no_stale_entry`](Self::assume_no_stale_entry)
/// discharges it. [`MapError::AlreadyMapped`](crate::MapError::AlreadyMapped) forces
/// break-before-make. Mapping a page that was invalid needs none, since the hardware may not cache
/// a faulting entry. See notes/page-tables.md, "The TLB obligation" and "Break-before-make".
#[must_use = "a page table changed: the TLB MUST be invalidated, or the CPU keeps using the old \
              translation and memory reads back as the previous owner's data"]
#[derive(Debug, PartialEq, Eq)]
pub struct TlbFlush {
    pub(crate) va: u64,
}

impl TlbFlush {
    /// Discharge the obligation by actually invalidating. The crate emits no instructions, so the
    /// caller supplies the architecture's invalidate (aarch64 `tlbi vaae1is`, RISC-V `sfence.vma`).
    pub fn flush(self, invalidate: impl FnOnce(u64)) {
        let va = self.va;
        core::mem::forget(self); // discharged: do not run the Drop below
        invalidate(va);
    }

    /// Discharge the obligation **without** invalidating.
    ///
    /// # Safety
    /// Only sound when the TLB provably cannot hold an entry for this address: e.g. these tables are
    /// not installed anywhere yet, so the hardware has never walked them. If you are wrong, the
    /// failure is a stale translation, and the page tables will look perfectly correct while you
    /// debug it.
    pub unsafe fn assume_no_stale_entry(self) {
        core::mem::forget(self);
    }

    /// The virtual address this obligation covers.
    pub fn address(&self) -> u64 {
        self.va
    }
}

/// **You cannot drop this on the floor.** `#[must_use]` alone warns on `mapper.unmap(va);` as a
/// statement but says nothing about `let (pa, _) = mapper.unmap(va)?;`, which is exactly the shape
/// the mistake takes. Rust has no linear types, so the only way to make "you must consume this"
/// enforceable is to make *not* consuming it fail loudly. A panic is the right failure: the
/// alternative is a stale TLB entry, memory that reads back as its previous owner's data, in a
/// kernel whose page tables are provably correct in memory.
impl Drop for TlbFlush {
    fn drop(&mut self) {
        panic!(
            "page table changed at {:#x} but the TLB was never invalidated: \
             the CPU is still using the old translation. \
             Call .flush() or (unsafely) .assume_no_stale_entry().",
            self.va
        );
    }
}

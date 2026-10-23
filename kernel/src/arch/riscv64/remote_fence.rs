//! **Remote fences through SBI RFENCE**: shooting down a translation, an address space, or the
//! instruction cache on other harts, and the count bench builds keep.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// The SBI RFENCE extension id, "RFNC" in ASCII. Both remote-fence calls below live in it.
pub(super) const SBI_RFENCE_EID: usize = 0x5246_4E43;

/// The RFENCE `size` meaning "every address": all-ones (OpenSBI's `SBI_TLB_FLUSH_ALL`).
///
/// For [`sbi_remote_sfence_vma_asid`] it reaches `sfence.vma x0, asid`; `start = size = 0` would
/// flush every ASID on every target. See notes/riscv-tlb-shootdown.md, "The `size` argument is a
/// trap".
pub(super) const SBI_RFENCE_ALL: usize = usize::MAX;

/// How many remote RFENCEs this boot has issued, in bench builds only, off the shipping TLB path.
///
/// Each is an `ecall` into firmware, and a single-hart boot must issue none. See
/// notes/benchmarks/riscv-map-new-and-the-rfence-probe.md.
#[cfg(feature = "bench")]
pub(super) static REMOTE_FENCES: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(0);

/// Record one issued RFENCE. See [`REMOTE_FENCES`].
#[cfg(feature = "bench")]
pub(super) fn note_remote_fence() {
    REMOTE_FENCES.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
}

/// Remote RFENCEs issued so far this boot. Bench builds only; see [`REMOTE_FENCES`].
#[cfg(feature = "bench")]
pub fn remote_fence_count() -> u64 {
    REMOTE_FENCES.load(core::sync::atomic::Ordering::Relaxed)
}

/// Shoot down the translations for `[start, start + size)` on the harts in `hart_mask` through SBI
/// RFENCE, since RISC-V has no hardware TLB broadcast. See `mmu::flush_tlb`.
pub fn sbi_remote_sfence_vma(hart_mask: usize, start: usize, size: usize) {
    const SBI_REMOTE_SFENCE_VMA_FID: usize = 1;
    #[cfg(feature = "bench")]
    note_remote_fence();
    // SAFETY: remote_sfence_vma with (hart bitmap, mask base 0, start, size). TLB maintenance on
    // other harts, which is always sound. The result is ignored, as it always was.
    unsafe {
        sbi::call(
            SBI_RFENCE_EID,
            SBI_REMOTE_SFENCE_VMA_FID,
            [hart_mask, 0, start, size, 0, 0],
        )
    };
}

/// Discard every translation tagged `asid` on the harts in `hart_mask`: the remote half of
/// `mmu::flush_asid`. Each target runs `sfence.vma x0, asid`.
///
/// Returns once every target has drained it, because OpenSBI's RFENCE is synchronous; that return
/// lets the ASID tag another space. Targets service it even with S-mode interrupts masked.
///
/// # BUGS
///
/// Asynchronous firmware would break this silently, and `hart_mask` reaches harts 0..63 only. See
/// notes/riscv-tlb-shootdown.md, BUGS.
pub fn sbi_remote_sfence_vma_asid(hart_mask: usize, asid: u16) {
    const SBI_REMOTE_SFENCE_VMA_ASID_FID: usize = 2;
    #[cfg(feature = "bench")]
    note_remote_fence();
    let args = [hart_mask, 0, 0, SBI_RFENCE_ALL, asid as usize, 0];
    // SAFETY: remote_sfence_vma_asid with (hart bitmap, mask base 0, start 0, size all, asid). TLB
    // maintenance on other harts, which is always sound. The result is ignored, as it always was.
    unsafe { sbi::call(SBI_RFENCE_EID, SBI_REMOTE_SFENCE_VMA_ASID_FID, args) };
}

/// Execute `fence.i` on the harts in `hart_mask` through SBI RFENCE: the cross-hart half of
/// [`sync_icache`], since `fence.i` orders only the local hart's fetch.
pub fn sbi_remote_fence_i(hart_mask: usize) {
    const SBI_REMOTE_FENCE_I_FID: usize = 0;
    // SAFETY: remote_fence_i with (hart bitmap, mask base 0). Instruction-fetch ordering on other
    // harts, which is always sound. The result is ignored, as it always was.
    unsafe {
        sbi::call(
            SBI_RFENCE_EID,
            SBI_REMOTE_FENCE_I_FID,
            [hart_mask, 0, 0, 0, 0, 0],
        )
    };
}

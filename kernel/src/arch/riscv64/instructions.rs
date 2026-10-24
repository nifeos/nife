//! One function per instruction: the riscv64 `asm!` that logic calls, kept out of the logic so a
//! Kani harness can replace it with `#[kani::stub]` and a model.
//!
//! *Name provisional (a lane minted it 2026-09-25; names are calef's). It says what every item here
//! is, one machine instruction, rather than what register it touches, because not all of them touch a
//! CSR: `wfi`, the fences and the two general-register moves live here too.*
//!
//! Each function stays one instruction, or one fixed sequence, and says what a model must honour.
//! A stub is an assumption about the hardware. See notes/kernel-proofs/stubbing-an-instruction.md.

//! # Codegen
//!
//! Every function is `#[inline(always)]`, with each call site's own `options`: `nomem` on an
//! `sstatus` write that changes what S-mode may load would let a user-page access move across it.
//!
//! What stays `asm!` elsewhere, on purpose: `exceptions.rs`'s register-preservation test,
//! `fp::touch`, `timer::calibration_loop`, `pmu::read_csr` (whose CSR number is an immediate), and
//! every SBI `ecall`, which goes through [`super::sbi::call`].

use core::arch::asm;

// ---- satp and the TLB ----

/// `csrr satp`. A model returns whatever the model's register holds; the hardware's `satp` is WARL,
/// so a model that honours writes exactly is claiming every field is implemented.
#[inline(always)]
pub(super) fn read_satp() -> u64 {
    let satp: u64;
    // SAFETY: reads a CSR. No side effects.
    unsafe { asm!("csrr {}, satp", out(reg) satp, options(nomem, nostack, preserves_flags)) };
    satp
}

/// `csrr th.sxstatus` (CSR `0x5c0`): T-Head's supervisor-visible extension status, whose bit 21 is
/// `MAEE`, whether the hart reads memory types out of PTE bits 63:59 (milestone 89 (Scaleway EM-RV1)).
///
/// Only on a T-Head hart: elsewhere the read is an illegal-instruction trap. QEMU's `thead-c906`
/// returns `MAEE` clear, since QEMU does not model MAE (`target/riscv/tcg/th_csr.c`).
#[inline(always)]
pub(super) fn read_th_sxstatus() -> u64 {
    let v: u64;
    // SAFETY: reads a CSR. No side effects. The caller has established that this is a T-Head hart.
    unsafe { asm!("csrr {}, 0x5c0", out(reg) v, options(nomem, nostack, preserves_flags)) };
    v
}

/// `csrw satp`: install an address space. **No `nomem`**: every load and store after this one
/// translates through the new root, so the compiler must not move one across it.
///
/// # Safety
/// `satp` must name a live, well-formed Sv39 root containing the kernel's high half, or the next
/// instruction fetch faults. See `mmu::write_satp`, which carries the full contract.
#[inline(always)]
pub(super) unsafe fn write_satp(satp: u64) {
    // SAFETY: the caller's contract.
    unsafe { asm!("csrw satp, {}", in(reg) satp, options(nostack)) };
}

/// `sfence.vma` with no operands: every translation this hart caches, every ASID, global ones too.
#[inline(always)]
pub(super) fn sfence_vma_all() {
    // SAFETY: TLB maintenance is always sound.
    unsafe { asm!("sfence.vma", options(nostack)) };
}

/// `sfence.vma zero, asid`: every non-global translation tagged `asid`, on this hart only.
#[inline(always)]
pub(super) fn sfence_vma_asid(asid: u64) {
    // SAFETY: TLB maintenance is always sound.
    unsafe { asm!("sfence.vma zero, {}", in(reg) asid, options(nostack)) };
}

/// `sfence.vma va, zero`: one page's translation, every ASID, on this hart only.
#[inline(always)]
pub(super) fn sfence_vma_page(va: u64) {
    // SAFETY: TLB maintenance is always sound.
    unsafe { asm!("sfence.vma {}, zero", in(reg) va, options(nostack)) };
}

// ---- sstatus ----

/// `csrr sstatus`.
#[inline(always)]
pub(super) fn read_sstatus() -> u64 {
    let sstatus: u64;
    // SAFETY: reads a CSR. No side effects.
    unsafe { asm!("csrr {}, sstatus", out(reg) sstatus, options(nomem, nostack, preserves_flags)) };
    sstatus
}

/// `csrs sstatus`, for the fields that gate **instructions** rather than memory (`FS`, `VS`), which
/// is why it carries `nomem`.
#[inline(always)]
pub(super) fn set_sstatus(bits: u64) {
    // SAFETY: sets bits in `sstatus`; the callers name only the FP and vector enables, which govern
    // which instructions execute and name no memory.
    unsafe { asm!("csrs sstatus, {}", in(reg) bits, options(nomem, nostack, preserves_flags)) };
}

/// `csrc sstatus`, for the same instruction-gating fields as [`set_sstatus`].
#[inline(always)]
pub(super) fn clear_sstatus(bits: u64) {
    // SAFETY: as `set_sstatus`.
    unsafe { asm!("csrc sstatus, {}", in(reg) bits, options(nomem, nostack, preserves_flags)) };
}

/// `csrrs sstatus`: set `bits`, return the old value. **No `nomem`**, because the one caller sets
/// `SUM`, which changes whether an S-mode load of a `U` page faults, so no memory access may move
/// across it. Test-only, like that caller.
#[cfg(any(test, feature = "system_tests"))]
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the system tests call it; a unit-test boot on some ISAs does not
#[inline(always)]
pub(super) fn read_and_set_sstatus(bits: u64) -> u64 {
    let previous: u64;
    // SAFETY: sets bits in `sstatus` and reports the old value. A permission change, not a
    // memory-safety one; the kernel's own mappings are unaffected.
    unsafe { asm!("csrrs {}, sstatus, {}", out(reg) previous, in(reg) bits, options(nostack)) };
    previous
}

/// `csrrc sstatus`: clear `bits`, return the old value. No `nomem`, as [`read_and_set_sstatus`].
#[cfg(any(test, feature = "system_tests"))]
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the system tests call it; a unit-test boot on some ISAs does not
#[inline(always)]
pub(super) fn read_and_clear_sstatus(bits: u64) -> u64 {
    let previous: u64;
    // SAFETY: as `read_and_set_sstatus`.
    unsafe { asm!("csrrc {}, sstatus, {}", out(reg) previous, in(reg) bits, options(nostack)) };
    previous
}

/// `csrrci sstatus, 2`: mask interrupts (`SIE`, bit 1), returning the old `sstatus`. The immediate
/// form, so masking costs one instruction and no register load; that is why this is its own function
/// rather than [`read_and_clear_sstatus`] with a constant.
#[inline(always)]
pub(super) fn read_and_clear_sstatus_sie() -> u64 {
    let previous: u64;
    // SAFETY: clears the interrupt-enable bit and reports the prior value. No memory effect.
    unsafe {
        asm!("csrrci {}, sstatus, 2", out(reg) previous, options(nomem, nostack, preserves_flags));
    };
    previous
}

/// `csrci sstatus, 2`: mask interrupts.
#[inline(always)]
pub(super) fn clear_sstatus_sie() {
    // SAFETY: clears SIE. No memory effect.
    unsafe { asm!("csrci sstatus, 2", options(nomem, nostack, preserves_flags)) };
}

/// `csrsi sstatus, 2`: unmask interrupts.
#[inline(always)]
pub(super) fn set_sstatus_sie() {
    // SAFETY: sets SIE. No memory effect.
    unsafe { asm!("csrsi sstatus, 2", options(nomem, nostack, preserves_flags)) };
}

// ---- interrupt enables, pending bits and the trap vector ----

/// `csrs sie`: unmask one or more interrupt sources. Takes effect only under `sstatus.SIE`.
#[inline(always)]
pub(super) fn set_sie(bits: u64) {
    // SAFETY: unmasks interrupt sources. No memory effect.
    unsafe { asm!("csrs sie, {}", in(reg) bits, options(nomem, nostack, preserves_flags)) };
}

/// `csrr sip`.
#[inline(always)]
pub(super) fn read_sip() -> u64 {
    let sip: u64;
    // SAFETY: reads a CSR. No side effects.
    unsafe { asm!("csrr {}, sip", out(reg) sip, options(nomem, nostack, preserves_flags)) };
    sip
}

/// `csrc sip`: clear pending bits. Only `SSIP` is software-writable in S-mode.
#[inline(always)]
pub(super) fn clear_sip(bits: u64) {
    // SAFETY: clears `sip` bits; `csrc` is an atomic read-clear. No memory effect.
    unsafe { asm!("csrc sip, {}", in(reg) bits, options(nomem, nostack)) };
}

/// `csrw stvec`.
///
/// # Safety
/// `vector` must be the 4-byte-aligned address of code that is a valid trap entry: the next trap,
/// of any kind, jumps there.
#[inline(always)]
pub(super) unsafe fn write_stvec(vector: usize) {
    // SAFETY: the caller's contract.
    unsafe { asm!("csrw stvec, {}", in(reg) vector, options(nomem, nostack)) };
}

/// `csrr stvec`.
#[cfg(any(test, feature = "system_tests"))]
#[inline(always)]
pub(super) fn read_stvec() -> u64 {
    let stvec: u64;
    // SAFETY: reads a CSR. No side effects.
    unsafe { asm!("csrr {}, stvec", out(reg) stvec, options(nomem, nostack)) };
    stvec
}

// ---- per-hart registers ----

/// `csrr sscratch`.
#[cfg(any(test, feature = "system_tests"))]
#[inline(always)]
pub(super) fn read_sscratch() -> usize {
    let sscratch: usize;
    // SAFETY: reads a CSR. No side effects.
    unsafe { asm!("csrr {}, sscratch", out(reg) sscratch, options(nomem, nostack)) };
    sscratch
}

/// `csrw sscratch`.
///
/// # Safety
/// `trap.s` reads `sscratch` as `&TrapStash` on every trap from U-mode, so it must name this hart's
/// live stash.
#[inline(always)]
pub(super) unsafe fn write_sscratch(value: usize) {
    // SAFETY: the caller's contract.
    unsafe { asm!("csrw sscratch, {}", in(reg) value, options(nomem, nostack, preserves_flags)) };
}

/// `mv {}, tp`: the per-CPU pointer.
#[inline(always)]
pub(super) fn read_tp() -> usize {
    let tp: usize;
    // SAFETY: reads a general register. No side effects.
    unsafe { asm!("mv {}, tp", out(reg) tp, options(nomem, nostack, preserves_flags)) };
    tp
}

/// `mv tp, {}`.
///
/// # Safety
/// `tp` is the kernel's per-CPU pointer on this ISA and `cpu::id()` trusts it, so it must name this
/// hart's `PerCpu`.
#[inline(always)]
pub(super) unsafe fn write_tp(value: usize) {
    // SAFETY: the caller's contract; a register write with no memory effect.
    unsafe { asm!("mv tp, {}", in(reg) value, options(nomem, nostack, preserves_flags)) };
}

/// `mv {}, sp`.
#[inline(always)]
pub(super) fn read_sp() -> u64 {
    let sp: u64;
    // SAFETY: reads a register. No side effects.
    unsafe { asm!("mv {}, sp", out(reg) sp, options(nomem, nostack, preserves_flags)) };
    sp
}

// ---- counters ----

/// `rdtime`.
#[inline(always)]
pub(super) fn read_time() -> u64 {
    let t: u64;
    // SAFETY: reads the time CSR. No side effects.
    unsafe { asm!("rdtime {}", out(reg) t, options(nomem, nostack, preserves_flags)) };
    t
}

/// `csrr scounteren`. `scounteren` is mandatory in S-mode, so the read cannot be illegal. Built with
/// its only callers, the cycle-counter grant and its test.
#[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
#[inline(always)]
pub(super) fn read_scounteren() -> u64 {
    let value: u64;
    // SAFETY: reading a supervisor CSR touches no memory and changes no state.
    unsafe {
        asm!("csrr {}, scounteren", out(reg) value, options(nomem, nostack, preserves_flags));
    };
    value
}

/// `csrw scounteren`. The CSR governs U-mode counter reads only, so no S-mode access depends on it.
#[inline(always)]
pub(super) fn write_scounteren(value: u64) {
    // SAFETY: as the doc says; touches no memory.
    unsafe { asm!("csrw scounteren, {}", in(reg) value, options(nomem, nostack, preserves_flags)) };
}

// ---- waiting, ordering, trapping ----

/// `wfi`: park until an interrupt is pending. May return early; every caller loops or tolerates it.
#[inline(always)]
pub(super) fn wfi() {
    // SAFETY: wait-for-interrupt only affects when the next instruction runs.
    unsafe { asm!("wfi", options(nomem, nostack)) };
}

/// `fence` with no operands (`fence iorw, iorw`): the full barrier. No `nomem`, because ordering
/// memory is its entire purpose.
#[inline(always)]
pub(super) fn full_fence() {
    // SAFETY: a fence has no memory effect of its own; it only constrains ordering.
    unsafe { asm!("fence", options(nostack, preserves_flags)) };
}

/// `fence.i`: order this hart's instruction fetch after its own prior stores.
#[inline(always)]
pub(super) fn fence_i() {
    // SAFETY: `fence.i` only orders instruction fetch against prior stores on this hart.
    unsafe { asm!("fence.i", options(nostack, preserves_flags)) };
}

/// `ebreak`: raise a breakpoint, which `exceptions.rs` counts and steps over.
#[inline(always)]
pub(super) fn ebreak() {
    // SAFETY: the dispatcher handles a breakpoint and resumes after it; nothing else happens.
    unsafe { asm!("ebreak") };
}

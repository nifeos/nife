//! **A thread's floating-point and SSE registers, `x86_64`.** The register file and the
//! control-register bits that decide who may touch it; the policy is [`crate::fp`]'s.
//!
//! `CR0.TS`, written per thread, is the enable bit: set, the next FP or SSE instruction raises
//! `#NM`. It never defers a restore (the `LazyFP` scheme of CVE-2018-3665), so the registers hold
//! the running thread's values or [`FpState::INITIAL`]. See notes/x86-port/cpu-state.md.
//!
//! # BUGS
//!
//! - `fxsave`, not `xsave`, so AVX state is never moved; safe only while `CR4.OSXSAVE` is clear.

use super::instructions::{self, read_cr0, read_cr4};

/// **How many vector registers this architecture saves**: `xmm0`-`xmm15`, sixteen rather than the
/// other two ISAs' thirty-two. See the aarch64 twin.
#[cfg(any(test, feature = "system_tests"))]
pub(crate) const REGISTERS: usize = 16;

/// **A thread's FP/SSE register file**, plus the flag that says whether any of it is worth moving.
///
/// `#[repr(C, align(16))]` is a contract with `fp.s` and with the hardware: `fxsave` faults on a
/// destination that is not 16-byte aligned, and the area sits at offset 16.
#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub(crate) struct FpState {
    /// Nonzero once this thread has executed an FP or SSE instruction. The save and the restore
    /// are both conditioned on it.
    live: u64,
    /// Padding, so the `fxsave` area lands at offset 16 and 16-byte aligned. Never read.
    _pad: u64,
    /// The `FXSAVE` area: x87 control and status, `MXCSR`, `ST0`-`ST7`, `xmm0`-`xmm15`. 512 bytes,
    /// laid out by the CPU rather than by this file.
    area: [u8; 512],
}

const _: () = assert!(size_of::<FpState>() == 528);
const _: () = assert!(core::mem::offset_of!(FpState, area) == 16);

/// `MXCSR` in the `FXSAVE` area.
const MXCSR: usize = 24;
/// `xmm0` in the `FXSAVE` area; the sixteen registers follow at 16 bytes each.
const XMM0: usize = 160;

impl FpState {
    /// **The state a thread starts with, and the state the registers are scrubbed to.**
    ///
    /// Not all zeros, and this is the one architecture where that matters. The x87 control word
    /// (offset 0) is `0x037f`, the value `finit` installs: extended precision, round to nearest,
    /// every exception masked. `MXCSR` (offset 24) is `0x1f80`, likewise all six SIMD exceptions
    /// masked and round to nearest. A zeroed `MXCSR` would unmask every exception, so the first
    /// underflow in a scrubbed thread would be a `#XM` nobody asked for.
    pub(crate) const INITIAL: Self = {
        let mut area = [0u8; 512];
        // x87 control word, little endian.
        area[0] = 0x7f;
        area[1] = 0x03;
        // MXCSR, little endian.
        area[MXCSR] = 0x80;
        area[MXCSR + 1] = 0x1f;
        FpState {
            live: 0,
            _pad: 0,
            area,
        }
    };

    /// A fresh thread's state.
    pub const fn new() -> Self {
        Self::INITIAL
    }

    /// Has this thread ever executed an FP or SSE instruction?
    pub(crate) fn is_live(&self) -> bool {
        self.live != 0
    }

    /// Record that it has. Called once, from the `#NM` the set `TS` produced.
    pub(crate) fn set_live(&mut self) {
        self.live = 1;
    }
}

impl Default for FpState {
    fn default() -> Self {
        Self::new()
    }
}

/// `CR0.MP`, the monitor-coprocessor bit: `wait`/`fwait` honours `TS`.
const CR0_MP: u64 = 1 << 1;
/// `CR0.EM`, 80387 emulation. **Must be clear**, or SSE raises `#UD` rather than `#NM`.
const CR0_EM: u64 = 1 << 2;
/// `CR0.TS`, task switched. Set means the next FP or SSE instruction raises `#NM`.
const CR0_TS: u64 = 1 << 3;
/// `CR0.NE`, native x87 exception reporting: an unmasked x87 exception arrives as `#MF` rather than
/// through the ancient external `FERR#`/IRQ 13 route, which no machine this kernel runs on wires.
const CR0_NE: u64 = 1 << 5;
/// `CR4.OSFXSR`: `fxsave`/`fxrstor` and the `xmm` registers are available.
const CR4_OSFXSR: u64 = 1 << 9;
/// `CR4.OSXMMEXCPT`: an unmasked SIMD exception arrives as `#XM` rather than `#UD`.
const CR4_OSXMMEXCPT: u64 = 1 << 10;

/// **Put this CPU into the state the rest of this module assumes**: SSE available to the kernel's
/// own save path, and the unit shut for whichever thread runs next.
///
/// Called from `sched::init` and `sched::adopt_secondary_idle` rather than from `arch::init`, for
/// the reason written at the call site: the invariant is about threads, and one architecture's boot
/// path does not pass through `arch::init` at all.
pub fn init() {
    let mut cr4 = read_cr4();
    cr4 |= CR4_OSFXSR | CR4_OSXMMEXCPT;
    // SAFETY: a read-modify-write of the two SSE enables; `PAE` and every other bit go back as read,
    // and both bits exist on every x86_64 part.
    unsafe { instructions::write_cr4(cr4) };

    let mut cr0 = read_cr0();
    cr0 &= !CR0_EM;
    cr0 |= CR0_MP | CR0_NE | CR0_TS;
    // SAFETY: a read-modify-write of the four FP bits above; paging and write protect go back as
    // read.
    unsafe { instructions::write_cr0(cr0) };
}

/// Let the current CPU execute FP and SSE instructions: clear `CR0.TS`.
///
/// `clts` is one byte and exists for exactly this, which is the clearest evidence available that
/// the whole `TS` mechanism was designed for the lazy scheme this kernel deliberately does not use.
pub fn enable() {
    instructions::clts();
}

/// Is the FP unit open on this CPU right now?
pub(crate) fn is_enabled() -> bool {
    read_cr0() & CR0_TS == 0
}

/// Take the unit away again: set `CR0.TS`.
///
/// **The caller owes the scrub.** That sentence is this architecture's in particular: see the
/// module header on CVE-2018-3665.
pub fn disable() {
    // SAFETY: sets `TS` alone; every other bit goes back as read.
    unsafe { instructions::write_cr0(read_cr0() | CR0_TS) };
}

/// Copy the live register file into `state`.
///
/// # Safety
/// `state` must be a valid, writable, 16-byte-aligned `FpState`, and `CR0.TS` must be clear. The
/// only caller is [`crate::fp::hand_over`].
pub(crate) unsafe fn save(state: *mut FpState) {
    // SAFETY: forwarded. `fp_save` writes 512 bytes at `state + 16` and touches nothing else.
    unsafe { fp_save(state) }
}

/// Load `state` into the live register file.
///
/// # Safety
/// As [`save`], readable rather than writable. `state`'s `MXCSR` field must be one this CPU's
/// `MXCSR_MASK` allows, or `fxrstor` raises `#GP`; every `FpState` this kernel builds comes from
/// [`FpState::INITIAL`] or from a [`save`], and both satisfy that.
pub(crate) unsafe fn restore(state: *const FpState) {
    // SAFETY: forwarded. `fp_restore` reads 512 bytes at `state + 16` and writes only registers.
    unsafe { fp_restore(state) }
}

unsafe extern "C" {
    /// The `FXSAVE` area out to memory. See fp.s.
    fn fp_save(state: *mut FpState);
    /// The mirror. See fp.s.
    fn fp_restore(state: *const FpState);
}

/// **One harmless SSE instruction, to take the first-use trap on purpose.** Tests only.
///
/// `xorps xmm0, xmm0` zeroes a register the caller is about to overwrite anyway.
#[cfg(any(test, feature = "system_tests"))]
pub(crate) fn touch() {
    // SAFETY: writes one `xmm` register. Under a set `CR0.TS` this raises `#NM`, which
    // `exceptions.rs` turns into an enable, and is re-executed after it.
    unsafe {
        core::arch::asm!(
            ".arch .default",
            "xorps xmm0, xmm0",
            options(nomem, nostack, preserves_flags),
        );
    }
}

#[cfg(any(test, feature = "system_tests"))]
impl FpState {
    /// Write [`crate::fp::register_pattern`] across all sixteen `xmm` registers, each one's
    /// complement in the high half so the full 128 bits are exercised.
    pub(crate) fn set_pattern(&mut self, seed: u64) {
        for index in 0..16 {
            let low = crate::fp::register_pattern(seed, index);
            let at = XMM0 + index * 16;
            self.area[at..at + 8].copy_from_slice(&low.to_le_bytes());
            self.area[at + 8..at + 16].copy_from_slice(&(!low).to_le_bytes());
        }
    }

    /// Is that pattern still there, in every `xmm` register?
    pub(crate) fn has_pattern(&self, seed: u64) -> bool {
        (0..16).all(|index| self.lane_low(index) == crate::fp::register_pattern(seed, index))
    }

    /// The low 64 bits of one `xmm` register, so a test can name the one that went wrong.
    pub(crate) fn lane_low(&self, index: usize) -> u64 {
        let at = XMM0 + index * 16;
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&self.area[at..at + 8]);
        u64::from_le_bytes(bytes)
    }

    /// Do the registers hold [`Self::INITIAL`]'s values, i.e. has the file been scrubbed?
    ///
    /// The `xmm` half only. The x87 stack and the tag word are also restored by the scrub, and
    /// `fxsave` writes a handful of status fields (the last instruction pointer among them) that
    /// are not the thread's data and would make a byte-for-byte comparison against `INITIAL` fail
    /// for reasons that are not leaks.
    pub(crate) fn is_scrubbed(&self) -> bool {
        (0..16).all(|index| {
            let at = XMM0 + index * 16;
            self.area[at..at + 16].iter().all(|&byte| byte == 0)
        })
    }
}

/// Proofs of the `CR0`/`CR4` edits, which stub the control-register accesses in [`instructions`].
///
/// The stubs model `CR0` and `CR4` as plain 64-bit registers: a write is visible to the next read,
/// and `clts` clears bit 3 of `CR0` and nothing else (Intel SDM vol. 2A, `CLTS`). Real parts
/// reserve bits and fault on writing them; the model does not, so these proofs say only that the
/// code writes back every bit it did not mean to change. See
/// notes/kernel-proofs/stubbing-an-instruction.md.
///
/// Proved on an `x86_64` host only: Kani compiles for its host, and CI's `prove` shards run on
/// x86_64.
#[cfg(kani)]
mod proofs {
    use core::sync::atomic::AtomicU64;
    use core::sync::atomic::Ordering::Relaxed;

    use super::*;

    /// The modelled `CR0`.
    static CR0: AtomicU64 = AtomicU64::new(0);
    /// The modelled `CR4`.
    static CR4: AtomicU64 = AtomicU64::new(0);

    #[allow(dead_code)] // named only by `#[kani::stub]`, which rustc cannot see
    fn read_cr0_model() -> u64 {
        CR0.load(Relaxed)
    }

    /// Model of the write.
    ///
    /// # Safety
    /// None of its own: `unsafe` only so its signature matches the `unsafe fn` it stands in for.
    /// It touches nothing but the model's statics.
    #[allow(dead_code)] // named only by `#[kani::stub]`
    unsafe fn write_cr0_model(value: u64) {
        CR0.store(value, Relaxed);
    }

    #[allow(dead_code)] // named only by `#[kani::stub]`
    fn clts_model() {
        CR0.fetch_and(!CR0_TS, Relaxed);
    }

    #[allow(dead_code)] // named only by `#[kani::stub]`
    fn read_cr4_model() -> u64 {
        CR4.load(Relaxed)
    }

    /// Model of the write.
    ///
    /// # Safety
    /// None of its own: `unsafe` only so its signature matches the `unsafe fn` it stands in for.
    /// It touches nothing but the model's statics.
    #[allow(dead_code)] // named only by `#[kani::stub]`
    unsafe fn write_cr4_model(value: u64) {
        CR4.store(value, Relaxed);
    }

    /// **Every FP edit of `CR0` and `CR4` changes exactly the bits it names, and [`is_enabled`]
    /// reads back what the last one did**, from every starting value. A mask one bit wide of its
    /// mark would turn off paging or write protection, which QEMU mostly does not show.
    ///
    /// Falsification: attested 2026-10-05. `kernel/falsifications/arch.x86_64.fp.proofs.every_fp_edit_changes_exactly_the_bits_it_names.patch`
    /// First replayed by run 37368707083 of that workflow. It makes `disable` store `CR0_TS` alone, which is the harness's second named mutation. The record
    /// is `attested` rather than `replayable` because a `#[kani::proof]` compiles for the host and
    /// only an x86_64 host compiles this module (`script/falsifications`' BUGS); the `sweep-x86_64`
    /// job in `.github/workflows/falsifications.yml` replays it with `--sweep-attested` and goes red
    /// if the harness survives.
    #[kani::proof]
    #[kani::stub(super::super::instructions::read_cr0, read_cr0_model)]
    #[kani::stub(super::super::instructions::write_cr0, write_cr0_model)]
    #[kani::stub(super::super::instructions::clts, clts_model)]
    #[kani::stub(super::super::instructions::read_cr4, read_cr4_model)]
    #[kani::stub(super::super::instructions::write_cr4, write_cr4_model)]
    fn every_fp_edit_changes_exactly_the_bits_it_names() {
        let cr0_before: u64 = kani::any();
        let cr4_before: u64 = kani::any();
        CR0.store(cr0_before, Relaxed);
        CR4.store(cr4_before, Relaxed);

        let which: u8 = kani::any();
        kani::assume(which < 3);
        // (CR0 bits touched, their value after, CR4 bits touched, their value after)
        let (cr0_touched, cr0_want, cr4_touched, cr4_want) = match which {
            0 => {
                init();
                let sse = CR4_OSFXSR | CR4_OSXMMEXCPT;
                (
                    CR0_EM | CR0_MP | CR0_NE | CR0_TS,
                    CR0_MP | CR0_NE | CR0_TS,
                    sse,
                    sse,
                )
            }
            1 => {
                enable();
                (CR0_TS, 0, 0, 0)
            }
            _ => {
                disable();
                (CR0_TS, CR0_TS, 0, 0)
            }
        };

        let cr0 = CR0.load(Relaxed);
        let cr4 = CR4.load(Relaxed);
        assert!(
            cr0 & cr0_touched == cr0_want,
            "a named CR0 bit did not reach its value"
        );
        assert!(
            cr0 & !cr0_touched == cr0_before & !cr0_touched,
            "a CR0 bit moved that is not FP's"
        );
        assert!(
            cr4 & cr4_touched == cr4_want,
            "a named CR4 bit did not reach its value"
        );
        assert!(
            cr4 & !cr4_touched == cr4_before & !cr4_touched,
            "a CR4 bit moved that is not FP's"
        );
        assert!(
            is_enabled() == (which == 1),
            "is_enabled disagrees with the last edit"
        );
    }
}

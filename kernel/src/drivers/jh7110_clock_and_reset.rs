//! The JH7110's clock and reset generator, the volatile half.
//!
//! `crates/jh7110_clock_and_reset` owns the offsets, bit positions, bring-up plan and device-tree
//! query, host-tested and pointer-free. This walks a plan it is handed against a base it is handed
//! and reports what the hardware said. It reaches into no kernel global (§4). It is in the kernel
//! because the window is shared by USB, PCIe, DMA and the security block, so granting it would
//! widen a driver's authority (notes/jh7110-clock-and-reset.md, "Where it lives, and who may drive
//! it").
//!

//! # SAFETY
//!
//! [`bring_up`] stores to whatever address it is given, and the pure crate's fallback can name
//! `0x1023_0000` on a machine with nothing there. The caller must establish that the machine is a
//! JH7110 first; `memory::jh7110_clock_and_reset` records a region only when the tree names a CRG
//! or a JH7110 TRNG, so QEMU's `virt` never reaches this driver.

use jh7110_clock_and_reset::{
    CLOCK_ENABLE, Domain, MAX_RECORDED_CLOCKS, Report, Step, is_deasserted, with_parent,
};

/// How many times [`bring_up`] reads the status word before giving up on a deassert.
///
/// An iteration count, not a duration, because no calibrated delay exists this early (Linux polls
/// with a 1000 µs timeout); recorded in `notes/jh7110-clock-and-reset.md`'s `BUGS`. A million reads
/// is far longer than the hardware needs and far shorter than a hung boot.
const POLL_LIMIT: u32 = 1_000_000;

/// Run `plan` against the clock-and-reset window at `base`, a kernel virtual address in the
/// direct map. `domain` bounds every identifier the plan names, so a wrong-domain plan counts
/// `rejected` steps rather than storing. Idempotent, since firmware may have done some of it.
///
/// # Safety
///
/// `base` must be a mapped, device-typed window of at least `domain`'s extent, belonging to a
/// JH7110 clock-and-reset generator (see the module's `SAFETY`).
pub unsafe fn bring_up(base: usize, domain: &Domain, plan: &[Step]) -> Report {
    let mut report = Report::default();

    for step in plan {
        match *step {
            Step::EnableClock(index) => {
                let Some(offset) = domain.clock_offset(index) else {
                    report.rejected += 1;
                    continue;
                };
                let reg = (base + offset as usize) as *mut u32;
                // SAFETY: `base` is a mapped device window per this function's contract, and
                // `clock_offset` returned `Some`, which bounds the offset inside the domain.
                let before = unsafe { core::ptr::read_volatile(reg) };
                // SAFETY: as above. A read-modify-write of a word this controller dedicates to
                // one clock, so there is no neighbouring field to preserve beyond what the
                // read already carries.
                unsafe { core::ptr::write_volatile(reg, before | CLOCK_ENABLE) };
                // SAFETY: as above. Read back rather than assumed: a window with nothing behind
                // it accepts the store and reads zero, and that is the case this exists to catch.
                let after = unsafe { core::ptr::read_volatile(reg) };
                if report.clocks < MAX_RECORDED_CLOCKS {
                    report.clock_before[report.clocks] = before;
                    report.clock_after[report.clocks] = after;
                    report.clocks += 1;
                } else {
                    report.truncated = true;
                }
            }
            Step::SelectParent { clock, parent } => {
                let Some(offset) = domain.clock_offset(clock) else {
                    report.rejected += 1;
                    continue;
                };
                let reg = (base + offset as usize) as *mut u32;
                // SAFETY: as `EnableClock`: a mapped device window per this function's contract,
                // and `clock_offset` bounded the offset inside the domain.
                let before = unsafe { core::ptr::read_volatile(reg) };
                // SAFETY: as above. A read-modify-write of this clock's own word, replacing only
                // the parent field (`with_parent`), so the enable bit and divider are kept.
                unsafe { core::ptr::write_volatile(reg, with_parent(before, parent)) };
                // SAFETY: as above; read back, for `EnableClock`'s reason.
                report.mux_after = unsafe { core::ptr::read_volatile(reg) };
                report.mux_before = before;
                report.had_mux = true;
            }
            Step::DeassertReset(id) => {
                let Some(bit) = domain.reset_bit(id) else {
                    report.rejected += 1;
                    continue;
                };
                let assert = (base + bit.assert_offset as usize) as *mut u32;
                let status = (base + bit.status_offset as usize) as *const u32;
                // SAFETY: `base` is a mapped device window per this function's contract, and
                // `reset_bit` returned `Some`, which bounds both offsets inside the domain.
                let before = unsafe { core::ptr::read_volatile(assert) };
                // SAFETY: as above. A read-modify-write, and it must be one: this word holds up
                // to 32 resets and clobbering it would release or hold every other line in the
                // domain. Nothing else in this kernel writes it (the only caller is the
                // boot/test thread that wires the service, the same single-writer argument
                // `entropy_service::WIRED` makes for its plain atomics), so the read and the
                // write cannot be separated by another writer.
                unsafe { core::ptr::write_volatile(assert, before & !bit.mask) };
                let mut polls = 0;
                let mut seen;
                loop {
                    // SAFETY: as above; a read of the status word inside the domain.
                    seen = unsafe { core::ptr::read_volatile(status) };
                    polls += 1;
                    if is_deasserted(seen, bit.mask) || polls >= POLL_LIMIT {
                        break;
                    }
                }
                // SAFETY: as above.
                report.reset_assert_after = unsafe { core::ptr::read_volatile(assert) };
                report.reset_assert_before = before;
                report.reset_status_after = seen;
                report.released = is_deasserted(seen, bit.mask);
                report.polls = polls;
                report.had_reset = true;
                report.resets += 1;
                report.resets_released += u32::from(report.released);
            }
        }
    }

    report
}

/// What [`set_card_clock`] did: the clock's word before and after, its parent's rate, and the rate
/// it now runs at, which is the number a driver dividing it for the card must be given.
#[cfg(feature = "storage_bench")] // the storage bench is the one boot that drives the SD/MMC controllers
#[derive(Debug, Clone, Copy)]
pub(crate) struct CardClock {
    pub before: u32,
    pub after: u32,
    pub parent_hz: u64,
    pub hz: u32,
}

/// Run an SD/MMC controller's card clock at no more than `want_hz` (the tree's
/// `assigned-clock-rates`), and say what rate it runs at, computed from the silicon's own words.
/// radon's firmware leaves the divider at 2, so the clock the tree calls 50 MHz runs at 198 MHz.
/// Gated while the divider moves, so the controller never sees a half-switched clock.
///
/// # Safety
///
/// `crg` must be the SYS CRG's mapped device window and `syscon` the SYS syscon's, and
/// `controller` must be 0 or 1.
#[cfg(feature = "storage_bench")] // the storage bench is the one boot that drives the SD/MMC controllers
pub(crate) unsafe fn set_card_clock(
    crg: usize,
    syscon: usize,
    controller: usize,
    want_hz: u32,
) -> CardClock {
    use jh7110_clock_and_reset::{
        SYSCLK_APB_BUS_FUNC, SYSCLK_AXI_CFG0, SYSCLK_BUS_ROOT, SYSCLK_SDIO_SDCARD,
        SYSCLK_STG_AXIAHB, axi_cfg0_hz, clock_divider, sdcard_divider_for, with_divider,
    };
    let reg = (crg + SYSCLK_SDIO_SDCARD[controller] as usize * 4) as *mut u32;
    // SAFETY: the two windows are this function's contract. The clock chain's words are read-only
    // status words at aligned offsets inside them; then the card clock's own word in the CRG: a
    // read, two stores that keep every field but the enable and the divider, and a read back.
    let (parent_hz, before, after) = unsafe {
        let word =
            |base: usize, offset: usize| core::ptr::read_volatile((base + offset) as *const u32);
        let parent_hz = axi_cfg0_hz(&jh7110_clock_and_reset::IcClkWords {
            bus_root: word(crg, SYSCLK_BUS_ROOT as usize * 4),
            axi_cfg0: word(crg, SYSCLK_AXI_CFG0 as usize * 4),
            stg_axiahb: word(crg, SYSCLK_STG_AXIAHB as usize * 4),
            apb_bus_func: word(crg, SYSCLK_APB_BUS_FUNC as usize * 4),
            pll2_dacpd_dsmpd_fbdiv: word(syscon, 0x2c),
            pll2_postdiv1: word(syscon, 0x30),
            pll2_prediv: word(syscon, 0x34),
        });
        let before = core::ptr::read_volatile(reg);
        let div = sdcard_divider_for(parent_hz, u64::from(want_hz));
        let gated = with_divider(before & !CLOCK_ENABLE, div);
        core::ptr::write_volatile(reg, gated);
        core::ptr::write_volatile(reg, gated | (before & CLOCK_ENABLE));
        (parent_hz, before, core::ptr::read_volatile(reg))
    };
    let div = u64::from(clock_divider(after).max(1));
    CardClock {
        before,
        after,
        parent_hz,
        hz: (parent_hz / div) as u32,
    }
}

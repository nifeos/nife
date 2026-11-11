//! The JH7110's clock and reset generator, the kernel's own volatile half: what the kernel's
//! watchdog needs to turn its clock on.
//!
//! `crates/jh7110_clock_and_reset` owns the offsets, bit positions, bring-up plan and device-tree
//! query, host-tested and pointer-free. This walks a plan it is handed against a base it is handed
//! and reports what the hardware said. It reaches into no kernel global (§4 (kernel shape)). Every
//! device a userspace driver serves asks the clock and reset server instead
//! (`user::jh7110_clock_and_reset_service`); the kernel keeps this walk for its own watchdog
//! (milestone 593 (a wedged kernel resets itself)), which the soak's supervisor pets from the
//! kernel. See notes/jh7110-clock-and-reset.md.
//!
//! # SAFETY
//!
//! [`bring_up`] stores to whatever address it is given, and the pure crate's fallback can name
//! `0x1023_0000` on a machine with nothing there. The caller must establish that the machine is a
//! JH7110 first; `memory::jh7110_clock_and_reset` records a region only when the tree names a CRG
//! or a JH7110 TRNG, so QEMU's `virt` never reaches this driver.

use jh7110_clock_and_reset::{
    BringUpReport, CLOCK_ENABLE, Domain, MAXIMUM_RECORDED_CLOCKS, Step, is_deasserted, with_parent,
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
pub unsafe fn bring_up(base: usize, domain: &Domain, plan: &[Step]) -> BringUpReport {
    let mut report = BringUpReport::default();

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
                if report.clocks < MAXIMUM_RECORDED_CLOCKS {
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
                report.select_parent_after = unsafe { core::ptr::read_volatile(reg) };
                report.select_parent_before = before;
                report.had_select_parent = true;
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
            // A step this driver predates is never stored through; the report says it was refused.
            _ => report.rejected += 1,
        }
    }

    report
}

//! **The reboot object: the authority to restart the machine, as a capability** (milestone 805
//! (`reboot` at the prompt), DECISIONS §251 (restarting the machine is a kernel object the
//! progenitor hands out)).
//!
//! PSCI and SBI calls are made from EL1 and S-mode, so on two of three architectures a user program
//! cannot reset the machine at all, and the kernel has to do it on somebody's behalf. Whose behalf
//! is the whole question, and §251's answer is: whoever holds this object. It has no payload, names
//! no device and has one method, [`abi::reboot::REBOOT`]. The kernel mints exactly one at boot and
//! grants it to the progenitor, which endows it only to a program whose manifest declares
//! `grant_plan::Manifest::reboot`.
//!
//! # What the method does, and what it deliberately does not
//!
//! - It calls [`prepare_reset_route`] (a no-op everywhere but a JH7110) and then
//!   `arch::reboot`. On success neither returns.
//! - If every route this architecture has was refused, `arch::reboot` prints the firmware's raw
//!   answer and returns the portable reason as one of four `abi::Error`s (`NoResetMechanism`,
//!   `ResetNotSupported`, `ResetDenied`, `ResetDidNotHappen`), and the method answers it: calef's ruling on §251's amendment, item 3 (2026-10-06 UTC).
//! - **It syncs nothing.** The kernel knows no filesystem, and a microkernel that did would be the
//!   bug. The `reboot` program sends `filesystem_protocol::fs::SYNC` first. A holder that skips it
//!   loses whatever the device had not flushed: a foot gun, recorded in §251 and in the program's
//!   `BUGS`, and the mitigation is that exactly one program is endowed.
//! - **It does not quiesce devices.** A DMA transfer in flight is cut off with everything else.
//!
//! # BUGS
//!
//! - **A firmware that accepts the call and hangs is indistinguishable from a slow reset**, from
//!   inside the machine. radon did exactly this on 2026-09-04; milestone 592 (radon's cold reboot
//!   dies in OpenSBI's PMIC write) holds the fix, which has not run on the board.
//! - **The caller gets the reason, not the number.** Four portable reasons cover every firmware;
//!   the raw code (PSCI's or SBI's own) is on the kernel's line just above.
//!
//! Name: provisional (milestone 805). The module, the object, its method and [`MARKER`] are
//! calef's to name; §251 calls them "the reboot object" and `REBOOT` for want of anything better.

use crate::{arch, println};

/// **The prefix on every line the reboot method prints** (milestone 805), so the gate that types
/// `reboot` and a person reading a bench capture can find what the kernel said about the reset in
/// one grep. Distinct from the soak's `soak-test-reboot:` on purpose: a capture that has both is a
/// capture of two different callers.
///
/// Name: provisional, milestone 805's lane, 2026-10-06 (UTC).
pub const MARKER: &str = "reboot:";

/// **`REBOOT`: restart the machine, or say why not** (DECISIONS §251, "The method").
///
/// Returns only when the firmware refused, with the refusal already printed; a successful reset
/// does not come back. The caller is `syscall::invoke`'s arm for `Object::Reboot`, and the
/// authority check is the capability itself: §251 asks for no rights bit beyond holding it.
pub fn restart() -> abi::Error {
    // Out of the ring first: once the reset starts, the drainer never runs again.
    crate::console::enter_reset();
    println!("{MARKER} the kernel was asked to restart the machine");
    let refused = cold_reset(MARKER);
    println!(
        "{MARKER} every reset route was refused ({refused:?}; the lines above say how); the \
         machine keeps running"
    );
    refused
}

/// **Try every reset route this machine has, board routes before firmware routes** (milestone 592,
/// 2026-10-10). The one place the order lives, so the reboot object and the rebooting soak cannot
/// disagree about it.
///
/// On a JH7110 whose tree names the PMIC's bus, the first route is the direct one: this kernel
/// programs the I2C controller's timing itself and writes the AXP15060's reset bit, which does not
/// depend on OpenSBI's driver at all. That is option B, built after the 2026-10-10 bench put the
/// firmware route in the outcome table's fourth row: bus up, PMIC read still failing ten times.
/// Whatever that attempt prints, the firmware route follows it, because on 2026-10-09 the firmware
/// route worked and one attempt's evidence is not a verdict either way.
pub fn cold_reset(marker: &str) -> abi::Error {
    prepare_reset_route(marker);
    pmic_reset_attempt(marker);
    arch::reboot(marker)
}

/// The AXP15060 direct-write route, on the machines that have the plan for it. A no-op with no
/// output everywhere else, including every machine CI boots. Returns only to say the attempt
/// failed and the firmware route is next; a write the PMIC honours never comes back.
#[cfg(target_arch = "riscv64")]
fn pmic_reset_attempt(marker: &str) {
    use jh7110_clock_and_reset::{
        IcClkWords, PMIC_RESET_BIT, PMIC_RESET_REG, SYS_SYSCON_BASE, SYSCLK_APB_BUS_FUNC,
        SYSCLK_AXI_CFG0, SYSCLK_BUS_ROOT, SYSCLK_STG_AXIAHB, i2c5_ic_clk, one_based_div,
        standard_mode_100k,
    };

    let Some((sys, bus)) = crate::memory::jh7110_pmic_bus() else {
        return;
    };
    let Some((controller, _size)) = bus.controller else {
        return;
    };
    let Some(address) = bus.pmic_address else {
        return;
    };

    // The input-clock chain, from the two windows the plan's guard mapped: the CRG words at their
    // ids' offsets, the PLL2 words in the syscon at the offsets the vendor's pll.c masks name.
    let word = |base: usize, offset: usize| unsafe {
        core::ptr::read_volatile((base + offset) as *const u32)
    };
    let crg = crate::arch::mmu::phys_to_virt(sys.base) as usize;
    let syscon = crate::arch::mmu::phys_to_virt(SYS_SYSCON_BASE) as usize;
    let words = IcClkWords {
        bus_root: word(crg, SYSCLK_BUS_ROOT as usize * 4),
        axi_cfg0: word(crg, SYSCLK_AXI_CFG0 as usize * 4),
        stg_axiahb: word(crg, SYSCLK_STG_AXIAHB as usize * 4),
        apb_bus_func: word(crg, SYSCLK_APB_BUS_FUNC as usize * 4),
        pll2_dacpd_dsmpd_fbdiv: word(syscon, 0x2c),
        pll2_postdiv1: word(syscon, 0x30),
        pll2_prediv: word(syscon, 0x34),
    };
    let ic = i2c5_ic_clk(&words);
    let mode = standard_mode_100k(ic);
    let fbdiv = (words.pll2_dacpd_dsmpd_fbdiv >> 17) & 0xfff;
    println!(
        "{marker} AXP15060: direct route, controller {controller:#x}, PMIC {address:#04x}, IC \
         clock {ic} Hz (bus_root {}, divs {}/{}/{}, pll2 fbdiv {fbdiv} prediv {} postdiv1 {}), \
         standard mode hcnt {} lcnt {} sda_hold {}",
        (words.bus_root >> 24) & 1,
        one_based_div(words.axi_cfg0, 2),
        one_based_div(words.stg_axiahb, 2),
        one_based_div(words.apb_bus_func, 4),
        words.pll2_prediv & 0x3f,
        1u32 << ((words.pll2_postdiv1 >> 28) & 3),
        mode.hcnt,
        mode.lcnt,
        mode.sda_hold,
    );

    let i2c = crate::designware_i2c::DesignWareI2c::new(
        crate::arch::mmu::phys_to_virt(controller) as usize
    );
    // The register is read first so the write sets the reset bit alone and the PMIC's other bits
    // keep whatever they hold, which is the difference between option B and the power-off bit 7
    // radon's OpenSBI sets unconditionally.
    let mut current = [0u8; 1];
    if let Err(failure) = i2c.write_read(&mode, address as u8, &[PMIC_RESET_REG], &mut current) {
        println!(
            "{marker} AXP15060: read of reg {PMIC_RESET_REG:#04x} failed: {failure:?}; the \
             firmware route is next"
        );
        return;
    }
    let value = current[0] | (1 << PMIC_RESET_BIT);
    println!(
        "{marker} AXP15060: reg {PMIC_RESET_REG:#04x} read {:#04x}, writing {value:#04x} (bit \
         {PMIC_RESET_BIT} set, every other bit as found); this line is the last if the PMIC honours it",
        current[0],
    );
    // The line above is the one the 2026-10-09 bench lost, so it goes out on the wire before the
    // byte that may cut power behind it.
    crate::console::drain();
    match i2c.write_read(&mode, address as u8, &[PMIC_RESET_REG, value], &mut []) {
        Ok(()) => {
            // The write completed and the board is still up: the PMIC acknowledged a byte it did
            // not act on, or acts slower than the controller's stop. Say so and let the firmware
            // route run rather than deciding the PMIC's timing from one round trip.
            println!(
                "{marker} AXP15060: write completed without a reset; the firmware route is next"
            );
        }
        Err(failure) => {
            println!("{marker} AXP15060: write failed: {failure:?}; the firmware route is next");
        }
    }
}

/// The aarch64 and x86_64 halves have no PMIC route: their firmware interfaces (PSCI, the FADT
/// register) are the whole story, so [`cold_reset`] goes straight to `arch::reboot`.
#[cfg(not(target_arch = "riscv64"))]
fn pmic_reset_attempt(marker: &str) {
    let _ = marker;
}

/// **Put back what the firmware's reset needs and U-Boot took away** (milestone 592 (radon's cold reboot dies in OpenSBI's PMIC write),
/// provisional).
///
/// On a JH7110 board, OpenSBI performs SBI SRST as an I2C write to the AXP15060 PMIC on I2C5, and
/// radon's U-Boot removes its I2C driver at `Starting kernel`, which gates the bus's clock and
/// asserts its reset. Radon's OpenSBI re-enables a clock, but it computes which one from the bus
/// node's name and U-Boot's tree names it `i2c@12050000`, so it ungates UART4's core clock
/// instead; and it never releases a reset. So the kernel ungates and releases I2C5 itself, from
/// the plan `memory::init` read out of the device tree, and prints every word it saw.
///
/// A no-op with no output on every machine that is not a JH7110 (`memory::jh7110_pmic_bus` is
/// `None` there), which is every machine CI boots. Two callers since milestone 805: the rebooting
/// soak and [`restart`], the two SBI resets nife makes on purpose. Moved here from `soak.rs` so the
/// second one could reach it without the soak's feature. The board test exit's shutdown takes the
/// same road and is recorded as a `BUGS` entry in milestone 592 rather than changed here.
///
/// Name: ratified 2026-10-06 (calef, #1783: "`prepare_reset_route`"), from
/// `prepare_the_reset_route`.
pub fn prepare_reset_route(marker: &str) {
    // Only a JH7110 has anything to prepare, and only riscv64 compiles the branch that reads it.
    #[cfg(not(target_arch = "riscv64"))]
    let _ = marker;
    #[cfg(target_arch = "riscv64")]
    if let Some((sys, bus)) = crate::memory::jh7110_pmic_bus() {
        use jh7110_clock_and_reset::Step;
        println!(
            "{marker} JH7110: bringing the PMIC's I2C bus back up first, because OpenSBI \
             resets this board with an I2C write to the AXP15060 (milestone 592). SYS CRG at \
             {:#x} ({}); plan {} ({} specifier(s) skipped{}).",
            sys.base,
            if sys.from_tree {
                "named by this machine's device tree"
            } else {
                "NOT named by this machine's tree: the constant mainline and the vendor agree on"
            },
            if bus.from_tree {
                "from the tree's own clocks and resets of the PMIC's bus"
            } else {
                "is the constant I2C5 plan (clock 143, reset 81), NOT read from this tree"
            },
            bus.skipped,
            if bus.truncated { ", TRUNCATED" } else { "" },
        );
        // SAFETY: `memory::init` recorded this window only for a machine whose tree names a
        // JH7110, and `mmu::map_everything` mapped exactly it, device-typed, in the direct map.
        // The plan's identifiers were bounded by `SYS` when it was built, and `bring_up` bounds
        // them again.
        let report = unsafe {
            crate::drivers::jh7110_clock_and_reset::bring_up(
                crate::arch::mmu::phys_to_virt(sys.base) as usize,
                &jh7110_clock_and_reset::SYS,
                bus.plan(),
            )
        };
        let clocks = bus.plan().iter().filter_map(|s| match s {
            Step::EnableClock(i) => Some(*i),
            Step::DeassertReset(_) | Step::SelectParent { .. } => None,
        });
        for (n, index) in clocks.enumerate().take(report.clocks) {
            println!(
                "{marker} JH7110: clock {index} {:#010x} -> {:#010x} ({})",
                report.clock_before[n],
                report.clock_after[n],
                if jh7110_clock_and_reset::is_clock_enabled(report.clock_after[n]) {
                    "running"
                } else {
                    "NOT running: the enable bit did not read back"
                },
            );
        }
        let reset = bus.plan().iter().rev().find_map(|s| match s {
            Step::DeassertReset(id) => Some(*id),
            Step::EnableClock(_) | Step::SelectParent { .. } => None,
        });
        if let Some(id) = reset {
            println!(
                "{marker} JH7110: reset {id} assert {:#010x} -> {:#010x}, status {:#010x} \
                 ({}, {} polls){}",
                report.reset_assert_before,
                report.reset_assert_after,
                report.reset_status_after,
                if report.released {
                    "released"
                } else {
                    "STILL HELD"
                },
                report.polls,
                if report.was_already_up() {
                    "; the bus was already up, so U-Boot's handover is NOT why the reset hangs"
                } else {
                    ""
                },
            );
        }
        if report.rejected > 0 {
            println!(
                "{marker} JH7110: {} step(s) REJECTED by the SYS domain's bounds: the plan \
                 and the domain disagree, which is a bug in this kernel, not the board",
                report.rejected
            );
        }
    }
}

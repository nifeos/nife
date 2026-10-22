//! **The PMIC reset attempt**: on a JH7110, asking the board's power controller over I2C before
//! the firmware; on every other machine, nothing.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

#[cfg(target_arch = "riscv64")]
use super::*;

/// The PMIC's bus, programmed for standard mode: the controller, its timing and the PMIC's
/// address, or `None` on a machine whose tree names no PMIC bus. Prints the clock chain it derived
/// the timing from, since a wrong IC clock is the first suspect when a transfer fails.
#[cfg(target_arch = "riscv64")]
fn pmic_bus(
    marker: &str,
) -> Option<(
    designware_i2c::DesignWareI2c,
    jh7110_clock_and_reset::StandardMode,
    u8,
)> {
    use jh7110_clock_and_reset::{
        IcClkWords, SYS_SYSCON_BASE, SYSCLK_APB_BUS_FUNC, SYSCLK_AXI_CFG0, SYSCLK_BUS_ROOT,
        SYSCLK_STG_AXIAHB, i2c5_ic_clk, one_based_div, standard_mode_100k,
    };

    let (sys, bus) = crate::memory::jh7110_pmic_bus()?;
    let (controller, _size) = bus.controller?;
    let address = bus.pmic_address?;

    // The input-clock chain, from the two windows the plan's guard mapped: the CRG words at their
    // ids' offsets, the PLL2 words in the syscon at the offsets the vendor's pll.c masks name.
    // SAFETY: every call passes one of those two mapped windows and an aligned offset inside it,
    // and these are read-only status words, so the read has no side effect.
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

    Some((
        designware_i2c::DesignWareI2c::new(crate::arch::mmu::phys_to_virt(controller) as usize),
        mode,
        address as u8,
    ))
}

#[cfg(target_arch = "riscv64")]
pub(super) fn pmic_reset_attempt(marker: &str) {
    use jh7110_clock_and_reset::{PMIC_RESET_BIT, PMIC_RESET_REG};
    let Some((i2c, mode, address)) = pmic_bus(marker) else {
        return;
    };
    // The register is read first so the write sets the reset bit alone and the PMIC's other bits
    // keep whatever they hold, which is the difference between option B and the power-off bit 7
    // radon's OpenSBI sets unconditionally.
    let mut current = [0u8; 1];
    if let Err(failure) = i2c.write_read(&mode, address, &[PMIC_RESET_REG], &mut current) {
        println!(
            "{marker} AXP15060: read of reg {PMIC_RESET_REG:#04x} failed: {failure:?}; the \
             firmware route is next"
        );
        return;
    }
    let value = current[0] | (1 << PMIC_RESET_BIT);
    println!(
        "{marker} AXP15060: reg {PMIC_RESET_REG:#04x} read {:#04x}, writing {value:#04x} (bit \
         {PMIC_RESET_BIT} set, every other bit as found); this line is the last if the PMIC honors it",
        current[0],
    );
    // The line above is the one the 2026-10-09 bench lost, so it goes out on the wire before the
    // byte that may cut power behind it.
    crate::console::drain();
    match i2c.write_read(&mode, address, &[PMIC_RESET_REG, value], &mut []) {
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

/// **What the PMIC remembers about the last power event**, printed and then cleared (milestone 225
/// (run the soak on radon, argon and xenon)). A reset that prints nothing from the kernel, as E27's
/// did, leaves no trace on the SoC side; the AXP15060 keeps its own. The registers are Linux's
/// `axp20x.h` names: the startup source (`0x00`), the PWROK and power-off control (`0x1f`), the
/// wake and shutdown-sequence controls (`0x31`, `0x32`), the interrupt enables (`0x40`, `0x41`) and
/// the latched interrupt status (`0x48`, `0x49`: die temperature, DCDC under-voltage, the power key).
/// The status is cleared after it is printed (write one to clear), so each boot reports only what
/// happened since the boot before it. Name: provisional.
#[cfg(all(target_arch = "riscv64", feature = "reboot_soak_test"))]
pub(crate) fn pmic_report(marker: &str) {
    const SHOWN: [u8; 8] = [0x00, 0x1f, 0x31, 0x32, 0x40, 0x41, 0x48, 0x49];
    const STATUS: [u8; 2] = [0x48, 0x49];
    // The PMIC's I2C controller comes out of the firmware gated and held in reset; the reset route
    // ungates it the same way before it speaks to the PMIC.
    super::prepare_reset_route(marker);
    let Some((i2c, mode, address)) = pmic_bus(marker) else {
        println!("{marker} AXP15060 report: this tree names no PMIC bus");
        return;
    };
    let mut values = [0u8; SHOWN.len()];
    for (reg, value) in SHOWN.iter().zip(values.iter_mut()) {
        let mut byte = [0u8; 1];
        if let Err(failure) = i2c.write_read(&mode, address, &[*reg], &mut byte) {
            println!("{marker} AXP15060 report: read of reg {reg:#04x} failed: {failure:?}");
            return;
        }
        *value = byte[0];
    }
    println!(
        "{marker} AXP15060 report: startup {:#04x} pwrok/voff {:#04x} wakeup {:#04x} down-seq \
         {:#04x} irq-enable {:#04x} {:#04x} irq-status {:#04x} {:#04x}",
        values[0], values[1], values[2], values[3], values[4], values[5], values[6], values[7],
    );
    for (reg, value) in STATUS.iter().zip(&values[6..]) {
        if *value != 0
            && let Err(failure) = i2c.write_read(&mode, address, &[*reg, *value], &mut [])
        {
            println!("{marker} AXP15060 report: clearing reg {reg:#04x} failed: {failure:?}");
        }
    }
}

/// The aarch64 and x86_64 halves have no PMIC route: their firmware interfaces (PSCI, the FADT
/// register) are the whole story, so [`cold_reset`](super::cold_reset) goes straight to `arch::reboot`.
#[cfg(not(target_arch = "riscv64"))]
pub(super) fn pmic_reset_attempt(marker: &str) {
    let _ = marker;
}

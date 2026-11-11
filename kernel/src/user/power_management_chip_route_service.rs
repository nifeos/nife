//! **The JH7110 PMIC route's wiring** (milestone 1062 (the device control planes leave the
//! kernel), step 5; §251 (restarting the machine is a kernel object the progenitor hands out), "A
//! board's PMIC route runs in userspace"), and the kernel's own calls to it.
//!
//! On a machine whose tree names the power controller's bus, [`ensure`] spawns
//! `components/src/power_management_chip_route.rs` once per boot. First it asks the clock and
//! reset server to bring the bus's I2C controller up (by the controller's base, the badge §320
//! (the JH7110 clock and reset wire) names it by). Then it derives the standard-mode timing from
//! the controller's input clock, read from the SYS CRG and syscon words in the direct map, and
//! hands the route the controller's first page, device-typed, and the receive side of one
//! endpoint. The progenitor holds the send side at slot 35 and hands it to `reboot` alone. Reached
//! only on riscv64: QEMU's `virt` names no PMIC bus, so [`ensure`] answers `None` on every machine
//! CI boots. See notes/board-reboot.md.
//!
//! # BUGS
//!
//! - A route that dies is not restarted, and `reboot` then blocks in its call (the route's BUGS).
//! - The timing is computed once, at boot, from the clock words then; a later change to the I2C
//!   controller's input clock is not seen.
//! - The soak no longer prints the AXP15060's latched status at boot: the route answers `RESET`
//!   alone.
//!
//! Name: provisional.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use driver_process_contract::power_management_chip_route::{
    self as wire, CONTROLLER_KIND, CONTROLLER_VIRTUAL_ADDRESS, REQUEST_SLOT, Status, Target,
};

use super::*;
use crate::capability_object::{Rights, rendezvous_capability};
use crate::println;

/// Whether this boot has tried to spawn the route, and the endpoint, `0` for none. One writer: the
/// boot thread that grants the progenitor its slots, before any other thread can ask.
static TRIED: AtomicBool = AtomicBool::new(false);
static REQUEST: AtomicU64 = AtomicU64::new(0);

/// **The route's endpoint, spawning the route on the first call**, or `None` on a machine whose
/// tree names no power controller bus, whose bus the clock and reset server did not bring up, or
/// whose archive has no route program. Each `None` on a JH7110 is printed once, with its reason.
///
/// Name: provisional.
pub(crate) fn ensure() -> Option<crate::sched::RendezvousId> {
    if !TRIED.swap(true, Ordering::AcqRel) {
        REQUEST.store(start().unwrap_or(0), Ordering::Release);
    }
    match REQUEST.load(Ordering::Acquire) {
        0 => None,
        request => Some(request),
    }
}

/// The line prefix, shared with the reboot object's so one grep finds every reset line.
const MARKER: &str = "reboot:";

/// Bring the bus up, derive the timing and spawn the route.
fn start() -> Option<crate::sched::RendezvousId> {
    let (_sys, bus) = crate::memory::jh7110_pmic_bus()?;
    let (controller, _size) = bus.controller_physical_window?;
    let address = u8::try_from(bus.power_management_chip_address?).ok()?;
    let brought_up = super::jh7110_clock_and_reset_service::bring_up(controller);
    if !brought_up.is_some_and(|b| b.is_up()) {
        println!(
            "{MARKER} JH7110: no PMIC route: the clock and reset server did not bring the I2C \
             controller at {controller:#x} up ({brought_up:?}), so `reboot` asks the firmware alone"
        );
        return None;
    }
    let timing = timing(controller, address)?;
    let Some(image) = crate::user::program("power_management_chip_route") else {
        println!("{MARKER} JH7110: no PMIC route: this archive carries no route program");
        return None;
    };
    let target = Target {
        address,
        register: jh7110_clock_and_reset::POWER_MANAGEMENT_CHIP_RESET_REGISTER,
        bit: jh7110_clock_and_reset::POWER_MANAGEMENT_CHIP_RESET_BIT,
    };
    let [arg1, arg2] = wire::pack_timing(&timing);
    let request = crate::sched::create_rendezvous();
    const _: () = assert!(REQUEST_SLOT == 0);
    crate::sched::spawn(move || {
        run(
            image,
            Spawn {
                arg0: target.pack(),
                arg1,
                arg2,
                // Slot 0: receive `reboot`'s call.
                grants: &[rendezvous_capability(request, Rights::READ)],
                maps: &[Mapping {
                    va: CONTROLLER_VIRTUAL_ADDRESS,
                    phys: controller,
                    flags: Flags::user_device(),
                }],
                windows: &[startup_page_contract::Window {
                    kind: CONTROLLER_KIND,
                    virtual_address: CONTROLLER_VIRTUAL_ADDRESS,
                    pages: 1,
                }],
            },
        )
    })
    .expect("could not spawn the PMIC route");
    Some(request)
}

/// **The standard-mode timing for the controller's input clock**, from the clock chain's words,
/// printed with them, since a wrong input clock is the first suspect when a transfer fails.
#[cfg(target_arch = "riscv64")]
fn timing(controller: u64, address: u8) -> Option<designware_inter_integrated_circuit::Timing> {
    use jh7110_clock_and_reset::{
        InterIntegratedCircuitClockWords,
        SYSTEM_CLOCK_ADVANCED_EXTENSIBLE_INTERFACE_CONFIGURATION_0,
        SYSTEM_CLOCK_ADVANCED_PERIPHERAL_BUS_FUNCTION, SYSTEM_CLOCK_BUS_ROOT,
        SYSTEM_CLOCK_SYSTEM_TOP_GROUP_ADVANCED_EXTENSIBLE_INTERFACE_ADVANCED_HIGH_PERFORMANCE_BUS,
        SYSTEM_CONTROLLER_BASE_PHYSICAL_ADDRESS, inter_integrated_circuit_5_clock_frequency,
        one_based_divider, standard_mode_100_kilohertz,
    };
    let (sys, _bus) = crate::memory::jh7110_pmic_bus()?;
    // The CRG words at their ids' offsets, the PLL2 words in the syscon at the offsets the
    // vendor's pll.c masks name.
    // SAFETY: `mmu::map_everything` maps the SYS CRG window and the syscon device-typed in the
    // direct map on every machine `jh7110_pmic_bus` answers for, every call passes one of those
    // two and an aligned offset inside it, and these are read-only status words, so the read has
    // no side effect.
    let word = |base: usize, offset: usize| unsafe {
        core::ptr::read_volatile((base + offset) as *const u32)
    };
    let crg = crate::arch::mmu::phys_to_virt(sys.base_physical_address) as usize;
    let syscon = crate::arch::mmu::phys_to_virt(SYSTEM_CONTROLLER_BASE_PHYSICAL_ADDRESS) as usize;
    let words = InterIntegratedCircuitClockWords {
        bus_root: word(crg, SYSTEM_CLOCK_BUS_ROOT as usize * 4),
        advanced_extensible_interface_configuration_0: word(
            crg,
            SYSTEM_CLOCK_ADVANCED_EXTENSIBLE_INTERFACE_CONFIGURATION_0 as usize * 4,
        ),
        system_top_group_advanced_extensible_interface_advanced_high_performance_bus: word(
            crg,
            SYSTEM_CLOCK_SYSTEM_TOP_GROUP_ADVANCED_EXTENSIBLE_INTERFACE_ADVANCED_HIGH_PERFORMANCE_BUS
                as usize
                * 4,
        ),
        advanced_peripheral_bus_function: word(
            crg,
            SYSTEM_CLOCK_ADVANCED_PERIPHERAL_BUS_FUNCTION as usize * 4,
        ),
        phase_locked_loop_2_dacpd_dsmpd_and_feedback_divider_word: word(syscon, 0x2c),
        phase_locked_loop_2_post_divider_1_word: word(syscon, 0x30),
        phase_locked_loop_2_pre_divider_word: word(syscon, 0x34),
    };
    let ic = inter_integrated_circuit_5_clock_frequency(&words);
    let mode = standard_mode_100_kilohertz(ic);
    let fbdiv = (words.phase_locked_loop_2_dacpd_dsmpd_and_feedback_divider_word >> 17) & 0xfff;
    println!(
        "{MARKER} AXP15060: PMIC route, controller {controller:#x}, PMIC {address:#04x}, IC clock \
         {ic} Hz (bus_root {}, divs {}/{}/{}, pll2 fbdiv {fbdiv} prediv {} postdiv1 {}), standard \
         mode hcnt {} lcnt {} sda_hold {}",
        (words.bus_root >> 24) & 1,
        one_based_divider(words.advanced_extensible_interface_configuration_0, 2),
        one_based_divider(
            words.system_top_group_advanced_extensible_interface_advanced_high_performance_bus,
            2
        ),
        one_based_divider(words.advanced_peripheral_bus_function, 4),
        words.phase_locked_loop_2_pre_divider_word & 0x3f,
        1u32 << ((words.phase_locked_loop_2_post_divider_1_word >> 28) & 3),
        mode.high_count,
        mode.low_count,
        mode.data_hold,
    );
    Some(designware_inter_integrated_circuit::Timing {
        control: mode.control,
        high_count: mode.high_count,
        low_count: mode.low_count,
        data_hold: mode.data_hold,
    })
}

/// No JH7110 outside riscv64, so no timing to derive.
#[cfg(not(target_arch = "riscv64"))]
fn timing(_controller: u64, _address: u8) -> Option<designware_inter_integrated_circuit::Timing> {
    None
}

/// **Ask the route to reset the board**, as the kernel's own bench paths do: returns only when the
/// board is still running, with the route's answer printed, and at once when there is no route.
///
/// Name: provisional.
pub(crate) fn reset(marker: &str) {
    let Some(request) = ensure() else {
        return;
    };
    println!(
        "{marker} AXP15060: asking the PMIC route to set the reset bit; this line is the last if \
         the PMIC honors it"
    );
    // Drained before the byte that may cut power, or the line above is lost.
    crate::console::drain();
    let [status, word, _] = crate::sched::ipc_call(request, [wire::request(wire::RESET), 0]);
    let (kind, failure) = wire::unpack_failure(word);
    match Status::from_word(status) {
        Some(Status::StillRunning) => println!(
            "{marker} AXP15060: the route wrote {word:#04x} and the board kept running; the \
             firmware route is next"
        ),
        Some(s) => println!(
            "{marker} AXP15060: the route answered {s:?} (I2C failure kind {kind}, word \
             {failure:#x}); the firmware route is next"
        ),
        None => println!(
            "{marker} AXP15060: the route answered {status:#x}, no status it sends; the firmware \
             route is next"
        ),
    }
}

//! The machine description, printed on every boot that is not a test or a measurement.
//!
//! `script/lint` reads [`print_machine_description`] from this file. See notes/boot-ladder.md.
//!
//! Name: provisional (milestone 876's lane, 2026-10-12 UTC).

use super::*;

/// Print what this machine is: the exception level, the ISA, firmware, the eight questions, the
/// paging geometry and the scheduler, ending with the `boot_ladder::MACHINE` line.
///
/// `boot_info_pointer` is the firmware handoff (a device tree, or `x86_64`'s PVH
/// `hvm_start_info`). On a board with no readable serial console these lines are the transcript,
/// so nothing in the body may be gated on a boot-mode feature, which `script/lint` checks. The
/// `test` and `bench` exclusions on the signature are not that switch: neither boot reaches here.
/// See notes/boot-ladder.md, "The machine description".
#[cfg(not(any(test, feature = "system_tests", feature = "bench")))]
pub(super) fn print_machine_description(boot_info_pointer: usize) {
    println!();
    println!("nife");
    // aarch64 only: `arch::entry_el` and `arch::current_el` exist nowhere else.
    #[cfg(target_arch = "aarch64")]
    {
        // Where the kernel is and where firmware put it: U-Boot enters at EL2 and `boot.s`
        // drops itself, and a new board's first boot shows which happened.
        let entered = arch::entry_el();
        let now = arch::current_el();
        if entered == now {
            println!("  exception level : EL{now}  (entered here)");
        } else {
            println!("  exception level : EL{now}  (entered at EL{entered}, dropped in boot.s)");
        }
    }
    arch::isa::print_summary();
    println!("  stack top       : {:#018x}", stack_top());
    println!(
        "  firmware handoff: {boot_info_pointer:#018x}  ({})",
        if cfg!(target_arch = "x86_64") {
            "PVH hvm_start_info"
        } else {
            "device tree"
        },
    );

    // The eight questions, in this order on every ISA, each answered in its own vocabulary. An
    // ISA that cannot answer says so in words, never with a blank.

    // 1. Processors: described (the firmware's tables) and online (this kernel's bring-up)
    //    disagree on a board where a core refused to start.
    println!(
        "  processors      : {} online of {} described, boot cpu hwid {:#x}",
        smp::online_count(),
        smp::described_count(),
        smp::hwid(cpu::id()).unwrap_or(0),
    );

    // 2. Memory.
    memory::print_summary();

    // 3. The console, which is the device carrying this sentence.
    console::print_summary();

    // 4. The interrupt controller, in each architecture's own vocabulary: a GICv2, a PLIC, or the
    //    local-APIC/IO-APIC pair.
    arch::irq::print_summary();

    // 5. The timer, and whether interrupts are unmasked: with them off nothing ever preempts.
    {
        use crate::arch::{interrupts, timer};
        println!(
            "  timer           : {} Hz tick, counter at {} MHz, interrupts {}",
            timer::TICK_HZ,
            timer::frequency() / 1_000_000,
            if interrupts::is_enabled() {
                "ON"
            } else {
                "off"
            },
        );
    }

    // 6. The initial ramdisk, with "passed but unparseable" apart from "none", which no test can
    //    tell apart.
    match memory::initial_ramdisk_region() {
        None => println!("  initial ramdisk : none (nothing was passed to this boot)"),
        Some((at, size)) => match user::initial_ramdisk().map(nifefs::Fs::parse) {
            Some(Ok(fs)) => println!(
                "  initial ramdisk : {size} bytes at {at:#018x}, a nifefs archive of {} program(s)",
                fs.len(),
            ),
            Some(Err(e)) => println!(
                "  initial ramdisk : {size} bytes at {at:#018x}, but it does not parse: {e:?}",
            ),
            None => println!(
                "  initial ramdisk : {size} bytes at {at:#018x}, recorded but not reachable",
            ),
        },
    }

    // 7. The PCIe window: configuration space, and the BAR window, which has been RAM on real
    //    hardware.
    match memory::pci_regions() {
        Some(((ecam, ecam_len), (bar, bar_len))) => {
            println!(
                "  pcie            : ecam {ecam:#018x}..{:#x}, bar window {bar:#x}..{:#x}",
                ecam + ecam_len,
                bar + bar_len,
            );
        }
        None => println!("  pcie            : none (this machine describes no host bridge)"),
    }

    // 8. The IOMMU, or its absence: without one, DMA confinement has no hardware half
    //    (notes/dma.md).
    arch::iommu::print_summary();

    // After the eight: what this kernel did to the machine, not what the machine is.
    arch::mmu::print_summary();
    println!(
        "  scheduler       : {} thread(s), round robin, preemptive",
        sched::thread_count(),
    );

    // The summary line, a contract: a stable prefix, identical on every ISA, that
    // `serial_console` and CI match. Last, so reaching it means the whole description printed.
    // Wording provisional (milestone 268 (every architecture boots the same way)).
    println!(
        "{}{}, {} processor(s), {} MiB, {} Hz",
        boot_ladder::MACHINE,
        arch::NAME,
        smp::online_count(),
        memory::stats().map_or(0, |s| (s.total as u64 * page_frames::FRAME_SIZE)
            / (1024 * 1024)),
        arch::timer::TICK_HZ,
    );
}

/// Return the address of the linker script's `__stack_top`.
///
/// The symbol's address is the value; there is nothing stored at it. See notes/linker-scripts.md.
#[cfg(not(any(test, feature = "system_tests")))]
#[cfg(not(feature = "bench"))] // tour-only, and the bench boot skips the tour
fn stack_top() -> usize {
    unsafe extern "C" {
        static __stack_top: core::ffi::c_void;
    }
    (&raw const __stack_top) as usize
}

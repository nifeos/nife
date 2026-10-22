//! **The machine description**, printed on every boot that is not a test or a measurement.
//!
//! `script/lint` reads [`print_machine_description`] from this file: the check from milestone 267
//! (the milestone tour is three things wearing one name).
//! Moved out of the crate root by milestone 876 (the kernel crate root is split along its
//! seams), bodies unchanged.
//!
//! Name: provisional (milestone 876's lane, 2026-10-12 UTC).

use super::*;

/// **The machine description: what this machine is, printed on every boot that reaches here.**
///
/// Paging, the ISA, firmware, the cores, the timer, the scheduler, and the memory map. It is
/// diagnostics rather than decoration, and it is the reason it is a function of its own rather
/// than the first half of a block whose second half is a demonstration.
///
/// **xenon is why.** At first light on that machine there was no serial console this project could
/// read, so these lines *were* the transcript, photographed off a monitor, and they are what
/// diagnosed the local-APIC collision and the PCI BAR window landing in RAM. A port is verified by
/// reading them.
///
/// **So nothing in here may be gated on a boot-mode feature.** `script/lint` checks that: the body
/// of this function must contain no `feature = "..."` cfg, because the failure it guards against is
/// somebody putting one line of a bring-up transcript behind the same switch that removes a
/// demonstration. Milestone 267 split the two for that reason; before it, one
/// `#[cfg(not(any(feature = "shell", feature = "initboot")))]` sat in the middle of a single block
/// and the boundary between the two audiences was a reader's inference rather than a name.
///
/// The `test` and `bench` exclusions on the signature are not that switch and are deliberately
/// kept. A `bench` boot diverges into `bench::run` before this point and never returns, and a
/// `test` boot exits through semihosting; neither is a boot anybody reads to bring up a board.
/// Every boot that reaches this line prints all of it.
#[cfg(not(any(test, feature = "system_tests", feature = "bench")))]
pub(super) fn print_machine_description(boot_info_pointer: usize) {
    println!();
    println!("nife");
    // The exception level is an aarch64 concept and reads an aarch64 system register, so the
    // line is gated rather than the whole banner. This is also what keeps `aarch64-cpu` out of
    // the other two architectures' dependency graphs; see the target table in kernel/Cargo.toml.
    #[cfg(target_arch = "aarch64")]
    {
        use aarch64_cpu::registers::CurrentEL;
        use tock_registers::interfaces::Readable;
        // Two numbers, because on a board they are the two different questions. Where the
        // kernel is now, and where firmware put it: U-Boot enters a payload at EL2 and
        // `boot.s` drops itself, and on the first boot of a new board this line is how
        // anyone finds out which of those happened. See milestone 127 (the seL4 machine).
        let entered = arch::entry_el();
        let now = CurrentEL.read(CurrentEL::EL);
        if entered == now {
            println!("  exception level : EL{now}  (entered here)");
        } else {
            println!("  exception level : EL{now}  (entered at EL{entered}, dropped in boot.s)");
        }
    }
    arch::isa::print_summary();
    println!("  stack top       : {:#018x}", stack_top());
    // **What the thing in the handoff register actually is**, which differs by architecture and is
    // the first thing a bring-up reader has to know: two of these machines pass a device tree and
    // one passes a PVH `hvm_start_info`. Milestone 268 replaced a line that called it a device tree
    // everywhere, which was true on two architectures out of three.
    println!(
        "  firmware handoff: {boot_info_pointer:#018x}  ({})",
        if cfg!(target_arch = "x86_64") {
            "PVH hvm_start_info"
        } else {
            "device tree"
        },
    );

    // ------------------------------------------------------------------------------------------
    // **The eight questions** (milestone 268, item 1), in this order on every architecture.
    //
    // The parity claim is *the same questions answered, not the same lines printed*: one of these
    // machines has ACPI and two have a device tree, so each answers in its own vocabulary. What is
    // not allowed is a blank. An architecture that cannot answer says so in words, because a
    // missing line and a line nobody wrote are indistinguishable to the person reading a
    // photograph of a monitor, which is the audience this whole block exists for.
    // ------------------------------------------------------------------------------------------

    // 1. Processors. Two counts, because they answer different questions: how many the machine
    //    *describes* is a fact about the firmware's tables, and how many are *online* is a fact
    //    about this kernel's bring-up. They disagree on a board where a core refused to start,
    //    which is the failure this line exists to make visible.
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

    // 5. The timer, and whether interrupts are unmasked at all. Both halves matter on a board: a
    //    correct tick rate with interrupts off is a machine that will never preempt anything.
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

    // 6. The initial ramdisk. Two facts, printed apart on purpose, because a boot where the
    //    loader passed an image the kernel cannot parse looks identical to a boot with no image at
    //    all from any test's point of view (the x86 arm learned this at milestone 161 (the
    //    `x86_64` kernel port)).
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

    // 7. The PCIe window: where configuration space is, and where this kernel places BARs. The
    //    second is the one that has actually been wrong on real hardware (milestone 256: a
    //    constant BAR window that was RAM on the first real machine), so it is printed rather
    //    than assumed.
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

    // 8. The IOMMU, or its absence. A machine without one still runs; what it does not have is the
    //    hardware half of DMA confinement (notes/dma.md), and that is a fact about the machine
    //    worth reading off the boot rather than inferring from a driver's silence.
    arch::iommu::print_summary();

    // The paging geometry, after the eight rather than inside them: it is a fact about what this
    // kernel did to the machine rather than about the machine, and on a bring-up it is read after
    // the questions above have said whether the machine is what was expected.
    arch::mmu::print_summary();
    println!(
        "  scheduler       : {} thread(s), round robin, preemptive",
        sched::thread_count(),
    );

    // **The summary line, and it is a contract** (milestone 268, item 3's sibling).
    //
    // Same shape and same reason as `self_test::VERDICT` below it: a **stable prefix, identical on
    // all three architectures**, so that `serial_console` and CI have something to match that does
    // not live inside one architecture's arm. Milestone 268's finding 3 is what happens without
    // one, and it hid for months.
    //
    // Printed last rather than first, deliberately: reaching it means the whole description
    // printed, which is the claim a ladder rung should make. A header would only mean the block
    // started.
    //
    // **Wording provisional** (milestone 268 (every architecture boots the same way)): a line two
    // programs agree on is an architect's under
    // AGENTS.md's *move fast on what can be undone* tenet, and a lane ships one and says so rather
    // than waiting.
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

/// Read `__stack_top` back out of the linker script, just to prove we can.
///
/// The linker invents this symbol and writes its address into the ELF; we declare
/// it here so Rust can see it. Note that we want the *address of* the symbol, not
/// its contents. There is no value there. See notes/linker-scripts.md.
#[cfg(not(any(test, feature = "system_tests")))]
#[cfg(not(feature = "bench"))] // tour-only, and the bench boot skips the tour
fn stack_top() -> usize {
    unsafe extern "C" {
        static __stack_top: core::ffi::c_void;
    }
    (&raw const __stack_top) as usize
}

//! **The kernel's Rust entry point, and the steps all three boots share.**
//!
//! `kernel_main` is the function `_start` calls on every architecture, and all three go through
//! [`bring_up_machine`] and everything after it here: the test suite, the measurement boots, the
//! ladder and the hand-over. The boot pointer it stashes ([`DTB`]) is here because all three boots
//! reach it. See notes/boot-ladder.md.
//!
//! Name: provisional (milestone 876's lane, 2026-10-12 UTC).

use super::*;

/// The physical address of the Device Tree Blob, as handed to us in `x0`.
///
/// Stashed here so the tests can assert that the boot protocol actually delivered one,
/// which is the whole point of shipping a flat arm64 Image instead of an ELF.
/// See notes/boot-protocol.md.
pub static DTB: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

/// **The boot device tree, parsed**, or the parse error if this machine did not hand us one.
///
/// The argument that the pointer is real is made once, here (DECISIONS §94 (what may live in a
/// userspace library, and what must be per-binary)). Returns `Err` rather than panicking: the early
/// RISC-V console continues without a tree, and on `x86_64` [`DTB`] holds a PVH `hvm_start_info`
/// pointer, which `from_ptr`'s magic check refuses. Callers that cannot continue keep their own
/// `expect`.
pub fn device_tree() -> Result<device_tree_blob::DeviceTreeBlob<'static>, device_tree_blob::Error> {
    let phys = DTB.load(core::sync::atomic::Ordering::Relaxed) as u64;
    // SAFETY: `kernel_main` stores the boot pointer here as its first statement, before any of
    // this function's callers can run, and firmware's blob stays where it is for the life of the
    // kernel, which is what `'static` claims. The value is physical (the boot protocol speaks in
    // physical addresses and we are running virtual), so the direct map names it. `from_ptr`
    // re-checks the magic before trusting anything else in the blob, which is what makes a wrong
    // pointer survivable rather than fatal.
    unsafe {
        device_tree_blob::DeviceTreeBlob::from_ptr(arch::mmu::phys_to_virt(phys) as *const u8)
    }
}

/// The kernel's Rust entry point, called from `_start` once we have a stack and a
/// zeroed `.bss`.
///
/// `extern "C"` matters: it tells Rust to follow the aarch64 calling convention
/// (AAPCS64), because assembly is about to call this and the two need to agree on
/// where arguments live. `boot_info_pointer` arrives in `x0`. See notes/registers.md.
///
/// `-> !` means this never returns, which is true: there is nowhere to return *to*.
// The icount boot parks before the bench boot it implies, so `bench::run()` and everything after
// it is deliberately unreachable in that one configuration (milestone 78).
// And riscv64's `storage_bench` boot diverges in `run_measurement_boot`, leaving the hand-over after
// it unreachable but compiled, which is what keeps the hand-over's callees linted in that build.
#[cfg_attr(
    any(feature = "icount", feature = "storage_bench"),
    allow(unreachable_code)
)]
#[unsafe(no_mangle)]
pub extern "C" fn kernel_main(boot_info_pointer: usize) -> ! {
    DTB.store(boot_info_pointer, core::sync::atomic::Ordering::Relaxed);

    // Per-CPU pointer FIRST, before anything takes a lock. The lock path reads this core's
    // held-rank out of its per-CPU block, so `TPIDR_EL1` must point at that block before
    // `console::init` (the first lock) runs. On one core this is pure setup with no visible
    // effect; it is the foundation SMP is built on. See cpu.rs and DECISIONS §11.
    cpu::init_this_cpu(arch::boot_cpu_id());

    // Console first, exceptions second, and the order is not arbitrary: the fault
    // handler's entire job is to print, so it is useless until the UART works. The
    // window between these two lines is the last place in the kernel where a fault
    // still kills us silently.
    console::init();

    // The console's register-block *shape* comes from the device tree, and it must be adopted
    // before the first println: on the JH7110 the DW-8250's registers are four bytes apart, so a
    // byte-strided LSR poll spins on garbage forever and the banner never appears
    // (notes/visionfive2.md; console::configure_from_dtb). On QEMU the tree restates the defaults
    // and this is a no-op in effect.
    #[cfg(target_arch = "riscv64")]
    console::configure_from_dtb();

    // The screen, before the banner: everything printed after this is on a monitor too. It needs
    // only the console lock and the tree pointer, and must run before `arch::mmu::init`. Its own
    // line is printed later, by a console that has the screen. See `screen::attach`.
    #[cfg(not(target_arch = "x86_64"))]
    let screen = screen::attach();

    // The earliest line, `nife on <arch>`, which `serial_console`'s `Stage::Banner` matches on
    // every architecture. aarch64's claims only what is true here: the console is up and the MMU
    // is not. See notes/boot-ladder.md.
    #[cfg(target_arch = "aarch64")]
    {
        use aarch64_cpu::registers::CurrentEL;
        use tock_registers::interfaces::Readable;
        println!();
        println!(
            "{}aarch64 (EL{}, MMU off: physical addresses until mmu::init)",
            boot_ladder::BANNER,
            CurrentEL.read(CurrentEL::EL),
        );
        screen::print_summary(&screen);
    }
    #[cfg(target_arch = "riscv64")]
    riscv64_banner(boot_info_pointer, &screen);
    #[cfg(target_arch = "x86_64")]
    x86_64_banner(boot_info_pointer);

    bring_up_machine(boot_info_pointer);

    #[cfg(any(test, feature = "system_tests"))]
    run_test_suite();

    // The instruction-count boot (`script/icount`) parks here. Its feature implies `bench`, since
    // both leave the same functions unreferenced, so the `bench::run` below is unreachable in it.
    #[cfg(feature = "icount")]
    icount::run();

    // The benchmark boot (milestone 21, `script/bench`): run the microbenchmarks and halt,
    // instead of the tour or the shell. Diverges, so everything below is untouched by it.
    #[cfg(feature = "bench")]
    bench::run();

    #[cfg(not(any(test, feature = "system_tests", feature = "bench")))]
    {
        print_machine_description(boot_info_pointer);

        // Rung two of the ladder: the self-test reports and does not gate, because a machine you
        // cannot log into is a machine you cannot fix. See notes/boot-ladder.md.
        self_test::run();

        // `boot_ladder::TOUR` (`Stage::Tour`): only the hand-over may lie between this line and the
        // prompt, because `serial_console` stops treating silence as a hang here.
        println!("{}{}.", boot_ladder::TOUR, arch::NAME);
        sched::note_boot_stage(self_test::TOUR_STAGE);

        // A measurement boot takes the place of the hand-over, so that what a soak stresses is
        // decided rather than incidental; otherwise the progenitor builds the system.
        run_measurement_boot();

        #[cfg(not(any(
            feature = "soak_test",
            feature = "job_mix",
            feature = "disk_throughput",
            feature = "network_bench"
        )))]
        hand_over();
    }

    // Every boot that did not diverge ends here, a failed hand-over included. The boot thread
    // leaves the scheduler rather than halting: a halted thread that is still runnable stops the
    // core until the next tick every time round robin reaches it.
    #[cfg(not(any(
        feature = "bench",
        feature = "soak_test",
        feature = "job_mix",
        feature = "disk_throughput",
        feature = "network_bench"
    )))]
    sched::exit()
}

/// **Hand the machine to the userspace progenitor**, on all three architectures.
///
/// The boot thread's work is done once this returns; the progenitor and the services it builds run
/// until halt. A boot with no initial ramdisk says so and has nothing to hand over.
#[cfg_attr(
    any(
        test,
        feature = "system_tests",
        feature = "bench",
        feature = "soak_test",
        feature = "job_mix",
        feature = "disk_throughput",
        feature = "network_bench"
    ),
    allow(dead_code)
)]
fn hand_over() {
    // The install offer, before the handoff: it asks on the console, and the progenitor gives the
    // console's UART to a userspace input driver. A no-op unless a UEFI loader left its file.
    #[cfg(target_arch = "x86_64")]
    user::install_service::offer();

    let Some(image) = user::initial_ramdisk() else {
        println!();
        println!("nife: no archive to hand the system to (no initial ramdisk was loaded).");
        return;
    };
    println!();
    println!("nife: handing the system to the userspace progenitor.");
    if let Err(e) = user::boot_progenitor(image) {
        println!("  handoff FAILED: {e:?}");
    }
}

/// riscv64's opening lines: the banner, where the kernel is running, and the screen. aarch64's
/// are inline in [`kernel_main`]; these are a function because they say more, since this port has
/// Sv39 on and is running in the high half by the time it prints.
///
/// The bring-up's own demonstration lines (`traps`, `timer`, `memory`, `paging`) are not printed
/// here: `self_test::run` makes each of those claims on all three architectures as its
/// `exceptions`, `timer`, `frames` and `mapping` checks, and `serial_console` reads the ladder.
#[cfg(target_arch = "riscv64")]
fn riscv64_banner(
    boot_info_pointer: usize,
    screen: &Result<(machine_discovery::framebuffer::Framebuffer, u32, u32), screen::Why>,
) {
    // A live code address proves we execute from the high-half alias, not merely reach the UART
    // through the identity map (the boot table maps both).
    let pc = kernel_main as *const () as usize;
    println!();
    println!("{}RISC-V (rv64, S-mode, Sv39)", boot_ladder::BANNER);
    println!("  hart 0 booted: high-half kernel, .bss, and the NS16550 console are up.");
    println!("  running at  : {pc:#018x}  (high half: Sv39 paging is on)");
    println!("  device tree : {boot_info_pointer:#018x}");
    screen::print_summary(screen);
}

/// `x86_64`'s opening lines: the screen, the banner, where the kernel is running, and the boot
/// handoff. A function for riscv64's reason, and the screen comes first in it because this port's
/// boot chain lights one and the other two do not.
#[cfg(target_arch = "x86_64")]
fn x86_64_banner(boot_info_pointer: usize) {
    // A live code address proves the far jump landed in the high half, not merely that the UART
    // is reachable through the identity map (the boot table maps both).
    let pc = kernel_main as *const () as usize;

    // The screen, before the first word: a commodity machine has no serial port. The boot tables
    // already cover the aperture, and `memory::record_framebuffer` (called inside) carries it
    // across to the fine map.
    let screen = arch::machine::attach_screen(boot_info_pointer);

    println!();
    println!(
        "{}x86_64 (long mode, ring 0, 4-level paging)",
        boot_ladder::BANNER,
    );
    println!("  cpu 0 booted: high-half kernel, .bss, and the 16550 console are up.");
    println!("  running at  : {pc:#018x}  (high half: the long-mode jump landed)");
    println!("  boot info   : {boot_info_pointer:#018x}  (PVH hvm_start_info, not a device tree)");
    // Said here, by a console that already has the screen it describes.
    match screen {
        Some((found, cols, rows)) => println!(
            "  screen      : {}x{} {} at {:#x}, {cols}x{rows} cells (boot cmdline)",
            found.width,
            found.height,
            found.order.token(),
            found.base,
        ),
        None => println!("  screen      : none in the boot cmdline; this console is the UART"),
    }
}

/// The shared bring-up, which all three architectures reach: arch, memory, paging, IOMMU,
/// interrupts, cores.
///
/// Each step reads the machine from what it describes itself with: a device tree on aarch64 and
/// riscv64, and on `x86_64` the PVH handoff and ACPI, which `memory::init` walks once and keeps
/// (`arch::discovery`). What riscv64 and `x86_64` need that aarch64 does not is a few calls into
/// their own `arch/`, each `cfg`-gated beside the aarch64-only line: the interrupt contexts and the
/// timer's rate before the MMU, the ISA line and the cycle counter after it, and their IOMMUs. Each
/// port's `arch::irq::init` is reached through [`interrupts_init`].
fn bring_up_machine(boot_info_pointer: usize) {
    arch::init();
    stack::init();
    // Paint the boot stack's unused region for the high-water report (milestone 84), before
    // memory::init and everything after it can push frames into it.
    #[cfg(any(test, feature = "system_tests"))]
    stack::paint_boot_stack();

    // After `arch::init`, so a fault in here is reported rather than silent.
    memory::init();

    // Before `mmu::init`, which takes `TCR_EL1.IPS` from this record and needs the 4 KiB granule
    // this checks. A machine that cannot run us says so here, not by faulting with the MMU on.
    arch::isa::init(boot_info_pointer);

    // The core list, read while the tree is reachable: `mmu::init` replaces the coarse boot map
    // the blob is only reachable through. `isa::init` above read `/psci`.
    smp::read_cpu_list();

    // `CNTFRQ_EL0` against the tree's counter rate, before `mmu::init` for the same reason: a
    // wrong-but-plausible firmware value is the one failure no later check can see.
    #[cfg(target_arch = "aarch64")]
    arch::timer::check_frequency_against_device_tree(boot_info_pointer);

    // The interrupt contexts and the counter's rate, on the two ports with no `CNTFRQ_EL0`, from
    // the same window: both before `pmu::init` times itself and `interrupts_init` arms a deadline.
    // On `x86_64` the rate is measured against the PIT through the local APIC, so contexts first.
    #[cfg(any(target_arch = "riscv64", target_arch = "x86_64"))]
    {
        arch::irq::init_contexts(boot_info_pointer);
        arch::timer::init_frequency(boot_info_pointer);
    }

    // And now the sketchiest moment in the kernel. The instant SCTLR_EL1.M is set, the very
    // next instruction is fetched through the MMU. See arch/aarch64/mmu.rs.
    arch::mmu::init();

    // Not one line earlier: the interrupt stacks' guard pages are holes in the map `mmu::init`
    // just installed, and the coarse boot map has none. See kernel/src/interrupt_stack.rs.
    interrupt_stack::init();

    // The ISA line, after `mmu::init` because one of riscv64's numbers is measured there, then the
    // cycle counter (riscv64 asks SBI; `x86_64` spins against the TSC rate measured above).
    // aarch64 starts its counter per core inside `timer::init`.
    #[cfg(any(target_arch = "riscv64", target_arch = "x86_64"))]
    {
        arch::isa::print_summary();
        arch::pmu::init();
    }

    // The SMMUv3, if present, after `mmu::init` mapped its block: default-deny, so every PCIe
    // stream aborts until `virtio::register` confines its device. See notes/iommu.md.
    #[cfg(not(target_arch = "x86_64"))]
    if let Some((smmu_base, _)) = memory::smmu_region() {
        arch::iommu::init(smmu_base);
    }
    // `x86_64`'s, at the same point: every VT-d or AMD-Vi unit the tables name.
    #[cfg(target_arch = "x86_64")]
    arch::discovery::bring_up_iommus();
    // The RISC-V IOMMU, a PCI function, here for the SMMUv3's reason: before any path confines a
    // virtio-pci device.
    #[cfg(target_arch = "riscv64")]
    pci::init_iommu();

    // The scheduler before interrupts, so the first timer tick has somewhere to send a reschedule.
    sched::init();
    interrupts_init(boot_info_pointer);

    // After interrupts, so the boot core keeps ticking while it waits for the others. See
    // notes/smp.md.
    smp::bring_up_secondaries();

    // Every core's cycle counter, here because it is the first point every core has answered.
    arch::pmu::print_summary();
}

/// Bring up the interrupt controller and the timer, then **unmask interrupts**.
///
/// This is the line the whole locking discipline was written for. From here, a timer interrupt
/// can land between any two instructions in the kernel, and every `IrqSafeMutex` starts
/// actually masking something. See DECISIONS §9 and notes/locking.md.
fn interrupts_init(_dtb: usize) {
    use crate::arch::{interrupts, irq, timer};

    // The interrupt controller before the timer: on `x86_64` its bring-up drives one device
    // interrupt through the IO APIC while the tick is not yet armed.
    irq::init();

    timer::init();

    // From here, preemptible.
    interrupts::enable();
}

#[cfg(all(feature = "system_tests", not(test)))]
unsafe extern "Rust" {
    /// The hook the system-test image defines (`system_tests/src/main.rs`). A kernel built with the
    /// `system_tests` feature calls it where a `cargo test` kernel calls `test_main`, so the same boot
    /// runs the other crate's suite.
    ///
    /// A foot gun that fails loudly: Cargo unifies features, so `cargo build --workspace` would
    /// build the kernel binary with this feature on, and the link fails for want of this symbol.
    /// `script/drift` builds `system_tests` separately for that reason.
    fn system_tests_main();
}

/// Runs the suite this image carries: the kernel's own unit tests under `cargo test -p kernel`, or
/// the system tests when `system_tests/` links this library with its feature on.
#[cfg(any(test, feature = "system_tests"))]
pub(super) fn run_test_suite() {
    // Keep `TRUST_ROOT` in a test image too: `uefi_loader/build.rs` and `sealed_pair` find a
    // kernel's archive by these digests in its bytes, and nothing in a test image calls
    // `trust::verify`, so the linker would drop them.
    core::hint::black_box(trust::TRUST_ROOT);
    #[cfg(test)]
    test_main();
    // SAFETY: `system_tests_main` is a plain Rust function the linked system-test image defines,
    // with this exact signature; it takes nothing and returns when its suite has run.
    #[cfg(all(feature = "system_tests", not(test)))]
    unsafe {
        system_tests_main();
    }
}

/// The measurement boots, each diverging; with none built, the no-op below stands in for this and
/// the boot hands over.
///
/// Two definitions, so the type says what the boot does: this one is `-> !` in a build with a
/// measurement feature, which lets `kernel_main` end without its `sched::exit()` there.
#[cfg(any(
    feature = "job_mix",
    feature = "soak_test",
    feature = "disk_throughput",
    feature = "network_bench",
    feature = "storage_bench"
))]
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)]
pub(super) fn run_measurement_boot() -> ! {
    // Each in place of the hand-over, after the tour, so the whole boot is evidence. The features
    // are alternatives rather than a stack.
    #[cfg(feature = "job_mix")]
    job_mix::run();
    #[cfg(feature = "soak_test")]
    soak::run();
    // It WRITES to the NVMe disk; see kernel/src/disk_throughput.rs.
    #[cfg(feature = "disk_throughput")]
    disk_throughput::run();
    #[cfg(feature = "network_bench")]
    network_bench::run();
    #[cfg(feature = "storage_bench")]
    storage_bench::run();
}

/// No measurement boot was built, so there is nothing to run, and the boot goes on to hand over.
#[cfg(not(any(
    feature = "job_mix",
    feature = "soak_test",
    feature = "disk_throughput",
    feature = "network_bench",
    feature = "storage_bench"
)))]
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)]
pub(super) fn run_measurement_boot() {}

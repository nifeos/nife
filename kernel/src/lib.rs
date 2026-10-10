//! nife
//!
//! # Why the attributes at the top
//!
//! `no_std` : there is no operating system beneath us, because we *are* the
//!             operating system. `std`'s `File::open` would make a syscall, and
//!             there is nobody to answer it. We link only `core`.
//!
//! `no_main`: in a normal program `main` is not the first thing to run. The C
//!             runtime (`crt0`) sets up the stack, initializes libc, builds `argv`,
//!             and *then* calls `main`. There is no libc here and nobody has set up
//!             a stack, so there can be no `main`. Our entry point is `_start`, in
//!             assembly, and it sets up the stack itself.
//!
//! See notes/no-std.md.
//!
//! # Why a library
//!
//! Two images link this crate (milestone 609 (the system tests leave the kernel crate)): the
//! kernel binary, `src/main.rs`, which is nothing but a link line, and the system-test image,
//! `system_tests/`, which is the same kernel with the whole-system suite on top. So everything
//! lives here, and `no_main` applies only when `cargo test` builds this library as its own
//! bootable test image for the kernel's unit tests.

#![no_std]
#![cfg_attr(any(test, feature = "system_tests"), no_main)]
// Not the crates/ library surface milestone 68's ratchet tracks (DECISIONS §107): the kernel binary
// is one crate root behind an ABI boundary, not a documented API.
#![allow(missing_docs)]
// And clippy's public-API lints, which fire in that image only because `system_test_access` makes
// kernel internals reachable from outside: `Holding::new` without a `Default`, a `Result<_, ()>`.
// They are advice about a crate's published surface, and the kernel publishes none.
#![cfg_attr(
    all(feature = "system_tests", not(test)),
    allow(clippy::new_without_default, clippy::result_unit_err)
)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::testing::runner)]
#![reexport_test_harness_main = "test_main"]
// The RISC-V boot runs a self-contained tour (in `kernel_main` below) that ends in `sched::exit()`,
// before the shared, still-aarch64-shaped full boot (userspace progenitor as the boot process, the shell,
// the virtio service). That full-boot code and its helpers are therefore unreferenced from a riscv64
// build and look like dead code, even though the `arch` layer itself is fully implemented. Allow it
// *only* on riscv64, so the aarch64 build keeps full dead-code checking; it goes away if the RISC-V
// boot is ever wired into the shared path instead of halting. See notes/riscv-port.md.

mod arch;
#[cfg(feature = "bench")]
mod bench;
mod cap;
mod console;
mod cpu;
mod drivers;
#[cfg(feature = "fastpath_pad")]
mod fastpath_pad;
mod fp;
#[cfg(feature = "icount")]
mod icount;
mod interrupt_stack;
mod iommu;
#[cfg(any(test, feature = "ipc_stack_depth"))]
mod ipc_stack_depth;
mod kmem;
mod machine_statistics;
// The kernel's ring and where each of its lines goes (milestone 342 (the kernel and the `console`
// server drive one UART from two address spaces)). See its module doc.
mod kernel_log;
mod memory;
mod panic;
#[cfg(test)]
mod preemption_window_tests;
// PCIe enumeration + virtio-pci bring-up (the PCIe transport, DECISIONS §18). Portable: the
// decode logic is crates/pci, and each arch supplies its window/irq constants. See
// kernel/src/pci.rs.
mod pci;
// The NVMe block driver (milestone 53's storage half): the volatile half of crates/non_volatile_memory_express, brought
// up over the PCIe transport above and confined behind the machine's IOMMU. See kernel/src/non_volatile_memory_express.rs.
// Only the test boot drives it today (nothing production-wired rides NVMe until the block-server
// question in notes/non-volatile-memory-express.md's BUGS is decided), so the non-test build allows it dead rather than
// cfg-gating a module whose next caller is already known.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
mod non_volatile_memory_express;
// The `e1000e` NIC's control plane (milestone 494 (a driver for the network card a PC actually
// has)): reset, MAC address, ring bases, then the queue pages go to `net_stack`. See
// kernel/src/e1000e.rs. Driven only by the test boot so far, like the NVMe module above.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
mod e1000e;
// The JH7110 Ethernet port's control plane (milestone 53 (the board's own peripherals: network and
// storage on real silicon)): clocks, PHY, the DMA-coherence probe, ring bases, then the DMA page goes
// to `net_stack`. riscv64-only because the JH7110 is; the module header carries the parity note.
// See kernel/src/designware_ethernet.rs.
#[cfg(target_arch = "riscv64")]
mod designware_ethernet;
mod designware_i2c;
// The xHCI bring-up policy (milestone 242 (USB host and HID)): find the controller, take it from
// the firmware, draw the driver's register window and confine its DMA, then hand the whole
// controller to `usb_keyboard_driver` at EL0. See kernel/src/extensible_host_controller_interface.rs.
mod extensible_host_controller_interface;
mod revoke;
mod sched;
// **A screen on the two architectures whose firmware never lights one**, milestone 243 (a machine
// with no serial port): the `ramfb` discovery half, and the question of whether a framebuffer is
// this kernel's own memory.
mod screen;
// The boot self-tests (milestone 268): the kernel proving it works on this machine, between the
// machine description and the hand-off to userspace. The same set on all three architectures. A
// `bench` or `icount` boot parks before userspace by design and never reaches this, and a `test`
// boot exits through semihosting; neither is a boot anybody reads to bring up a board, which is the
// same exclusion `print_machine_description` carries and for the same reason.
#[cfg(not(any(test, feature = "system_tests", feature = "bench")))]
mod self_test;
mod smp;
// The sustained multicore workload (milestone 219). Behind its own feature because an ordinary boot
// must still halt: this module is the thing that makes a boot never end.
#[cfg(feature = "job_mix")]
mod job_mix;
// How often the job mix's threads found a kernel lock held (2026-10-04, fatal risk 4). Its own
// feature on top of `job_mix`, because its counting sits on the lock path and the job-mix image
// that is not asked for it should be the kernel that ships.
#[cfg(feature = "lock_wait")]
mod lock_wait;
// Fatal risk 6's bench boot (milestone 261 (the NVMe driver leaves the kernel)): preflight the two night-of conditions, then measure a
// confined EL0 NVMe driver's throughput and halt. Behind a feature because it writes to the disk.
#[cfg(feature = "disk_throughput")]
mod disk_throughput;
#[cfg(feature = "network_bench")]
mod network_bench;
// Milestone 53 (the board's own peripherals: network and storage on real silicon)'s storage bench
// boot on radon: the SD/MMC controller's first contact with silicon, read-only unless built to
// write. riscv64 only, because the controller is the JH7110's. See kernel/src/storage_bench.rs.
#[cfg(all(feature = "storage_bench", target_arch = "riscv64"))]
mod storage_bench;
#[cfg(all(feature = "storage_bench", not(target_arch = "riscv64")))]
compile_error!("storage_bench probes the JH7110's SD/MMC controller and builds only for riscv64");
// The JH7110's SD/MMC controller's volatile half (milestone 53): a mapped window and a clock for
// `designware_mobile_storage`'s driver. riscv64-only because the JH7110 is; the module header
// carries the parity note. See kernel/src/designware_mobile_storage.rs.
#[cfg(target_arch = "riscv64")]
#[cfg_attr(not(feature = "storage_bench"), allow(dead_code))]
mod designware_mobile_storage;
#[cfg(feature = "soak_test")]
mod soak;
// The progenitor's stack high-water gauge and its headroom floor (name provisional).
mod progenitor_stack;
// The reboot object's method (milestone 805 (`reboot` at the prompt), DECISIONS §251 (restarting
// the machine is a kernel object the progenitor hands out)), and the JH7110 reset preparation it
// shares with the rebooting soak.
mod reboot;
mod stack;
mod sync;
mod syscall;
mod thread;
// The measured-boot trust root: the digest of the boot program this kernel image was built
// against (milestone 22 phase B.1, DECISIONS §22). Generated into the image by build.rs.
mod memory_region;
// The test kernel's Nth-retype fault (milestone 757 (a test kernel fails a process on its Nth
// retype), provisional). Only the system-test image has
// it, and every call into it carries the same `cfg`, so a shipping build has no code that could
// fire. See the module's own header.
#[cfg(feature = "system_tests")]
mod retype_fault;
// The test kernel's pause at the start of a delegation (the revocation-race lane, provisional),
// under the same `cfg` as `retype_fault` and for its reason. See the module's own header.
#[cfg(feature = "system_tests")]
mod delegation_pause;
mod trust;
mod user;
mod virtio;

// `print!` expands to `$crate::_print`, and a macro another crate expands can only name what that
// crate can see: `console` is private, so its entry point is re-exported here (milestone 609 (the
// system tests leave the kernel crate)).
#[doc(hidden)]
pub use console::_print;

#[cfg(any(test, feature = "system_tests"))]
mod testing;

// The two statics `skip!` (testing.rs) writes, at a path any crate that expands it can name. The
// macro is exported because most of its callers now live in `system_tests/`, and from there
// `$crate::testing` is a private module (milestone 609 (the system tests leave the kernel crate)).
#[cfg(any(test, feature = "system_tests"))]
#[doc(hidden)]
pub use testing::{SKIP_REASON, SKIP_REASON_LEN};

/// **The kernel's insides, for the system-test image and nothing else** (milestone 609 (the system
/// tests leave the kernel crate)). The whole-system suite lives in `system_tests/`, a second image
/// that links this library, and its tests observe state no syscall exposes: the scheduler's queues,
/// capability slots, region accounting. So under the `system_tests` feature, and only there, each
/// module that suite names is re-exported here, and the test crate glob-imports this module so its
/// `crate::sched::...` paths read exactly as they did when the files lived in this crate.
///
/// The modules themselves stay private. Making them `pub` would have been one word each and would
/// have switched off dead-code warnings for the whole kernel, because a library's public items are
/// never dead. A facade that exists only in the test image keeps every ordinary build checked.
///
/// Each `pub use` brings a module's `pub` items only. A test that needs something private asks for
/// it to be made `pub`; that is the visibility cost of the move, paid where it is visible.
#[cfg(feature = "system_tests")]
#[doc(hidden)]
pub mod system_test_access {
    pub mod arch {
        pub use crate::arch::*;
    }
    pub mod cap {
        pub use crate::cap::*;
    }
    pub mod console {
        pub use crate::console::*;
    }
    pub mod cpu {
        pub use crate::cpu::*;
    }
    pub mod iommu {
        pub use crate::iommu::*;
    }
    pub mod kernel_log {
        pub use crate::kernel_log::*;
    }
    pub mod memory {
        pub use crate::memory::*;
    }
    pub mod machine_statistics {
        pub use crate::machine_statistics::*;
    }
    pub mod memory_region {
        pub use crate::memory_region::*;
    }
    pub mod non_volatile_memory_express {
        pub use crate::non_volatile_memory_express::*;
    }
    pub mod e1000e {
        pub use crate::e1000e::*;
    }
    pub mod retype_fault {
        pub use crate::retype_fault::*;
    }
    pub mod delegation_pause {
        pub use crate::delegation_pause::*;
    }
    pub mod revoke {
        pub use crate::revoke::*;
    }
    pub mod sched {
        pub use crate::sched::*;
    }
    pub mod smp {
        pub use crate::smp::*;
    }
    pub mod syscall {
        pub use crate::syscall::*;
    }
    pub mod testing {
        pub use crate::testing::*;
    }
    pub mod thread {
        pub use crate::thread::*;
    }
    pub mod trust {
        pub use crate::trust::*;
    }
    pub mod user {
        pub use crate::user::*;
    }
}

#[cfg(all(feature = "system_tests", not(test)))]
unsafe extern "Rust" {
    /// The hook the system-test image defines (`system_tests/src/main.rs`). A kernel built with the
    /// `system_tests` feature calls it where a `cargo test` kernel calls `test_main`, so the same boot
    /// runs the other crate's suite.
    ///
    /// **A foot gun, and why it fails loudly rather than quietly.** Cargo unifies features across one
    /// invocation, so `cargo build --workspace` for a bare-metal target would build the kernel *binary*
    /// with this feature on. Nothing defines this symbol in that binary, so the link fails. That is the
    /// wanted outcome: a kernel that would run a test suite instead of booting never gets built by
    /// accident. `script/drift` builds `system_tests` separately for exactly this reason.
    fn system_tests_main();
}

/// Runs the suite this image carries: the kernel's own unit tests under `cargo test -p kernel`, or
/// the system tests when `system_tests/` links this library with its feature on.
#[cfg(any(test, feature = "system_tests"))]
fn run_test_suite() {
    // **Every kernel image carries its trust root, a test image included** (milestone 609 (the
    // system tests leave the kernel crate)). `uefi_loader/build.rs` and `sealed_pair` tell which
    // archive a kernel belongs to by finding these digests in its bytes. The unit-test image never
    // enters the archive, so nothing in it calls `trust::verify`, and without this the linker drops
    // `TRUST_ROOT`: `uefi-test` then refused the kernel image as "NOT SEALED" against the archive
    // it was built with. Before the split, the system tests in the same image kept it.
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

/// The physical address of the Device Tree Blob, as handed to us in `x0`.
///
/// Stashed here so the tests can assert that the boot protocol actually delivered one,
/// which is the whole point of shipping a flat arm64 Image instead of an ELF.
/// See notes/boot-protocol.md.
pub static DTB: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

/// **The boot device tree, parsed**, or the parse error if this machine did not hand us one.
///
/// **This exists so the "is that pointer real" argument is made once.** Five places used to read
/// [`DTB`] (or take the same value as an argument) and hand it to
/// `device_tree_blob::DeviceTreeBlob::from_ptr` under a hand-written `// SAFETY:` comment, each
/// rewording the same two facts: the pointer is the one firmware put in `x0`/`a1` and
/// [`kernel_main`] stashed here before anything else ran, and it is physical, so it is named
/// through the direct map. That is one fact about this module's own static, and this module is the
/// only place it can be checked. Milestone 139 round 8, and DECISIONS §94's rule about a body
/// copied verbatim into every caller.
///
/// Returns `Err` rather than panicking, because two of the callers legitimately continue without a
/// tree: the early RISC-V console keeps its defaults, and `x86_64` stores a PVH `hvm_start_info`
/// pointer in [`DTB`] rather than an FDT, so the magic check inside `from_ptr` is what tells those
/// callers apart from a real failure. Callers that cannot continue keep their own `expect`.
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
// On riscv64, the boot tour below ends in `sched::exit()`, so the rest of `kernel_main` (the shared
// full boot) is deliberately unreachable there. Scoped to riscv so aarch64 keeps the lint.
// And the icount boot parks before the bench boot it implies, so `bench::run()` and everything after
// it is deliberately unreachable in that one configuration (milestone 78).
#[cfg_attr(
    any(target_arch = "riscv64", target_arch = "x86_64", feature = "icount"),
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

    // **The screen, before the first word** (milestone 243), on the two architectures whose boot
    // chain never lights one. The x86_64 arm below does the same thing a hundred lines down with
    // the same comment and a different discovery: there the loader measured a UEFI aperture, here
    // the kernel asks `fw_cfg` for a `ramfb` and supplies the memory itself. Both end at
    // `console::attach_screen`.
    //
    // It is deliberately here, before the banner: everything after this line is on a monitor and
    // everything before it is not, so the earliest possible line is the one worth buying. It needs
    // nothing brought up first beyond the console lock and the device tree pointer, and it must run
    // before `arch::mmu::init`, which it does by a wide margin. See `screen::attach`.
    //
    // The line itself is printed with the rest of the tour rather than here, for the same reason
    // the x86 arm gives: a line describing the screen has to be printed by a console that already
    // has one in order to appear on it.
    #[cfg(not(target_arch = "x86_64"))]
    let screen = screen::attach();

    // **The earliest line, and every architecture now has one** (milestone 268).
    //
    // riscv64 and x86_64 have opened with `nife on <arch>` since their ports were written, as the
    // first thing after the console comes up: it is the one line that says the console works and
    // says nothing else, which is exactly the claim wanted at first light on a board. aarch64 had
    // no such line at all, so `board_console`'s `Stage::Banner` (which matches `nife on `,
    // deliberately generic so that a healthy aarch64 or x86_64 board would not read as never having
    // booted) was **unreachable on aarch64** for as long as the recogniser has existed. That is
    // milestone 268's finding 3 again, one rung lower, and it was found the same way: by looking
    // for the marker rather than by anything failing.
    //
    // Levelling up, not down: the other two keep their own opening lines, which say more than this
    // one because those architectures know more at this point (long mode is on, Sv39 is on). This
    // is what aarch64 can honestly claim here, which is that the console is up and the MMU is not.
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

    // **The x86_64 boot is a self-contained tour and it halts at the end**, the same shape the
    // RISC-V boot took on its first day and for the same reason: the arch layer beneath the shared
    // path is not built yet, so there is nothing honest to fall through to. What it proves is
    // exactly the part that is real, one line per step, and it stops the moment it would have to
    // guess (milestone 161). See notes/x86-port.md.
    #[cfg(target_arch = "x86_64")]
    {
        // A live code address proves the far jump in boot.s landed in the high half and that we are
        // executing there rather than merely reaching the UART through the identity map (the boot
        // table maps both). If this reads 0xffffffff801xxxxx, long mode is on and the kernel is in
        // the high half.
        let pc = kernel_main as *const () as usize;

        // **The screen, before the first word** (milestone 243). On a commodity machine there is no
        // serial port, so unless this runs here the entire tour below is written to a device that
        // is not there. It is deliberately the first thing the x86 arm does: everything after it is
        // visible on a monitor, and everything before it is not, so the earliest possible line is
        // the one worth buying.
        //
        // It needs nothing brought up first. The screen's address arrives in the boot handoff and
        // the boot page tables `boot.s` installed already cover the low 4 GiB, which is where a
        // framebuffer aperture is; `arch::mmu::map_everything` later carries the mapping across to
        // the fine map (`memory::record_framebuffer`, called inside).
        let screen = arch::machine::attach_screen(boot_info_pointer);

        println!();
        println!(
            "{}x86_64 (long mode, ring 0, 4-level paging)",
            boot_ladder::BANNER,
        );
        println!("  cpu 0 booted: high-half kernel, .bss, and the 16550 console are up.");
        println!("  running at  : {pc:#018x}  (high half: the long-mode jump landed)");
        println!(
            "  boot info   : {boot_info_pointer:#018x}  (PVH hvm_start_info, not a device tree)"
        );
        // Said here rather than where it happens, because the line has to be printed by a console
        // that already exists in order to be on the screen it is describing. A machine with no
        // framebuffer says so and is not otherwise different, which is xenon: it has a serial port,
        // which is why milestone 87 chose it.
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

        // What machine is this. CPUID needs nothing to be brought up first, which makes x86 the
        // only one of the three where this can run before anything else.
        arch::isa::init(boot_info_pointer);
        arch::isa::print_summary();

        // The GDT, the TSS and the IDT. Until this line a fault is a triple fault and a silent
        // machine reset; after it, a fault prints. That is the whole value of the step, and it is
        // why it is the second thing the tour does rather than the tenth.
        arch::init();
        let caught = arch::exceptions::self_test();
        println!(
            "  traps       : idt installed; a breakpoint was caught and stepped over ({caught})"
        );

        // What the loader said: the PVH memory map and the ACPI root pointer. The x86 stand-in for
        // the device tree, and the only thing that reads the map so far; `memory::init` is a
        // device-tree parser, so the frame allocator cannot come up here until there is a discovery
        // seam between the two. See notes/x86-port/acpi-and-pci.md.
        let Some(info) = arch::machine::boot_info(boot_info_pointer) else {
            println!(
                "  memory      : no PVH boot info at {boot_info_pointer:#x}; nothing else can be found"
            );
            println!("nife x86_64: early boot cannot continue, halting.");
            arch::halt(arch::HaltReason::before_scheduler());
        };
        arch::machine::print_memory_map(&info);

        // ACPI: what x86 has instead of a device tree. The RSDP is scanned for rather than taken
        // from the handoff, because QEMU's PVH loader leaves that field zero (measured); the
        // checksum inside the parser is what separates a real hit from a coincidence.
        let acpi = arch::machine::read_acpi(info.rsdp);
        arch::machine::print_acpi_summary(&acpi);

        // Which CPUs does this machine have (milestone 161's SMP item)? Here, right beside the rest
        // of the ACPI walk, mirroring where the other two architectures read their own roster
        // (`smp::read_cpu_list`, right after the device tree that describes it is parsed). Seats
        // every core the MADT lists, boot core included, at the slot its own local APIC id names.
        smp::seat_cpus_from_acpi(&acpi.cpus[..acpi.cpu_count]);

        // COM1's interrupt line (milestone 176 (the x86_64 discovery seam's wide half: COM1's IRQ
        // and a CMOS RTC)), filling the same static `memory::uart_irq()` the
        // other two architectures fill from their device tree. **It is the legacy number, 4, and
        // not the GSI it resolves to**, because an intid on this architecture is a legacy IRQ:
        // `arch::irq::enable` resolves it through the MADT's overrides (`isa_irqs`, recorded above)
        // when it arms the line. Recording the GSI here was harmless while nothing armed it, since
        // the two agree on every machine without an override; since milestone 505 (an x86_64 input
        // driver that never lets the core idle) the input driver waits on it, and a machine that
        // overrides IRQ 4 would have had its GSI resolved a second time as though it were legacy.
        memory::record_uart_irq(user::UART_RX_INTID);

        // VT-d's register window, recorded now (before `arch::mmu::init()` a few lines down)
        // rather than where it is actually brought up. `mmu::map_everything` reads
        // `memory::vtd_region()` to decide what to map device-typed, and it has to know before it
        // runs; `arch::iommu::init` itself is called later, once the fine map it needs exists.
        //
        // **It moved above the PCI block on 2026-09-04** (milestone 256) and the order is now
        // load-bearing rather than incidental: `arch::mmu::memory_mapped_io_window` takes the windows this
        // kernel already knows about out of the hole it picks a BAR window from, and VT-d's
        // register file is one of them. Recorded afterwards, it would be a window the choice below
        // could not see.
        for d in acpi.dmar.units() {
            memory::record_vtd_region(d.register_base, d.register_size);
        }
        // AMD-Vi's, for the same reason and in the same place (lane `amd-vi`). Its IVHD carries no
        // size, so the window is the 16 KiB every register this kernel reads lives in.
        for d in acpi.ivrs.units() {
            memory::record_amd_vi_region(d.register_base, arch::amd_vi::REGISTER_SIZE);
        }

        // Turn the MCFG's ECAM window on and record it where kernel/src/pci.rs already knows to
        // look: `memory::pci_regions()`, the same static a device-tree machine fills from its
        // `pci-host-ecam-generic` node. No MCFG, no PCI at all, the same treatment the other two
        // architectures give a tree with no such node. The BAR window has no ACPI or AML source
        // and is derived from the machine instead (`arch::mmu::memory_mapped_io_window`, milestone 256).
        match acpi.ecam {
            // The MCFG's base is the configuration space of its FIRST bus, and `pci.rs` addresses a
            // function as `base + (bus << 20 | ...)` with an absolute bus number. Those agree only
            // when the first bus is zero, which every machine seen so far reports and no machine is
            // required to. Refused rather than adjusted: subtracting `lo << 20` names a base below
            // the window `mmu::map_everything` maps, so the arithmetic that looks like the fix
            // produces config reads into whatever is underneath it.
            Some((_, lo, _)) if lo != 0 => {
                println!(
                    "  pci         : skipped, the MCFG's first bus is {lo} and this port assumes 0"
                );
            }
            Some((base, lo, hi)) => {
                let buses = hi as u32 - lo as u32 + 1;
                let decode = arch::machine::enable_pcie_ecam(base, buses);
                let ecam = (base, buses as u64 * 0x10_0000);
                // **Where BARs may go, asked of the machine** (milestone 256). A failure here is a
                // panic and not a fallback: the constant this replaced was checked against one
                // emulated machine and was RAM on the first real one, so a kernel that carried on
                // with it would be placing device registers over memory the allocator owns. The
                // message is the whole diagnosis, because the machine that produces it has no
                // serial cable and a photograph of the screen is the entire transcript.
                let bar = match arch::mmu::memory_mapped_io_window(ecam) {
                    Ok(window) => window,
                    Err(why) => panic!("no PCI BAR window on this machine: {why}"),
                };
                memory::record_pci_regions(ecam, bar);
                println!(
                    "  pci         : ecam at {base:#x} (buses {lo}..={hi}), decode {}",
                    match decode {
                        arch::machine::EcamDecode::AlreadyOn => "was already on",
                        arch::machine::EcamDecode::Programmed => "programmed here",
                        arch::machine::EcamDecode::Unencodable =>
                            "LEFT AS FOUND: this bus count has no PCIEXBAR length field",
                    }
                );
                println!(
                    "                bar window {:#x}..{:#x}, from the firmware map{}",
                    bar.0,
                    bar.0 + bar.1,
                    match arch::machine::top_of_low_dram() {
                        Some(t) => {
                            let _ = t;
                            " and TOLUD, agreeing"
                        }
                        None => " alone (this host bridge reports no TOLUD)",
                    }
                );
                // **The topology, read from the bridges, and it must happen here** (milestone 320).
                // The boot tables still cover the low 4 GiB indiscriminately at this point, so
                // every bus the MCFG describes is readable without a single mapping; `mmu::init`
                // a hundred lines down then maps exactly the buses this found. Run after it, this
                // would read its way off the end of the window it is trying to size.
                //
                // It is in the x86 arm rather than beside `memory::init` for the same reason the
                // BAR census below is: both `virt` boards describe a flat root complex in their
                // device tree, QEMU puts nothing behind a bridge on either, and a walk there would
                // print bus 0 and stop. See `pci::survey`.
                pci::survey();
                // The census is here, in the x86 arm, because this is the only architecture whose
                // BARs may already be placed by something else when the kernel arrives. Both
                // `virt` boards boot with `-bios default` and every BAR is zero, so there would be
                // nothing to report. See `pci::bar_census` for why the second number is the one to
                // watch on real firmware.
                let (functions, outside) = pci::bar_census();
                println!(
                    "                {functions} function(s) on the bus, \
                     {outside} with a BAR this kernel can neither use nor adopt",
                );
            }
            // **No MCFG means no PCI, and deliberately no fallback to the legacy 0xcf8/0xcfc
            // configuration mechanism**, which this kernel could reach (`enable_pcie_ecam` uses it)
            // and which would enumerate bus 0 without any ACPI at all. Milestone 215 refused the
            // same shape one level down and the reason carries: the ports see only the first 256
            // bytes of a function's configuration space, so a machine that fell back would
            // enumerate a DIFFERENT set of capabilities from one that did not, with every extended
            // capability simply absent, and a driver that then failed would fail somewhere else
            // entirely. A machine that describes no MCFG is a machine this port does not run on
            // yet, and saying so here costs less than discovering it downstream.
            None => {
                println!("  pci         : skipped, no MCFG: no PCI at all (no legacy fallback)");
            }
        }

        // The local APIC, and then a real hardware interrupt. Until this point the only trap the
        // kernel has taken is one it raised itself with `int3`; a periodic timer proves the other
        // half, that an interrupt the CPU did not ask for arrives, is dispatched by vector, and is
        // acknowledged so that a second one can follow.
        if let Some(apic) = acpi.local_apic {
            // SAFETY: the address came from the machine's own ACPI MADT, and the identity map the
            // boot tables installed still covers it.
            unsafe { arch::irq::init_local_apic(apic) };
            println!(
                "  apic        : local apic {apic:#x} up, id {}, version {:#x}, 8259s masked",
                arch::irq::local_apic_id(),
                arch::irq::local_apic_version(),
            );

            arch::timer::init_frequency(boot_info_pointer);
            let calibration = arch::timer::calibration();
            println!(
                // The worst window is printed beside the chosen one on purpose. The chosen rate is
                // the *smallest* of several timed windows, because a window's error is one-sided,
                // so the gap between them is this boot's own evidence of how much the host was
                // descheduling the vCPU mid-calibration. A quiet host prints two numbers a
                // megahertz apart; the boot that measured 4330 MHz would have printed the gap that
                // said so. See arch::x86_64::timer's CALIBRATION_WINDOWS.
                "  clocks      : tsc {} MHz ({}, best of {} windows, worst {} MHz), apic timer {} MHz (measured against the PIT)",
                arch::timer::frequency() / 1_000_000,
                arch::timer::frequency_source(),
                calibration.windows().len(),
                calibration.worst() / 1_000_000,
                arch::timer::apic_timer_frequency() / 1_000_000,
            );

            // The TSC-vs-RTC measurement instrument, off unless the `tsc_probe` feature is on.
            // It halts when it is done, so it never reaches the rest of the tour, the same shape
            // `bench` and `icount` take. See notes/tsc-under-tcg.md.
            #[cfg(feature = "tsc_probe")]
            {
                arch::tsc_probe::probe();
                arch::halt(arch::HaltReason::measurement_boot());
            }

            arch::timer::init();
            arch::interrupts::enable();
            let start = arch::timer::now();
            while arch::timer::now().wrapping_sub(start) < arch::timer::frequency() / 5 {
                arch::wait_for_interrupt();
            }
            arch::interrupts::disable();
            // The tick count is the delivery evidence; `ROUTED_IRQS` used to be printed beside
            // it and no longer counts a timer tick, because nothing routes one to a driver.
            println!(
                "  timer       : {} ticks in ~0.2s at {} Hz ({} spurious)",
                arch::timer::ticks(),
                arch::timer::TICK_HZ,
                arch::exceptions::SPURIOUS_IRQS.load(core::sync::atomic::Ordering::Relaxed),
            );
        } else {
            println!("  apic        : skipped, the MADT did not say where the local apic is");
        }

        // The IO APIC, and with it a real *device* interrupt. The local APIC timer above proves the
        // CPU takes an interrupt it did not ask for; this proves a line outside the CPU reaches it,
        // which is a different claim and needs a different device.
        //
        // The PIT is that device, and it is the right one twice over: `timer.rs` already drives it
        // for calibration, and its legacy IRQ 0 is the line every PC rewires, arriving as global
        // system interrupt 2. Arming redirection entry 0 for "the timer" would arm the 8259 cascade
        // and produce no interrupts and no error, so routing this line is simultaneously the
        // easiest device to reach and the strongest test of the override table.
        if let (Some((io_id, io_addr, gsi_base)), true) =
            (acpi.io_apic, arch::irq::is_local_apic_ready())
        {
            // SAFETY: the address and global-interrupt base came from the machine's own ACPI MADT,
            // and the boot map covers the whole low 4 GiB this device sits in.
            unsafe { arch::irq::init_io_apic(io_addr as u64, gsi_base) };
            arch::irq::record_isa_routing(&acpi.isa_irqs);
            let chip_id = arch::irq::io_apic_id();
            println!(
                "  io apic     : id {chip_id} at {io_addr:#x} up, version {:#x}, {} redirection entries, gsi base {gsi_base}",
                arch::irq::io_apic_version(),
                arch::irq::io_apic_entries(),
            );
            if chip_id != io_id {
                // Worth a line rather than an average: the MADT and the chip disagreeing about
                // which IO APIC this is means one of the two is describing a different machine.
                println!(
                    "                the MADT calls it id {io_id}; the register above is the chip's own answer"
                );
            }

            // The local APIC timer is masked for the window below so the count is the PIT's alone.
            // Both would be delivered correctly; one number that means one thing is worth more.
            arch::irq::mask_timer();

            let pit = arch::irq::isa_routing(arch::timer::PIT_IRQ);
            let hz = arch::timer::start_pit_ticking(arch::timer::TICK_HZ);
            arch::irq::enable(arch::timer::PIT_IRQ);
            // Read after `enable` rather than before, which is what makes the `expect` honest
            // rather than hopeful: `enable` has just routed this same GSI and panics with a named
            // reason if the IO APIC does not own it, so an index exists by the time this runs. It
            // also makes the transcript's field the vector that was programmed instead of one
            // computed alongside it.
            let vector = arch::irq::gsi_vector(pit.gsi)
                .expect("enable routed this gsi a line ago, so the io apic owns it");

            arch::exceptions::enable_external();
            arch::interrupts::enable();
            let start = arch::timer::now();
            while arch::timer::now().wrapping_sub(start) < arch::timer::frequency() / 5 {
                arch::wait_for_interrupt();
            }
            arch::interrupts::disable();
            arch::irq::mask_gsi(pit.gsi);

            println!(
                "  device irq  : pit irq {} -> gsi {} on vector {vector:#x}: {} interrupts in ~0.2s at {hz} Hz",
                arch::timer::PIT_IRQ,
                pit.gsi,
                arch::exceptions::DEVICE_IRQS.load(core::sync::atomic::Ordering::Relaxed),
            );
        } else {
            println!("  io apic     : skipped, the MADT did not describe one");
        }

        // The frame allocator, from the PVH memory map. This is the seam: `memory::init` is a
        // device-tree front end and there is no tree here, so the x86 side assembles the same two
        // slices (RAM, and what is already spoken for) and hands them to the half that does not
        // care where they came from.
        stack::init();
        // Paint the boot stack's unused region for the high-water instrument (milestone 84), before
        // anything else can push a frame into it. Both other boots do this here, in the same breath
        // as `stack::init`; without it the instrument reads whatever the trampoline left and reports
        // 100% of a 64 KiB stack in use, which is what the first x86 test run said.
        #[cfg(any(test, feature = "system_tests"))]
        stack::paint_boot_stack();
        let regions = arch::machine::bring_up_memory(&info);
        let first = memory::alloc().expect("no frame from the allocator");
        println!(
            "  frames      : allocator up over {regions} ram region(s) (first frame {:#x})",
            first.addr(),
        );
        memory::print_summary();

        // Replace the boot map (4 GiB identity, everything present/writable/executable) with
        // fine-grained W^X tables and switch `CR3` to them. We keep running, and keep printing,
        // across the switch, which is what proves the fine map covers this code and this stack.
        // The identity map is gone afterwards, and with it the last alias of physical memory in the
        // half ring 3 will get. See notes/x86-port/interrupts-and-the-fine-map.md.
        arch::mmu::init();
        arch::mmu::print_summary();
        println!(
            "  image       : text {:#x}..{:#x}, stack {:#x}..{:#x}",
            arch::mmu::text_start(),
            arch::mmu::text_end(),
            arch::mmu::stack_bottom(),
            // `arch::mmu`'s rather than this file's `stack_top`, which is `#[cfg(not(test))]`
            // because it belongs to the aarch64 tour. Same linker symbol, same answer, and this
            // one exists in a test build, which is the boot this tour now has to survive.
            arch::mmu::stack_top(),
        );

        // Milestone 162: RDSEED needs no ring 3, no capability, and nothing this port has not
        // already built, so it is provable here even though the entropy service itself cannot run
        // yet (no userspace to spawn it into). `draw_random_seed` already checked CPUID leaf 7
        // EBX.18 before ever executing the instruction.
        match arch::isa::draw_random_seed() {
            Some(v) => {
                println!("  entropy     : rdseed supported (cpuid leaf 7 ebx.18), drew {v:#018x}");
            }
            None if arch::isa::get().has_random_seed_instruction() => {
                println!("  entropy     : rdseed supported but stayed dry across every retry");
            }
            None => println!("  entropy     : rdseed not supported (cpuid leaf 7 ebx.18 clear)"),
        }

        // **Unhalted core cycles** (milestone 309), the x86_64 half of milestone 74's measurement
        // side. Here rather than beside `isa::print_summary` at the top of this tour, which is
        // where the riscv64 boot puts its own `pmu::init`: this one's did-it-count check spins
        // against the TSC, so it has to come after `timer::init_frequency` above. Beside `entropy`
        // because the two are the same shape, a CPUID-gated probe that reports what it found and
        // refuses to guess.
        //
        // Silent and harmless where there is no performance monitoring, which is every QEMU boot
        // unless its `pmu` property is on: the counter is then `None` forever and no MSR is ever
        // read. `arch::x86_64::pmu` is emphatic about why this is not `rdtsc`.
        arch::pmu::init();
        arch::pmu::print_summary();

        // The per-CPU interrupt stacks go live here, for the reason both other boots arm them right
        // after their own `mmu::init`: their guard pages are holes in the map that was just
        // installed, and before that they are covered by the coarse boot map and are not holes yet.
        interrupt_stack::init();

        // VT-d (milestone 161, roadmap item 6), if the DMAR named a DRHD. The same position the
        // SMMUv3 and the RISC-V IOMMU come up in on the other two boots: after the fine page
        // tables (a DRHD's register file is device-typed MMIO, reachable only through the map
        // `mmu::init` just installed) and before anything that could attach a device. The
        // kernel-resident NVMe driver (`kernel/src/non_volatile_memory_express.rs`, decisions §86) is the first PCI
        // device this architecture confines through it, on the same terms as the aarch64/riscv64
        // legs' SMMUv3/riscv-iommu confinement; a boot with no NVMe controller attached (no
        // NIFE_NVME on this leg) still proves the driver stands up against real hardware: root
        // table installed, translation enabled, read back from the register the hardware itself
        // reports status through.
        if !acpi.dmar.units().is_empty() {
            // **Every unit the DMAR names, each translating the devices it owns** (milestone 594 (every VT-d unit translates its own devices),
            // provisional number). `init` polls GSTS.RTPS then GSTS.TES itself on each unit and
            // panics rather than returning if either write never takes, so a unit reported up
            // below is the hardware's own status register, not an assumption that the write
            // succeeded. It prints one `vt-d` line per unit, including any it refused and why.
            arch::iommu::init(&acpi.dmar);
        } else if !acpi.ivrs.units().is_empty() {
            // **An AMD machine** (lane `amd-vi`): no DMAR, an IVRS instead. Every unit it names,
            // each enabled over an all-blocked device table; one `amd-vi` line per unit, read back
            // from the unit's own status register, as the VT-d line is.
            println!("  vt-d        : skipped, no DMAR (this machine's IOMMU is AMD-Vi)");
            arch::amd_vi::init(&acpi.ivrs);
        } else {
            // Loud, because the alternative is a boot that reads like every other one while
            // nothing stands between a device and all of memory.
            println!("  vt-d        : skipped, no DMAR");
            println!(
                "  iommu       : NONE: this machine's ACPI names neither a DMAR (VT-d) nor an IVRS \
                 (AMD-Vi), so no device's DMA is confined; every driver below runs unconfined"
            );
        }

        // **The scheduler** (milestone 161, roadmap item 4). Everything below this line is a
        // process rather than a program, which is the distinction item 3 stopped at.
        //
        // Interrupts are off here (the two measurement windows above turned them back off), which
        // is what `sched::init` needs: a timer tick landing mid-init would run the deferred
        // `schedule()` before the idle thread is registered and hit "nothing runnable and no idle
        // thread". The RISC-V boot masks them explicitly at this point for the same reason.
        sched::init();

        // Re-arm the local APIC timer and let it run for good. `timer::init` rewrites the LVT
        // unmasked, which is what undoes the `mask_timer` the PIT window needed so its count would
        // be the PIT's alone. From here this timer is what preempts.
        arch::timer::init();
        arch::interrupts::enable();
        println!(
            "  scheduler   : up on 1 cpu, preempting at {} Hz (idle thread registered)",
            arch::timer::TICK_HZ,
        );

        // **SMP** (milestone 161's SMP item): INIT-SIPI-SIPI through the local APIC, same call and
        // same position as the other two boots. What it does *first*, whether or not any secondary
        // starts, is mark the boot core in `ONLINE_MASK`, and that is not optional: everything that
        // broadcasts (`online_cpus`, `nth_online`, the shootdown loops) reads that mask, so a kernel
        // that never called this has an empty online set while `online_count` says one, and the two
        // disagreeing is what `the_online_set_is_the_mask_and_the_sampler_stays_inside_it` caught on
        // the first x86 test run.
        smp::bring_up_secondaries();

        // **The ladder** (milestone 268): say what this machine is, then prove the kernel works on
        // it, then hand over. The third rung is what this architecture cannot reach yet: there is
        // no entry point that hands x86_64 to a `swish` prompt until DECISIONS §149 says how a
        // shell gets a console here and milestone 182 builds it, so this boot still ends by
        // halting a few hundred lines below. Everything under that rung runs before userspace
        // exists and waits on neither, which is why it lands now.
        //
        // The `bench` exclusion is on the functions rather than here; see the riscv64 site.
        #[cfg(not(any(test, feature = "system_tests", feature = "bench")))]
        {
            print_machine_description(boot_info_pointer);
            self_test::run();
        }

        // The benchmark boot (milestone 21, `script/bench`; DECISIONS §121's amendment measuring
        // the TSS I/O-bitmap switch cost): run the microbenchmarks and halt, instead of the rest of
        // the tour. Everything `bench::run()` needs is up by this line (the scheduler, `sched::spawn`,
        // the timer tick), and it diverges, so the kernel thread proof, the userspace demo and the
        // `#[cfg(test)]` suite below are untouched by it. The same shape the other two architectures
        // already use (see `icount`/`bench` above in this function's aarch64 half).
        #[cfg(feature = "bench")]
        bench::run();

        // A kernel thread, which is the cheapest proof that `kmem`, `untyped` and the context
        // switch all work on this architecture: its stack came from the kernel's own budget and
        // running it at all is one `switch_to` out and one back.
        {
            use core::sync::atomic::AtomicU64;
            static SAW: AtomicU64 = AtomicU64::new(0);
            let captured = 0x1610_0004u64;
            match sched::spawn(move || SAW.store(captured, core::sync::atomic::Ordering::SeqCst)) {
                Some(_) => {
                    // **Yield until it runs, with a bound, rather than yielding a fixed number
                    // of times and assuming that was enough.** The fixed count was 8, which held
                    // for a year under emulation and reported `0x0  FAILED` on xenon's second
                    // boot (2026-09-17): eight yields on four real 2.7 GHz cores is a far smaller
                    // window than eight under QEMU, and the thread had simply not been picked up
                    // yet. The count is printed because it is the diagnostic that tells a late
                    // thread apart from one that never ran: a thread that never runs still
                    // exhausts the bound and still reports FAILED, so this waits out a race
                    // without hiding a hang.
                    let mut spins = 0;
                    while SAW.load(core::sync::atomic::Ordering::SeqCst) != captured
                        && spins < 10_000
                    {
                        sched::yield_now();
                        spins += 1;
                    }
                    let saw = SAW.load(core::sync::atomic::Ordering::SeqCst);
                    println!(
                        "  kernel task : a spawned thread ran and carried its captured state ({saw:#x}) after {spins} yield(s){}",
                        if saw == captured { "" } else { "  FAILED" },
                    );
                }
                None => println!("  kernel task : FAILED: could not spawn a kernel thread"),
            }
        }

        // **Userspace**, which is the step this whole tour has been building toward: every line
        // above it is about the kernel talking to the machine, and this one is about the kernel
        // running a program and refusing it. Two hand-assembled children, each built out of its own
        // untyped region the way a real spawn builds one; see `user::x86_userspace_demo`.
        match user::x86_userspace_demo() {
            Ok(report) => {
                println!(
                    "  userspace   : a process built from untyped ran at cpl 3 and sent {:#x} on a granted cap",
                    report.reported,
                );
                println!(
                    "                thread {} died at pc {:#x} on addr {:#x}, delivered to its supervisor",
                    report.faulted_tid, report.fault_pc, report.fault_addr,
                );
                println!(
                    "                two children cost {} frames the first round and {} the second{}",
                    report.first_round_frames,
                    report.second_round_frames,
                    if report.second_round_frames == 0 {
                        " (steady state)"
                    } else {
                        "  (LEAKED)"
                    },
                );
            }
            Err(why) => println!("  userspace   : FAILED: {why}"),
        }

        // **The initrd** (milestone 161, item 4's hand-off). Not a demo but a report, and the two
        // facts it carries are the ones every `cfg(initrd)` test depends on: that the PVH module
        // list reached the kernel at all, and that what it points at parses as an archive. A boot
        // where the first is true and the second is not looks identical to a boot with no initrd
        // from any test's point of view, which is why they are printed apart.
        match user::initrd() {
            None => println!("  initrd      : none (no -initrd passed to QEMU)"),
            Some(image) => match nifefs::Fs::parse(image) {
                Ok(fs) => println!(
                    "  initrd      : {} bytes at {:#x}, {} programs, from the PVH module list",
                    image.len(),
                    memory::initrd_region().unwrap().0,
                    fs.len(),
                ),
                Err(e) => println!(
                    "  initrd      : {} bytes at {:#x}, but it does not parse: {e:?}",
                    image.len(),
                    memory::initrd_region().unwrap().0,
                ),
            },
        }

        // **The boot file** (milestone 198 (a package manager, and the trivial install that makes a
        // second customer possible)'s rung 2a), printed for the same reason the initrd is: it is
        // the one fact that says whether this system can install itself, and a boot that cannot is
        // not broken. `uefi_loader` reads its own file back off the volume it was started from and
        // hands it over as the second PVH module; QEMU's `-kernel` path passes no such thing, so
        // "none" is the normal answer on most of this tree's boots.
        match memory::boot_file_region() {
            None => println!("  boot file   : none (this boot did not come from a file)"),
            Some((at, size)) => {
                println!("  boot file   : {size} bytes at {at:#x}, from the PVH module list");
            }
        }

        // A test build runs the kernel suite right here and exits via semihosting, instead of the
        // rest of the tour. Everything the tests need is now up: the frame allocator, the fine page
        // tables, the scheduler and its idle thread, the timer and interrupts. The x86 equivalent of
        // the `#[cfg(test)] test_main()` both other boots reach.
        #[cfg(any(test, feature = "system_tests"))]
        {
            run_test_suite();
            arch::halt(arch::HaltReason::test_build());
        }

        // **The tour ends and the soak begins** (milestone 219), before the halting line rather
        // than after it: a boot that says it is halting and then does not would be the tool's
        // problem and the reader's.
        // **The sweep, when this build asked for one** (milestone 168). Before the soak arm and
        // the halt for the same reason the soak sits before the halt: the tour has finished, so the
        // whole boot is still evidence, and this run ends by halting rather than by beating
        // forever. The two features are alternatives rather than a stack; `script/job-mix` builds
        // only this one.
        #[cfg(feature = "job_mix")]
        job_mix::run();
        #[cfg(feature = "soak_test")]
        soak::run();
        // **Fatal risk 6's bench boot, when this build asked for one** (milestone 261). The same
        // position and the same reason as the two above: the tour is evidence, and this replaces
        // the hand-over. It WRITES to the NVMe disk; see kernel/src/disk_throughput.rs.
        #[cfg(feature = "disk_throughput")]
        disk_throughput::run();
        // **Milestone 494 (a driver for the network card a PC actually has)'s bench boot**, in the
        // same position for the same reason; see kernel/src/network_bench.rs.
        #[cfg(feature = "network_bench")]
        network_bench::run();
        // **Nothing halts by default** (milestone 268), on this architecture as on the other two:
        // the boot hands the machine to the progenitor, loaded from the archive and measured, and
        // the boot thread parks in a preemptible `wfi` loop so it gets scheduled. That is milestone
        // 182's entry point. What it cannot reach yet is a prompt, because a shell needs a console
        // and this one is port I/O; `x86_hand_over` says so in the transcript rather than leaving
        // a silent machine to be read as a hang.
        #[cfg(not(any(
            feature = "soak_test",
            feature = "job_mix",
            feature = "disk_throughput",
            feature = "network_bench"
        )))]
        {
            // **The install offer** (milestone 198 (a package manager, and the trivial install that
            // makes a second customer possible), rung 2a), and it has to be here rather than after
            // the handoff: it asks its question on the console, and the progenitor gives the
            // console's UART to a userspace input driver. It is a no-op on every boot that did not
            // come from a file the loader could read back, which is every boot but a UEFI one.
            user::install_service::offer();
            x86_hand_over();
            // **The boot thread leaves the scheduler rather than halting in it** (milestone 628
            // (provisional), measured 2026-10-03). It used to `arch::halt()` here, which is `hlt` in
            // a loop on a thread that is still runnable: every time round-robin reached it, the core
            // stopped until the next tick, up to 10 ms, with the shell, the console and the input
            // driver all ready behind it. x86_64's input driver polled and yielded then (no COM1
            // interrupt reached userspace until milestone 505), so the rotation reached it constantly, and the swish-check leg
            // paid about three seconds a line in ticks: 488 s for 118 lines under TCG on patagonia,
            // 126 s after this line, and the second boot's eight lines went from 23.8 s to 0.9 s.
            // `exit` marks it Finished and the idle thread, which halts only when nothing else can
            // run, takes the core. The boot thread owns no kernel stack (it runs on `boot.s`'s), so
            // reaping it frees nothing. notes/benchmarks/swish-check-x86-leg.md has the numbers.
            sched::exit();
        }
    }

    // The RISC-V boot is a self-contained tour, right here in this block, and it halts at the end
    // rather than falling through to the shared boot path below. From OpenSBI's S-mode handoff it
    // brings up its own arch (traps, the Sv39 MMU, the SBI timer) and then demonstrates the whole
    // capability core on RISC-V: the scheduler and preemption, U-mode programs and syscalls,
    // capability invocation, userspace progenitor building a child out of the initrd, and a userspace
    // driver servicing a device interrupt through the PLIC. It stops before the shared full boot
    // (userspace progenitor as the boot process, the shell, the virtio service), which is still
    // aarch64-shaped; making those portable is what would let RISC-V join the shared path instead of
    // halting here. See notes/riscv-port.md.
    #[cfg(target_arch = "riscv64")]
    {
        // A live code address proves we jumped to the high-half alias in boot.s and are executing
        // there, not merely reaching the UART through the identity map (both are mapped by the boot
        // table). If this reads 0xffffffc0_8020_xxxx, Sv39 is on and the kernel is in the high half.
        let pc = kernel_main as *const () as usize;
        println!();
        println!("{}RISC-V (rv64, S-mode, Sv39)", boot_ladder::BANNER);
        println!("  hart 0 booted: high-half kernel, .bss, and the NS16550 console are up.");
        println!("  running at  : {pc:#018x}  (high half: Sv39 paging is on)");
        println!("  device tree : {boot_info_pointer:#018x}");
        screen::print_summary(&screen);

        // Traps: install stvec and prove the round-trip by taking a breakpoint and returning.
        arch::exceptions::init();
        let caught = arch::exceptions::self_test();
        println!("  traps       : stvec set; a breakpoint was caught and stepped over ({caught})");

        // Timer + interrupts: arm the SBI timer, unmask interrupts, and let the S-mode timer
        // interrupt fire for ~0.2 s. A nonzero tick count proves the whole interrupt path (SBI
        // set_timer, sie.STIE, sstatus.SIE, the trap vector routing scause=timer to timer::tick).
        //
        // The counter's rate comes out of the device tree first (milestone 100). It used to be a
        // 10 MHz constant, which is QEMU's; RISC-V has no CNTFRQ_EL0 to read, so the tree is the
        // architected source and this is the earliest point anything needs the number.
        arch::timer::init_frequency(boot_info_pointer);
        arch::timer::init();
        arch::interrupts::enable();
        let start = arch::timer::now();
        while arch::timer::now().wrapping_sub(start) < arch::timer::frequency() / 5 {
            arch::wait_for_interrupt();
        }
        println!(
            "  timer       : {} ticks in ~0.2s (SBI timer + S-mode interrupt at {} Hz)",
            arch::timer::ticks(),
            arch::timer::TICK_HZ,
        );

        // Memory: parse the device tree OpenSBI handed us (via its high-half alias) for RAM, and
        // bring up the frame allocator. Portable code, reached through the real phys_to_virt now.
        stack::init();
        // Paint the boot stack's unused region for the high-water report (milestone 84), before
        // memory::init and everything after it can push frames into it.
        #[cfg(any(test, feature = "system_tests"))]
        stack::paint_boot_stack();
        memory::init();
        let f = memory::alloc().expect("no frame from the allocator");
        println!(
            "  memory      : device tree parsed, frame allocator up (first frame {:#x})",
            f.addr(),
        );

        // What machine is this (milestone 60). Before the MMU, because this is where a machine that
        // cannot run us says so and stops, and building fine-grained tables the hardware will not
        // walk is a worse place to find out. It reads the same device tree `memory::init` just
        // parsed, then asks OpenSBI what it implements. The summary prints after paging, because
        // one of the numbers in it is measured by `mmu::init`.
        arch::isa::init(boot_info_pointer);

        // Which harts does this machine have (milestone 100)? Here, while the device tree is still
        // reachable through the boot map, rather than at bring-up time after `mmu::init` has
        // replaced it. The list used to be `0..cpu::MAX_CPUS`, a constant.
        smp::read_cpu_list();

        // And how does the PLIC number their S-mode contexts? Read here, in the same window and
        // before anything touches a context: the `2*hart + 1` formula this replaces is QEMU's
        // layout, and the JH7110's disabled S7 shifts every context down one (arch::irq,
        // notes/visionfive2.md). On QEMU the tree reproduces the formula exactly.
        arch::irq::init_contexts(boot_info_pointer);

        // Replace the coarse RWX boot table with fine-grained W^X Sv39 kernel tables. We keep
        // running (and printing) across the satp switch, which proves the fine map covers this code.
        arch::mmu::init();
        println!(
            "  paging      : fine-grained W^X Sv39 tables installed, satp switched (paging on: {})",
            arch::mmu::is_enabled(),
        );

        // The per-CPU interrupt stacks go live here, for the reason the aarch64 boot arms them
        // right after its own `mmu::init`: their guard pages are holes in the map that was just
        // installed. This hart has had interrupts on since the timer step above, so every trap
        // before this one ran on the stack it interrupted, exactly as it used to.
        interrupt_stack::init();

        // And say what the machine is. Here rather than in the tour below, so the test, shell and
        // bench boots report it too: it is the line whoever brings up a board reads first.
        arch::isa::print_summary();

        // Ask firmware for a cycle counter (milestone 74). After `isa::init`, because it reads the
        // SBI extension set that probe filled in, and beside the summary because whether this
        // machine has one is a fact about the machine and belongs in the same paragraph. Silent and
        // harmless where the PMU extension is absent: the counter is then simply `None` forever.
        arch::pmu::init();
        arch::pmu::print_summary();

        // The scheduler comes up before the tour (and, in a test build, before the tests): both
        // need threads to switch between, and preemption needs somewhere to go. aarch64 brings it
        // up in its main boot flow for the same reason.
        //
        // Interrupts have been on since the timer step above, so mask them across `sched::init`: a
        // timer tick landing mid-init would run the deferred `schedule()` before the idle thread is
        // registered, and hit "nothing runnable and no idle thread" (sched.rs). aarch64 gets this
        // ordering for free by bringing the scheduler up before it ever enables interrupts.
        let irqs = arch::interrupts::disable();
        sched::init();
        arch::interrupts::restore(irqs);

        // SMP: bring the other harts online (parity workstream A). First arm this (boot) hart's own
        // software-interrupt source, so a secondary can hand work back to it; then start the
        // secondaries. Each starts via SBI HSM at secondary_boot, adopts the fine kernel map, sets
        // its own trap vector and per-CPU state (its own per-hart trap stash, A1), arms its timer and
        // its IPI source, and becomes a scheduler participant. Before the tests (and the tour), so the
        // SMP tests find the cores online, mirroring aarch64's `bring_up_secondaries` then `test_main`.
        arch::irq::init_this_cpu();
        smp::bring_up_secondaries();

        // The RISC-V IOMMU (milestone 16b), if QEMU presents one (`-device riscv-iommu-pci`). It is
        // a PCI function, so the kernel enumerates it, places its BAR, and brings it up here, before
        // any path (test, shell, tour) confines a virtio-pci device. Absent, this is a no-op and the
        // kernel runs exactly as before. See kernel/src/iommu.rs, notes/iommu.md.
        pci::init_iommu();

        // **The ladder** (milestone 268): say what this machine is, then prove the kernel works on
        // it, then hand over. Here, and not further down the tour, because every build that is not
        // a measurement build passes this line: the `shell` boot below hands to the progenitor from
        // here, and a board's boot is read from exactly these lines.
        //
        // The `bench` and `icount` boots park before userspace by design and are excluded on the
        // functions themselves rather than here (see `print_machine_description`'s own doc and
        // `kernel/Cargo.toml`'s feature comments: a board has no command line, so a measurement
        // build is compile-time or it is nothing). A `test` boot exits through semihosting a few
        // lines below and is excluded the same way.
        #[cfg(not(any(test, feature = "system_tests", feature = "bench")))]
        {
            print_machine_description(boot_info_pointer);
            self_test::run();
        }

        // The instruction-count boot (milestone 78, `script/icount`) diverges here, on the ISA whose
        // claim it was written for: SBI's `set_timer` is write-only, so this is the only place in
        // the tree that proves the firmware was armed with the deadline the kernel recorded. Before
        // the bench boot, whose feature it implies; see the aarch64 site for why.
        #[cfg(feature = "icount")]
        icount::run();

        // A bench build runs the primitive suite here and parks, instead of the tour. It needs the
        // `os_primitives_benchmarker` and `coremark` programs in the initrd (cargo xtask initrd-riscv packs them). The
        // RISC-V equivalent of the aarch64 boot's `#[cfg(feature = "bench")] bench::run()`.
        #[cfg(feature = "bench")]
        bench::run();

        // A `shell` build hands the machine to userspace progenitor here and parks, instead of the tour.
        // The kernel loads `system_initializer` from the initrd, grants it the NS16550 and the UART interrupt,
        // and it builds the console server, input driver, and shell out of its own budget; the shell
        // is interactive over the serial. This is the RISC-V equivalent of the aarch64 `shell`
        // path (named `initboot` there until milestone 296 found the two features identical).
        // Needs the shell programs in the initrd (cargo xtask initrd-riscv packs them).
        #[cfg(feature = "shell")]
        {
            riscv_hand_over();
            // The boot thread's work is done, and it leaves the scheduler rather than parking on its
            // run queue; the progenitor and its children (console/input/shell) run from here on, and
            // the idle thread takes the hart when none of them can. Milestone 720 (provisional) has
            // why a parked boot thread costs a tick each time round robin reaches it; it is the
            // reason `arch::halt` takes a `HaltReason` this build cannot make.
            sched::exit();
        }

        // A test build runs the kernel suite right here and exits via semihosting, instead of the
        // demonstration tour below. Everything the tests need is now up: memory and the frame
        // allocator, the Sv39 paging, the scheduler and its idle thread, the timer, interrupts, and
        // the other harts. The RISC-V equivalent of the `#[cfg(test)] test_main()` on the aarch64 boot.
        #[cfg(any(test, feature = "system_tests"))]
        {
            // The PLIC too: the parity-C disk tests route a device interrupt, and `plic::enable`
            // through a never-initialized PLIC is a store through a zero base (a fault this test
            // path found the hard way; the shell and tour paths already did this). SEIE as well:
            // enabling a source at the PLIC delivers nothing while supervisor external interrupts
            // are masked in `sie`, and that bit is otherwise only set by the UART driver paths.
            if let Some((plic_phys, _)) = memory::plic_region() {
                // SAFETY: the PLIC is device-mapped in the direct map; the context is the boot
                // hart's S context (derived, not hardcoded: OpenSBI's hart lottery).
                unsafe {
                    drivers::plic::init(
                        arch::mmu::phys_to_virt(plic_phys) as usize,
                        arch::irq::boot_s_context(),
                    );
                };
                arch::exceptions::enable_external();
            }
            run_test_suite();
            arch::halt(arch::HaltReason::test_build());
        }

        // **The kernel mapping check and the context-switch check both moved into
        // `self_test::run`** (milestone 268, item 2).
        //
        // They were here, unnamed, as `kmap test` and `scheduler   : 2 of 2 ...`, and they were two
        // of the three self-tests this tree already had. Collecting them is the milestone's second
        // item: the same two checks now run on aarch64 and x86_64 as well, report through one
        // verdict line that CI reads, and are still measured the way this arm measured them (the
        // address computed from the machine's own RAM rather than a constant, the wait bounded by
        // the clock rather than by a yield count, both learned on the VisionFive 2).
        //
        // **Nothing was deleted to make three arms agree.** The claims went up into the ladder and
        // gained two architectures; that is what levelling up looks like, and it is the opposite of
        // the trap this milestone's block warns about.

        // User address spaces (the single-satp model): build a process address space, switch satp
        // to it, and keep running. That only survives if share_kernel_half copied the kernel high
        // half into the process root (otherwise this kernel code, at a high VA, unmaps itself on the
        // csrw satp). Then map and translate a user page in the live process space.
        {
            use paging::Flags;
            let aspace = user::AddressSpace::new(1).expect("no process aspace");
            let user_va = 0x40_0000u64;
            // Uninstalled again before the space drops: `while_installed` holds both halves.
            let mapped = aspace.while_installed(|| {
                let frame = memory::alloc().expect("no user frame").addr();
                arch::mmu::map_current_user_page_frame(user_va, frame, Flags::user_data(), || {
                    memory::alloc().map(|f| f.addr())
                })
                .expect("user map failed");
                arch::mmu::translate_user(user_va)
            });
            println!(
                "  user address space : process satp activated (kernel half shared), user {user_va:#x} -> {mapped:x?}",
            );
        }

        // The capstone: run a real program at U-mode. The loader builds an address space from the
        // `outlaw` ELF's segments and drops to U-mode via enter_user's sret. The program yields
        // (round-tripping U-mode -> trap -> dispatch -> yield -> sret -> U-mode) twice, then exits.
        //
        // **This line used to print a syscall count, and the count proved nothing.** It read a
        // global counter before and after four yields of its own, but the program is placed on
        // whichever core the scheduler picks and had not run yet when the second read happened:
        // every captured riscv64 boot, under QEMU and on radon, printed "made 0 syscalls". The
        // counter itself was a `fetch_add` on one cache line from every syscall on every core,
        // which is what made the cheapest syscall cost more when more cores were busy
        // (notes/job-mix/null-syscall-under-load.md, 2026-10-04). It is gone rather than
        // per-core because this was its only reader. Proving the path for real means waiting for
        // the program's exit, which is a tour change and not this one.
        //
        // This step used to run a hand-assembled RISC-V blob through a one-page raw loader. It runs
        // a compiled ELF out of the initrd now (milestone 19's user-test port), so it needs one, and
        // says so rather than quietly proving nothing when there is none.
        {
            match user::program("outlaw") {
                Some(image) => {
                    sched::spawn(move || {
                        user::run(
                            image,
                            user::Spawn {
                                arg0: user::OUTLAW_ROUND_TRIP,
                                arg1: 0,
                                arg2: 0,
                                grants: &[],
                                maps: &[],
                            },
                        )
                    });
                    for _ in 0..4 {
                        sched::yield_now();
                    }
                    println!(
                        "  userspace   : a program was started at U-mode (yield/yield/exit via ecall)",
                    );
                }
                None => println!("  userspace   : skipped (no 'outlaw' program in the initrd)"),
            }
        }

        // The tour-stage breadcrumb (first-silicon diagnostics, 2026-08-15): every dump_threads
        // header prints the last stage reached, so a bench log whose serial lines went missing
        // (boots 7 through 9, notes/visionfive2.md fifth stop) still says how far the boot
        // thread got, re-stated every dump. The table, so a stage number reads without the code:
        //
        //   3 = the outlaw step finished        7 = the UART-driver step finished
        //   4 = the user-ELF step was entered   8 = the virtio probe finished
        //   5 = the user-ELF step returned      9 = the PCIe probe finished
        //   6 = the preemption step finished   10 = the hardware-entropy step finished
        //                                      11 = the banner printed; the tour is over
        //
        // **10 is not the end, and reading it as one is the mistake this table used to invite.**
        // It said `10 = the final banner printed; halting` and stopped there, which was true until
        // milestone 159 put the hardware-entropy step after what had been the last one and moved
        // the meaning down a row. 11 is the number that means finished, and it is the one the hang
        // watcher keys on (`user.rs`, `boot_stage() >= 11`), so a board log reporting 10 is a boot
        // that got as far as the entropy step and then stopped, not a boot that completed.
        sched::note_boot_stage(3);

        // Running a real compiled ELF at U-mode, on the boots that carry one to run.
        if let Some(initrd) = user::initrd() {
            // Entered for either shape of initrd, which it was not before milestone 295: the
            // breadcrumb used to fire only on the archive arm, so a bare-ELF boot that died here
            // reported the stage before it and read as having died one step earlier than it did.
            sched::note_boot_stage(4);

            // **An archive initrd has no demonstration of its own here any more** (milestone 295,
            // calef's ruling of 2026-09-14). It used to have the best one on this architecture:
            // the kernel loaded `builder` out of the archive, granted it a budget and a report
            // endpoint and nothing else, and `builder` parsed `least_authority_demo` out of the
            // same archive **in userspace**, built it as a child from its own budget and started
            // it. The kernel never touched the child's bytes. That was milestone 20's proof that
            // userspace, not the kernel, composes a process, and the `init/build` line was it
            // announcing itself.
            //
            // **Milestone 268's item 4 is why it could go.** Nothing halts by default any more:
            // this same boot now ends in `riscv_hand_over`, where the progenitor composes the
            // console server, the line discipline, the input driver and `swish` out of its own
            // budget through the same granular verbs. That is the identical claim at a larger
            // scale, on the build `script/board-image` writes to a card, so the step below it was
            // making it twice.
            //
            // **What the retirement drops is the minimality half, and it is dropped knowingly.**
            // `builder` composed a process from **exactly two** capabilities; the progenitor is
            // granted the NS16550 and the UART's interrupt line as well, because it is building a
            // system rather than demonstrating a floor. See
            // design/roadmap/0295-retire-the-builder-program.md for where that claim went.
            //
            // **And the measured-boot refusal moved rather than went.** The archive used to be
            // checked against this kernel's trust root here (`trust::require("builder", ...)` and
            // the measurement table with it); it is now checked in `boot_progenitor` at the
            // handoff, which every default boot reaches. A card with the wrong archive still halts
            // with `MEASURED BOOT REFUSED`, later in the transcript than it used to.
            //
            // **A board reader who knew the `init/build` line wants `boot_ladder::PROMPT` now.**
            // That was the marker `crates/board_console`'s `Progress::userspace_ran` matched, and
            // no kernel prints it after this milestone. The live rung is `Stage::Prompt`, which
            // cannot appear unless userspace built the whole console stack, so it says more than
            // the line it replaces. `userspace_ran` itself stays, for the captured VisionFive 2
            // transcript that carries the old line and cannot be re-taken; its doc says so.
            let is_archive = nifefs::Fs::parse(initrd)
                .map(|fs| fs.read(user::PROGENITOR_ENTRY).is_some())
                .unwrap_or(false);
            if is_archive {
                println!(
                    "  user ELF    : an archive; the progenitor composes this machine's userspace at the handoff below"
                );
            } else {
                const N: u64 = 7;
                match user::riscv_least_authority_demo(initrd, N) {
                    Ok(sq) => println!(
                        "  user ELF    : loaded a {}-byte riscv ELF, ran least_authority_demo({N}) at U-mode, it sent {sq} (expected {})",
                        initrd.len(),
                        N * N,
                    ),
                    Err(e) => println!("  user ELF    : load failed: {e:?}"),
                }
            }
        } else {
            println!("  user ELF    : skipped (no -initrd passed to QEMU)");
        }
        sched::note_boot_stage(5);

        // Preemption: the property that separates a kernel from a runtime. Spawn two threads whose
        // entire body is a tight loop, no yield, no syscall, not even a function call. Under any
        // cooperative scheduler the first one owns the CPU forever. The S-mode timer fires every
        // 10ms, riscv_trap_dispatch records the tick and defers a schedule() to the trap tail, and
        // both threads make progress anyway. This is the RISC-V half of DECISIONS §5: an arbitrary
        // binary that refuses to yield is preempted regardless. `STOP` lets them exit cleanly so
        // nothing lingers past the demo. Interrupts have been unmasked since the timer step above.
        {
            use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
            static A: AtomicU64 = AtomicU64::new(0);
            static B: AtomicU64 = AtomicU64::new(0);
            static STOP: AtomicBool = AtomicBool::new(false);

            sched::spawn(|| {
                while !STOP.load(Ordering::Relaxed) {
                    A.fetch_add(1, Ordering::Relaxed);
                }
            });
            sched::spawn(|| {
                while !STOP.load(Ordering::Relaxed) {
                    B.fetch_add(1, Ordering::Relaxed);
                }
            });

            let p0 = sched::preemptions();
            arch::timer::spin_for(arch::timer::frequency() / 5); // ~0.2s, doing nothing but be preemptible
            STOP.store(true, Ordering::Relaxed);
            // Let the two spinners observe STOP and exit, so they are gone before we halt.
            for _ in 0..8 {
                sched::yield_now();
            }
            // The claim only prints when the numbers back it: both spinners made progress and at
            // least one preemption happened. The scheduler smoke line above earned the same
            // honesty the hard way (an unconditional success claim over a raced count).
            let (a, b, p) = (
                A.load(Ordering::Relaxed),
                B.load(Ordering::Relaxed),
                sched::preemptions() - p0,
            );
            if a > 0 && b > 0 && p > 0 {
                println!(
                    "  preemption  : two never-yield threads ran {a} and {b} iterations, {p} preemptions (a thread that refuses to yield is preempted anyway)",
                );
            } else {
                println!(
                    "  preemption  : FAILED: {a} and {b} iterations, {p} preemptions (a never-yield thread did not run, or nothing was preempted)",
                );
            }
        }
        sched::note_boot_stage(6);

        // Device interrupts, serviced by an unprivileged userspace driver: the last piece of the
        // interrupt story, in its real form. The kernel loads `driver` from the initrd, maps the
        // NS16550 into it device-typed, and grants it an Irq capability and a report endpoint. A
        // keystroke raises a line into the PLIC; the PLIC delivers a supervisor external interrupt;
        // riscv_trap_dispatch claims it, masks the source, and notifies the endpoint the Irq cap
        // waits on; the driver (a U-mode process owning no privilege) wakes, reads the byte through
        // its own device mapping, SENDs it, and ACKs, which re-arms the source through arch::irq. The
        // kernel is never in the data path. The `cargo run` harness is non-interactive, so pipe a
        // byte to QEMU's serial *after boot*: `( sleep 4; printf A ) | ...` (the console clears the
        // RX FIFO during init, so a byte sent at t=0 is dropped).
        if let Some((plic_phys, _)) = memory::plic_region() {
            // The UART's PLIC source, from the machine's own tree: 10 on QEMU virt, 32 on the
            // JH7110. This step used to arm a QEMU constant, and on the board that enabled an
            // unrelated source, so a real keystroke could never reach the driver; boot 13 proved
            // it on silicon (notes/visionfive2.md, BUGS). The fallback when the tree does not say
            // is that same constant (user::UART_RX_INTID), and the line below names which source
            // won, so a bench transcript is diagnosable.
            let (uart_irq, uart_irq_source) = user::uart_irq_and_source();
            // SAFETY: the PLIC is device-mapped in the direct map (mmu::map_everything); this is
            // its VA. The context is the boot hart's S context (2*hart + 1), derived rather than
            // hardcoded to 1 because OpenSBI elects the boot hart by lottery; see irq::boot_s_context.
            unsafe {
                drivers::plic::init(
                    arch::mmu::phys_to_virt(plic_phys) as usize,
                    arch::irq::boot_s_context(),
                );
            };

            println!("  uart irq    : source {uart_irq} ({uart_irq_source})");

            let started = user::initrd()
                .filter(|a| {
                    nifefs::Fs::parse(a)
                        .map(|fs| fs.read("serial_driver").is_some())
                        .unwrap_or(false)
                })
                .and_then(|a| user::riscv_uart_driver_demo(a, uart_irq).ok());
            match started {
                Some(report) => {
                    // A receiver for the driver's reports, so the boot tour does not block on input.
                    sched::spawn(move || {
                        let byte = sched::ipc_receive(report)[0] as u8;
                        println!(
                            "  device IRQ  : an unprivileged userspace driver serviced the UART via its Irq cap and sent {byte:#04x} ({:?})",
                            byte as char,
                        );
                    });
                    println!(
                        "  device IRQ  : userspace driver started (holds an Irq cap + a UART mapping); pipe a byte to see it"
                    );
                    // Give a byte already piped a turn to flow through the driver before the banner.
                    for _ in 0..8 {
                        sched::yield_now();
                    }
                }
                None => println!(
                    "  device IRQ  : skipped (no 'driver' in the initrd; run `cargo xtask initrd-riscv`)"
                ),
            }
        }
        sched::note_boot_stage(7);

        // virtio block device discovery (parity C). Probe the virtio-mmio slots the `virt` machine
        // lays out (0x1000_1000..); a block device shows up when a disk is attached (NIFE_DISK).
        // The kernel owns the transport and will hand a userspace driver the device's registers, an
        // Irq capability, and a DMA region; here we just prove the discovery works on RISC-V.
        match virtio::find_block_device() {
            Some(dev) => println!(
                "  virtio      : block device found at {:#x}, PLIC IRQ {} (kernel owns the transport)",
                dev.mmio_phys, dev.intid,
            ),
            None => {
                println!("  virtio      : no block device attached (pass NIFE_DISK to attach one)");
            }
        }
        sched::note_boot_stage(8);

        // PCIe enumeration + virtio-pci bring-up (the PCIe transport, P1/P2). The disk QEMU
        // attaches as `virtio-blk-pci` arrives over the transport real hardware uses: found by
        // walking ECAM config space, its BARs placed by the kernel (OpenSBI does no PCI setup),
        // its register blocks resolved through the virtio vendor capabilities, its INTx line
        // swizzled to a PLIC input.
        match pci::find_block_device() {
            Some(d) => println!(
                "  pcie        : virtio-blk at {:02x}:{:02x}.{}, common {:#x}, notify {:#x} (mult {}), isr {:#x}, PLIC IRQ {}",
                d.bdf.bus,
                d.bdf.dev,
                d.bdf.func,
                d.common,
                d.notify_base,
                d.notify_mult,
                d.isr,
                d.intid,
            ),
            None => {
                println!("  pcie        : no virtio-blk on the bus (pass NIFE_DISK to attach one)");
            }
        }
        sched::note_boot_stage(9);

        // **The clock the `hw entropy` step is measured with**
        // (design/roadmap/0306-time-the-hw-entropy-step.md, milestone 159's own follow-on).
        //
        // Read here, on the line after the `pcie` print, because the gap a bench session has been
        // timing by eye is exactly `pcie` to `hw entropy`: those two lines are adjacent in the
        // transcript, so the wall time between them is what a stopwatch at a serial console
        // measures, and a person watching one resolves it to about a second. `design/fatal-risks/README.md`
        // risk 6's third part is *at real speed*, and a bytes-per-second figure worth quoting needs
        // the machine to time itself.
        //
        // **The span deliberately includes the `hw clock` step**, which prints between the two, so
        // that the number printed below is the same quantity the stopwatch was measuring rather
        // than a subset of it that happens to be tidier. The bring-up and draw costs are timed
        // separately underneath it, and those are the two the rate question actually wants: a
        // reseed plus a generation is a once-per-boot cost, and the round trips are the rate.
        //
        // `arch::timer::now()` is the mechanism the tour already uses (the timer step above spins
        // on it), so nothing new is introduced here; on riscv64 it is the `time` CSR, whose rate
        // came out of this machine's device tree at `init_frequency`.
        let entropy_step_start = arch::timer::now();

        // **A real, non-virtio device, driven by a confined userspace process** (milestone 159,
        // design/roadmap/0159-jh7110-trng-driver.md; fatal risk 6 in design/fatal-risks/README.md). The
        // JH7110's TRNG is a register block on the `SoC`'s own fabric: no transport to negotiate,
        // no queue, no DMA. The kernel's whole part is the two lines below (ask the device tree
        // whether the device exists, then hand a userspace program one page of its registers and
        // two endpoints) and then playing a client over the same request endpoint any other client
        // would hold. The kernel never reads a `RAND` register.
        //
        // **The skip is the honest answer on every machine CI boots.** QEMU's riscv64 `virt` board
        // has no TRNG node under either spelling, so `jh7110_trng_device` returns `None` there and
        // this prints the skip rather than a claim. Only a `StarFive` VisionFive 2 (radon, the one
        // this project benches on) can print the other line, and the line only prints when bytes
        // actually arrived.
        //
        // **Both spellings are named in the skip because milestone 239 (radon's device tree does
        // not describe the TRNG, so a working driver never runs) was mis-diagnosed off the old
        // wording.** On 2026-09-03 this line said "describes no starfive,jh7110-trng" on a board
        // whose firmware tree does describe the device, under the vendor U-Boot's own
        // `starfive,trng`; the line was true and the conclusion drawn from it was not. A skip that
        // names everything it looked for cannot be read that way twice.
        // **The clock step announces itself even when there is nothing to do** (milestone 220).
        // On every machine this repository's CI boots there is no clock controller and no TRNG,
        // and a step that printed nothing there would leave a reader of the transcript to go and
        // check the source for whether it ran. The `Some` case is printed below, beside the
        // bring-up report, because the report is what makes the address worth reading.
        if user::entropy_service::jh7110_crg_window().is_none() {
            println!(
                "  hw clock    : skipped (this machine's tree names no JH7110 clock controller and no JH7110 TRNG, so no window was mapped and nothing was stored; QEMU virt has neither)"
            );
        }
        match user::entropy_service::jh7110_trng_device() {
            // **The skip, and a reference measurement beside it when this machine can give one.**
            //
            // The skip itself is the honest answer on every machine but radon and it has not
            // changed. What is new is the number after it, and the reason it is worth printing is
            // that the TRNG figure this step exists to produce is **not interpretable on its own**.
            // A draw through this service is one `entropy_protocol` exchange per 8 bytes
            // (`MAX_BYTES`), so 32 bytes is four round trips through a userspace process, and a
            // measured microsecond count over 64 bytes mixes the device's cost with the IPC's
            // without saying in what proportion. The proposal names that as the thing to state
            // before the number leaves the machine: *does it count the IPC, the poll loop, the
            // process spawn?*
            //
            // The virtio-rng backend answers it, because it is **the same path with a different
            // device at the end**: the same `entropy_protocol`, the same `Wiring::fill`, the same
            // eight round trips, the same confined userspace process holding the same two
            // rendezvous capabilities. Whatever it costs here is what the JH7110's number would
            // cost with a free device, so the difference between the two is the driver's.
            //
            // **It is a reference, not a measurement of hardware**, and the line says so in those
            // words: QEMU backs virtio-rng from the host's `/dev/urandom` and its cost is the
            // emulator's, not silicon's. `notes/benchmarks.md`'s standard is to say where a
            // comparison is not apples-to-apples, and this is where.
            //
            // Only when this machine actually has such a device, which on the default riscv64
            // boot it does not: `NIFE_RNG` is what attaches one (DECISIONS §120's QEMU-only
            // stopgap), and without it `ensure` returns `None` after the bus scan and the line
            // reads exactly as it did before plus its own microsecond count.
            None => {
                let reference = user::program("entropy").and_then(|image| {
                    let w = user::entropy_service::ensure(image, user::entropy_service::Bus::Mmio)?;
                    let report = w.wait_for_ready().unwrap_or([0; 5]);
                    let ready_at = arch::timer::now();
                    let (mut a, mut b) = ([0u8; 32], [0u8; 32]);
                    let (na, nb) = (w.fill(&mut a), w.fill(&mut b));
                    let drawn_at = arch::timer::now();
                    Some((report[0], na + nb, ready_at, drawn_at))
                });
                match reference {
                    Some((verdict, bytes, ready_at, drawn_at))
                        if verdict == entropy_protocol::READY && bytes > 0 =>
                    {
                        println!(
                            "  hw entropy  : skipped (this machine's tree names no TRNG: neither starfive,jh7110-trng nor the vendor U-Boot's starfive,trng; QEMU virt has neither); since the pcie line {} us. Reference, NOT a TRNG and NOT hardware: this emulator's virtio-rng over the same entropy_protocol path, bring-up {} us, {} bytes in {} us ({} bytes/s over {} round trips)",
                            micros_between(entropy_step_start, arch::timer::now()),
                            micros_between(entropy_step_start, ready_at),
                            bytes,
                            micros_between(ready_at, drawn_at),
                            bytes_per_second(bytes as u64, drawn_at.wrapping_sub(ready_at)),
                            bytes as u64 / entropy_protocol::MAX_BYTES,
                        );
                    }
                    // A device was there and the service did not come up, or came up dry. Said
                    // rather than swallowed, because a reference that silently degrades to the
                    // plain skip would make a machine with a broken virtio-rng look like one with
                    // no virtio-rng at all.
                    Some((verdict, bytes, _, _)) => println!(
                        "  hw entropy  : skipped (this machine's tree names no TRNG: neither starfive,jh7110-trng nor the vendor U-Boot's starfive,trng; QEMU virt has neither); since the pcie line {} us. The virtio-rng reference did not run: report {:#x}, {} bytes drawn",
                        micros_between(entropy_step_start, arch::timer::now()),
                        verdict,
                        bytes,
                    ),
                    None => println!(
                        "  hw entropy  : skipped (this machine's tree names no TRNG: neither starfive,jh7110-trng nor the vendor U-Boot's starfive,trng; QEMU virt has neither); since the pcie line {} us, which is the device-tree query and nothing else (no virtio-rng on this machine either, so there is no reference draw to time; pass NIFE_RNG to attach one)",
                        micros_between(entropy_step_start, arch::timer::now()),
                    ),
                }
            }
            Some(device) => match user::program("jh7110_entropy") {
                None => println!(
                    "  hw entropy  : JH7110 TRNG at {:#x}, but no 'jh7110_entropy' in the initrd (run `cargo xtask initrd-riscv`)",
                    device.reg_base,
                ),
                Some(image) => {
                    // **The wiring is timed, and the console output between the segments is not**
                    // (the proposal above). `ensure` maps the register window and spawns the
                    // driver; the readiness wait is the driver's own bring-up (a reseed, then a
                    // first generation). The `hw clock` line prints between the two, and a
                    // `println!` here is a polled UART: on radon at 115200 baud that line is
                    // roughly 30 ms of the kernel doing nothing but shift bits out, which is the
                    // same order as the bring-up it would otherwise be added to. So the bring-up
                    // figure is the sum of two measured segments rather than one span across
                    // them, and the whole-gap figure beside it (`since the pcie line`) is the one
                    // that still includes the console, because that is the quantity a stopwatch
                    // at the serial port was measuring.
                    let wire_start = arch::timer::now();
                    match user::entropy_service::ensure(image, user::entropy_service::Bus::Jh7110) {
                        None => println!(
                            "  hw entropy  : JH7110 TRNG at {:#x}, but the service would not wire",
                            device.reg_base,
                        ),
                        Some(w) => {
                            let wired_at = arch::timer::now();
                            // **The clock and reset controller's own answer, first** (milestone
                            // 220), because it decides how the line below should be read. On
                            // 2026-09-04 radon printed an all-zero TRNG register file, and the
                            // two candidate explanations (a block nobody had powered, or a
                            // driver talking to the wrong address) are told apart here and
                            // nowhere else. The `before` words are what separate them again:
                            // clocks that were already enabled mean this milestone's premise was
                            // wrong and the zeros have some other cause.
                            //
                            // Printed even when it says nothing happened, because "no controller
                            // in this tree" is itself the finding on any machine but radon, and a
                            // silent step is one a bench reader has to go and check the source
                            // for.
                            match w.clock {
                                Some(report) => {
                                    let found = user::entropy_service::jh7110_crg_window();
                                    println!(
                                        "  hw clock    : JH7110 STG CRG at {:#x} ({}): clocks {:#010x},{:#010x} -> {:#010x},{:#010x} ({}); reset {} assert {:#010x} -> {:#010x}, status {:#010x} ({}, {} polls){}",
                                        found.map_or(0, |f| f.base),
                                        match found {
                                            Some(f) if f.from_tree =>
                                                "named by this machine's device tree",
                                            Some(_) =>
                                                "NOT named by this machine's tree: the constant both published trees agree on",
                                            None => "address unknown",
                                        },
                                        report.clock_before[0],
                                        report.clock_before[1],
                                        report.clock_after[0],
                                        report.clock_after[1],
                                        if report.has_clocks_running() {
                                            "running"
                                        } else {
                                            "NOT running: the enable bit did not read back, so nothing is behind this window"
                                        },
                                        jh7110_clock_and_reset::STGRST_SEC_AHB,
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
                                            "; the firmware had already done all of this, so a gated clock is NOT why the TRNG reads zeros"
                                        } else {
                                            ""
                                        },
                                    );
                                }
                                // Unreachable in practice, and printed rather than ignored for
                                // that reason: this arm is inside `Some(device)`, so the tree
                                // named a TRNG, and `memory::init` records a window for exactly
                                // that case. If it ever fires, the guard and the caller have
                                // drifted apart and a bench transcript should say so.
                                None => println!(
                                    "  hw clock    : no JH7110 clock-and-reset window was mapped even though this tree names a TRNG, so nothing was ungated"
                                ),
                            }
                            // The driver's bring-up report next:
                            // `[READY, first_refill_ok, bytes_in_hand]`, or a 0xDEAD_.. word whose
                            // low byte names the step. A device that never answered says so here,
                            // and since 2026-09-04 so does one that answered with zeros:
                            // `entropy_protocol::readiness` decides that word from the bytes, which
                            // is what the boot below found it was not doing.
                            let wait_start = arch::timer::now();
                            let report = w.wait_for_ready().unwrap_or([0; 5]);
                            let ready_at = arch::timer::now();
                            // Then two draws through the request endpoint, as a client. Two,
                            // because one proves only that *something* was returned: a stuck
                            // register file, a driver serving its buffer twice, and a device that
                            // never started all present as a repeat.
                            //
                            // **Each draw is four round trips, not one**, and that is the fix for
                            // the failure radon printed on 2026-09-04. `Wiring::get` is a single
                            // `entropy_protocol` exchange, and that protocol carries `MAX_BYTES = 8`,
                            // so `get(32, ..)` returns 8 and can never return 32: the success line
                            // below was unreachable on any device, working or not. It went
                            // unnoticed for three days because this branch runs on exactly one
                            // machine in the world and QEMU takes the `skipped` arm.
                            //
                            // Filling all 32 bytes is also the stronger test, which is why the
                            // count stays 32 rather than dropping to 8. The driver's pool holds
                            // one 32-byte generation, so draw `a` empties it and draw `b` forces a
                            // second trip to the hardware. Two 8-byte draws would both have come
                            // out of the same buffer, and `a != b` would then prove only that the
                            // cursor advanced. Now it compares two device generations, which is
                            // what the line below claims.
                            let (mut a, mut b) = ([0u8; 32], [0u8; 32]);
                            let (na, nb) = (w.fill(&mut a), w.fill(&mut b));
                            let drawn_at = arch::timer::now();
                            // The two segments the console does not sit inside: the wiring (window
                            // mapped, driver spawned) and the readiness wait (the driver's reseed
                            // and first generation).
                            let bringup_ticks = wired_at.wrapping_sub(wire_start)
                                + ready_at.wrapping_sub(wait_start);
                            let draw_ticks = drawn_at.wrapping_sub(ready_at);
                            // The service refuses to report ready on an all-zero first bufferful
                            // now, so this is a client checking a claim rather than the only thing
                            // standing between a boot and zeros served as randomness. It stays
                            // because a tour that only repeated the service's own verdict would
                            // have caught nothing on 2026-09-04.
                            let zeros = a.iter().all(|&x| x == 0);
                            if report[0] == entropy_protocol::READY
                                && na == 32
                                && nb == 32
                                && !zeros
                                && a != b
                            {
                                println!(
                                    "  hw entropy  : JH7110 TRNG at {:#x} served 32+32 bytes to a client through a capability that names no device; first draw {:02x}{:02x}{:02x}{:02x}.., second differs; STAT after init {:#010x} ({}); since the pcie line {} us, of which bring-up {} us (window mapped, driver spawned, reseed, first generation; the hw clock line's own console time excluded) and {} bytes in {} us ({} bytes/s over {} round trips)",
                                    device.reg_base,
                                    a[0],
                                    a[1],
                                    a[2],
                                    a[3],
                                    (report[1] >> 32) as u32,
                                    mode_note((report[1] >> 32) as u32),
                                    micros_between(entropy_step_start, arch::timer::now()),
                                    micros(bringup_ticks),
                                    na + nb,
                                    micros(draw_ticks),
                                    bytes_per_second((na + nb) as u64, draw_ticks),
                                    (na + nb) as u64 / entropy_protocol::MAX_BYTES,
                                );
                            } else {
                                // `report[2]` is the driver's bring-up diagnostic and it is the
                                // number a bench session reads first. It is **always** the raw
                                // `(STAT << 32) | ISTAT` now, on success as well as on failure:
                                // it used to be the byte count when the report word said READY,
                                // and on 2026-09-04 that cost an hour, because the `0x20` printed
                                // here was read as an undocumented `ISTAT` bit 5 when it was the
                                // number 32 wearing a register's clothes. All zeros means the
                                // register window read as nothing at all (a gated clock, an
                                // undeasserted reset, or a base that is not the TRNG) rather than
                                // a device that answered wrongly. See components/src/jh7110_entropy.rs.
                                // The tree's own two words about this node come with the failure,
                                // not in a separate line, because they are what a bench session
                                // reads next: an all-zero diagnostic on a node the firmware calls
                                // `disabled` is a clock or reset the firmware had no reason to
                                // ungate (milestone 220's territory), while the same zeros on a
                                // node it calls `okay` are a different problem entirely.
                                println!(
                                    "  hw entropy  : FAILED: JH7110 TRNG at {:#x} (tree says {}, status {}): report {:#x}, bring-up diagnostic {:#018x}, STAT after init {:#010x} ({}), draws {na}/{nb} bytes, first-all-zero {zeros}, draws-differ {}; since the pcie line {} us, of which bring-up {} us and draws {} us",
                                    device.reg_base,
                                    core::str::from_utf8(device.compatible).unwrap_or("?"),
                                    if device.status_okay {
                                        "okay"
                                    } else {
                                        "disabled"
                                    },
                                    report[0],
                                    report[2],
                                    (report[1] >> 32) as u32,
                                    mode_note((report[1] >> 32) as u32),
                                    a != b,
                                    micros_between(entropy_step_start, arch::timer::now()),
                                    micros(bringup_ticks),
                                    micros(draw_ticks),
                                );
                            }
                        }
                    }
                }
            },
        }
        sched::note_boot_stage(10);

        println!("{}RISC-V.", boot_ladder::TOUR);
        // 11, not 10: the hang watcher falls silent at "the tour has finished" and milestone 159
        // added a stage after what used to be the last one. See `user.rs`'s `boot_stage() >= 11`.
        sched::note_boot_stage(11);
        // **The tour ends and the soak begins** (milestone 219). After the tour's last line, so a
        // soak boot is a superset of an ordinary one and the whole boot is still evidence; before
        // the halt, because the halt is the thing it replaces.
        // **The sweep, when this build asked for one** (milestone 168). Before the soak arm and
        // the halt for the same reason the soak sits before the halt: the tour has finished, so the
        // whole boot is still evidence, and this run ends by halting rather than by beating
        // forever. The two features are alternatives rather than a stack; `script/job-mix` builds
        // only this one.
        #[cfg(feature = "job_mix")]
        job_mix::run();
        #[cfg(feature = "soak_test")]
        soak::run();
        // **Fatal risk 6's bench boot, when this build asked for one** (milestone 261). The same
        // position and the same reason as the two above: the tour is evidence, and this replaces
        // the hand-over. It WRITES to the NVMe disk; see kernel/src/disk_throughput.rs.
        #[cfg(feature = "disk_throughput")]
        disk_throughput::run();
        // **Milestone 494 (a driver for the network card a PC actually has)'s bench boot**, in the
        // same position for the same reason; see kernel/src/network_bench.rs.
        #[cfg(feature = "network_bench")]
        network_bench::run();
        // **Milestone 53's storage bench boot**, in the same position for the same reason; see
        // kernel/src/storage_bench.rs.
        #[cfg(feature = "storage_bench")]
        storage_bench::run();
        // **Nothing halts by default** (milestone 268, item 4). The tour used to end here in
        // `arch::halt()`, and that was the right thing to do while the arch layer beneath the
        // shared path was still being built: there was nothing honest to fall through to. There is
        // now. `riscv_hand_over` is the same call the `shell` build makes a few hundred lines
        // above, so this architecture's default boot ends where aarch64's already did, at a
        // `swish` prompt, and **the prompt is the signal that the boot finished**.
        //
        // What this buys on a board is the whole reason calef decided it: a machine that reaches a
        // prompt can be logged into and diagnosed, and one that halted could only be power-cycled.
        // The VisionFive 2's card boots exactly this configuration (`script/board-image` builds
        // `--features board`, not `shell`), so it is the card that gains most.
        //
        // **The tour is untouched and still runs first.** Every demonstration above this line still
        // runs, still prints, and still ends with the line `board_console` calls `Stage::Tour`.
        // Those steps are the only thing in this tree proving the boot's middle stages on real
        // silicon (notes/visionfive2.md), so this adds a rung above them rather than removing any.
        //
        // `halt` afterwards, and it is not dead: the boot thread's own work is done and it parks in
        // a preemptible `wfi` loop so the progenitor and its children get scheduled. A boot with no
        // archive says so inside `riscv_hand_over` and parks the same way.
        #[cfg(not(any(
            feature = "soak_test",
            feature = "job_mix",
            feature = "disk_throughput",
            feature = "network_bench",
            feature = "storage_bench"
        )))]
        {
            riscv_hand_over();
            // Leaves the scheduler rather than parking in it, as the `shell` build's hand-over above
            // does (milestone 720 (provisional)).
            sched::exit();
        }
    }

    arch::init();
    stack::init();
    // Paint the boot stack's unused region for the high-water report (milestone 84), before
    // memory::init and everything after it can push frames into it.
    #[cfg(any(test, feature = "system_tests"))]
    stack::paint_boot_stack();

    // Now that faults are reportable, go find out how much RAM we actually have. A bug
    // in here is a fault, and a fault is now legible rather than fatal-and-silent.
    memory::init();

    // What machine is this (milestone 60). Three `mrs` reads, and the reason they happen HERE is
    // the line below: `mmu::init` takes `TCR_EL1.IPS` from this record and builds tables on a 4 KiB
    // granule this checks the part actually has. A machine that cannot run us says so and stops,
    // rather than turning the MMU on and faulting on the next instruction fetch with no console
    // line to explain it.
    arch::isa::init(boot_info_pointer);

    // Which cores does this machine have, and how are they started (milestone 100)? Both come out
    // of the device tree, and both have to be read HERE: `mmu::init` on the next line replaces the
    // coarse boot map, and the blob is only reachable through that one. The list used to be
    // `0..cpu::MAX_CPUS` and the PSCI conduit used to be `hvc`, compiled in; `isa::init` above took
    // the `/psci` half.
    smp::read_cpu_list();

    // Does the machine agree with its own firmware about how fast the counter runs (2026-09-21)?
    // Here for the same reason as the two reads above: the blob is only reachable through the
    // coarse boot map, which `mmu::init` on the next line replaces. `CNTFRQ_EL0` is firmware-set,
    // and a firmware that writes a wrong-but-plausible number is the one failure no later check can
    // see. Silent on every machine we test on, which states nothing to compare against; see
    // `arch::timer::check_frequency_against_device_tree` for what that costs and why it refuses
    // rather than preferring one source. (Gated even though only aarch64 reaches this line at run
    // time: the riscv64 arm above ends in `sched::exit()`, so everything below it is still
    // *compiled* for that architecture, which has no such function and needs none.)
    #[cfg(target_arch = "aarch64")]
    arch::timer::check_frequency_against_device_tree(boot_info_pointer);

    // And now the sketchiest moment in the kernel. The instant SCTLR_EL1.M is set, the very
    // next instruction is fetched through the MMU. See arch/aarch64/mmu.rs.
    arch::mmu::init();

    // The per-CPU interrupt stacks go live here, and not one line earlier: their guard pages are
    // holes in the map `mmu::init` just installed, and the coarse boot map has no holes at all. A
    // trap before this point runs its handler on whatever stack it interrupted, which is what every
    // trap did before milestone 124. See kernel/src/interrupt_stack.rs.
    interrupt_stack::init();

    // The SMMUv3 (milestone 16b), if the machine was started with `iommu=smmuv3`. Its register
    // block is a device-tree node (memory::smmu_region), mapped by mmu::init just above; absent, the
    // kernel runs exactly as before. Bringing it up here installs an all-invalid stream table and
    // sets default-deny, so every PCIe stream aborts until virtio::register confines its device.
    // The CPU's own ECAM and BAR reads are not DMA, so PCI enumeration below is unaffected. See
    // kernel/src/iommu.rs, notes/iommu.md. Not compiled on x86_64, whose VT-d `init` takes the
    // DMAR's units rather than one base (milestone 594) and runs in the x86 arm above; the region
    // is never recorded there anyway.
    #[cfg(not(target_arch = "x86_64"))]
    if let Some((smmu_base, _)) = memory::smmu_region() {
        arch::iommu::init(smmu_base);
    }

    // The heap must come AFTER the MMU: it hands out addresses, and with paging on an
    // address is only usable if something has mapped it. From here, `Vec` works.

    // And now interrupts, which is where every lock in the kernel stops being a formality.
    //
    // The scheduler comes up FIRST, so that the very first timer tick already has somewhere to
    // send a reschedule. Adopting the boot context as thread 0 costs one allocation.
    sched::init();
    interrupts_init(boot_info_pointer);

    // Bring the other cores online. They come up idle: step 2 proves the bring-up path works
    // (PSCI, per-core stacks, the MMU replay), and leaves real multi-core scheduling to step 3.
    // Core 0 has IRQs on by now, so it keeps ticking while it waits for the others to check in.
    // See smp.rs and DECISIONS §11.
    smp::bring_up_secondaries();

    // Whether this machine has a running cycle counter (milestone 74's aarch64 half). Every core
    // started and checked its own in `timer::init`; this is the one line that reports them all,
    // here because it is the first point every core has answered. Every build prints it, test and
    // bench included, for the reason the riscv64 boot gives: whether the machine has one is a fact
    // about the machine, and the test and bench transcripts are where QEMU's answer gets read.
    arch::pmu::print_summary();

    #[cfg(any(test, feature = "system_tests"))]
    run_test_suite();

    // The instruction-count boot (milestone 78, `script/icount`): assert the two timing claims on
    // the deterministic clock and park. **Before the bench boot, and its feature implies that one**,
    // because the two park at the same point and therefore leave the same functions unreferenced;
    // riding on `bench`'s existing conditions is what keeps this from duplicating a dozen `cfg`s
    // across five files. The `bench::run` below is then unreachable, which is what this function's
    // `allow(unreachable_code)` names.
    #[cfg(feature = "icount")]
    icount::run();

    // The benchmark boot (milestone 21, `script/bench`): run the microbenchmarks and halt,
    // instead of the tour or the shell. Diverges, so everything below is untouched by it.
    #[cfg(feature = "bench")]
    bench::run();

    #[cfg(not(any(test, feature = "system_tests", feature = "bench")))]
    {
        print_machine_description(boot_info_pointer);

        // **Rung two of the ladder** (milestone 268): the kernel proving it works on the machine it
        // has just described, before anything is handed to userspace. The same five checks on all
        // three architectures, and the verdict line is what CI and `board_console` read. It
        // reports and does not gate: a failure prints and the boot carries on to the prompt,
        // because a machine you cannot log into is a machine you cannot fix.
        self_test::run();

        // **The milestone tour, and what is left of it after milestone 267.**
        //
        // It was three things wearing one name: a machine description, a narrative, and a set of
        // demonstrations. The description is `print_machine_description` above and prints on every
        // boot. The narrative was `user/src/narrator.rs`, a program at EL0, and is deleted
        // (milestone 267's block records what it said and why it went). What remains here is the
        // third thing, and every entry is here because it needs a privilege a program does not
        // have. The list is short on purpose, and it is the whole list:
        //
        //   - **Two threads that never yield**, spawned with `sched::spawn`, counted against
        //     `sched::preemptions()`, timed with `timer::spin_for`. No syscall, no function call,
        //     no cooperation. A program at EL0 cannot make this claim about the scheduler because
        //     an EL0 spinner proves only that EL0 was preempted; these run *inside* the kernel and
        //     are still taken off the CPU, which is the stronger statement and the thesis in
        //     miniature.
        //   - **The virtio and PCIe block demos.** The kernel enumerates the bus, mints the
        //     device's registers, a DMA page and an interrupt into a driver's world, and receives
        //     the driver's report with `sched::ipc_receive`. Every one of those is an authority whose
        //     whole point is that the driver did not have it until the kernel granted it, so the
        //     granter cannot be the grantee.
        //   - **The outlaw.** `&raw const USER_FAULTS` is the address of a kernel static, handed to
        //     a program that then faults reading it and increments the very counter it reached for.
        //     A program cannot name that address, which is the demonstration.
        //   - **The memory-region demo.** It prints `memory::stats().used` before and after a
        //     process spends its own budget, and the claim is that the number did not move. The
        //     number is the kernel's own frame accounting and is not exposed to EL0 (see the
        //     preemption-counter note in design/roadmap/0267-*.md: an ambient fact nobody needs yet).
        //
        // The console server is started at the top of this block rather than being on that list.
        // It is not a survivor either, and since the narrator was deleted it is not the move
        // either: it comes up with nothing to print. See the block below.
        //
        // Compiled out by `cargo xtask shell`, which boots straight to the system instead of
        // scrolling all of this first. There were two features here until milestone 296: `initboot`
        // named nothing `shell` did not, and a build of each produced the same 3,109 symbols at the
        // same total size. calef ruled it deleted rather than renamed on 2026-09-14.
        #[cfg(not(feature = "shell"))]
        {
            // **The console server, with nothing to print through it.** Milestone 267 moved the
            // nine-line milestone narrative out of `kernel_main` into `user/src/narrator.rs` and
            // spawned it here as this server's client; calef ruled the narrator deleted on
            // 2026-09-13 and the client went with it. What is left is the server: `user/src/
            // console.rs`, a UART driver at EL0, spawned and then idle on its request endpoint
            // because nothing else in this boot holds a capability to it.
            //
            // **That is a question, not a design**, and it is deliberately left open here rather
            // than answered by deleting one more thing: whether a boot-time console server earns
            // its bring-up once its only client is gone is an architect's call, because deleting it
            // removes infrastructure rather than a demonstration. Milestone 267's block states the
            // case both ways. The interactive system does not reach this code at all; it builds
            // its own console through `boot_progenitor` further down.
            //
            // The initrd is asked for first because both `expect`s inside
            // `console_service::start` are about the archive rather than the machine: a run with no
            // `-initrd` has no console program to start, and the initrd step further down is where
            // that boot says so.
            //
            // The `map` form is milestone 267's and is kept rather than rewritten. It was chosen
            // over `is_some() && let` on a measurement (340 bytes of `.text`), but that comparison
            // was made against a two-condition `if let` that no longer exists here, so the number
            // is history rather than a live claim about this line.
            let _console = user::initrd().map(|_| user::console_service::start());

            // The whole argument, executable.
            {
                use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

                use crate::arch::timer;

                static HOSTILE: AtomicU64 = AtomicU64::new(0);
                static POLITE: AtomicU64 = AtomicU64::new(0);
                static STOP: AtomicBool = AtomicBool::new(false);

                // A thread whose entire body is a tight loop. No yield. No syscall. Not even a
                // function call. Under ANY cooperative scheduler this owns the CPU forever and the
                // machine is gone. This is the arbitrary ELF binary, in miniature.
                sched::spawn(|| {
                    while !STOP.load(Ordering::Relaxed) {
                        HOSTILE.fetch_add(1, Ordering::Relaxed);
                    }
                });

                // A thread that also never yields, but would like a turn.
                sched::spawn(|| {
                    while !STOP.load(Ordering::Relaxed) {
                        POLITE.fetch_add(1, Ordering::Relaxed);
                    }
                });

                let p0 = sched::preemptions();
                timer::spin_for(timer::frequency() / 2); // half a second, doing nothing
                STOP.store(true, Ordering::Relaxed);

                let (hostile, polite, preempted) = (
                    HOSTILE.load(Ordering::Relaxed),
                    POLITE.load(Ordering::Relaxed),
                    sched::preemptions() - p0,
                );
                println!("  half a second later, having spawned two threads that NEVER yield:");
                println!();
                println!("    thread 1 (hostile) : {hostile:>10} iterations");
                println!("    thread 2 (polite)  : {polite:>10} iterations");
                println!("    preemptions        : {preempted:>10}");
                println!();
                // The closing claim only prints when the numbers above back it. The RISC-V tour's
                // scheduler smoke line earned this the hard way: an unconditional success claim
                // over a raced count read "0 of 2 ... works" on the VisionFive 2
                // (notes/visionfive2.md, boots 12 and 13). The window here is wall-clock, not a
                // yield count, so timing is sound; only the wording was unconditional.
                if hostile > 0 && polite > 0 && preempted > 0 {
                    println!("  neither asked to be interrupted. both were.");
                } else {
                    println!("  FAILED: a spinner did not run, or nothing was preempted.");
                }
            }

            // 7a. EL0.
            {
                use core::sync::atomic::Ordering;

                use crate::arch::timer;

                println!();
                println!("  and now the other side of the boundary:");
                println!();

                // Milestone 9: a virtio block device, driven from userspace.
                match crate::virtio::find_block_device() {
                    None => println!("    virtio : no block device attached"),
                    Some(d) => {
                        println!(
                            "    virtio : block device at {:#x}, INTID {}, handing it to a driver at EL0",
                            d.mmio_phys, d.intid,
                        );
                        if let Some(report) = user::virtio_service::start(image_for_virtio()) {
                            // The driver reads block 0 and sends us its first 8 bytes. We check they
                            // are the nifefs magic, which proves real disk bytes crossed DMA and
                            // the EL0 boundary. This RECEIVE blocks until the driver has done the read.
                            let word = sched::ipc_receive(report)[0];
                            let head = word.to_le_bytes();
                            println!();
                            if &head == b"nife: re" {
                                println!(
                                    "      a driver at EL0 read the file 'motd' off a virtio disk,"
                                );
                                println!("      through a nifefs superblock it parsed itself,");
                                println!(
                                    "      woken by the device's interrupt delivered as a message."
                                );
                                println!(
                                    "      the kernel issued no virtio command and touched no DMA."
                                );
                            } else {
                                println!(
                                    "      the driver reported {head:?}, not the motd contents"
                                );
                            }
                        }
                    }
                }

                // The same disk again, over PCIe (DECISIONS §18): found by ECAM enumeration,
                // BARs placed by the kernel, INTx through the GIC, the identical driver and
                // confinement behind the transport seam.
                match crate::pci::find_block_device() {
                    None => println!("    pcie   : no virtio-blk on the bus"),
                    Some(d) => {
                        println!(
                            "    pcie   : virtio-blk at {:02x}:{:02x}.{}, INTID {}, same driver, same confinement",
                            d.bdf.bus, d.bdf.dev, d.bdf.func, d.intid,
                        );
                        if let Some(report) = user::virtio_service::start_pci(image_for_virtio()) {
                            let word = sched::ipc_receive(report)[0];
                            if &word.to_le_bytes() == b"nife: re" {
                                println!(
                                    "      the same file, over the transport real hardware uses."
                                );
                            } else {
                                println!("      the pcie driver reported the wrong bytes");
                            }
                        }
                    }
                }

                // The privilege boundary, still real: a program that reaches for a kernel address is
                // killed, and the kernel is not. The address is the kernel's own fault counter, so
                // it is certainly mapped and certainly not the process's; the demo hands it to the
                // program rather than baking a constant in, which is what makes it the same demo on
                // either ISA. It used to run `user::outlaw()`, a hand-written aarch64 blob.
                if let Some(image) = user::program("outlaw") {
                    use crate::arch::exceptions::USER_FAULTS;
                    let kernel_addr = &raw const USER_FAULTS as u64;
                    let faults0 = USER_FAULTS.load(Ordering::Relaxed);
                    sched::spawn(move || {
                        user::run(
                            image,
                            user::Spawn {
                                arg0: user::OUTLAW_READ_KERNEL,
                                arg1: kernel_addr,
                                arg2: 0,
                                grants: &[],
                                maps: &[],
                            },
                        )
                    });
                    timer::spin_for(timer::frequency() / 20);
                    println!(
                        "    outlaw : reached {kernel_addr:#018x}, was killed, kernel survived ({} fault)",
                        USER_FAULTS.load(Ordering::Relaxed) - faults0,
                    );
                }

                // Milestone 8. The console driver is no longer in the kernel.
                match user::initrd() {
                    None => println!("    initrd : none (no -initrd passed to QEMU)"),
                    Some(image) => {
                        println!(
                            "    initrd : {} bytes at {:#x}, from the device tree",
                            image.len(),
                            memory::initrd_region().unwrap().0,
                        );
                        // The console server and its client both came up at the top of the tour
                        // (milestone 267): the narrative above is what they printed. This line
                        // says where their images came from, which is the milestone-8 claim, and
                        // no longer starts a second server to restate it.
                    }
                }

                // Milestone 11: a process spends its own memory; the kernel allocates nothing.
                if let Some(image) = user::program("memory_region_depleter")
                    && let Some((_region, report, _demo)) =
                        user::memory_region_service::start(image, 24)
                {
                    sched::ipc_receive(report); // the process signals it is loaded and ready
                    let before = memory::stats().unwrap().used;
                    let mapped = sched::ipc_receive(report)[0]; // it maps until its untyped is spent
                    let after = memory::stats().unwrap().used;
                    println!();
                    println!(
                        "  milestone 11: a process mapped {mapped} pages out of an untyped it was handed,"
                    );
                    println!(
                        "  and the kernel's used-frame count went {before} -> {after} (it did not move)."
                    );
                    println!(
                        "  a process cannot make the kernel allocate, so it cannot exhaust it."
                    );
                }
            }
        } // end of the milestone tour (#[cfg(not(feature = "shell"))])

        // **Milestone 19d.2c, completed at 28: userspace progenitor is the boot path.** The kernel stops
        // wiring services itself. It hands off to the progenitor, which brings up the console server, the line
        // discipline (`line_editor`, milestone 28), the input driver, and the shell out of its own budget
        // through the granular verbs. This is the line that retires the kernel as the system's
        // builder. Every aarch64 interactive build reaches it: `--features shell` and the milestone
        // tour hand off straight away (the tour after running its demos), and the same way
        // RISC-V's `--features shell` hands off to `system_initializer`. The
        // legacy kernel-wired `user::shell_service` is retired as a boot path (it cannot host the
        // milestone-28 shell, which speaks the terminal contract, not the raw console protocol) and
        // is kept only as dead code for reference.
        // **The tour ends and the soak begins** (milestone 219), and on this architecture it takes
        // the place of the progenitor handoff rather than of the halt below it. A soak boot that also
        // brought up a console, a line discipline and a shell would be soaking those too, and the
        // point of this workload is that what it stresses is decided rather than incidental.
        // **The sweep, when this build asked for one** (milestone 168). Before the soak arm and
        // the halt for the same reason the soak sits before the halt: the tour has finished, so the
        // whole boot is still evidence, and this run ends by halting rather than by beating
        // forever. The two features are alternatives rather than a stack; `script/job-mix` builds
        // only this one.
        #[cfg(feature = "job_mix")]
        job_mix::run();
        #[cfg(feature = "soak_test")]
        soak::run();
        // **Fatal risk 6's bench boot, when this build asked for one** (milestone 261). The same
        // position and the same reason as the two above: the tour is evidence, and this replaces
        // the hand-over. It WRITES to the NVMe disk; see kernel/src/disk_throughput.rs.
        #[cfg(feature = "disk_throughput")]
        disk_throughput::run();
        // **Milestone 494 (a driver for the network card a PC actually has)'s bench boot**, in the
        // same position for the same reason; see kernel/src/network_bench.rs.
        #[cfg(feature = "network_bench")]
        network_bench::run();

        #[cfg(not(any(
            feature = "soak_test",
            feature = "job_mix",
            feature = "disk_throughput",
            feature = "network_bench"
        )))]
        if let Some(image) = user::initrd() {
            println!();
            println!("nife: handing the system to the userspace progenitor.");
            if let Err(e) = user::boot_progenitor(image) {
                println!("  handoff FAILED: {e:?}");
            }
            // The boot thread's work is done; the progenitor and the services it builds run until halt.
        }
    }

    // bench::run diverged above, and so does soak::run (milestone 219); this is everyone else's
    // ending. **The boot thread leaves the scheduler rather than halting on its run queue**
    // (milestone 720 (provisional)), as x86_64's has since milestone 628: a halted thread that is
    // still runnable stops the core until the next tick every time round robin reaches it. The
    // idle thread, which waits only when nothing else can run, takes the core. A boot whose
    // hand-over failed ends here too, and it ends the same way: nothing is left to run.
    #[cfg(not(any(
        feature = "bench",
        feature = "soak_test",
        feature = "job_mix",
        feature = "disk_throughput",
        feature = "network_bench"
    )))]
    sched::exit()
}

/// Bring up the interrupt controller and the timer, then **unmask interrupts**.
///
/// This is the line the whole locking discipline was written for. From here, a timer interrupt
/// can land between any two instructions in the kernel, and every `IrqSafeMutex` starts
/// actually masking something. See DECISIONS §9 and notes/locking.md.
/// **Say what the TRNG's `STAT` says about its output width** (milestone 159), for the boot tour's
/// `hw entropy` line.
///
/// This is the one bit that decides whether the 32 bytes that line reports are 32 bytes of device
/// output. In 128-bit mode only `RAND0..RAND3` carry a generation's answer, so a driver that
/// assembles all eight words is serving 16 real bytes and 16 of whatever the upper registers hold.
/// The driver writes `MODE.R256` during bring-up precisely because the width a given JH7110 resets
/// to is a build-time parameter of the silicon; `STAT.R256` is the read-back that says it took, and
/// nothing has ever read it on hardware.
///
/// An all-zero `STAT` is called out separately because it is not a mode report at all: it is the
/// signature of a register window that answered with nothing, which milestone 220's clock line
/// above is what explains.
// riscv64-only because the JH7110 is; `allow(dead_code)` because the boot tour that calls it is
// itself compiled out of the shell and bench builds, the same way `image_for_virtio` below is.
/// **Counter ticks as microseconds** (design/roadmap/0306-time-the-hw-entropy-step.md), for
/// the boot tour's `hw entropy` line.
///
/// Microseconds rather than milliseconds because the interesting half of the number is an IPC round
/// trip, and rather than raw ticks because a tick is a different amount of time on every machine
/// this runs on: QEMU's riscv64 `virt` counts at 10 MHz and the JH7110 at 4 MHz, so a transcript
/// quoting ticks could not be compared against another board's without the rate beside it. The rate
/// itself came out of this machine's own device tree (`arch::timer::init_frequency`), which is why
/// there is no constant here to go stale.
///
/// Truncating division, deliberately: this is an instrument, and a figure that rounded up would
/// report a nonzero duration for work that took no measurable time at all.
///
/// Provisional name (an architect's call): `micros`, with `micros_between` and `bytes_per_second`
/// beside it.
#[cfg(target_arch = "riscv64")]
#[allow(dead_code)]
fn micros(ticks: u64) -> u64 {
    // `frequency()` asserts the rate is nonzero rather than returning a poison value, and it has
    // been read from the tree since the timer step near the top of this tour, so there is no
    // pre-init case to guard here. `saturating_mul` bounds the scaling rather than the duration: a
    // gap long enough to overflow is one nobody is timing.
    ticks.saturating_mul(1_000_000) / arch::timer::frequency()
}

/// The two-timestamp form of [`micros`]. `wrapping_sub` because the `time` CSR is a free-running
/// 64-bit counter and this is the arithmetic the rest of this file already uses on it (see the
/// timer step above); at 10 MHz it wraps after about 58,000 years.
#[cfg(target_arch = "riscv64")]
#[allow(dead_code)]
fn micros_between(start: u64, end: u64) -> u64 {
    micros(end.wrapping_sub(start))
}

/// **Bytes per second over a measured span**, for the same line.
///
/// Computed from ticks rather than from the microsecond figure beside it, so the rate does not
/// inherit that figure's truncation. Zero for a span too short to measure, which is the honest
/// answer: a rate derived from a zero-length interval is not a large number, it is no number.
///
/// **What it counts, which has to be stated before it is quoted.** Everything between the readiness
/// report and the last byte landing: the `entropy_protocol` round trips (one per
/// `entropy_protocol::MAX_BYTES`, so four per 32-byte draw), the two context switches each one costs,
/// the driver's own poll loop, and the device. It does **not** count the process spawn or the
/// bring-up, which are the once-per-boot cost reported separately. It is therefore not comparable
/// to a Linux `hwrng` throughput figure, which is a read from an already-running kernel driver with
/// no IPC in it at all; `notes/benchmarks.md`'s rule is to say so where the number is printed.
#[cfg(target_arch = "riscv64")]
#[allow(dead_code)]
fn bytes_per_second(bytes: u64, ticks: u64) -> u64 {
    if ticks == 0 {
        return 0;
    }
    bytes.saturating_mul(arch::timer::frequency()) / ticks
}

#[cfg(target_arch = "riscv64")]
#[allow(dead_code)]
fn mode_note(stat: u32) -> &'static str {
    if stat == 0 {
        "the whole status register read zero, so this says nothing about the mode"
    } else if stat & jh7110_entropy::STAT_R256 != 0 {
        "256-bit: all eight RAND words are the answer"
    } else {
        "128-BIT: only RAND0..3 are the answer, so 16 of every 32 bytes are not device output"
    }
}

/// The driver the virtio service spawns. `block_driver` on every architecture since milestone 291;
/// on aarch64 this used to be a role of `hello`, which was the same `crates/virtio` code behind a
/// second dispatch table. Panics if absent (the demo checked `initrd()` above).
#[cfg(not(any(test, feature = "system_tests")))]
// Tour-only: the shell and bench boots both skip the milestone tour where it is used.
#[cfg_attr(feature = "shell", allow(dead_code))]
#[cfg(not(feature = "bench"))]
fn image_for_virtio() -> &'static [u8] {
    user::program("block_driver").expect("no block_driver program in the initrd")
}

fn interrupts_init(_dtb: usize) {
    use crate::arch::{interrupts, irq, timer};

    // Bring up the interrupt controller (the GIC on aarch64, the PLIC on RISC-V), reading its
    // location from the device tree. Portable code names `arch::irq`, never a specific controller,
    // which is what lets a second architecture be a new `arch/` directory rather than a diff here.
    irq::init();

    timer::init();

    // The point of no return, in a much friendlier sense than the MMU's. After this, we are
    // preemptible.
    interrupts::enable();
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

/// **Hand the machine to the userspace progenitor**, riscv64's half of the ladder's top rung
/// (milestone 268, item 4).
///
/// The kernel loads the progenitor from the initrd, grants it the NS16550 and the UART's interrupt,
/// and it builds the console server, the line discipline, the input driver and `swish` out of its
/// own budget through the granular verbs. This is the line that retires the kernel as the system's
/// builder on this architecture, and since milestone 295 it is the **only** place this architecture
/// makes that claim: **userspace, not the kernel, composes a process.** `components/src/builder.rs`
/// used to make it in miniature one step up the tour, from exactly two capabilities, and calef
/// retired it on 2026-09-14 on the ground that this carries it at a larger scale on the same boot.
/// The difference is that this one composes the whole running system rather than one child, and a
/// person can then type at it; what it does not carry is the minimality half, which is why
/// design/roadmap/0295-retire-the-builder-program.md exists and says where that went.
///
/// **Two callers, one body** (milestone 268). It was the inside of the `#[cfg(feature = "shell")]`
/// block and nothing else; since this milestone the *default* boot ends here too, so the two paths
/// cannot drift. A `shell` build reaches it early and parks; a default build runs the whole tour
/// first and then reaches it.
///
/// Excluded from `test` and `bench` builds for `print_machine_description`'s reasons: a test boot
/// exits through semihosting before the tour and a bench boot diverges into `bench::run`, so
/// neither has a system to hand over.
///
/// Name: provisional (milestone 268 (every architecture boots the same way)). This is the x86
/// caller of the shared `user::boot_progenitor` loader (milestone 166 (one boot loader, reached two
/// inconsistent ways)): it finds the initrd, hands it over, then watches the boot thread bounded
/// and reports how it left; `riscv_hand_over` is its riscv twin. calef names what a reader meets.
#[cfg(target_arch = "riscv64")]
// A `soak` or `job_mix` build replaces the handoff with its own workload and never calls this, and
// a `test` or `bench` build parks before it; all four are boots with nothing to hand over. Allowed
// rather than `cfg`-ed out, so the function still compiles in every configuration: a handoff that
// only type-checks in the configurations that use it is one that rots in the others.
#[cfg_attr(
    any(
        test,
        feature = "bench",
        feature = "soak_test",
        feature = "job_mix",
        feature = "disk_throughput",
        feature = "network_bench",
        feature = "storage_bench"
    ),
    allow(dead_code)
)]
fn riscv_hand_over() {
    if let Some((plic_phys, _)) = memory::plic_region() {
        // SAFETY: the PLIC is device-mapped in the direct map; the context is the boot hart's S
        // context (derived, not hardcoded: OpenSBI's hart lottery).
        unsafe {
            drivers::plic::init(
                arch::mmu::phys_to_virt(plic_phys) as usize,
                arch::irq::boot_s_context(),
            );
        };
    }
    let Some(initrd) = user::initrd() else {
        println!();
        println!("nife: no archive to hand the system to (run `cargo xtask initrd-riscv`).");
        return;
    };
    println!();
    println!("nife: handing the system to the userspace progenitor.");
    // `boot_progenitor` discovers the UART's interrupt line itself and prints it with its source
    // (10 on QEMU virt, 32 on the JH7110; notes/visionfive2.md, BUGS), so a bench transcript names
    // which source won.
    if let Err(e) = user::boot_progenitor(initrd) {
        println!("  handoff FAILED: {e:?}");
    }
}

/// **Hand `x86_64` to the progenitor, and say how far it got** (milestone 182, inside milestone
/// 268's lane).
///
/// The same call `riscv_hand_over` makes, so the three architectures load, measure, endow and start
/// the first process through one body, and now reach the same place: the progenitor builds a
/// userspace console and a `swish` prompt announces the boot finished. Until milestone 299 this
/// architecture could not, because the console is port I/O (DECISIONS §121) with no page a driver
/// could map; §121's reversal gave x86 a `PortRange` capability, so the console and input drivers are
/// userspace processes holding COM1's ports, and the prompt above is transmitted by an `out` from
/// ring 3 like every other line the shell prints.
///
/// **The boot thread still watches, bounded, and reports what it saw**, because it is the boot
/// thread and has nothing else to do once the system is handed over: a progenitor still running when
/// the bound expires is the ordinary interactive outcome (the shell is up and waiting for input), and
/// a thread that left through a ring-3 fault left a record (`arch::exceptions::last_user_fault`) the
/// kernel's own fault report above it corroborates.
///
/// Name: provisional (milestone 182 (`x86_64`'s own interactive-boot entry point)), matching
/// `riscv_hand_over`.
#[cfg(target_arch = "x86_64")]
// Uncalled in the four configurations `riscv_hand_over` is, for the same reasons.
#[cfg_attr(
    any(
        test,
        feature = "bench",
        feature = "soak_test",
        feature = "job_mix",
        feature = "disk_throughput",
        feature = "network_bench"
    ),
    allow(dead_code)
)]
fn x86_hand_over() {
    use core::sync::atomic::Ordering;

    let Some(initrd) = user::initrd() else {
        println!();
        println!("nife: no archive to hand the system to (run `cargo xtask initrd-x86`).");
        return;
    };
    println!();
    println!("nife: handing the system to the userspace progenitor.");

    let faults_before = arch::exceptions::USER_FAULTS.load(Ordering::Acquire);
    let tid = match user::boot_progenitor(initrd) {
        Ok(tid) => tid,
        Err(e) => {
            println!("  handoff FAILED: {e:?}");
            return;
        }
    };

    // Ten seconds of TCG is far past the progenitor's first console build, which is microseconds of
    // real time after it starts. The bound exists so the boot thread parks rather than waits.
    let deadline = arch::timer::now() + 10 * arch::timer::frequency();
    while sched::is_thread_present(tid) && arch::timer::now() < deadline {
        sched::yield_now();
    }

    let faults = arch::exceptions::USER_FAULTS
        .load(Ordering::Acquire)
        .wrapping_sub(faults_before);
    if sched::is_thread_present(tid) {
        println!(
            "nife x86_64: the progenitor is running at ring 3; {faults} of the processes it built \
             stopped on purpose."
        );
        // DECISIONS §121, reversed 2026-09-15, made the console server and the input driver
        // userspace processes holding a `PortRange` capability for COM1's ports (milestone 299), so
        // `swish`'s prompt above was transmitted by an `out` from ring 3, not by the kernel. A
        // holdover from §121's kernel-console era printed a "no prompt" line here; the prompt is now
        // the last thing above this, and `faults` above is zero because the two device drivers no
        // longer trap on first use.
        println!(
            "  prompt     : the shell above is served by userspace console and input drivers \
             holding COM1 as a port capability (milestone 299)."
        );
        return;
    }
    match arch::exceptions::last_user_fault() {
        Some((fault, addr)) if faults > 0 => println!(
            "nife x86_64: the progenitor ran at ring 3 and stopped ({fault:?} at {addr:#x}); \
             its fault report is above."
        ),
        _ => println!("nife x86_64: the progenitor left ring 3 without faulting."),
    }
}

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
/// (That cfg is quoted as it stood: milestone 296 deleted `initboot`, and the live gate on the
/// block below it now reads `not(feature = "shell")`.)
///
/// The `test` and `bench` exclusions on the signature are not that switch and are deliberately
/// kept. A `bench` boot diverges into `bench::run` before this point and never returns, and a
/// `test` boot exits through semihosting; neither is a boot anybody reads to bring up a board.
/// Every boot that reaches this line prints all of it.
#[cfg(not(any(test, feature = "system_tests", feature = "bench")))]
fn print_machine_description(boot_info_pointer: usize) {
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

    // 6. The initrd. Two facts, printed apart on purpose, because a boot where the loader passed
    //    an image the kernel cannot parse looks identical to a boot with no image at all from any
    //    test's point of view (the x86 arm learned this at milestone 161).
    match memory::initrd_region() {
        None => println!("  initrd          : none (nothing was passed to this boot)"),
        Some((at, size)) => match user::initrd().map(nifefs::Fs::parse) {
            Some(Ok(fs)) => println!(
                "  initrd          : {size} bytes at {at:#018x}, a nifefs archive of {} program(s)",
                fs.len(),
            ),
            Some(Err(e)) => println!(
                "  initrd          : {size} bytes at {at:#018x}, but it does not parse: {e:?}",
            ),
            None => println!(
                "  initrd          : {size} bytes at {at:#018x}, recorded but not reachable",
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
    // all three architectures**, so that `board_console` and CI have something to match that does
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

#[cfg(test)]
mod tests {
    //! Tests for the boot path itself: the things `boot.s` and the boot protocol had to get
    //! right before any other code could run at all.
    //!
    //! Everything else lives beside the code it tests. `cargo test -p kernel` still collects
    //! all of them: `custom_test_frameworks` gathers every `#[test_case]` in the crate,
    //! wherever it is.

    /// Proves the harness itself works. If this fails, nothing else is meaningful.
    #[test_case]
    fn harness_runs() {
        // black_box so this is a real runtime check, not a constant clippy folds to `2 == 2`.
        let two = core::hint::black_box(1) + core::hint::black_box(1);
        assert_eq!(two, 2);
    }

    /// Proves `boot.s` zeroed `.bss`.
    ///
    /// A zero-initialized static lands in `.bss`, which occupies no bytes in the
    /// ELF file. Nobody loaded it. If our zeroing loop were wrong, this would hold
    /// whatever garbage was in RAM at power-on. See notes/elf.md.
    #[test_case]
    fn bss_was_zeroed() {
        use core::sync::atomic::{AtomicU64, Ordering};
        static CANARY: AtomicU64 = AtomicU64::new(0);
        assert_eq!(CANARY.load(Ordering::Relaxed), 0);
    }

    /// Proves `boot.s` gave us a usable, correctly aligned stack.
    ///
    /// Both aarch64 and RISC-V require a 16-byte-aligned `sp`, and both fault or corrupt
    /// silently on a misaligned one, so a bug here would show up as a mysterious early crash
    /// rather than as anything legible. Reads `sp` through `arch::current_sp`, so it is portable.
    /// See notes/stack.md.
    #[test_case]
    fn stack_pointer_is_16_byte_aligned() {
        let sp = crate::arch::current_sp();
        assert_eq!(sp % 16, 0, "sp = {sp:#x}");
    }

    /// Proves we are where we think we are.
    ///
    /// QEMU's `virt` machine starts a kernel at EL1 by default, which is exactly where a kernel
    /// belongs. **A machine can hand us EL2 instead, and then this test is the one that proves
    /// the drop worked** (milestone 127, the seL4 machine): `virt,virtualization=on` starts the
    /// kernel at EL2 the way U-Boot does on a board, `boot.s` reads `CurrentEL` and `eret`s down,
    /// and the assertion below is unchanged either way. The older comment here said that if this
    /// ever read EL2 "we will need to drop down ourselves"; that is now what happens, and the
    /// entry level is recorded rather than lost (`arch::entry_el`). See notes/aarch64.md.
    ///
    /// EL is an aarch64 concept, so this is gated. **RISC-V needs no twin, and the older
    /// comment here promising one "with the RISC-V boot path" outlived the boot path it
    /// was waiting for.** That ISA deliberately gives S-mode no way to read its own
    /// privilege level, and it does not need one: `arch::riscv64::exceptions`'s
    /// `breakpoint_is_caught_and_execution_resumes` proves the same thing sideways, because
    /// the breakpoint arm it counts is guarded on the trap having come from S-mode, and an
    /// M-mode `ebreak` would have gone to OpenSBI's `mtvec` and never reached us at all.
    /// See notes/riscv-arch-tests.md.
    #[cfg(target_arch = "aarch64")]
    #[test_case]
    fn running_at_el1() {
        use aarch64_cpu::registers::CurrentEL;
        use tock_registers::interfaces::Readable;
        assert_eq!(CurrentEL.read(CurrentEL::EL), 1);
    }

    /// Proves the boot protocol actually delivered a device tree.
    ///
    /// This is the test that closes the correction from milestone 1. Back then we
    /// shipped an ELF, QEMU took its bare-metal path, and `x0` arrived as zero. Now
    /// we ship a flat binary carrying an arm64 Image header, QEMU recognizes it as a
    /// kernel, follows the Linux boot protocol, and hands us a real pointer.
    ///
    /// A zero here means we have silently regressed to the ELF path, which would be
    /// easy to do by editing the runner script and hard to notice any other way.
    #[test_case]
    fn device_tree_pointer_was_provided() {
        use core::sync::atomic::Ordering;
        assert_ne!(
            crate::DTB.load(Ordering::Relaxed),
            0,
            "no DTB pointer in x0: did we fall back to booting as an ELF?"
        );
    }

    /// Proves the pointer points at an actual device tree, not just at something.
    ///
    /// A nonzero pointer is necessary but not sufficient. Every flattened device tree
    /// begins with the magic `0xd00dfeed`, stored **big-endian** (the format predates
    /// the little-endian consensus and never changed), so we have to byte-swap on the
    /// way in. If this passes, the machine is genuinely describing itself to us.
    /// **Not on `x86_64`**, and the reason is the whole of what that architecture's boot handoff
    /// differs by: what arrives in `kernel_main`'s one pointer there is PVH's `hvm_start_info`,
    /// which carries the same *kind* of thing (the memory map, the root of everything else
    /// discoverable) in an entirely different format. `machine_discovery::x86_64` decodes it and
    /// is host-tested against a real dump, which is the stronger place for that check to live;
    /// this test's subject genuinely does not exist there.
    #[test_case]
    fn device_tree_has_the_right_magic() {
        use core::sync::atomic::Ordering;

        if cfg!(target_arch = "x86_64") {
            crate::testing::skip!(
                "this machine hands over PVH hvm_start_info, not a device tree (see \
                 machine_discovery::x86_64, host-tested)"
            );
        }

        // DTB holds the PHYSICAL address QEMU gave us in x0. Since the kernel moved to the
        // high half, TTBR0 is disabled and a low address does not exist: dereferencing it
        // directly faults, which is exactly what we want and exactly what this line used to
        // do. Name it through the direct map instead.
        let pa = crate::DTB.load(Ordering::Relaxed) as u64;
        let ptr = crate::arch::mmu::phys_to_virt(pa) as *const u32;

        // SAFETY: QEMU put a device tree at that physical address, and the direct map makes
        // it readable.
        let magic = unsafe { core::ptr::read_volatile(ptr) };

        assert_eq!(
            u32::from_be(magic),
            0xd00d_feed,
            "no device tree magic at {ptr:p}"
        );
    }
}

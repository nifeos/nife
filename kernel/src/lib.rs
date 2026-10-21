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
// The five x86_64 tables the CPU reads by address (`IDT`, `TSS`, `GDT`, the installed port grant
// and the bench I/O bitmap) are `core::cell::SyncUnsafeCell`s, by calef's ruling of 2026-10-09 on
// milestone 891 (a mutable static becomes an atomic, a lock or a recorded cell). The gate is a
// dependency on an unstable `core` API: if it changes shape, the fallback is a hand-written wrapper;
// if it stabilizes, delete this line.
#![feature(sync_unsafe_cell)]
#![test_runner(crate::testing::runner)]
#![reexport_test_harness_main = "test_main"]

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

// The boot itself (milestone 876 (the kernel crate root is split along its seams)): `kernel_main`
// and the steps all three boots share, and the machine description.
mod boot;
#[cfg(not(any(test, feature = "system_tests", feature = "bench")))]
mod machine_description;

pub use boot::{DTB, device_tree, kernel_main};
// `print!` expands to `$crate::_print`, and a macro another crate expands can only name what that
// crate can see: `console` is private, so its entry point is re-exported here (milestone 609 (the
// system tests leave the kernel crate)).
#[doc(hidden)]
pub use console::_print;
#[cfg(not(any(test, feature = "system_tests", feature = "bench")))]
use machine_description::*;

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

#[cfg(test)]
mod tests;

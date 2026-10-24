//! Userspace: entering user mode, and the confined services the kernel wires there. From here on
//! the machine runs code the kernel did not compile and does not trust, and the kernel's job is
//! deciding what is allowed. Entering user mode is returning from an exception that never happened:
//! a forged [`TrapFrame`] falls into `exception_restore`. See notes/userspace.md, and
//! notes/kernel-wired-services.md for the services.

use elf::Elf;
use page_frames::{FRAME_SIZE, PageFrame};
use paging::{Flags, Half, MapError, Mapper};

use crate::arch::exceptions::{TrapFrame, enter_user};
use crate::arch::mmu::{self, phys_to_ptr};
use crate::arch::sync_icache;
use crate::memory;

/// Where a user program's stack goes: one page, with `sp` at its top; a program given a deeper
/// stack gets the rest mapped below it, a page at a time. Derived from `address_space_map`'s stack
/// band. A program names its own load address in its ELF header, which [`load`] checks against the
/// map's image band.
pub const USER_STACK_VA: u64 = address_space_map::STACK_TOP_PAGE;
pub const USER_STACK_TOP: u64 = address_space_map::STACK_TOP;
const _: () = assert!(USER_STACK_TOP == USER_STACK_VA + FRAME_SIZE);

// Every type and function of `user` lives in a child named for it, and this file is the index (§281
// (the kernel's user-mode file splits into child modules of `user`)). Each child takes `use
// super::*` and is re-exported at its items' visibility, so every `user::` path resolves.
mod address_space;
pub use address_space::*;
mod loader;
pub use loader::*;
mod entry;
pub use entry::*;
mod console_device;
pub use console_device::*;
mod progenitor_boot;
pub use progenitor_boot::*;
mod device_starters;
use device_starters::*;
mod virtio_dma;
pub(crate) use virtio_dma::*;
mod terminal_print;
use terminal_print::*;
#[cfg(any(test, feature = "system_tests"))]
mod test_support;
// A unit-test build without the system tests names no `wait_for`, as `wait_for` itself allows.
#[cfg(any(test, feature = "system_tests"))]
#[cfg_attr(not(feature = "system_tests"), allow(unused_imports))]
pub use test_support::*;

// The address-space registry: the table that owns every space a capability or a thread can name,
// and the holder each is bound to. A sibling of `address_space`, so it reaches `AddressSpace`'s
// `pub(super)` fields, which no module outside `user` can see.
mod address_space_registry;
pub use address_space_registry::*;

/// The hand-assembled `x86_64` programs. They stay beside the compiled ones because the boot tour
/// runs before any archive is parsed, and a fixture that needs no initial ramdisk lets the
/// userspace demo run on a `cargo run` with no `-initrd` at all.
#[cfg(target_arch = "x86_64")]
pub mod x86_programs;

/// A supervised child built from a hand-assembled stub, its whole world in one untyped region:
/// the boot self-test's `supervised` and `teardown` checks, and the system tests that watch a
/// child die. See notes/self-test-checks.md.
// A unit-test build runs neither the self-test nor the suite, and a `bench` build has no self-test.
#[cfg_attr(
    any(test, all(feature = "bench", not(feature = "system_tests"))),
    allow(dead_code)
)]
pub mod supervised_child;

/// Bringing the virtio block driver up in userspace.
///
/// **Milestone 9's headline.** The kernel enumerates the bus (kernel/src/virtio.rs) to find the
/// block device, then hands a userspace driver everything it needs and nothing it does not: the
/// device's registers, a DMA page, an interrupt, and an endpoint to report what it read. The
/// kernel does not touch the device.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
// the tour spawns it; the tests drive it
pub mod virtio_service;

/// **A service's memory, remembered so a finished test can hand it back.** The bookkeeping half of
/// DECISIONS §16 object revocation, applied to the services the test boot builds: without it a boot
/// that runs many service-shaped tests runs out of frames, and does so in whichever innocent test
/// happens to allocate next. See the module note and notes/frames.md.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
// the tests are the callers; the tour never tears down
pub mod holding;

/// The RedoxFS filesystem service: a block server, an FS server and a client, confined, and the
/// endpoints and shared pages that wire them. `None` if this run has no second MMIO block disk. See
/// notes/kernel-wired-services.md.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // spawned only by the phase-2 test
pub mod fs_service;

/// The block-device roster and the disk surveyor: a read-only listing of every block device, and a
/// block-service endpoint for exactly one. The kernel never reads a partition table. See
/// notes/kernel-wired-services.md and notes/block-devices.md.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the tests are its only caller today
pub mod disk_service;

/// The display service: a confined virtio-gpu driver and a client that draws, wired by the kernel
/// and then left alone. The driver's DMA region is the rings page plus the surface, one contiguous
/// run, so the device can reach the pixels and nothing else. See notes/kernel-wired-services.md and
/// notes/framebuffer-contract.md.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
// spawned only by the milestone-29 test
pub mod display_service;

/// The compositor: one screen, several mutually distrusting clients. The isolation is a property of
/// the grants' shape: each client's surface is its own frames at the same virtual address, the
/// clients' frames are physically adjacent so the attack test is fair, and the screen is mapped
/// read-only only into a client granted it. See notes/kernel-wired-services.md.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
// spawned only by the milestone-33 tests
pub mod compositor_service;

/// The keyboard service: a confined virtio-input driver that turns key events into terminal bytes
/// on a ring the compositor reads. The driver gets no client's endpoint, so it cannot choose who
/// receives what it types; focus is the compositor's. See notes/kernel-wired-services.md.
// The driver is spawned only by the milestone-29 test; the boot calls `wire_device` alone and the
// progenitor builds the driver (milestone 600 (provisional)).
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub mod keyboard_service;

/// **The USB keyboard driver's wiring** (milestone 242 (USB host and HID)): the whole xHCI
/// controller, confined, handed to one EL0 process that turns a boot keyboard's reports into the
/// terminal contract's bytes. What it holds and what it is refused is written in that module's own
/// header. Spawned by [`boot_progenitor`] alone, so it is dead in exactly the builds that function is.
#[cfg_attr(
    any(
        test,
        feature = "bench",
        feature = "soak_test",
        feature = "job_mix",
        feature = "disk_throughput"
    ),
    allow(dead_code)
)]
pub mod usb_keyboard_service;

/// The clock service: the RTC's registers, the wall clock's offset and the propose endpoint, in one
/// confined process. The kernel finds the RTC and hands it over, and never reads the clock. See
/// notes/kernel-wired-services.md and notes/clock.md.
// The interactive boot calls `start` on both ISAs since milestone 51's wiring lane; the rest of the
// module (the propose helper, the kernel-side page reader) is still the tests' alone.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub mod clock_service;

/// The entropy service: a virtio-rng device, its DMA page, its interrupt, and the request endpoint
/// clients hold, in one confined process. The service holds the device; a client holds only "you
/// may obtain randomness". The kernel holds no entropy. See notes/kernel-wired-services.md and
/// notes/entropy.md.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
// the system tests, std_service, the installer and boot_progenitor are its callers
pub mod entropy_service;

/// **radon's SD/MMC block server's wiring** (milestone 53 (the board's own peripherals: network
/// and storage on real silicon)): the kernel ungates the controller and hands a process one page of
/// it, a transfer region and a window of the card. riscv64-only because the controller is the
/// JH7110's; `kernel/src/designware_mobile_storage.rs` carries the parity note. The bench boot is
/// its only caller until `PROVEN_ON_SILICON`.
#[cfg(target_arch = "riscv64")]
#[cfg_attr(not(feature = "storage_bench"), allow(dead_code))]
pub mod designware_mobile_storage_service;

/// **The EL0 NVMe block server's wiring** (milestone 261; DECISIONS §86's option 2a).
///
/// The kernel keeps the admin plane, which is the authority to say where a queue lives, and hands
/// a process the doorbell page and the data plane's pages of one confined DMA region. What it is
/// granted and what it is refused is written out in that module's own header, in the shape
/// milestone 159's TRNG driver established, because the confinement is the claim and the driver is
/// only what exercises it.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the tests are its callers
pub mod non_volatile_memory_express_service;

/// **The disk tree** (milestone 834 (ioping on nife and Linux)): the whole NVMe namespace as one read-only, uncached file, the
/// installed system's second mounted tree. The NVMe server's second client, in its own window.
pub mod disk_tree_service;

/// **`net_stack` over the `e1000e` NIC** (milestone 494 (a driver for the network card a PC
/// actually has)): the kernel resets the controller and programs its rings, and the process is
/// handed the two queue pages of BAR0 and the confined DMA region, in milestone 261 (the NVMe driver leaves the kernel)'s shape. What
/// it holds and what it is refused is in that module's header.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
// the tests, the bench boot and boot_progenitor are its callers
pub mod e1000e_service;

/// **`net_stack` over the JH7110's Ethernet port** (milestone 53 (the board's own peripherals:
/// network and storage on real silicon)): `e1000e_service`'s shape for radon. riscv64-only because
/// the JH7110 is; `kernel/src/designware_ethernet.rs` carries the parity note.
#[cfg(target_arch = "riscv64")]
pub mod designware_ethernet_service;

/// The offer a booted stick makes (milestone 198 (a package manager, and the trivial install that
/// makes a second customer possible), rung 2a): ask whether to put this system on the machine's own
/// disk, and wire the two confined programs that do it. Boot policy only; nothing in it holds a
/// disk. `x86_64` only, because the other loaders hand over no copy of the boot file and the
/// aarch64 console has no receive path. See notes/kernel-wired-services.md.
#[cfg(target_arch = "x86_64")]
pub mod install_service;

/// The credential service, its provisioner and its clients. Four processes, three of them differing
/// by one field of a `Spawn` literal. The kernel never sees a secret: it creates two endpoints, two
/// separate frames and a budget, and hands each process a different subset. See
/// notes/kernel-wired-services.md and notes/credentials.md.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
// the milestone-56 credential tests are its callers
pub mod credential_service;

/// **The login service: authentication produces capabilities, not a mutated identity** (milestone
/// 49, DECISIONS §109). The kernel spawns it exactly as it spawns `credentialer`: the archive
/// mapped read-only, a construction budget, and the endpoints it needs, so what is under test is
/// `components/src/login.rs`'s own choices rather than a privileged shortcut.
///
/// Not arch-gated, for `credential_service`'s own reason: `nifefs`, `elf`, and
/// `supervision_protocol::build_child` are portable, and a login service that mints capabilities on
/// one instruction set and not another is not the claim this milestone makes.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
// the milestone-49 login tests are its callers
pub mod login_service;

/// **A provisioning tool: create an identity and its home subtree together** (milestone 155,
/// DECISIONS §117). Spawned once per identity, against a credential service's still-open provision
/// endpoint and a directory capability wide enough to hold the new subtree, exactly as
/// `credentialer_test_client`'s provisioner role is spawned, except this is the first real caller
/// rather than a test harness.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
// the milestone-155 provisioning tests are its callers
pub mod identity_provisioner_service;

/// The NTP client, and the test server that answers it. The client gets five slots and no mapping
/// of the clock page; the test server holds `READ` on the endpoint the client holds `WRITE` on, so
/// substituting the peer is invisible to the client. See notes/kernel-wired-services.md and
/// notes/ntp.md.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the tests are its callers
pub mod ntp_service;

/// Milestone 11: hand a process an untyped budget and let it spend it.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the tour and the userspace tests
pub mod memory_region_service;

/// **The raw-keystroke input primitive** (milestone 169): a real `line_editor` process, wired
/// exactly as the boot path wires it except that the test plays both the input driver and the
/// application, so `OPERATION_RAWMODE` and `OPERATION_READRAW` can be driven directly with real keystrokes.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
// the milestone-169 raw-mode tests are its only caller
pub mod raw_mode_service;

/// **`rmle`'s wiring** (milestone 169): a real terminal ([`raw_mode_service`]'s own shape) and a
/// real filesystem ([`fs_service::narrow_dir`]'s shape) composed for the one program in this tree
/// that needs both at once.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
// the milestone-169 rmle tests are its only caller
pub mod rmle_service;

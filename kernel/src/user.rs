//! Userspace. EL0. The actual operating system boundary.
//!
//! Everything before this was a Rust program that boots. From here on, the machine runs code
//! that **we did not compile and do not trust**, and the kernel's job stops being "do things"
//! and starts being "decide what is allowed."
//!
//! # Entering EL0 is returning from an exception that never happened
//!
//! There is no "drop to EL0" instruction. There is only `eret`, which restores whatever
//! `SPSR_EL1` says and jumps to `ELR_EL1`, and the exception level to return to is *in*
//! `SPSR_EL1`. So we do not need a new way down. We need a **fake way back**: fabricate a
//! [`TrapFrame`] with `SPSR = EL0t`, point `sp` at it, and fall into the `exception_restore`
//! that milestone 2 already wrote.
//!
//! This is the second time the project has pulled exactly this trick. `Thread::spawn_into` fakes a
//! `switch_to` frame so that the `ret` which *resumes* a thread also *starts* one
//! (notes/threads.md). Both times the "start" path turned out to be the "resume" path with a
//! forged frame, and no new code at all.
//!
//! # What milestone 4 already paid for
//!
//! The kernel lives entirely in `TTBR1`, at `0xffff_...`. Userspace lives in `TTBR0`, at
//! `0x0000_...`. **The hardware picks the table register from bits 63:48 of the address**, so:
//!
//! - The kernel is mapped in every address space, for free. Nobody had to copy anything.
//! - A syscall **does not switch page tables**. There is nothing to sync and nothing to remap.
//! - Installing a process is one `msr ttbr0_el1`.
//!
//! None of that was written for milestone 7. It fell out of a higher-half decision made three
//! milestones ago, and `Flags::user_code()` / `Flags::user_data()` have been sitting in the
//! `paging` crate, unused, waiting for today.
//!
//! # What is deliberately NOT here
//!
//! **A syscall ABI.** The user program below executes `svc #0` and asks for nothing. There is
//! no syscall number, no argument convention, no return value. DECISIONS §10 chose
//! capabilities, and the syscall surface gets designed against a capability table at 7d, in one
//! piece, on purpose. Not accreted here because it was convenient.

use elf::Elf;
use page_frames::{FRAME_SIZE, PageFrame};
use paging::{Flags, Half, MapError, Mapper};

use crate::arch::exceptions::{TrapFrame, enter_user};
use crate::arch::mmu::{self, phys_to_ptr};
use crate::arch::sync_icache;
use crate::memory;

/// Where a user program's stack goes. One page, and `sp` starts at the top of it: stacks grow down.
/// A program given a deeper stack gets the rest mapped below this, one page at a time.
///
/// **Derived from `address_space_map`**, the band the map gives the stack, rather than chosen here.
/// It was `0x50_0000` until 2026-09-26 (milestone 206 (a program image has under 896 KiB)), directly above an image linked at
/// `0x40_0000`, which left a program image under 896 KiB; the map moved the image and the stack
/// together to the top of the second gigabyte.
///
/// There is no matching `USER_CODE_VA` any more: it existed for `exec`, the one-page raw
/// machine-code loader the hand-assembled programs needed, and every program the kernel runs now
/// names its own load address in its ELF header, which [`load`] checks against the map's image band.
pub const USER_STACK_VA: u64 = address_space_map::STACK_TOP_PAGE;
pub const USER_STACK_TOP: u64 = address_space_map::STACK_TOP;
const _: () = assert!(USER_STACK_TOP == USER_STACK_VA + FRAME_SIZE);

// **Every type and function of `user` lives in a child named for it**, and this file is the
// index (calef's ruling on §281 (the kernel's user-mode file splits into child modules of
// `user`), 2026-10-11 UTC). Each child takes `use super::*` and is re-exported at the visibility
// its items had here, so every `user::` path a caller names still resolves. Cut by milestone 872
// (the kernel's user-mode file is split along its seams).
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

// **The address-space registry, in a file of its own**: the table that owns every space a
// capability or a thread can name, and the holder each is bound to. Cut out along this seam by
// milestone 812 (`std::thread::spawn` runs real threads in one address space), whose process
// object changes who may hold a space, because this file is at its §266 (a Rust source file stays
// under 2,000 lines) ceiling. A sibling of `address_space`, so it reaches `AddressSpace`'s fields
// through their `pub(super)`, which no module outside `user` can see.
mod address_space_registry;
pub use address_space_registry::*;

/// **Load and run a real compiled ELF at U-mode on RISC-V** (milestone 20, the user-ELF step).
///
/// This takes the bytes of the `least_authority_demo` program (a Rust binary compiled to a riscv64 ELF, delivered
/// as the initial ramdisk)
/// and runs them through the kernel's *real* ELF loader. [`load`] parses the file, builds an address
/// space with each `PT_LOAD` segment mapped W^X at the VA it names, and maps a stack; nothing here is
/// riscv-specific except that the loader was just taught to accept `EM_RISCV`. The `least_authority_demo` is granted
/// WRITE on one endpoint as its slot 0, started with the input `n` in its second argument register
/// (`a1`), squares it, and SENDs the answer home.
///
/// Receiving `n*n` proves the whole ELF path works on RISC-V: parse, segment mapping with correct
/// permissions, the entry point, argument passing across the `START` boundary, and the endpoint
/// SEND, all from a program the kernel did not hand-write. `load` is arch-neutral; this is the same
/// code aarch64 runs, now on the RISC-V address space and trap path.
/// The hand-assembled `x86_64` programs. Compiled ones now exist (item 4's hand-off), but these
/// stay: the boot tour runs before any archive is parsed, and a fixture that needs no initial
/// ramdisk is what lets the userspace demo run on a `cargo run` with no `-initrd` at all.
#[cfg(target_arch = "x86_64")]
pub mod x86_programs;

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

/// **The RedoxFS filesystem service** (milestone 32 phase 2): three confined processes and the
/// endpoints and shared pages that wire them, spawned by the test that proves the stack end to end.
///
/// ```text
///   disk ──virtio──► block server ──blk IPC──► FS server ──file IPC──► client ──► report to kernel
/// ```
///
/// The kernel builds the wiring and hands each process exactly its world (a `Spawn` literal each);
/// it never sees a filesystem operation, an opcode, or a byte of file data. The FS server owns
/// RedoxFS and its own heap; the block server owns the DMA confinement; the client holds only a
/// directory capability. This is the same shape as `virtio_service` and the console, one level up.
///
/// The service drives the **second** mmio block disk (the RedoxFS image); the first is the nifefs
/// disk the phase-1 driver tests use. `None` if there is no such disk attached to this run.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // spawned only by the phase-2 test
pub mod fs_service;

/// **The block-device roster and the disk surveyor** (milestone 57, notes/block-devices.md).
///
/// Two authorities that every other operating system hands out as one: a **read-only mapping**
/// listing what block devices exist, and a **block-service endpoint** for exactly one of them. A
/// program with the first can see the machine's disks and open none of them; a program with the
/// second was handed one disk and has no way to name a second.
///
/// The kernel's part is small and stops early: scan the buses, write the page, confine one device
/// under a block server, spawn. It never reads a partition table. Every byte of GPT judgement is in
/// `crates/globally_unique_identifier_partition_table`, whose tests run on the host against tables
/// `sgdisk` and macOS `diskutil` wrote.
///
/// Arch-neutral, like the clock and entropy wirings: one portable binary over one host-tested
/// contract, so **both ISAs run literally the same test** (DECISIONS §19).
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the tests are its only caller today
pub mod disk_service;

/// **The display service** (milestone 29, the display ladder's rung one): a confined virtio-gpu
/// driver and a client that draws, wired by the kernel and then left alone.
///
/// ```text
///   virtio-gpu ──virtio (PCIe, behind the IOMMU)──► gpu_driver ──display IPC──► painter
///        │                                              │                             │
///        └──── DMA: the whole region ───────────────────►│                             │
///                                    the surface (pages 1..) ─────── shared ──────────┘
/// ```
///
/// The kernel's part is the same as every other service here: build the wiring, hand each process a
/// `Spawn` literal, and know nothing about what they do. It never sees a virtio-gpu command, a
/// pixel, or a rectangle. What is new is the **size** of the DMA region, and that is the whole
/// memory story: a framebuffer does not fit in the single page the disk and NIC drivers get, so the
/// region is `1 + graphics_contract::SURFACE_PAGE_FRAMES` **contiguous** frames, page 0 for the rings and the
/// control buffers and the rest for the surface. Registering the whole run as the driver's DMA region
/// is what keeps the framebuffer inside the grant: the shadow-ring validator bounds every descriptor
/// to it, and `iommu::confine` maps exactly it, so the device can reach the pixels and nothing else.
/// The block server already took two pages this way (milestone 32); this is the same move, wider.
///
/// The client maps only the surface frames. It never sees page 0, so it cannot touch a descriptor
/// ring, and it holds no `Virtio` capability, no interrupt, and no physical address. See
/// notes/framebuffer-contract.md.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
// spawned only by the milestone-29 test
pub mod display_service;

/// **The compositor: one screen, several mutually distrusting clients** (milestone 33, the display
/// ladder's rung two).
///
/// ```text
///   display (or a kernel stand-in) ──gfx FLUSH(damage)──► compositor ◄──one doorbell──── window clients
///                                            the scanout, shared ──┘  │                 (a surface each)
///                                                                     └─► one input endpoint per focusable
/// ```
///
/// The kernel's part is what it always is: allocate the frames, mint the endpoints, hand each process
/// a `Spawn` literal, and know nothing about what they do. It never sees a pixel, a window, or a
/// damage rectangle. What is worth reading here is the **shape of the grants**, because the isolation
/// this rung exists to prove is a property of exactly that shape:
///
/// - every client's control page and surface are its own frames, mapped **at the same virtual
///   addresses** in every client. Two clients' surfaces are the same address in different address
///   spaces, so "my neighbour's surface" is not somewhere a client can reach by guessing;
/// - the clients' frames are allocated as **one contiguous run**, deliberately, so that the page just
///   past a client's grant really is its neighbour's memory. That makes the attack in
///   `a_client_holds_no_capability_for_its_neighbours_pixels_or_the_screen` a fair one: the attacker is
///   handed the exact address, the bytes it wants are physically adjacent, and the mapping is the only
///   thing in its way;
/// - the screen and the window list are mapped **read-only** and **only** into a client granted them.
///   That mapping is the screenshot capability and the enumeration capability; there is no verb for
///   either, and a client without the mapping has nothing to ask and nowhere to look.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
// spawned only by the milestone-33 tests
pub mod compositor_service;

/// **The keyboard service** (milestone 29's input): a confined userspace virtio-input driver that
/// turns key events into the bytes a terminal understands, and publishes them where the compositor
/// reads them.
///
/// ```text
///   virtio-input ──virtio (PCIe, IOMMU)──► keyboard_driver ──the input ring──► whoever maps it
///                                           └──doorbell COMMIT──► "look at the surfaces"
/// ```
///
/// The grant shape is the whole security argument and it is worth reading beside
/// `compositor_service`: the driver gets the device, its interrupt, its DMA page, the doorbell, and
/// **the ring page**. It does not get any client's endpoint, so it cannot choose who receives what
/// it types; that is focus, and focus is the compositor's decision expressed as which of the input
/// capabilities *it* holds it uses (DECISIONS §33). And the ring is what makes typing possible at
/// all: the doorbell every client holds is content-free, so a client that rang it forever could not
/// produce a single character.
///
/// In the test below the **kernel** plays the compositor, which is the same substitution three of the
/// four rung-two tests make: it holds the doorbell and the ring, so the bytes a real keyboard
/// produced are a value it can read and compare rather than a picture it has to infer.
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

/// **The clock service** (milestone 51 lane A, DECISIONS §43): the RTC's registers, the wall
/// clock's offset, and the propose endpoint, in one confined userspace process.
///
/// The kernel's whole part in wall-clock time is here and it is small: find the RTC in the device
/// tree (by `compatible`, `memory::rtc_region`), allocate one frame for the clock page, and hand
/// the service the registers, the page read/write, and an endpoint. It does not read the clock, does
/// not know what time it is, and has no notion of an offset. Everything after the spawn is
/// userspace agreeing with userspace over `clock_contract`.
///
/// Arch-neutral, like the display and compositor wiring: the component is one portable binary
/// carrying both RTC drivers, and the *machine* says which one it has, so **both ISAs run literally
/// the same test** (DECISIONS §19).
// The interactive boot calls `start` on both ISAs since milestone 51's wiring lane; the rest of the
// module (the propose helper, the kernel-side page reader) is still the tests' alone.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub mod clock_service;

/// **The entropy service** (milestone 56, DECISIONS §44): a virtio-rng device, its DMA page, its
/// interrupt, and the request endpoint clients hold, in one confined userspace process.
///
/// The kernel's whole part in randomness is here and it is smaller than the clock's: find an RNG on
/// whichever bus the caller named, confine it to one DMA page, and hand the service the transport,
/// the interrupt, and two endpoints. **The kernel never reads the device and holds no entropy of
/// its own.** Everything after the spawn is userspace agreeing with userspace over
/// `entropy_contract`.
///
/// The authority split is the point, and it is one sentence: the service holds the device; a client
/// holds an endpoint that means *"you may obtain randomness"*. Those are different powers, and only
/// the second one is safe to hand around. A client cannot program the queue, cannot map the page
/// the device writes into, and cannot ask for anything the service did not ask on its behalf.
///
/// Arch-neutral: one portable binary, both transports, both ISAs (DECISIONS §19).
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

/// **The offer a booted stick makes** (milestone 198 (a package manager, and the trivial install
/// that makes a second customer possible), rung 2a): ask whether to put this system on the
/// machine's own disk, and wire the two confined programs that do it. Boot policy only; nothing in
/// it holds a disk.
///
/// **`x86_64` only, and the gap is the loader's rather than this module's.** It needs two things
/// the other two architectures do not have: a copy of the file this machine booted from
/// (`memory::boot_file_region`, which is `None` wherever `uefi_loader` cannot hand over a second
/// module, and a device-tree handoff has one initial ramdisk slot in `/chosen` and no second one),
/// and a console it can read a line back from (`console::read_line`, which the aarch64 console's
/// PL011 driver has no receive path for). Both are recorded where they are, and both would have to
/// move before this module could. Written as a `cfg` rather than as a no-op body on purpose: a
/// module that compiled everywhere and could only ever decline on two of three would read as
/// portable.
#[cfg(target_arch = "x86_64")]
pub mod install_service;

/// **The credential service, its provisioner, and its clients** (milestone 56, the credential half;
/// notes/credentials.md).
///
/// The kernel's part is the wiring, and here more than anywhere the wiring *is* the argument. Four
/// processes, and the difference between three of them is one field of a `Spawn` literal:
///
/// | process | slot 0 | what that means |
/// |---|---|---|
/// | `credentialer` | the provision endpoint (READ) **and** the verify endpoint (READ, slot 1) | holds the store |
/// | `credentialer_test_client` provisioner | the **provision** endpoint (WRITE) | may write the store, until the seal |
/// | `credentialer_test_client` client | the **verify** endpoint (WRITE) | may ask a question about the store |
/// | `credentialer_test_client` attacker | the **verify** endpoint (WRITE) | the identical endowment, used otherwise |
///
/// The kernel never sees a secret, holds no store, and computes no hash. It creates two endpoints,
/// two frames, and a budget, and hands each process a different subset. Everything after the spawn
/// is userspace agreeing with userspace over `credential_contract`.
///
/// **Two frames and not one**, which is the detail worth stating: the provisioner writes plaintext
/// secrets into its page, so a client sharing that frame would read them. The two pages are
/// separate physical frames and neither process is ever given the other's.
///
/// Arch-neutral: one portable binary each, both ISAs (DECISIONS §19). Argon2id is arithmetic on
/// `u64`s and neither the service nor its clients contain a line of assembly.
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

/// **The NTP client, and the test server that answers it** (milestone 51; DECISIONS §43, §44).
///
/// The kernel's part is the wiring, and the wiring *is* the argument. An NTP client here gets five
/// slots: a report endpoint, the socket contract's endpoint, an untyped budget, the clock service's
/// **propose** endpoint, and the entropy service's endpoint. What it does not get is a mapping of
/// the clock page, in either direction, which is the whole difference between this and a Unix
/// `ntpd` running as root.
///
/// The test server is a separate program holding `READ` on the endpoint the client holds `WRITE`
/// on. Substituting the peer at a capability boundary is how a capability system tests a client:
/// the client's code does not change and cannot tell. See
/// components/src/network_time_client.rs for what that proves and what it leaves to milestone 30's
/// socket-contract tests. It was a role of the client's own binary until milestone 290, and nothing
/// about the substitution depended on that: the boundary is the capability.
///
/// Arch-neutral: three portable binaries, both ISAs (DECISIONS §19).
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

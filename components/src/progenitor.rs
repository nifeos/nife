//! **The progenitor: the first process, on all three architectures.**
//!
//! The kernel loads this as the boot process, maps the initial ramdisk into it, and grants it the
//! capabilities [`GRANTS`] names. From those alone it builds the interactive system out of its own
//! budget (the console server, the input driver, the line discipline, the shell, the terminal's
//! sink adapter and the job undertaker), then stays alive as the spawn service. All of that is
//! `crates/system_initializer` (milestone 96 (one init)); this file is the boot entry, one program
//! with one `GRANTS` table and no `cfg` on every architecture (notes/progenitor-and-loading.md,
//! "One program and one grant table on every architecture").
//!
//! Name: ratified 2026-09-08 (calef, milestone 266). `progenitor` replaces `init` as the archive
//! entry and `system_initializer` as this program's name. `init` is a truncated verb where the rule
//! asks for a noun, and the thing is the first process rather than an action. Refused `prime` (reads
//! as *chief* in English and as number theory in a systems tree), `origin` (not an agent noun) and
//! `first` (generic enough to name anything). The crate this program is a boot entry for keeps the
//! name `system_initializer`, ratified 2026-08-01; see this milestone's `## Follow-on`.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use system_initializer::BootEndowment;

/// **What `kernel::user::boot_progenitor` grants, in order, on every architecture** (milestone 166).
/// The kernel inserts these into this process's capability table before it starts, and the numbers
/// below are that function's `assert_eq!`s read from the other side. Slots 5 and 6 hold nothing when
/// this boot attached no RedoxFS disk, which is what `fs_rights` (0 for no disk) says.
///
/// The clock and the inert-configuration page are both granted ahead of the filesystem pair on
/// purpose, so their slots are the same on every boot whether or not a disk was attached.
const GRANTS: BootEndowment = BootEndowment {
    untyped: 0,
    uart_dev: 1,
    uart_irq: 2,
    clock_page: 3,
    config_page: 4,
    fs_ep: 5,
    fs_page: 6,
    // Always these three (`kernel::user::boot_progenitor` grants them at explicit slots, not
    // `thread_control_block_insert_cap`'s first-free `None`, for exactly this reason): a boot with
    // no virtio-rng device leaves them empty, and `system_initializer::boot`'s own probe is what
    // tells it apart from a granted one. Fixed past the filesystem pair's own max reach (slot 6),
    // not slot 5, because the inert-configuration page (slot 4) shifted that pair down by one.
    virtio_rng: 7,
    virtio_rng_irq: 8,
    virtio_rng_dma: 9,
    // A terminal on the firmware's screen (milestone 198 (a package manager)'s rung 1b), past the virtio-rng trio's
    // floor (slot 9) for that trio's reason: a boot whose console has no screen leaves both empty.
    // (They were the kernel-built virtio-gpu stack's slots until milestone 600 (provisional).)
    disp_term_ep: 10,
    disp_term_page: 11,
    // The gpu's surface run (milestone 600 (provisional)), in the slot the kernel-built stack's
    // keyboard endpoint used to take. The other three gpu grants are at 17-19, below.
    gpu_surface: 12,
    // The network card (milestone 590 (the booted system starts its network stack)), past the
    // slot 12 floor for the same reason again: a boot with no virtio-net device leaves all three
    // empty, and `system_initializer::boot` probes for it.
    virtio_net: 13,
    virtio_net_irq: 14,
    virtio_net_dma: 15,
    // An entropy service the kernel built on the CPU's seed instruction (milestone 595
    // (provisional)), granted only on a boot with no virtio-rng, so a boot has one of slot 7's trio
    // or this, never both. Past the network card's floor (slot 15) for the reason every group above
    // gives.
    entropy_ep: 16,
    // The graphical terminal stack's raw materials (milestone 600 (provisional)): a virtio-gpu's
    // transport, interrupt and DMA run, then a virtio keyboard's trio. Nothing is built from
    // these at boot (milestone 632 (provisional), calef's 2026-09-30 ruling): the boot is the
    // minimal UART system, and `boot` places these in the shell, which holds them until a
    // `graphical_terminal` session's spawn sends them back for the drivers to be built from. Empty with no
    // GPU; the keyboard's three are empty when a session's keystrokes would come from the UART,
    // which is every real board.
    gpu: 17,
    gpu_irq: 18,
    gpu_dma: 19,
    keyboard: 20,
    keyboard_irq: 21,
    keyboard_dma: 22,
    // The machine statistics page (milestone 126 (the `procps` package), DECISIONS §225 (`free`
    // sees the machine and your share)), granted on every boot, past the keyboard's trio. Slot 23
    // was the kernel's fault slot until the table grew to 32.
    machine_page: 23,
    // The kernel's ring, its cursor page and its notification (milestone 342 (the kernel and the
    // `console` server drive one UART from two address spaces)), past the machine page. Empty on a
    // boot whose kernel could not allocate them; `system_initializer::boot` probes.
    kernel_ring: 24,
    kernel_ring_cursor: 25,
    kernel_ring_notification: 26,
    // A USB keyboard driver's attach endpoint (milestone 242 (USB host and HID)), past the kernel
    // ring's floor. Empty on a machine with no xHCI controller; `system_initializer::boot` probes.
    usb_keyboard_attach: 27,
    // This process's own address space (§249 (a running address space stays nameable), its
    // 2026-10-05 amendment), `WRITE` alone and no `GRANT`, granted on every boot: what lets the
    // loader `UNMAP` each scratch page once it is in the child (milestone 95 (an unmap primitive)).
    own_space: 28,
    // A network stack the kernel built on an `e1000e` NIC (milestone 198 (a package manager)), its
    // endpoint and the endpoint its lease arrives on, past `own_space`. Empty on a boot with a
    // virtio-net NIC (slot 13's trio) and on one with no NIC the kernel may drive;
    // `system_initializer::boot` probes.
    net_stack_ep: 29,
    net_stack_report: 30,
    // The reboot object (milestone 805 (`reboot` at the prompt), DECISIONS §251 (restarting the
    // machine is a kernel object the progenitor hands out)), granted on every boot past the e1000e
    // stack's pair. Kept by the spawn service for the one program that declares it.
    reboot: 31,
    // Nothing. Since milestone 166 the boot loader is not shared with milestone 19d's test roles on
    // any architecture, so the kernel grants exactly what the interactive system uses. aarch64 once
    // carried a report endpoint (slot 1) and the 19d.2b test interrupt (slot 3) here.
    for_test_roles: &[],
};

#[unsafe(no_mangle)]
pub extern "C" fn _start(uart_shape: u64, initial_ramdisk_len: u64, fs_rights: u64) -> ! {
    // `uart_shape` is the console UART's register shape as `abi::uart` encodes it, read from the
    // device tree by the kernel, which this process cannot read; `boot` hands it to the console and
    // input drivers, the two programs that map the UART.
    //
    // **The second mounted tree is the disk tree** (milestone 834 (ioping on nife and Linux)): the
    // whole NVMe namespace as one read-only, uncached file at `/disk`. DECISIONS §126 (a real,
    // single, moving `cwd`) left the second subtree to an architect, and calef ruled it on
    // nifeos/nife#2371, 2026-10-18 21:36 UTC: "Yes, read-only at prompt". The kernel grants slots
    // 32 and 33 only on a machine with an NVMe disk, and `boot` probes them.
    system_initializer::boot(
        &GRANTS,
        initial_ramdisk_len,
        fs_rights,
        uart_shape,
        Some(system_initializer::MountedTreeGrant { ep: 32, pool: 33 }),
    )
}

user_mode_runtime::panic_handler!();
